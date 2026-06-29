use std::sync::{Arc, Mutex};
#[cfg(feature = "kafka")]
use std::time::Duration;

#[cfg(feature = "kafka")]
use rdkafka::{
    config::ClientConfig,
    producer::{BaseProducer, BaseRecord, Producer},
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::{
    order_book::{OrderSide, Price, Quantity, Sequence},
    service::EngineEvent,
    shard::MarketEvent,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderProjectionStatus {
    Active,
    Filled,
    Partial,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionEvent {
    BookDelta {
        market_id: String,
        seq_no: Sequence,
        side: OrderSide,
        price_pts: Price,
        quantity_delta: i64,
    },
    Trade {
        market_id: String,
        seq_no: Sequence,
        maker_order_id: String,
        taker_order_id: String,
        maker_user_id: u64,
        taker_user_id: u64,
        trade_sequence: Sequence,
        price_pts: Price,
        quantity: Quantity,
    },
    OrderUpdate {
        market_id: String,
        seq_no: Sequence,
        user_id: u64,
        order_id: String,
        status: OrderProjectionStatus,
        filled_qty: Quantity,
        remaining_qty: Quantity,
    },
}

#[derive(Serialize)]
struct BookDeltaMessage<'a> {
    #[serde(rename = "type")]
    message_type: &'static str,
    market_id: &'a str,
    seq: Sequence,
    side: OrderSide,
    price_pts: Price,
    qty_delta: i64,
}

#[derive(Serialize)]
struct TradeMessage<'a> {
    #[serde(rename = "type")]
    message_type: &'static str,
    market_id: &'a str,
    seq: Sequence,
    maker_order_id: &'a str,
    taker_order_id: &'a str,
    maker_user_id: u64,
    taker_user_id: u64,
    trade_seq: Sequence,
    price_pts: Price,
    qty: Quantity,
}

#[derive(Serialize)]
struct OrderUpdateMessage<'a> {
    #[serde(rename = "type")]
    message_type: &'static str,
    market_id: &'a str,
    seq: Sequence,
    user_id: u64,
    order_id: &'a str,
    status: OrderProjectionStatus,
    filled_qty: Quantity,
    remaining_qty: Quantity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedProjection {
    pub topic: String,
    pub key: String,
    pub payload: String,
}

pub trait ProjectionSink {
    /// Publishes one serialized projection payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the sink cannot accept the payload.
    fn publish(&self, topic: &str, key: &str, payload: &str) -> anyhow::Result<()>;
}

#[derive(Debug, Clone)]
pub struct ProjectionPublisher<S> {
    sink: S,
}

impl<S> ProjectionPublisher<S>
where
    S: ProjectionSink,
{
    #[must_use]
    pub const fn new(sink: S) -> Self {
        Self { sink }
    }

    /// Serializes and publishes one projection event.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization or sink publishing fails.
    pub fn publish(&self, event: &ProjectionEvent) -> anyhow::Result<()> {
        let topic = projection_topic(event);
        let key = projection_key(event);
        let payload = serialize_projection_event(event)?;
        self.sink.publish(topic, key, &payload)
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryProjectionSink {
    published: Arc<Mutex<Vec<PublishedProjection>>>,
}

impl InMemoryProjectionSink {
    #[must_use]
    pub fn published(&self) -> Vec<PublishedProjection> {
        self.published.lock().map_or_else(
            |poisoned| poisoned.into_inner().clone(),
            |published| published.clone(),
        )
    }
}

impl ProjectionSink for InMemoryProjectionSink {
    fn publish(&self, topic: &str, key: &str, payload: &str) -> anyhow::Result<()> {
        if let Ok(mut published) = self.published.lock() {
            published.push(PublishedProjection {
                topic: topic.to_string(),
                key: key.to_string(),
                payload: payload.to_string(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KafkaProducerSettings {
    pub acks: &'static str,
    pub enable_idempotence: bool,
    pub message_timeout_ms: u64,
}

impl KafkaProducerSettings {
    #[must_use]
    pub const fn spec_008() -> Self {
        Self {
            acks: "all",
            enable_idempotence: true,
            message_timeout_ms: 5_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KafkaTopicDurabilityPolicy {
    pub replication_factor: i16,
}

impl KafkaTopicDurabilityPolicy {
    #[must_use]
    pub const fn spec_008() -> Self {
        Self {
            replication_factor: 3,
        }
    }
}

#[cfg(feature = "kafka")]
pub struct KafkaProjectionSink {
    producer: BaseProducer,
    flush_timeout: Duration,
}

#[cfg(feature = "kafka")]
impl KafkaProjectionSink {
    /// Builds a Kafka projection sink with idempotent producer settings.
    ///
    /// # Errors
    ///
    /// Returns a Kafka error when the producer cannot be constructed from the provided brokers.
    pub fn from_brokers(brokers: &str) -> Result<Self, rdkafka::error::KafkaError> {
        let settings = KafkaProducerSettings::spec_008();
        let producer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("acks", settings.acks)
            .set(
                "enable.idempotence",
                if settings.enable_idempotence {
                    "true"
                } else {
                    "false"
                },
            )
            .set(
                "message.timeout.ms",
                settings.message_timeout_ms.to_string(),
            )
            .create()?;

        Ok(Self {
            producer,
            flush_timeout: Duration::from_millis(settings.message_timeout_ms),
        })
    }
}

#[cfg(feature = "kafka")]
impl ProjectionSink for KafkaProjectionSink {
    fn publish(&self, topic: &str, key: &str, payload: &str) -> anyhow::Result<()> {
        self.producer
            .send(BaseRecord::to(topic).key(key).payload(payload))
            .map_err(|(error, _message)| anyhow::anyhow!(error))?;
        self.producer.poll(Duration::ZERO);
        self.producer.flush(self.flush_timeout)?;
        Ok(())
    }
}

#[must_use]
pub const fn projection_topic(event: &ProjectionEvent) -> &'static str {
    match event {
        ProjectionEvent::BookDelta { .. } => "engine.book_delta",
        ProjectionEvent::Trade { .. } => "engine.trade",
        ProjectionEvent::OrderUpdate { .. } => "engine.order_update",
    }
}

#[must_use]
pub fn projection_key(event: &ProjectionEvent) -> &str {
    match event {
        ProjectionEvent::BookDelta { market_id, .. }
        | ProjectionEvent::Trade { market_id, .. }
        | ProjectionEvent::OrderUpdate { market_id, .. } => market_id,
    }
}

#[must_use]
pub fn book_stream_message(market: &str, event: &ProjectionEvent) -> Option<String> {
    let ProjectionEvent::BookDelta {
        market_id,
        seq_no,
        side,
        price_pts,
        quantity_delta,
    } = event
    else {
        return None;
    };
    if market_id != market {
        return None;
    }

    serde_json::to_string(&BookDeltaMessage {
        message_type: "delta",
        market_id,
        seq: *seq_no,
        side: *side,
        price_pts: *price_pts,
        qty_delta: *quantity_delta,
    })
    .ok()
}

#[must_use]
pub fn trade_stream_message(market: &str, event: &ProjectionEvent) -> Option<String> {
    let ProjectionEvent::Trade {
        market_id,
        seq_no,
        maker_order_id,
        taker_order_id,
        maker_user_id,
        taker_user_id,
        trade_sequence,
        price_pts,
        quantity,
    } = event
    else {
        return None;
    };
    if market_id != market {
        return None;
    }

    serde_json::to_string(&TradeMessage {
        message_type: "trade",
        market_id,
        seq: *seq_no,
        maker_order_id,
        taker_order_id,
        maker_user_id: *maker_user_id,
        taker_user_id: *taker_user_id,
        trade_seq: *trade_sequence,
        price_pts: *price_pts,
        qty: *quantity,
    })
    .ok()
}

#[must_use]
pub fn order_update_stream_message(user: u64, event: &ProjectionEvent) -> Option<String> {
    let ProjectionEvent::OrderUpdate {
        market_id,
        seq_no,
        user_id,
        order_id,
        status,
        filled_qty,
        remaining_qty,
    } = event
    else {
        return None;
    };
    if *user_id != user {
        return None;
    }

    serde_json::to_string(&OrderUpdateMessage {
        message_type: "order_update",
        market_id,
        seq: *seq_no,
        user_id: *user_id,
        order_id,
        status: *status,
        filled_qty: *filled_qty,
        remaining_qty: *remaining_qty,
    })
    .ok()
}

/// Serializes a projection event into a stable transport envelope.
///
/// # Errors
///
/// Returns an error if JSON serialization fails.
pub fn serialize_projection_event(event: &ProjectionEvent) -> serde_json::Result<String> {
    #[derive(Serialize)]
    struct Envelope<'a, T> {
        topic: &'static str,
        market_id: &'a str,
        seq_no: Sequence,
        kind: &'static str,
        payload: T,
    }

    #[derive(Serialize)]
    struct BookDeltaPayload {
        side: OrderSide,
        price_pts: Price,
        quantity_delta: i64,
    }

    #[derive(Serialize)]
    struct TradePayload<'a> {
        maker_order_id: &'a str,
        taker_order_id: &'a str,
        maker_user_id: u64,
        taker_user_id: u64,
        trade_sequence: Sequence,
        price_pts: Price,
        quantity: Quantity,
    }

    #[derive(Serialize)]
    struct OrderUpdatePayload<'a> {
        order_id: &'a str,
        status: OrderProjectionStatus,
        filled_qty: Quantity,
        remaining_qty: Quantity,
    }

    match event {
        ProjectionEvent::BookDelta {
            market_id,
            seq_no,
            side,
            price_pts,
            quantity_delta,
        } => serde_json::to_string(&Envelope {
            topic: projection_topic(event),
            market_id,
            seq_no: *seq_no,
            kind: "book_delta",
            payload: BookDeltaPayload {
                side: *side,
                price_pts: *price_pts,
                quantity_delta: *quantity_delta,
            },
        }),
        ProjectionEvent::Trade {
            market_id,
            seq_no,
            maker_order_id,
            taker_order_id,
            maker_user_id,
            taker_user_id,
            trade_sequence,
            price_pts,
            quantity,
        } => serde_json::to_string(&Envelope {
            topic: projection_topic(event),
            market_id,
            seq_no: *seq_no,
            kind: "trade",
            payload: TradePayload {
                maker_order_id,
                taker_order_id,
                maker_user_id: *maker_user_id,
                taker_user_id: *taker_user_id,
                trade_sequence: *trade_sequence,
                price_pts: *price_pts,
                quantity: *quantity,
            },
        }),
        ProjectionEvent::OrderUpdate {
            market_id,
            seq_no,
            user_id: _,
            order_id,
            status,
            filled_qty,
            remaining_qty,
        } => serde_json::to_string(&Envelope {
            topic: projection_topic(event),
            market_id,
            seq_no: *seq_no,
            kind: "order_update",
            payload: OrderUpdatePayload {
                order_id,
                status: *status,
                filled_qty: *filled_qty,
                remaining_qty: *remaining_qty,
            },
        }),
    }
}

#[derive(Debug, Clone)]
pub struct ProjectionBroadcaster {
    events: broadcast::Sender<ProjectionEvent>,
}

impl ProjectionBroadcaster {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (events, _event_rx) = broadcast::channel(capacity);
        Self { events }
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<ProjectionEvent> {
        self.events.subscribe()
    }

    /// Projects and publishes one engine event.
    ///
    /// # Errors
    ///
    /// Returns a send error when there are no active projection subscribers.
    pub fn publish_engine_event(
        &self,
        event: EngineEvent,
    ) -> Result<(), broadcast::error::SendError<ProjectionEvent>> {
        for projected in project_engine_event(event) {
            self.events.send(projected)?;
        }

        Ok(())
    }
}

#[must_use]
pub fn project_engine_event(envelope: EngineEvent) -> Vec<ProjectionEvent> {
    match envelope.event {
        MarketEvent::OrderAccepted { .. } => Vec::new(),
        MarketEvent::OrderResting {
            order_id,
            user_id,
            remaining,
        } => vec![ProjectionEvent::OrderUpdate {
            market_id: envelope.market_id,
            seq_no: envelope.sequence,
            user_id,
            order_id: order_id.as_str().to_string(),
            status: OrderProjectionStatus::Active,
            filled_qty: 0,
            remaining_qty: remaining,
        }],
        MarketEvent::Trade {
            maker_order_id,
            taker_order_id,
            maker_user_id,
            taker_user_id,
            sequence,
            price,
            quantity,
        } => vec![ProjectionEvent::Trade {
            market_id: envelope.market_id,
            seq_no: envelope.sequence,
            maker_order_id: maker_order_id.as_str().to_string(),
            taker_order_id: taker_order_id.as_str().to_string(),
            maker_user_id,
            taker_user_id,
            trade_sequence: sequence,
            price_pts: price,
            quantity,
        }],
        MarketEvent::OrderCancelled {
            order_id,
            user_id,
            canceled_quantity: _,
        } => user_id.map_or_else(Vec::new, |user_id| {
            vec![ProjectionEvent::OrderUpdate {
                market_id: envelope.market_id,
                seq_no: envelope.sequence,
                user_id,
                order_id: order_id.as_str().to_string(),
                status: OrderProjectionStatus::Cancelled,
                filled_qty: 0,
                remaining_qty: 0,
            }]
        }),
        MarketEvent::BookDelta {
            side,
            price,
            quantity_delta,
        } => vec![ProjectionEvent::BookDelta {
            market_id: envelope.market_id,
            seq_no: envelope.sequence,
            side,
            price_pts: price,
            quantity_delta,
        }],
        MarketEvent::OrderFilled {
            order_id,
            user_id,
            total_filled,
        } => vec![ProjectionEvent::OrderUpdate {
            market_id: envelope.market_id,
            seq_no: envelope.sequence,
            user_id,
            order_id: order_id.as_str().to_string(),
            status: OrderProjectionStatus::Filled,
            filled_qty: total_filled,
            remaining_qty: 0,
        }],
        MarketEvent::OrderPartiallyFilled {
            order_id,
            user_id,
            filled_now,
            remaining,
        } => vec![ProjectionEvent::OrderUpdate {
            market_id: envelope.market_id,
            seq_no: envelope.sequence,
            user_id,
            order_id: order_id.as_str().to_string(),
            status: OrderProjectionStatus::Partial,
            filled_qty: filled_now,
            remaining_qty: remaining,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::{KafkaProducerSettings, KafkaTopicDurabilityPolicy};

    #[test]
    fn kafka_producer_settings_require_all_acks_and_idempotence() {
        let settings = KafkaProducerSettings::spec_008();

        assert_eq!(settings.acks, "all");
        assert!(settings.enable_idempotence);
    }

    #[test]
    fn kafka_topic_durability_policy_requires_three_replicas() {
        let policy = KafkaTopicDurabilityPolicy::spec_008();

        assert_eq!(policy.replication_factor, 3);
    }
}
