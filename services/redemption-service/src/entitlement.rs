//! Entitlement domain: issued asset held by a user after a redemption order
//! commits. SPEC-024 §3.3.
//!
//! Entitlement state mostly mirrors the parent order, but they are split
//! intentionally — fulfillment workers (P3) and consumption events
//! (e.g. ticket scan) act on the entitlement directly, not the order.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// SPEC-024 §3.3 entitlement lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntitlementState {
    /// Issued at order commit; awaiting fulfillment.
    Issued,
    /// Fulfilled by worker / inline; may still be `Consumed` later for items
    /// with "use" semantics (tickets, service slots).
    Fulfilled,
    /// Optionally tracked for fulfilled items that have a discrete usage
    /// event (e.g. one-time codes). Out of P2 wiring scope.
    Consumed,
    /// Refunded — order returned via cancel window or CS path.
    Refunded,
    /// Expired without fulfillment; ledger breakage entry follows.
    Expired,
}

impl EntitlementState {
    /// Transitions permitted by SPEC-024 §3.3.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        use EntitlementState::{Consumed, Expired, Fulfilled, Issued, Refunded};
        matches!(
            (self, next),
            (Issued, Fulfilled | Refunded | Expired) | (Fulfilled, Consumed | Refunded)
        )
    }

    /// True for [`Consumed`], [`Refunded`], [`Expired`]. [`Fulfilled`] is
    /// terminal for items without usage semantics but is intentionally
    /// **not** flagged as terminal here so consumption can still apply.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Consumed | Self::Refunded | Self::Expired)
    }
}

/// SPEC-024 §3.3 `Entitlement`. Bound to a single `owner_user_id`; INV-7
/// (SPEC-002 v0.1.14 / ADR-0007) prevents the owner from drifting across
/// related ledger txns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entitlement {
    pub entitlement_id: String,
    pub order_id: String,
    /// **The** ownership field. SPEC-024 §3.3: all queries / fulfillment /
    /// consumption APIs must filter by this; v1 prohibits transfer.
    pub owner_user_id: String,
    pub sku_id: String,
    pub state: EntitlementState,
    /// Mode-specific delivery payload populated at fulfillment time.
    /// Examples: `{"code": "AAA-001"}` for `digital_code`,
    /// `{"booking_id": "01HX...", "slot": "..."}` for `service_async`.
    /// `None` while the entitlement is `Issued` (P3.2+ workers fill this).
    #[serde(default)]
    pub fulfillment_payload: Option<Value>,
}

impl Entitlement {
    /// Issues a new entitlement from a committed order.
    ///
    /// # Errors
    ///
    /// Returns [`EntitlementError::EmptyField`] when any required field is
    /// empty.
    pub fn issue(
        order_id: impl Into<String>,
        owner_user_id: impl Into<String>,
        sku_id: impl Into<String>,
    ) -> Result<Self, EntitlementError> {
        let order_id = order_id.into();
        let owner_user_id = owner_user_id.into();
        let sku_id = sku_id.into();
        if order_id.is_empty() {
            return Err(EntitlementError::EmptyField { field: "order_id" });
        }
        if owner_user_id.is_empty() {
            return Err(EntitlementError::EmptyField {
                field: "owner_user_id",
            });
        }
        if sku_id.is_empty() {
            return Err(EntitlementError::EmptyField { field: "sku_id" });
        }
        Ok(Self {
            entitlement_id: ulid::Ulid::new().to_string(),
            order_id,
            owner_user_id,
            sku_id,
            state: EntitlementState::Issued,
            fulfillment_payload: None,
        })
    }

    /// Attempts a state transition. Returns the prior state on success.
    ///
    /// # Errors
    ///
    /// Returns [`EntitlementError::IllegalTransition`] when `next` is not
    /// permitted from the current state.
    pub fn transition_to(
        &mut self,
        next: EntitlementState,
    ) -> Result<EntitlementState, EntitlementError> {
        if !self.state.can_transition_to(next) {
            return Err(EntitlementError::IllegalTransition {
                from: self.state,
                to: next,
            });
        }
        let prior = self.state;
        self.state = next;
        Ok(prior)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EntitlementError {
    #[error("required entitlement field {field} is empty")]
    EmptyField { field: &'static str },
    #[error("illegal entitlement transition from {from:?} to {to:?}")]
    IllegalTransition {
        from: EntitlementState,
        to: EntitlementState,
    },
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::{Entitlement, EntitlementError, EntitlementState};

    #[test]
    fn issued_to_fulfilled_to_consumed_path_passes() {
        let mut ent = Entitlement::issue("order_1", "user_42", "sku_a").expect("issue");
        assert_eq!(ent.state, EntitlementState::Issued);
        ent.transition_to(EntitlementState::Fulfilled)
            .expect("fulfill");
        ent.transition_to(EntitlementState::Consumed)
            .expect("consume");
    }

    #[test]
    fn issued_to_refunded_passes() {
        let mut ent = Entitlement::issue("order_1", "user_42", "sku_a").expect("issue");
        ent.transition_to(EntitlementState::Refunded)
            .expect("refund");
    }

    #[test]
    fn issued_to_expired_passes() {
        let mut ent = Entitlement::issue("order_1", "user_42", "sku_a").expect("issue");
        ent.transition_to(EntitlementState::Expired)
            .expect("expire");
    }

    #[test]
    fn fulfilled_to_refunded_cs_path_passes() {
        let mut ent = Entitlement::issue("order_1", "user_42", "sku_a").expect("issue");
        ent.transition_to(EntitlementState::Fulfilled)
            .expect("fulfill");
        ent.transition_to(EntitlementState::Refunded)
            .expect("cs refund");
    }

    #[test]
    fn consumed_is_terminal() {
        let mut ent = Entitlement::issue("order_1", "user_42", "sku_a").expect("issue");
        ent.transition_to(EntitlementState::Fulfilled)
            .expect("fulfill");
        ent.transition_to(EntitlementState::Consumed)
            .expect("consume");
        for next in [
            EntitlementState::Issued,
            EntitlementState::Fulfilled,
            EntitlementState::Refunded,
            EntitlementState::Expired,
            EntitlementState::Consumed,
        ] {
            assert!(matches!(
                ent.transition_to(next),
                Err(EntitlementError::IllegalTransition { .. })
            ));
        }
    }

    #[test]
    fn issue_rejects_empty_owner() {
        let error = Entitlement::issue("order_1", "", "sku_a").expect_err("empty owner");
        assert!(matches!(
            error,
            EntitlementError::EmptyField {
                field: "owner_user_id"
            }
        ));
    }

    #[test]
    fn expired_and_refunded_reject_further_transitions() {
        for terminal in [EntitlementState::Refunded, EntitlementState::Expired] {
            for next in [
                EntitlementState::Issued,
                EntitlementState::Fulfilled,
                EntitlementState::Consumed,
                EntitlementState::Refunded,
                EntitlementState::Expired,
            ] {
                assert!(
                    !terminal.can_transition_to(next),
                    "{terminal:?} -> {next:?} must be rejected"
                );
            }
        }
    }
}
