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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    ImmediateOrCancel,
    FillOrKill,
    PostOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOrder {
    pub order_id: OrderId,
    pub user_id: u64,
    pub symbol: Symbol,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Price,
    pub quantity: Quantity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeEvent {
    pub sequence: Sequence,
    pub symbol: Symbol,
    pub maker_order_id: OrderId,
    pub taker_order_id: OrderId,
    pub maker_user_id: u64,
    pub taker_user_id: u64,
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
    PriceOutOfRange,
    ZeroQuantity,
    UnknownOrder(OrderId),
    FillOrKillWouldNotFill,
    PostOnlyWouldCross,
    SelfTradePrevented,
}

#[derive(Debug, Clone)]
struct RestingOrder {
    order_id: OrderId,
    user_id: u64,
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

        if order.order_type == OrderType::PostOnly && self.would_cross(&order) {
            return Err(OrderBookError::PostOnlyWouldCross);
        }

        if self.would_self_trade(&order) {
            return Err(OrderBookError::SelfTradePrevented);
        }

        if order.order_type == OrderType::FillOrKill && !self.can_fill_completely(&order) {
            return Err(OrderBookError::FillOrKillWouldNotFill);
        }

        let mut remaining_quantity = order.quantity;
        let trades = match order.side {
            OrderSide::Buy => self.match_buy(&order, &mut remaining_quantity),
            OrderSide::Sell => self.match_sell(&order, &mut remaining_quantity),
        };

        let rested = remaining_quantity > 0 && order.order_type == OrderType::Limit;
        if rested {
            self.rest_order(
                order.side,
                order.price,
                order.order_id.clone(),
                order.user_id,
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

        if order.price >= 100 {
            return Err(OrderBookError::PriceOutOfRange);
        }

        if order.quantity == 0 {
            return Err(OrderBookError::ZeroQuantity);
        }

        if self.orders.contains_key(&order.order_id) {
            return Err(OrderBookError::DuplicateOrderId(order.order_id.clone()));
        }

        Ok(())
    }

    fn would_cross(&self, order: &NewOrder) -> bool {
        match order.side {
            OrderSide::Buy => self
                .asks
                .keys()
                .next()
                .is_some_and(|best_ask| *best_ask <= order.price),
            OrderSide::Sell => self
                .bids
                .keys()
                .next()
                .is_some_and(|Reverse(best_bid)| *best_bid >= order.price),
        }
    }

    fn can_fill_completely(&self, order: &NewOrder) -> bool {
        let mut fillable_quantity = 0;

        match order.side {
            OrderSide::Buy => {
                for (price, level) in &self.asks {
                    if *price > order.price {
                        break;
                    }
                    fillable_quantity += level.iter().map(|resting| resting.quantity).sum::<u64>();
                    if fillable_quantity >= order.quantity {
                        return true;
                    }
                }
            }
            OrderSide::Sell => {
                for (Reverse(price), level) in &self.bids {
                    if *price < order.price {
                        break;
                    }
                    fillable_quantity += level.iter().map(|resting| resting.quantity).sum::<u64>();
                    if fillable_quantity >= order.quantity {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn would_self_trade(&self, order: &NewOrder) -> bool {
        match order.side {
            OrderSide::Buy => self
                .asks
                .iter()
                .take_while(|(price, _)| **price <= order.price)
                .filter_map(|(_, level)| level.front())
                .any(|maker| maker.user_id == order.user_id),
            OrderSide::Sell => self
                .bids
                .iter()
                .take_while(|(Reverse(price), _)| *price >= order.price)
                .filter_map(|(_, level)| level.front())
                .any(|maker| maker.user_id == order.user_id),
        }
    }

    fn rest_order(
        &mut self,
        side: OrderSide,
        price: Price,
        order_id: OrderId,
        user_id: u64,
        quantity: Quantity,
    ) {
        let resting = RestingOrder {
            order_id: order_id.clone(),
            user_id,
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
        let maker_user_id;
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
            maker_user_id = maker.user_id;

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

        trades.push(self.trade_event(order, &maker_order_id, maker_user_id, price, fill_quantity));
    }

    fn match_at_bid_price(
        &mut self,
        order: &NewOrder,
        price: Price,
        remaining_quantity: &mut Quantity,
        trades: &mut Vec<TradeEvent>,
    ) {
        let maker_order_id;
        let maker_user_id;
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
            maker_user_id = maker.user_id;

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

        trades.push(self.trade_event(order, &maker_order_id, maker_user_id, price, fill_quantity));
    }

    fn trade_event(
        &mut self,
        taker: &NewOrder,
        maker_order_id: &OrderId,
        maker_user_id: u64,
        price: Price,
        quantity: Quantity,
    ) -> TradeEvent {
        let event = TradeEvent {
            sequence: self.next_sequence,
            symbol: self.symbol.clone(),
            maker_order_id: maker_order_id.clone(),
            taker_order_id: taker.order_id.clone(),
            maker_user_id,
            taker_user_id: taker.user_id,
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
        let user_id = match side {
            OrderSide::Buy => 101,
            OrderSide::Sell => 202,
        };
        user_order(order_id, user_id, side, price, quantity)
    }

    fn user_order(
        order_id: &str,
        user_id: u64,
        side: OrderSide,
        price: Price,
        quantity: Quantity,
    ) -> NewOrder {
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

    fn typed_order(
        order_id: &str,
        side: OrderSide,
        order_type: OrderType,
        price: Price,
        quantity: Quantity,
    ) -> NewOrder {
        NewOrder {
            order_id: id(order_id),
            user_id: match side {
                OrderSide::Buy => 101,
                OrderSide::Sell => 202,
            },
            symbol: symbol(),
            side,
            order_type,
            price,
            quantity,
        }
    }

    #[test]
    fn non_crossing_buy_rests_with_full_remaining_quantity() {
        let mut book = OrderBook::new(symbol());

        let result = book
            .add_order(order("buy-1", OrderSide::Buy, 50, 7))
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
            .add_order(order("sell-1", OrderSide::Sell, 60, 4))
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

        book.add_order(order("dup", OrderSide::Buy, 50, 1))
            .expect("first order should be accepted");

        let error = book
            .add_order(order("dup", OrderSide::Sell, 51, 1))
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
    fn price_at_or_above_settlement_value_is_rejected() {
        let mut book = OrderBook::new(symbol());

        let at_settlement = book
            .add_order(order("price-100", OrderSide::Buy, 100, 1))
            .expect_err("price at settlement value should be rejected");
        let above_settlement = book
            .add_order(order("price-101", OrderSide::Buy, 101, 1))
            .expect_err("price above settlement value should be rejected");

        assert_eq!(at_settlement, OrderBookError::PriceOutOfRange);
        assert_eq!(above_settlement, OrderBookError::PriceOutOfRange);
    }

    #[test]
    fn zero_quantity_is_rejected() {
        let mut book = OrderBook::new(symbol());

        let error = book
            .add_order(order("bad-qty", OrderSide::Buy, 50, 0))
            .expect_err("zero quantity should be rejected");

        assert_eq!(error, OrderBookError::ZeroQuantity);
    }

    #[test]
    fn symbol_mismatch_is_rejected() {
        let mut book = OrderBook::new(symbol());
        let mut wrong_symbol = order("wrong-symbol", OrderSide::Buy, 50, 1);
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
        book.add_order(order("maker-sell", OrderSide::Sell, 55, 3))
            .expect("maker should rest");

        let result = book
            .add_order(order("taker-buy", OrderSide::Buy, 60, 2))
            .expect("taker should match");

        assert_eq!(
            result.trades,
            vec![TradeEvent {
                sequence: 1,
                symbol: symbol(),
                maker_order_id: id("maker-sell"),
                taker_order_id: id("taker-buy"),
                maker_user_id: 202,
                taker_user_id: 101,
                aggressor_side: OrderSide::Buy,
                price: 55,
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
        book.add_order(order("maker-buy", OrderSide::Buy, 50, 5))
            .expect("maker should rest");

        let result = book
            .add_order(order("taker-sell", OrderSide::Sell, 45, 4))
            .expect("taker should match");

        assert_eq!(
            result.trades,
            vec![TradeEvent {
                sequence: 1,
                symbol: symbol(),
                maker_order_id: id("maker-buy"),
                taker_order_id: id("taker-sell"),
                maker_user_id: 101,
                taker_user_id: 202,
                aggressor_side: OrderSide::Sell,
                price: 50,
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
        book.add_order(order("older-worse", OrderSide::Sell, 55, 1))
            .expect("older worse ask should rest");
        book.add_order(order("newer-better", OrderSide::Sell, 51, 1))
            .expect("newer better ask should rest");

        let result = book
            .add_order(order("buyer", OrderSide::Buy, 60, 2))
            .expect("buyer should match both asks");

        assert_eq!(result.trades[0].maker_order_id, id("newer-better"));
        assert_eq!(result.trades[0].price, 51);
        assert_eq!(result.trades[1].maker_order_id, id("older-worse"));
        assert_eq!(result.trades[1].price, 55);
    }

    #[test]
    fn same_price_matches_fifo() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("first", OrderSide::Sell, 55, 1))
            .expect("first ask should rest");
        book.add_order(order("second", OrderSide::Sell, 55, 1))
            .expect("second ask should rest");

        let result = book
            .add_order(order("buyer", OrderSide::Buy, 55, 2))
            .expect("buyer should match both asks");

        assert_eq!(result.trades[0].maker_order_id, id("first"));
        assert_eq!(result.trades[1].maker_order_id, id("second"));
    }

    #[test]
    fn partial_fill_leaves_maker_remainder_in_place() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("maker", OrderSide::Sell, 55, 10))
            .expect("maker should rest");

        let result = book
            .add_order(order("buyer", OrderSide::Buy, 55, 4))
            .expect("buyer should partially fill maker");

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].quantity, 4);
        assert_eq!(book.remaining_quantity(&id("maker")), Some(6));
    }

    #[test]
    fn partial_taker_remainder_rests_when_no_more_crossing_liquidity_exists() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("maker", OrderSide::Sell, 55, 2))
            .expect("maker should rest");

        let result = book
            .add_order(order("buyer", OrderSide::Buy, 55, 5))
            .expect("buyer should partially match then rest");

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].quantity, 2);
        assert_eq!(result.remaining_quantity, 3);
        assert!(result.rested);
        assert_eq!(book.remaining_quantity(&id("buyer")), Some(3));
        assert_eq!(book.remaining_quantity(&id("maker")), None);
    }

    #[test]
    fn ioc_cancels_unfilled_remainder_without_resting() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("maker", OrderSide::Sell, 55, 2))
            .expect("maker should rest");

        let result = book
            .add_order(typed_order(
                "ioc-buyer",
                OrderSide::Buy,
                OrderType::ImmediateOrCancel,
                55,
                5,
            ))
            .expect("ioc order should be accepted");

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].quantity, 2);
        assert_eq!(result.remaining_quantity, 3);
        assert!(!result.rested);
        assert_eq!(book.remaining_quantity(&id("ioc-buyer")), None);
    }

    #[test]
    fn fok_rejects_when_full_quantity_cannot_fill() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("maker", OrderSide::Sell, 55, 2))
            .expect("maker should rest");

        let result = book.add_order(typed_order(
            "fok-buyer",
            OrderSide::Buy,
            OrderType::FillOrKill,
            55,
            5,
        ));

        assert_eq!(result, Err(OrderBookError::FillOrKillWouldNotFill));
        assert_eq!(book.remaining_quantity(&id("maker")), Some(2));
        assert_eq!(book.remaining_quantity(&id("fok-buyer")), None);
    }

    #[test]
    fn fok_fills_when_full_quantity_is_available() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("maker-a", OrderSide::Sell, 54, 2))
            .expect("first maker should rest");
        book.add_order(order("maker-b", OrderSide::Sell, 55, 3))
            .expect("second maker should rest");

        let result = book
            .add_order(typed_order(
                "fok-buyer",
                OrderSide::Buy,
                OrderType::FillOrKill,
                55,
                5,
            ))
            .expect("fok should fill");

        assert_eq!(result.trades.len(), 2);
        assert_eq!(result.remaining_quantity, 0);
        assert!(!result.rested);
        assert_eq!(book.remaining_quantity(&id("maker-a")), None);
        assert_eq!(book.remaining_quantity(&id("maker-b")), None);
    }

    #[test]
    fn post_only_rejects_when_order_would_cross() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("maker", OrderSide::Sell, 55, 2))
            .expect("maker should rest");

        let result = book.add_order(typed_order(
            "post-only-buyer",
            OrderSide::Buy,
            OrderType::PostOnly,
            55,
            1,
        ));

        assert_eq!(result, Err(OrderBookError::PostOnlyWouldCross));
        assert_eq!(book.remaining_quantity(&id("maker")), Some(2));
        assert_eq!(book.remaining_quantity(&id("post-only-buyer")), None);
    }

    #[test]
    fn self_trade_prevention_rejects_incoming_order_without_touching_maker() {
        let mut book = OrderBook::new(symbol());
        book.add_order(user_order("maker", 7, OrderSide::Sell, 55, 2))
            .expect("maker should rest");

        let result = book.add_order(user_order("same-user-buyer", 7, OrderSide::Buy, 55, 1));

        assert_eq!(result, Err(OrderBookError::SelfTradePrevented));
        assert_eq!(book.remaining_quantity(&id("maker")), Some(2));
        assert_eq!(book.remaining_quantity(&id("same-user-buyer")), None);
    }

    #[test]
    fn cancel_removes_resting_order_and_prevents_future_matching() {
        let mut book = OrderBook::new(symbol());
        book.add_order(order("seller", OrderSide::Sell, 55, 3))
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
            .add_order(order("buyer", OrderSide::Buy, 60, 3))
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
