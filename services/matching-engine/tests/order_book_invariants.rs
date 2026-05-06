#![allow(clippy::expect_used)]

use matching_engine::order_book::{NewOrder, OrderBook, OrderId, OrderSide, OrderType, Symbol};

#[test]
fn generated_order_stream_preserves_book_and_sequence_invariants() {
    let mut book = OrderBook::new(symbol());
    let mut rng = Lcg::new(42);
    let mut last_trade_sequence = 0;

    for index in 0..500 {
        let side = if rng.next() & 1 == 0 {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        };
        let order = NewOrder {
            order_id: OrderId::new(format!("ord-{index}")),
            user_id: 1_000 + u64::from(rng.next() % 97),
            symbol: symbol(),
            side,
            order_type: order_type(rng.next()),
            price: 1 + u64::from(rng.next() % 98),
            quantity: 1 + u64::from(rng.next() % 10),
        };

        if let Ok(result) = book.add_order(order) {
            for trade in result.trades {
                assert!(
                    trade.sequence > last_trade_sequence,
                    "trade sequence must be strictly increasing"
                );
                last_trade_sequence = trade.sequence;
            }
        }

        let snapshot = book.snapshot();
        if let (Some(best_bid), Some(best_ask)) = (snapshot.bids.first(), snapshot.asks.first()) {
            assert!(
                best_bid.price < best_ask.price,
                "resting book must never be crossed"
            );
        }
    }
}

fn symbol() -> Symbol {
    Symbol::new("mkt-election")
}

fn order_type(value: u32) -> OrderType {
    match value % 4 {
        0 => OrderType::Limit,
        1 => OrderType::ImmediateOrCancel,
        2 => OrderType::FillOrKill,
        _ => OrderType::PostOnly,
    }
}

struct Lcg {
    state: u32,
}

impl Lcg {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        self.state
    }
}
