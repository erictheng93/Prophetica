use matching_engine::{
    order_book::{OrderId, OrderSide},
    projection::{OrderProjectionStatus, ProjectionEvent, project_engine_event},
    service::EngineEvent,
    shard::MarketEvent,
};

fn id(value: &str) -> OrderId {
    OrderId::new(value)
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
            canceled_quantity: 3,
        },
    });

    assert_eq!(
        projected,
        vec![ProjectionEvent::OrderUpdate {
            market_id: "mkt-election".to_string(),
            seq_no: 9,
            order_id: "ioc-taker".to_string(),
            status: OrderProjectionStatus::Cancelled,
            filled_qty: 0,
            remaining_qty: 0,
        }]
    );
}
