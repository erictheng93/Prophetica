#![allow(clippy::expect_used)]

use std::path::PathBuf;

use matching_engine::order_book::{NewOrder, OrderBook, OrderId, OrderSide, OrderType, Symbol};
use matching_engine::wal::{WalRecord, append_record, read_records, replay_records};

fn id(value: &str) -> OrderId {
    OrderId::new(value)
}

fn symbol() -> Symbol {
    Symbol::new("mkt-election")
}

fn order(order_id: &str, user_id: u64, side: OrderSide, price: u64, quantity: u64) -> NewOrder {
    NewOrder {
        order_id: id(order_id),
        user_id,
        symbol: symbol(),
        side,
        order_type: OrderType::Limit,
        price,
        quantity,
    }
}

#[test]
fn wal_records_round_trip_and_replay_into_order_book() {
    let wal_path = temp_wal_path();

    append_record(
        &wal_path,
        &WalRecord::PlaceOrder(order("maker", 10, OrderSide::Sell, 55, 2)),
    )
    .expect("maker record should append");
    append_record(
        &wal_path,
        &WalRecord::PlaceOrder(order("first-taker", 20, OrderSide::Buy, 55, 1)),
    )
    .expect("taker record should append");

    let records = read_records(&wal_path).expect("records should read");
    assert_eq!(records.len(), 2);

    let mut recovered =
        replay_records(symbol(), records).expect("records should replay into order book");
    assert_eq!(recovered.remaining_quantity(&id("maker")), Some(1));

    let result = recovered
        .add_order(order("second-taker", 21, OrderSide::Buy, 55, 1))
        .expect("recovered book should keep sequence");
    assert_eq!(result.trades[0].sequence, 2);
    assert_eq!(result.trades[0].maker_order_id, id("maker"));

    std::fs::remove_file(wal_path).expect("temp wal should be removed");
}

#[test]
fn wal_replay_produces_byte_equal_snapshot() {
    let wal_path = temp_wal_path();
    let records = vec![
        WalRecord::PlaceOrder(order("maker-a", 10, OrderSide::Sell, 55, 2)),
        WalRecord::PlaceOrder(order("maker-b", 11, OrderSide::Sell, 56, 3)),
        WalRecord::PlaceOrder(order("taker", 20, OrderSide::Buy, 55, 1)),
    ];
    let mut live = OrderBook::new(symbol());
    for record in &records {
        append_record(&wal_path, record).expect("wal record should append");
        if let WalRecord::PlaceOrder(order) = record {
            live.add_order(order.clone())
                .expect("live order should apply");
        }
    }

    let recovered = replay_records(
        symbol(),
        read_records(&wal_path).expect("records should read"),
    )
    .expect("wal should replay");
    let live_bytes = serde_json::to_vec(&live.snapshot()).expect("live snapshot should serialize");
    let recovered_bytes =
        serde_json::to_vec(&recovered.snapshot()).expect("recovered snapshot should serialize");

    assert_eq!(recovered_bytes, live_bytes);
    std::fs::remove_file(wal_path).expect("temp wal should be removed");
}

fn temp_wal_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "prophetica-matching-engine-{}.wal.jsonl",
        ulid::Ulid::new()
    ))
}
