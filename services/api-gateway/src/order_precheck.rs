#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderKind {
    Limit,
    Market,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Bid,
    Ask,
}

#[must_use]
pub fn required_minor_units(
    order_kind: OrderKind,
    side: OrderSide,
    price_pts: i64,
    quantity: i64,
    fee_bps_cap: u32,
) -> i64 {
    let quantity = quantity.max(0);
    let max_trade_value = match (order_kind, side) {
        (OrderKind::Limit, OrderSide::Bid) => price_pts.max(0).saturating_mul(quantity),
        (OrderKind::Market, OrderSide::Bid) => 100_i64.saturating_mul(quantity),
        (_, OrderSide::Ask) => 0,
    };
    let max_fee = max_trade_value.saturating_mul(i64::from(fee_bps_cap)) / 10_000;
    max_trade_value.saturating_add(max_fee)
}

#[cfg(test)]
mod tests {
    use super::{OrderKind, OrderSide, required_minor_units};

    #[test]
    fn limit_bid_uses_limit_price_plus_fee_cap() {
        assert_eq!(
            required_minor_units(OrderKind::Limit, OrderSide::Bid, 65, 10, 200),
            663
        );
    }

    #[test]
    fn market_bid_uses_worst_case_price_plus_fee_cap() {
        assert_eq!(
            required_minor_units(OrderKind::Market, OrderSide::Bid, 65, 10, 250),
            1025
        );
    }

    #[test]
    fn ask_orders_skip_gateway_balance_precheck() {
        assert_eq!(
            required_minor_units(OrderKind::Limit, OrderSide::Ask, 65, 10, 200),
            0
        );
        assert_eq!(
            required_minor_units(OrderKind::Market, OrderSide::Ask, 65, 10, 200),
            0
        );
    }
}
