//! Redemption order state machine + idempotency.
//!
//! SPEC-024 §3.2. Server-side states start at [`OrderState::Submitted`] — the
//! `draft` state shown in the spec diagram is a client-side concept and is
//! never persisted here. Transitions are enforced via
//! [`OrderState::can_transition_to`]; the [`RedemptionOrder`] aggregate owns
//! the canonical mutation methods.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Lifecycle state of a [`RedemptionOrder`]. SPEC-024 §3.2 diagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderState {
    /// Received from client; stock not yet reserved, ledger not yet written.
    Submitted,
    /// Stock reserved; awaiting commit (ledger write).
    Accepted,
    /// Ledger `redemption_purchase` written; entitlement issued.
    Committed,
    /// Delivery completed (P3 worker). Terminal unless CS-driven refund.
    Fulfilled,
    /// Cancel-window or CS-approved refund. Terminal.
    Refunded,
    /// TTL reached without fulfillment. Terminal (breakage ledger entry
    /// follows in P4 sweep).
    Expired,
}

impl OrderState {
    /// Returns whether transitioning into `next` is permitted by the SPEC-024
    /// §3.2 diagram. Anything not enumerated (including no-op self-transitions)
    /// is rejected.
    ///
    /// Targets grouped by source for clarity:
    /// - `Submitted → Accepted`
    /// - `Accepted → Committed`
    /// - `Committed → {Fulfilled, Refunded, Expired}` (refund here is cancel-window)
    /// - `Fulfilled → Refunded` (CS-approved only; happy path otherwise terminal)
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        use OrderState::{Accepted, Committed, Expired, Fulfilled, Refunded, Submitted};
        matches!(
            (self, next),
            (Submitted, Accepted)
                | (Accepted, Committed)
                | (Committed, Fulfilled | Refunded | Expired)
                | (Fulfilled, Refunded)
        )
    }

    /// True for [`Refunded`], [`Expired`], and [`Fulfilled`] (CS refund still
    /// possible but `Fulfilled` is the steady-state terminal for happy path).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Refunded | Self::Expired)
    }
}

/// Aggregate root for a redemption attempt. SPEC-024 §3.2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedemptionOrder {
    pub order_id: String,
    pub user_id: String,
    pub sku_id: String,
    pub price_minor_units: i64,
    pub idempotency_key: String,
    pub state: OrderState,
    /// Unix epoch seconds at which state advanced to `Committed`. `None`
    /// while the order is still pre-Committed. Used by the cancel-window
    /// deadline check ([`crate::service::RedemptionService::cancel_order`]).
    #[serde(default)]
    pub committed_at_unix_secs: Option<i64>,
    /// Unix epoch seconds at which the entitlement expires (=
    /// `committed_at + sku.entitlement_ttl_days × 86400`). `None` while
    /// pre-Committed. Drives the P5.2 expiry sweep.
    #[serde(default)]
    pub expires_at_unix_secs: Option<i64>,
}

impl RedemptionOrder {
    /// Constructs a freshly [`OrderState::Submitted`] order. Caller supplies
    /// the (already validated) idempotency key; `order_id` is generated as a
    /// ULID so it sorts by creation time.
    ///
    /// # Errors
    ///
    /// Returns [`OrderError::EmptyField`] when any required field is empty;
    /// [`OrderError::InvalidPrice`] when price is non-positive.
    pub fn submit(
        user_id: impl Into<String>,
        sku_id: impl Into<String>,
        price_minor_units: i64,
        idempotency_key: impl Into<String>,
    ) -> Result<Self, OrderError> {
        let user_id = user_id.into();
        let sku_id = sku_id.into();
        let idempotency_key = idempotency_key.into();

        if user_id.is_empty() {
            return Err(OrderError::EmptyField { field: "user_id" });
        }
        if sku_id.is_empty() {
            return Err(OrderError::EmptyField { field: "sku_id" });
        }
        if idempotency_key.is_empty() {
            return Err(OrderError::EmptyField {
                field: "idempotency_key",
            });
        }
        if price_minor_units <= 0 {
            return Err(OrderError::InvalidPrice { price_minor_units });
        }

        Ok(Self {
            order_id: ulid::Ulid::new().to_string(),
            user_id,
            sku_id,
            price_minor_units,
            idempotency_key,
            state: OrderState::Submitted,
            committed_at_unix_secs: None,
            expires_at_unix_secs: None,
        })
    }

    /// Attempts a state transition. Returns the prior state on success.
    ///
    /// # Errors
    ///
    /// Returns [`OrderError::IllegalTransition`] if `next` is not permitted
    /// from the current state per SPEC-024 §3.2.
    pub fn transition_to(&mut self, next: OrderState) -> Result<OrderState, OrderError> {
        if !self.state.can_transition_to(next) {
            return Err(OrderError::IllegalTransition {
                from: self.state,
                to: next,
            });
        }
        let prior = self.state;
        self.state = next;
        Ok(prior)
    }
}

/// Builds the ledger idempotency key for the `redemption_purchase` transaction
/// associated with `order_id`. Pairing one order ↔ one ledger txn lets
/// retries replay deterministically (SPEC-002 §6.1).
#[must_use]
pub fn ledger_idempotency_key_for_purchase(order_id: &str) -> String {
    format!("redemption:purchase:{order_id}")
}

/// Builds the ledger idempotency key for an `entitlement_fulfillment` txn.
#[must_use]
pub fn ledger_idempotency_key_for_fulfillment(order_id: &str) -> String {
    format!("redemption:fulfillment:{order_id}")
}

/// Builds the ledger idempotency key for a `redemption_refund` txn.
#[must_use]
pub fn ledger_idempotency_key_for_refund(order_id: &str) -> String {
    format!("redemption:refund:{order_id}")
}

/// Builds the ledger idempotency key for an `entitlement_expiry` (breakage)
/// txn.
#[must_use]
pub fn ledger_idempotency_key_for_expiry(order_id: &str) -> String {
    format!("redemption:expiry:{order_id}")
}

/// Errors raised by the order aggregate.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OrderError {
    #[error("required order field {field} is empty")]
    EmptyField { field: &'static str },
    #[error("order price must be positive (got {price_minor_units})")]
    InvalidPrice { price_minor_units: i64 },
    #[error("illegal redemption order transition from {from:?} to {to:?}")]
    IllegalTransition { from: OrderState, to: OrderState },
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::{
        OrderError, OrderState, RedemptionOrder, ledger_idempotency_key_for_expiry,
        ledger_idempotency_key_for_fulfillment, ledger_idempotency_key_for_purchase,
        ledger_idempotency_key_for_refund,
    };

    fn submit_test_order() -> RedemptionOrder {
        RedemptionOrder::submit("user_42", "sku_a", 500, "client-uuid-1").expect("submit")
    }

    #[test]
    fn submit_starts_in_submitted_state() {
        let order = submit_test_order();
        assert_eq!(order.state, OrderState::Submitted);
        assert!(!order.order_id.is_empty(), "order_id should be generated");
    }

    #[test]
    fn submit_rejects_empty_fields_and_bad_price() {
        assert!(matches!(
            RedemptionOrder::submit("", "sku_a", 500, "k"),
            Err(OrderError::EmptyField { field: "user_id" })
        ));
        assert!(matches!(
            RedemptionOrder::submit("u", "", 500, "k"),
            Err(OrderError::EmptyField { field: "sku_id" })
        ));
        assert!(matches!(
            RedemptionOrder::submit("u", "sku_a", 500, ""),
            Err(OrderError::EmptyField {
                field: "idempotency_key"
            })
        ));
        assert!(matches!(
            RedemptionOrder::submit("u", "sku_a", 0, "k"),
            Err(OrderError::InvalidPrice { .. })
        ));
    }

    #[test]
    fn happy_path_transitions_submitted_to_fulfilled() {
        let mut order = submit_test_order();
        assert_eq!(
            order.transition_to(OrderState::Accepted).expect("accept"),
            OrderState::Submitted
        );
        assert_eq!(
            order.transition_to(OrderState::Committed).expect("commit"),
            OrderState::Accepted
        );
        assert_eq!(
            order.transition_to(OrderState::Fulfilled).expect("fulfill"),
            OrderState::Committed
        );
        assert_eq!(order.state, OrderState::Fulfilled);
    }

    #[test]
    fn cancel_window_refund_from_committed_passes() {
        let mut order = submit_test_order();
        order.transition_to(OrderState::Accepted).expect("accept");
        order.transition_to(OrderState::Committed).expect("commit");
        order
            .transition_to(OrderState::Refunded)
            .expect("cancel window refund");
        assert_eq!(order.state, OrderState::Refunded);
    }

    #[test]
    fn cs_refund_from_fulfilled_passes() {
        let mut order = submit_test_order();
        order.transition_to(OrderState::Accepted).expect("accept");
        order.transition_to(OrderState::Committed).expect("commit");
        order.transition_to(OrderState::Fulfilled).expect("fulfill");
        order
            .transition_to(OrderState::Refunded)
            .expect("cs refund");
    }

    #[test]
    fn expiry_only_from_committed() {
        // Committed → Expired OK
        let mut order = submit_test_order();
        order.transition_to(OrderState::Accepted).expect("accept");
        order.transition_to(OrderState::Committed).expect("commit");
        order.transition_to(OrderState::Expired).expect("expire");

        // Fulfilled → Expired NOT OK
        let mut order = submit_test_order();
        order.transition_to(OrderState::Accepted).expect("accept");
        order.transition_to(OrderState::Committed).expect("commit");
        order.transition_to(OrderState::Fulfilled).expect("fulfill");
        assert!(matches!(
            order.transition_to(OrderState::Expired),
            Err(OrderError::IllegalTransition { .. })
        ));

        // Submitted → Expired NOT OK
        let mut order = submit_test_order();
        assert!(matches!(
            order.transition_to(OrderState::Expired),
            Err(OrderError::IllegalTransition { .. })
        ));
    }

    #[test]
    fn no_op_self_transition_is_rejected() {
        let order = submit_test_order();
        assert!(!OrderState::Submitted.can_transition_to(OrderState::Submitted));
        assert!(!OrderState::Refunded.can_transition_to(OrderState::Refunded));
        assert!(!order.state.can_transition_to(order.state));
    }

    #[test]
    fn skipping_intermediate_states_is_rejected() {
        // Submitted -> Committed not allowed (must go through Accepted)
        let mut order = submit_test_order();
        assert!(matches!(
            order.transition_to(OrderState::Committed),
            Err(OrderError::IllegalTransition { .. })
        ));

        // Submitted -> Fulfilled not allowed
        assert!(matches!(
            order.transition_to(OrderState::Fulfilled),
            Err(OrderError::IllegalTransition { .. })
        ));

        // Accepted -> Fulfilled not allowed (must commit first)
        let mut order = submit_test_order();
        order.transition_to(OrderState::Accepted).expect("accept");
        assert!(matches!(
            order.transition_to(OrderState::Fulfilled),
            Err(OrderError::IllegalTransition { .. })
        ));
    }

    #[test]
    fn refunded_and_expired_are_terminal() {
        assert!(OrderState::Refunded.is_terminal());
        assert!(OrderState::Expired.is_terminal());
        assert!(!OrderState::Committed.is_terminal());
        assert!(!OrderState::Fulfilled.is_terminal());
        assert!(!OrderState::Submitted.is_terminal());
        assert!(!OrderState::Accepted.is_terminal());

        // Reverse direction from terminal is always rejected.
        for from in [OrderState::Refunded, OrderState::Expired] {
            for to in [
                OrderState::Submitted,
                OrderState::Accepted,
                OrderState::Committed,
                OrderState::Fulfilled,
                OrderState::Refunded,
                OrderState::Expired,
            ] {
                assert!(
                    !from.can_transition_to(to),
                    "{from:?} -> {to:?} must be rejected"
                );
            }
        }
    }

    #[test]
    fn ledger_idempotency_keys_are_unique_per_kind() {
        let order_id = "01HXABC";
        let purchase = ledger_idempotency_key_for_purchase(order_id);
        let fulfillment = ledger_idempotency_key_for_fulfillment(order_id);
        let refund = ledger_idempotency_key_for_refund(order_id);
        let expiry = ledger_idempotency_key_for_expiry(order_id);

        assert!(purchase.ends_with(order_id));
        assert_ne!(purchase, fulfillment);
        assert_ne!(fulfillment, refund);
        assert_ne!(refund, expiry);

        // All keys must reference the order so retries collide deterministically.
        for key in [&purchase, &fulfillment, &refund, &expiry] {
            assert!(key.starts_with("redemption:"));
            assert!(key.contains(order_id));
        }
    }
}
