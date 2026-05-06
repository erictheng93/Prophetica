#![allow(clippy::expect_used)]

use matching_engine::order_book::{
    CancelOrderResult, NewOrder, OrderId, OrderSide, OrderType, Symbol,
};
use matching_engine::shard::{MarketCommand, MarketEvent, MarketShard};
use tokio::sync::{mpsc, oneshot};

fn id(value: &str) -> OrderId {
    OrderId::new(value)
}

#[tokio::test]
async fn shard_processes_cancel_orders_through_same_command_queue() {
    let (command_tx, command_rx) = mpsc::channel(8);
    let (event_tx, mut event_rx) = mpsc::channel(8);
    let shard = MarketShard::new(symbol());
    let shard_task = tokio::spawn(shard.run(command_rx, event_tx));

    let (maker_reply_tx, maker_reply_rx) = oneshot::channel();
    command_tx
        .send(MarketCommand::PlaceOrder {
            order: order("maker", 10, OrderSide::Sell, 55, 2),
            reply_to: maker_reply_tx,
        })
        .await
        .expect("command receiver should stay open");
    maker_reply_rx
        .await
        .expect("maker reply should be sent")
        .expect("maker should rest");
    event_rx
        .recv()
        .await
        .expect("maker accepted event should be published");
    event_rx
        .recv()
        .await
        .expect("maker resting event should be published");

    let (cancel_reply_tx, cancel_reply_rx) = oneshot::channel();
    command_tx
        .send(MarketCommand::CancelOrder {
            order_id: id("maker"),
            reply_to: cancel_reply_tx,
        })
        .await
        .expect("command receiver should stay open");

    let cancel_result = cancel_reply_rx
        .await
        .expect("cancel reply should be sent")
        .expect("cancel should succeed");
    assert_eq!(
        cancel_result,
        CancelOrderResult {
            order_id: id("maker"),
            canceled_quantity: 2,
        }
    );

    let cancel_event = event_rx
        .recv()
        .await
        .expect("cancel event should be published");
    assert_eq!(
        cancel_event,
        MarketEvent::OrderCancelled {
            order_id: id("maker"),
            canceled_quantity: 2,
        }
    );

    let (taker_reply_tx, taker_reply_rx) = oneshot::channel();
    command_tx
        .send(MarketCommand::PlaceOrder {
            order: order("taker", 20, OrderSide::Buy, 55, 2),
            reply_to: taker_reply_tx,
        })
        .await
        .expect("command receiver should stay open");
    let taker_result = taker_reply_rx
        .await
        .expect("taker reply should be sent")
        .expect("taker should rest because maker was canceled");
    assert!(taker_result.trades.is_empty());
    assert!(taker_result.rested);

    drop(command_tx);
    shard_task.await.expect("shard task should finish");
}

fn symbol() -> Symbol {
    Symbol::new("mkt-election")
}

fn order(order_id: &str, user_id: u64, side: OrderSide, price: u64, quantity: u64) -> NewOrder {
    typed_order(order_id, user_id, side, OrderType::Limit, price, quantity)
}

fn typed_order(
    order_id: &str,
    user_id: u64,
    side: OrderSide,
    order_type: OrderType,
    price: u64,
    quantity: u64,
) -> NewOrder {
    NewOrder {
        order_id: id(order_id),
        user_id,
        symbol: symbol(),
        side,
        order_type,
        price,
        quantity,
    }
}

#[tokio::test]
async fn shard_processes_place_orders_in_command_order() {
    let (command_tx, command_rx) = mpsc::channel(8);
    let (event_tx, mut event_rx) = mpsc::channel(8);
    let shard = MarketShard::new(symbol());
    let shard_task = tokio::spawn(shard.run(command_rx, event_tx));

    let (maker_reply_tx, maker_reply_rx) = oneshot::channel();
    command_tx
        .send(MarketCommand::PlaceOrder {
            order: order("maker", 10, OrderSide::Sell, 55, 2),
            reply_to: maker_reply_tx,
        })
        .await
        .expect("command receiver should stay open");

    let maker_result = maker_reply_rx
        .await
        .expect("maker reply should be sent")
        .expect("maker should rest");
    assert!(maker_result.rested);

    let first_event = event_rx
        .recv()
        .await
        .expect("maker event should be published");
    assert_eq!(
        first_event,
        MarketEvent::OrderAccepted {
            order_id: id("maker")
        }
    );
    let second_event = event_rx
        .recv()
        .await
        .expect("maker resting event should be published");
    assert_eq!(
        second_event,
        MarketEvent::OrderResting {
            order_id: id("maker")
        }
    );

    let (taker_reply_tx, taker_reply_rx) = oneshot::channel();
    command_tx
        .send(MarketCommand::PlaceOrder {
            order: order("taker", 20, OrderSide::Buy, 55, 2),
            reply_to: taker_reply_tx,
        })
        .await
        .expect("command receiver should stay open");

    let taker_result = taker_reply_rx
        .await
        .expect("taker reply should be sent")
        .expect("taker should match");
    assert_eq!(taker_result.trades.len(), 1);
    assert_eq!(taker_result.trades[0].maker_order_id, id("maker"));
    assert_eq!(taker_result.trades[0].taker_order_id, id("taker"));

    let accepted_event = event_rx
        .recv()
        .await
        .expect("taker accepted event should be published");
    assert_eq!(
        accepted_event,
        MarketEvent::OrderAccepted {
            order_id: id("taker")
        }
    );

    let trade_event = event_rx
        .recv()
        .await
        .expect("trade event should be published");
    assert_eq!(
        trade_event,
        MarketEvent::Trade {
            maker_order_id: id("maker"),
            taker_order_id: id("taker"),
            sequence: 1,
            price: 55,
            quantity: 2,
        }
    );
    let filled_event = event_rx
        .recv()
        .await
        .expect("filled event should be published");
    assert_eq!(
        filled_event,
        MarketEvent::OrderFilled {
            order_id: id("taker"),
            total_filled: 2,
        }
    );

    drop(command_tx);
    shard_task.await.expect("shard task should finish");
}

#[tokio::test]
async fn shard_publishes_partial_and_ioc_cancel_status_events() {
    let (command_tx, command_rx) = mpsc::channel(8);
    let (event_tx, mut event_rx) = mpsc::channel(8);
    let shard = MarketShard::new(symbol());
    let shard_task = tokio::spawn(shard.run(command_rx, event_tx));

    let (maker_reply_tx, maker_reply_rx) = oneshot::channel();
    command_tx
        .send(MarketCommand::PlaceOrder {
            order: order("maker", 10, OrderSide::Sell, 55, 2),
            reply_to: maker_reply_tx,
        })
        .await
        .expect("command receiver should stay open");
    maker_reply_rx
        .await
        .expect("maker reply should be sent")
        .expect("maker should rest");
    event_rx
        .recv()
        .await
        .expect("accepted event should publish");
    event_rx.recv().await.expect("resting event should publish");

    let (taker_reply_tx, taker_reply_rx) = oneshot::channel();
    command_tx
        .send(MarketCommand::PlaceOrder {
            order: typed_order(
                "ioc-taker",
                20,
                OrderSide::Buy,
                OrderType::ImmediateOrCancel,
                55,
                5,
            ),
            reply_to: taker_reply_tx,
        })
        .await
        .expect("command receiver should stay open");
    taker_reply_rx
        .await
        .expect("taker reply should be sent")
        .expect("ioc taker should match partially");

    assert_eq!(
        event_rx
            .recv()
            .await
            .expect("accepted event should publish"),
        MarketEvent::OrderAccepted {
            order_id: id("ioc-taker")
        }
    );
    assert_eq!(
        event_rx.recv().await.expect("trade event should publish"),
        MarketEvent::Trade {
            maker_order_id: id("maker"),
            taker_order_id: id("ioc-taker"),
            sequence: 1,
            price: 55,
            quantity: 2,
        }
    );
    assert_eq!(
        event_rx.recv().await.expect("partial event should publish"),
        MarketEvent::OrderPartiallyFilled {
            order_id: id("ioc-taker"),
            filled_now: 2,
            remaining: 3,
        }
    );
    assert_eq!(
        event_rx.recv().await.expect("cancel event should publish"),
        MarketEvent::OrderCancelled {
            order_id: id("ioc-taker"),
            canceled_quantity: 3,
        }
    );

    drop(command_tx);
    shard_task.await.expect("shard task should finish");
}
