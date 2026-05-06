#![allow(clippy::expect_used)]

use matching_engine::{
    order_book::{OrderId, OrderSide},
    projection::{
        InMemoryProjectionSink, OrderProjectionStatus, ProjectionBroadcaster, ProjectionEvent,
        ProjectionPublisher, book_stream_message, project_engine_event, projection_topic,
        serialize_projection_event,
    },
    service::EngineEvent,
    shard::MarketEvent,
};

fn id(value: &str) -> OrderId {
    OrderId::new(value)
}

#[tokio::test]
async fn projection_broadcaster_fans_out_projected_events() {
    let broadcaster = ProjectionBroadcaster::new(16);
    let mut subscriber = broadcaster.subscribe();

    broadcaster
        .publish_engine_event(EngineEvent {
            market_id: "mkt-election".to_string(),
            sequence: 11,
            event: MarketEvent::BookDelta {
                side: OrderSide::Buy,
                price: 54,
                quantity_delta: 7,
            },
        })
        .expect("projection event should publish");

    assert_eq!(
        subscriber
            .recv()
            .await
            .expect("projected event should be received"),
        ProjectionEvent::BookDelta {
            market_id: "mkt-election".to_string(),
            seq_no: 11,
            side: OrderSide::Buy,
            price_pts: 54,
            quantity_delta: 7,
        }
    );
}

#[test]
fn projects_book_delta_events_for_depth_streams() {
    let projected = project_engine_event(EngineEvent {
        market_id: "mkt-election".to_string(),
        sequence: 7,
        event: MarketEvent::BookDelta {
            side: OrderSide::Sell,
            price: 55,
            quantity_delta: -2,
        },
    });

    assert_eq!(
        projected,
        vec![ProjectionEvent::BookDelta {
            market_id: "mkt-election".to_string(),
            seq_no: 7,
            side: OrderSide::Sell,
            price_pts: 55,
            quantity_delta: -2,
        }]
    );
}

#[test]
fn projects_trade_events_for_trade_streams() {
    let projected = project_engine_event(EngineEvent {
        market_id: "mkt-election".to_string(),
        sequence: 8,
        event: MarketEvent::Trade {
            maker_order_id: id("maker"),
            taker_order_id: id("taker"),
            maker_user_id: 10,
            taker_user_id: 20,
            sequence: 3,
            price: 56,
            quantity: 4,
        },
    });

    assert_eq!(
        projected,
        vec![ProjectionEvent::Trade {
            market_id: "mkt-election".to_string(),
            seq_no: 8,
            maker_order_id: "maker".to_string(),
            taker_order_id: "taker".to_string(),
            maker_user_id: 10,
            taker_user_id: 20,
            trade_sequence: 3,
            price_pts: 56,
            quantity: 4,
        }]
    );
}

#[test]
fn projects_status_events_for_order_update_streams() {
    let projected = project_engine_event(EngineEvent {
        market_id: "mkt-election".to_string(),
        sequence: 9,
        event: MarketEvent::OrderCancelled {
            order_id: id("ioc-taker"),
            user_id: Some(20),
            canceled_quantity: 3,
        },
    });

    assert_eq!(
        projected,
        vec![ProjectionEvent::OrderUpdate {
            market_id: "mkt-election".to_string(),
            seq_no: 9,
            user_id: 20,
            order_id: "ioc-taker".to_string(),
            status: OrderProjectionStatus::Cancelled,
            filled_qty: 0,
            remaining_qty: 0,
        }]
    );
}

#[test]
fn serializes_projection_events_with_stable_transport_envelope() {
    let event = ProjectionEvent::BookDelta {
        market_id: "mkt-election".to_string(),
        seq_no: 12,
        side: OrderSide::Sell,
        price_pts: 57,
        quantity_delta: -3,
    };

    assert_eq!(projection_topic(&event), "engine.book_delta");
    assert_eq!(
        serialize_projection_event(&event).expect("projection should serialize"),
        r#"{"topic":"engine.book_delta","market_id":"mkt-election","seq_no":12,"kind":"book_delta","payload":{"side":"Sell","price_pts":57,"quantity_delta":-3}}"#
    );
}

#[test]
fn projection_publisher_sends_topic_key_and_payload() {
    let sink = InMemoryProjectionSink::default();
    let publisher = ProjectionPublisher::new(sink.clone());
    let event = ProjectionEvent::Trade {
        market_id: "mkt-election".to_string(),
        seq_no: 13,
        maker_order_id: "maker".to_string(),
        taker_order_id: "taker".to_string(),
        maker_user_id: 10,
        taker_user_id: 20,
        trade_sequence: 4,
        price_pts: 58,
        quantity: 2,
    };

    publisher
        .publish(&event)
        .expect("projection event should publish");

    let emitted = sink.published();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].topic, "engine.trade");
    assert_eq!(emitted[0].key, "mkt-election");
    assert!(emitted[0].payload.contains(r#""kind":"trade""#));
    assert!(emitted[0].payload.contains(r#""seq_no":13"#));
}

#[test]
fn book_stream_message_filters_market_book_deltas() {
    let ignored_trade = ProjectionEvent::Trade {
        market_id: "mkt-election".to_string(),
        seq_no: 1,
        maker_order_id: "maker".to_string(),
        taker_order_id: "taker".to_string(),
        maker_user_id: 10,
        taker_user_id: 20,
        trade_sequence: 1,
        price_pts: 55,
        quantity: 2,
    };
    assert_eq!(book_stream_message("mkt-election", &ignored_trade), None);

    let other_market = ProjectionEvent::BookDelta {
        market_id: "other-market".to_string(),
        seq_no: 2,
        side: OrderSide::Sell,
        price_pts: 57,
        quantity_delta: -3,
    };
    assert_eq!(book_stream_message("mkt-election", &other_market), None);

    let delta = ProjectionEvent::BookDelta {
        market_id: "mkt-election".to_string(),
        seq_no: 3,
        side: OrderSide::Sell,
        price_pts: 57,
        quantity_delta: -3,
    };
    assert_eq!(
        book_stream_message("mkt-election", &delta).expect("book delta should serialize"),
        r#"{"type":"delta","market_id":"mkt-election","seq":3,"side":"Sell","price_pts":57,"qty_delta":-3}"#
    );
}
