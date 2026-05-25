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
    let max_trade_value = match side {
        OrderSide::Bid => buy_order_lock_minor_units(order_kind, price_pts, quantity),
        OrderSide::Ask => 0,
    };
    let max_fee = match side {
        OrderSide::Bid => {
            max_curve_taker_fee_minor_units(order_kind, price_pts, quantity, fee_bps_cap)
        }
        OrderSide::Ask => 0,
    };
    max_trade_value.saturating_add(max_fee)
}

#[must_use]
pub fn buy_order_lock_minor_units(order_kind: OrderKind, price_pts: i64, quantity: i64) -> i64 {
    let quantity = quantity.max(0);
    match order_kind {
        OrderKind::Limit => price_pts.max(0).saturating_mul(quantity),
        OrderKind::Market => 100_i64.saturating_mul(quantity),
    }
}

#[must_use]
pub fn curve_taker_fee_bps(price_pts: i32, fee_curve_bps: u32) -> u32 {
    let price_pts = u128::try_from(price_pts.clamp(0, 100)).unwrap_or(0);
    let complement_pts = 100_u128.saturating_sub(price_pts);
    let numerator = u128::from(fee_curve_bps)
        .saturating_mul(price_pts)
        .saturating_mul(complement_pts);
    u32::try_from((numerator + 5_000) / 10_000).unwrap_or(u32::MAX)
}

#[must_use]
pub fn curve_taker_fee_minor_units(price_pts: i32, quantity: i64, fee_curve_bps: u32) -> i64 {
    let trade_value = i64::from(price_pts.max(0)).saturating_mul(quantity.max(0));
    trade_value.saturating_mul(i64::from(curve_taker_fee_bps(price_pts, fee_curve_bps))) / 10_000
}

fn max_curve_taker_fee_minor_units(
    order_kind: OrderKind,
    price_pts: i64,
    quantity: i64,
    fee_curve_bps: u32,
) -> i64 {
    let max_price = match order_kind {
        OrderKind::Limit => price_pts.clamp(0, 100),
        OrderKind::Market => 100,
    };
    (0..=max_price)
        .map(|price| {
            curve_taker_fee_minor_units(
                i32::try_from(price).unwrap_or(i32::MAX),
                quantity,
                fee_curve_bps,
            )
        })
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{
        OrderKind, OrderSide, buy_order_lock_minor_units, curve_taker_fee_bps,
        curve_taker_fee_minor_units, required_minor_units,
    };

    #[test]
    fn limit_bid_uses_limit_price_plus_fee_cap() {
        assert_eq!(
            required_minor_units(OrderKind::Limit, OrderSide::Bid, 65, 10, 400),
            655
        );
    }

    #[test]
    fn market_bid_uses_worst_case_price_plus_fee_cap() {
        assert_eq!(
            required_minor_units(OrderKind::Market, OrderSide::Bid, 65, 10, 400),
            1005
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

    #[test]
    fn curve_taker_fee_bps_peaks_at_midpoint_and_falls_at_extremes() {
        assert_eq!(curve_taker_fee_bps(50, 400), 100);
        assert_eq!(curve_taker_fee_bps(10, 400), 36);
        assert_eq!(curve_taker_fee_bps(90, 400), 36);
    }

    #[test]
    fn buy_order_lock_excludes_fee_cap() {
        assert_eq!(buy_order_lock_minor_units(OrderKind::Limit, 65, 10), 650);
        assert_eq!(buy_order_lock_minor_units(OrderKind::Market, 65, 10), 1000);
    }

    #[test]
    fn curve_taker_fee_minor_units_applies_effective_bps_to_notional() {
        assert_eq!(curve_taker_fee_minor_units(50, 10, 400), 5);
        assert_eq!(curve_taker_fee_minor_units(90, 10, 400), 3);
    }
}
