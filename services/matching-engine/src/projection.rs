use serde::{Deserialize, Serialize};

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
        trade_sequence: Sequence,
        price_pts: Price,
        quantity: Quantity,
    },
    OrderUpdate {
        market_id: String,
        seq_no: Sequence,
        order_id: String,
        status: OrderProjectionStatus,
        filled_qty: Quantity,
        remaining_qty: Quantity,
    },
}

#[must_use]
pub fn project_engine_event(envelope: EngineEvent) -> Vec<ProjectionEvent> {
    match envelope.event {
        MarketEvent::OrderAccepted { .. } => Vec::new(),
        MarketEvent::OrderResting { order_id } => vec![ProjectionEvent::OrderUpdate {
            market_id: envelope.market_id,
            seq_no: envelope.sequence,
            order_id: order_id.as_str().to_string(),
            status: OrderProjectionStatus::Active,
            filled_qty: 0,
            remaining_qty: 0,
        }],
        MarketEvent::Trade {
            maker_order_id,
            taker_order_id,
            sequence,
            price,
            quantity,
        } => vec![ProjectionEvent::Trade {
            market_id: envelope.market_id,
            seq_no: envelope.sequence,
            maker_order_id: maker_order_id.as_str().to_string(),
            taker_order_id: taker_order_id.as_str().to_string(),
            trade_sequence: sequence,
            price_pts: price,
            quantity,
        }],
        MarketEvent::OrderCancelled {
            order_id,
            canceled_quantity: _,
        } => vec![ProjectionEvent::OrderUpdate {
            market_id: envelope.market_id,
            seq_no: envelope.sequence,
            order_id: order_id.as_str().to_string(),
            status: OrderProjectionStatus::Cancelled,
            filled_qty: 0,
            remaining_qty: 0,
        }],
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
            total_filled,
        } => vec![ProjectionEvent::OrderUpdate {
            market_id: envelope.market_id,
            seq_no: envelope.sequence,
            order_id: order_id.as_str().to_string(),
            status: OrderProjectionStatus::Filled,
            filled_qty: total_filled,
            remaining_qty: 0,
        }],
        MarketEvent::OrderPartiallyFilled {
            order_id,
            filled_now,
            remaining,
        } => vec![ProjectionEvent::OrderUpdate {
            market_id: envelope.market_id,
            seq_no: envelope.sequence,
            order_id: order_id.as_str().to_string(),
            status: OrderProjectionStatus::Partial,
            filled_qty: filled_now,
            remaining_qty: remaining,
        }],
    }
}
