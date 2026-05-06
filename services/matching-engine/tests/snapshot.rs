#![allow(clippy::expect_used)]

use matching_engine::{
    order_book::{NewOrder, OrderBook, OrderId, OrderSide, OrderType, Symbol},
    snapshot::{read_snapshot, write_snapshot},
};

#[test]
fn snapshot_round_trips_resting_orders_and_sequence() {
    let mut book = OrderBook::new(symbol());
    book.add_order(order("maker", 10, OrderSide::Sell, 55, 3))
        .expect("maker should rest");
    book.add_order(order("taker", 20, OrderSide::Buy, 55, 1))
        .expect("taker should match");
    let snapshot = book.snapshot();
    let path = unique_snapshot_path();

    write_snapshot(&path, &snapshot).expect("snapshot should write");
    let restored = read_snapshot(&path).expect("snapshot should read");

    assert_eq!(restored, snapshot);
    let restored_book = OrderBook::from_snapshot(restored);
    assert_eq!(
        restored_book.remaining_quantity(&OrderId::new("maker")),
        Some(2)
    );
}

fn symbol() -> Symbol {
    Symbol::new("mkt-election")
}

fn order(order_id: &str, user_id: u64, side: OrderSide, price: u64, quantity: u64) -> NewOrder {
    NewOrder {
        order_id: OrderId::new(order_id),
        user_id,
        symbol: symbol(),
        side,
        order_type: OrderType::Limit,
        price,
        quantity,
    }
}

fn unique_snapshot_path() -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("prophetica-engine-snapshot-{nanos}.json"))
}
