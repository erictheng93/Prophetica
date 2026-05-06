use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, VecDeque};

pub type Price = u64;
pub type Quantity = u64;
pub type Sequence = u64;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderId(String);

impl OrderId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Symbol(String);

impl Symbol {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOrder {
    pub order_id: OrderId,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub price: Price,
    pub quantity: Quantity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeEvent {
    pub sequence: Sequence,
    pub symbol: Symbol,
    pub maker_order_id: OrderId,
    pub taker_order_id: OrderId,
    pub aggressor_side: OrderSide,
    pub price: Price,
    pub quantity: Quantity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddOrderResult {
    pub order_id: OrderId,
    pub trades: Vec<TradeEvent>,
    pub remaining_quantity: Quantity,
    pub rested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelOrderResult {
    pub order_id: OrderId,
    pub canceled_quantity: Quantity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderBookError {
    DuplicateOrderId(OrderId),
    SymbolMismatch { expected: Symbol, actual: Symbol },
    ZeroPrice,
    ZeroQuantity,
    UnknownOrder(OrderId),
}

#[derive(Debug, Clone)]
struct RestingOrder {
    order_id: OrderId,
    quantity: Quantity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrderLocation {
    side: OrderSide,
    price: Price,
}

#[derive(Debug)]
pub struct OrderBook {
    symbol: Symbol,
    bids: BTreeMap<Reverse<Price>, VecDeque<RestingOrder>>,
    asks: BTreeMap<Price, VecDeque<RestingOrder>>,
    orders: HashMap<OrderId, OrderLocation>,
    next_sequence: Sequence,
}

impl OrderBook {
    #[must_use]
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: HashMap::new(),
            next_sequence: 1,
        }
    }

    /// Adds a limit order to the book, matching against crossing resting
    /// liquidity before resting any unfilled remainder.
    ///
    /// # Errors
    ///
    /// Returns an [`OrderBookError`] when the order has a duplicate id, a
    /// different symbol, a zero price, or a zero quantity.
    pub fn add_order(&mut self, order: NewOrder) -> Result<AddOrderResult, OrderBookError> {
        self.validate_new_order(&order)?;

        let mut remaining_quantity = order.quantity;
        let trades = match order.side {
            OrderSide::Buy => self.match_buy(&order, &mut remaining_quantity),
            OrderSide::Sell => self.match_sell(&order, &mut remaining_quantity),
        };

        let rested = remaining_quantity > 0;
        if rested {
            self.rest_order(
                order.side,
                order.price,
                order.order_id.clone(),
                remaining_quantity,
            );
        }

        Ok(AddOrderResult {
            order_id: order.order_id,
            trades,
            remaining_quantity,
            rested,
        })
    }

    /// Cancels an open resting order.
    ///
    /// # Errors
    ///
    /// Returns [`OrderBookError::UnknownOrder`] when the order id is not
    /// currently resting in this book.
    pub fn cancel_order(
        &mut self,
        order_id: &OrderId,
    ) -> Result<CancelOrderResult, OrderBookError> {
        let location = self
            .orders
            .remove(order_id)
            .ok_or_else(|| OrderBookError::UnknownOrder(order_id.clone()))?;

        let canceled_quantity = match location.side {
            OrderSide::Buy => {
                let (quantity, remove_level) =
                    Self::remove_from_level(self.bids.get_mut(&Reverse(location.price)), order_id)
                        .ok_or_else(|| OrderBookError::UnknownOrder(order_id.clone()))?;
                if remove_level {
                    self.bids.remove(&Reverse(location.price));
                }
                quantity
            }
            OrderSide::Sell => {
                let (quantity, remove_level) =
                    Self::remove_from_level(self.asks.get_mut(&location.price), order_id)
                        .ok_or_else(|| OrderBookError::UnknownOrder(order_id.clone()))?;
                if remove_level {
                    self.asks.remove(&location.price);
                }
                quantity
            }
        };

        Ok(CancelOrderResult {
            order_id: order_id.clone(),
            canceled_quantity,
        })
    }

    #[must_use]
    pub fn remaining_quantity(&self, order_id: &OrderId) -> Option<Quantity> {
        match self.orders.get(order_id)? {
            OrderLocation {
                side: OrderSide::Buy,
                price,
            } => self
                .bids
                .get(&Reverse(*price))?
                .iter()
                .find(|order| &order.order_id == order_id)
                .map(|order| order.quantity),
            OrderLocation {
                side: OrderSide::Sell,
                price,
            } => self
                .asks
                .get(price)?
                .iter()
                .find(|order| &order.order_id == order_id)
                .map(|order| order.quantity),
        }
    }

    fn validate_new_order(&self, order: &NewOrder) -> Result<(), OrderBookError> {
        if order.symbol != self.symbol {
            return Err(OrderBookError::SymbolMismatch {
                expected: self.symbol.clone(),
                actual: order.symbol.clone(),
            });
        }

        if order.price == 0 {
            return Err(OrderBookError::ZeroPrice);
        }

        if order.quantity == 0 {
            return Err(OrderBookError::ZeroQuantity);
        }

        if self.orders.contains_key(&order.order_id) {
            return Err(OrderBookError::DuplicateOrderId(order.order_id.clone()));
        }

        Ok(())
    }

    fn rest_order(&mut self, side: OrderSide, price: Price, order_id: OrderId, quantity: Quantity) {
        let resting = RestingOrder {
            order_id: order_id.clone(),
            quantity,
        };

        match side {
            OrderSide::Buy => {
                self.bids
                    .entry(Reverse(price))
                    .or_default()
                    .push_back(resting);
            }
            OrderSide::Sell => {
                self.asks.entry(price).or_default().push_back(resting);
            }
        }

        self.orders.insert(order_id, OrderLocation { side, price });
    }

    fn match_buy(
        &mut self,
        order: &NewOrder,
        remaining_quantity: &mut Quantity,
    ) -> Vec<TradeEvent> {
        let mut trades = Vec::new();

        while *remaining_quantity > 0 {
            let Some(best_price) = self.asks.keys().next().copied() else {
                break;
            };

            if best_price > order.price {
                break;
            }

            self.match_at_ask_price(order, best_price, remaining_quantity, &mut trades);
        }

        trades
    }

    fn match_sell(
        &mut self,
        order: &NewOrder,
        remaining_quantity: &mut Quantity,
    ) -> Vec<TradeEvent> {
        let mut trades = Vec::new();

        while *remaining_quantity > 0 {
            let Some(Reverse(best_price)) = self.bids.keys().next().copied() else {
                break;
            };

            if best_price < order.price {
                break;
            }

            self.match_at_bid_price(order, best_price, remaining_quantity, &mut trades);
        }

        trades
    }

    fn match_at_ask_price(
        &mut self,
        order: &NewOrder,
        price: Price,
        remaining_quantity: &mut Quantity,
        trades: &mut Vec<TradeEvent>,
    ) {
        let maker_order_id;
        let fill_quantity;
        let remove_level;

        {
            let Some(level) = self.asks.get_mut(&price) else {
                return;
            };
            let Some(maker) = level.front_mut() else {
                self.asks.remove(&price);
                return;
            };
            fill_quantity = (*remaining_quantity).min(maker.quantity);
            maker_order_id = maker.order_id.clone();

            maker.quantity -= fill_quantity;
            *remaining_quantity -= fill_quantity;

            if maker.quantity == 0 {
                if let Some(filled) = level.pop_front() {
                    self.orders.remove(&filled.order_id);
                }
            }

            remove_level = level.is_empty();
        }

        if remove_level {
            self.asks.remove(&price);
        }

        trades.push(self.trade_event(order, &maker_order_id, price, fill_quantity));
    }

    fn match_at_bid_price(
        &mut self,
        order: &NewOrder,
        price: Price,
        remaining_quantity: &mut Quantity,
        trades: &mut Vec<TradeEvent>,
    ) {
        let maker_order_id;
        let fill_quantity;
        let remove_level;

        {
            let Some(level) = self.bids.get_mut(&Reverse(price)) else {
                return;
            };
            let Some(maker) = level.front_mut() else {
                self.bids.remove(&Reverse(price));
                return;
            };
            fill_quantity = (*remaining_quantity).min(maker.quantity);
            maker_order_id = maker.order_id.clone();

            maker.quantity -= fill_quantity;
            *remaining_quantity -= fill_quantity;

            if maker.quantity == 0 {
                if let Some(filled) = level.pop_front() {
                    self.orders.remove(&filled.order_id);
                }
            }

            remove_level = level.is_empty();
        }

        if remove_level {
            self.bids.remove(&Reverse(price));
        }

        trades.push(self.trade_event(order, &maker_order_id, price, fill_quantity));
    }

    fn trade_event(
        &mut self,
        taker: &NewOrder,
        maker_order_id: &OrderId,
        price: Price,
        quantity: Quantity,
    ) -> TradeEvent {
        let event = TradeEvent {
            sequence: self.next_sequence,
            symbol: self.symbol.clone(),
            maker_order_id: maker_order_id.clone(),
            taker_order_id: taker.order_id.clone(),
            aggressor_side: taker.side,
            price,
            quantity,
        };
        self.next_sequence += 1;
        event
    }

    fn remove_from_level(
        level: Option<&mut VecDeque<RestingOrder>>,
        order_id: &OrderId,
    ) -> Option<(Quantity, bool)> {
        let level = level?;
        let position = level.iter().position(|order| &order.order_id == order_id)?;
        let removed = level.remove(position)?;

        Some((removed.quantity, level.is_empty()))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn id(value: &str) -> OrderId {
        OrderId::new(value)
    }

    fn symbol() -> Symbol {
        Symbol::new("BTC-USD")
    }

    fn order(order_id: &str, side: OrderSide, price: Price, quantity: Quantity) -> NewOrder {
        NewOrder {
            order_id: id(order_id),
            symbol: symbol(),
            side,
            price,
            quantity,
        }
    }

    #[test]
    fn non_crossing_buy_rests_with_full_remaining_quantity() {
        let mut book = OrderBook::new(symbol());

        let result = book
            .add_order(order("buy-1", OrderSide::Buy, 100, 7))
            .expect("order should be accepted");

        assert_eq!(result.order_id, id("buy-1"));
        assert!(result.trades.is_empty());
        assert_eq!(result.remaining_quantity, 7);
        assert!(result.rested);
        assert_eq!(book.remaining_quantity(&id("buy-1")), Some(7));
    }

    #[test]
    fn non_crossing_sell_rests_with_full_remaining_quantity() {
        let mut book = OrderBook::new(symbol());

        let result = book
            .add_order(order("sell-1", OrderSide::Sell, 110, 4))
            .expect("order should be accepted");

        assert_eq!(result.order_id, id("sell-1"));
        assert!(result.trades.is_empty());
        assert_eq!(result.remaining_quantity, 4);
        assert!(result.rested);
        assert_eq!(book.remaining_quantity(&id("sell-1")), Some(4));
    }

    #[test]
    fn duplicate_order_id_is_rejected_while_order_is_resting() {
        let mut book = OrderBook::new(symbol());

        book.add_order(order("dup", OrderSide::Buy, 100, 1))
            .expect("first order should be accepted");

        let error = book
            .add_order(order("dup", OrderSide::Sell, 101, 1))
            .expect_err("duplicate order id should be rejected");

        assert_eq!(error, OrderBookError::DuplicateOrderId(id("dup")));
    }

    #[test]
    fn zero_price_is_rejected() {
        let mut book = OrderBook::new(symbol());

        let error = book
            .add_order(order("bad-price", OrderSide::Buy, 0, 1))
            .expect_err("zero price should be rejected");

        assert_eq!(error, OrderBookError::ZeroPrice);
    }

    #[test]
    fn zero_quantity_is_rejected() {
        let mut book = OrderBook::new(symbol());

        let error = book
            .add_order(order("bad-qty", OrderSide::Buy, 100, 0))
            .expect_err("zero quantity should be rejected");

        assert_eq!(error, OrderBookError::ZeroQuantity);
    }

    #[test]
    fn symbol_mismatch_is_rejected() {
        let mut book = OrderBook::new(symbol());
        let mut wrong_symbol = order("wrong-symbol", OrderSide::Buy, 100, 1);
        wrong_symbol.symbol = Symbol::new("ETH-USD");

        let error = book
            .add_order(wrong_symbol)
            .expect_err("symbol mismatch should be rejected");

        assert_eq!(
            error,
            OrderBookError::SymbolMismatch {
                expected: symbol(),
                actual: Symbol::new("ETH-USD"),
            }
        );
    }

    #[test]
    fn crossing_buy_trades_at_resting_ask_price() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("maker-sell", OrderSide::Sell, 105, 3))
            .expect("maker should rest");

        let result = book
            .add_order(order("taker-buy", OrderSide::Buy, 110, 2))
            .expect("taker should match");

        assert_eq!(
            result.trades,
            vec![TradeEvent {
                sequence: 1,
                symbol: symbol(),
                maker_order_id: id("maker-sell"),
                taker_order_id: id("taker-buy"),
                aggressor_side: OrderSide::Buy,
                price: 105,
                quantity: 2,
            }]
        );
        assert_eq!(result.remaining_quantity, 0);
        assert!(!result.rested);
        assert_eq!(book.remaining_quantity(&id("maker-sell")), Some(1));
        assert_eq!(book.remaining_quantity(&id("taker-buy")), None);
    }

    #[test]
    fn crossing_sell_trades_at_resting_bid_price() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("maker-buy", OrderSide::Buy, 100, 5))
            .expect("maker should rest");

        let result = book
            .add_order(order("taker-sell", OrderSide::Sell, 95, 4))
            .expect("taker should match");

        assert_eq!(
            result.trades,
            vec![TradeEvent {
                sequence: 1,
                symbol: symbol(),
                maker_order_id: id("maker-buy"),
                taker_order_id: id("taker-sell"),
                aggressor_side: OrderSide::Sell,
                price: 100,
                quantity: 4,
            }]
        );
        assert_eq!(result.remaining_quantity, 0);
        assert!(!result.rested);
        assert_eq!(book.remaining_quantity(&id("maker-buy")), Some(1));
        assert_eq!(book.remaining_quantity(&id("taker-sell")), None);
    }

    #[test]
    fn better_price_matches_before_older_worse_price() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("older-worse", OrderSide::Sell, 105, 1))
            .expect("older worse ask should rest");
        book.add_order(order("newer-better", OrderSide::Sell, 101, 1))
            .expect("newer better ask should rest");

        let result = book
            .add_order(order("buyer", OrderSide::Buy, 110, 2))
            .expect("buyer should match both asks");

        assert_eq!(result.trades[0].maker_order_id, id("newer-better"));
        assert_eq!(result.trades[0].price, 101);
        assert_eq!(result.trades[1].maker_order_id, id("older-worse"));
        assert_eq!(result.trades[1].price, 105);
    }

    #[test]
    fn same_price_matches_fifo() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("first", OrderSide::Sell, 105, 1))
            .expect("first ask should rest");
        book.add_order(order("second", OrderSide::Sell, 105, 1))
            .expect("second ask should rest");

        let result = book
            .add_order(order("buyer", OrderSide::Buy, 105, 2))
            .expect("buyer should match both asks");

        assert_eq!(result.trades[0].maker_order_id, id("first"));
        assert_eq!(result.trades[1].maker_order_id, id("second"));
    }

    #[test]
    fn partial_fill_leaves_maker_remainder_in_place() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("maker", OrderSide::Sell, 105, 10))
            .expect("maker should rest");

        let result = book
            .add_order(order("buyer", OrderSide::Buy, 105, 4))
            .expect("buyer should partially fill maker");

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].quantity, 4);
        assert_eq!(book.remaining_quantity(&id("maker")), Some(6));
    }

    #[test]
    fn partial_taker_remainder_rests_when_no_more_crossing_liquidity_exists() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("maker", OrderSide::Sell, 105, 2))
            .expect("maker should rest");

        let result = book
            .add_order(order("buyer", OrderSide::Buy, 105, 5))
            .expect("buyer should partially match then rest");

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].quantity, 2);
        assert_eq!(result.remaining_quantity, 3);
        assert!(result.rested);
        assert_eq!(book.remaining_quantity(&id("buyer")), Some(3));
        assert_eq!(book.remaining_quantity(&id("maker")), None);
    }

    #[test]
    fn cancel_removes_resting_order_and_prevents_future_matching() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("seller", OrderSide::Sell, 105, 3))
            .expect("seller should rest");

        let cancel = book
            .cancel_order(&id("seller"))
            .expect("resting seller should cancel");

        assert_eq!(
            cancel,
            CancelOrderResult {
                order_id: id("seller"),
                canceled_quantity: 3,
            }
        );
        assert_eq!(book.remaining_quantity(&id("seller")), None);

        let result = book
            .add_order(order("buyer", OrderSide::Buy, 110, 3))
            .expect("buyer should rest because seller was canceled");

        assert!(result.trades.is_empty());
        assert_eq!(result.remaining_quantity, 3);
        assert!(result.rested);
    }

    #[test]
    fn cancel_unknown_order_is_rejected() {
        let mut book = OrderBook::new(symbol());

        let error = book
            .cancel_order(&id("missing"))
            .expect_err("missing order should not cancel");

        assert_eq!(error, OrderBookError::UnknownOrder(id("missing")));
    }
}
