//! Two-stage CS refund proposal / approval. SPEC-024 §3.2 / SPEC-012 §11.
//!
//! Pure domain types — the orchestrator + store + HTTP handlers compose
//! them. Decoupled from `RedemptionOrder` to keep refund workflow churn
//! from spilling onto the core order aggregate.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Lifecycle state of a CS refund proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    /// Created by proposer; awaiting a distinct approver.
    Pending,
    /// Approved by a second identity → refund posted.
    Approved,
    /// Approver explicitly rejected (different from cancel — rejection is
    /// a definitive "no, this should not refund").
    Rejected,
    /// Proposer (or admin) withdrew the proposal before it was approved.
    Cancelled,
}

impl ProposalStatus {
    /// True if the proposal is in a terminal state — no further status
    /// transitions allowed.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Approved | Self::Rejected | Self::Cancelled)
    }
}

/// SPEC-012 §11 compensation proposal restricted to the CS refund use case.
/// The wider compensation workflow is tracked separately; this aggregate
/// covers only the redemption-service surface area required by
/// ADR-0007 §7.1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CsRefundProposal {
    pub proposal_id: String,
    pub order_id: String,
    pub proposed_by: String,
    pub reason: String,
    pub status: ProposalStatus,
    pub proposed_at_unix_secs: i64,
    pub resolved_at_unix_secs: Option<i64>,
    pub resolved_by: Option<String>,
}

impl CsRefundProposal {
    /// Constructs a fresh `Pending` proposal. `proposal_id` is a server-side
    /// ULID — callers don't supply it (clients address proposals via the
    /// id returned from the propose endpoint).
    ///
    /// # Errors
    ///
    /// Returns [`CsRefundError::EmptyField`] when any required field is
    /// empty.
    pub fn propose(
        order_id: impl Into<String>,
        proposed_by: impl Into<String>,
        reason: impl Into<String>,
        proposed_at_unix_secs: i64,
    ) -> Result<Self, CsRefundError> {
        let order_id = order_id.into();
        let proposed_by = proposed_by.into();
        let reason = reason.into();
        if order_id.is_empty() {
            return Err(CsRefundError::EmptyField { field: "order_id" });
        }
        if proposed_by.is_empty() {
            return Err(CsRefundError::EmptyField {
                field: "proposed_by",
            });
        }
        if reason.trim().is_empty() {
            return Err(CsRefundError::EmptyField { field: "reason" });
        }
        Ok(Self {
            proposal_id: ulid::Ulid::new().to_string(),
            order_id,
            proposed_by,
            reason,
            status: ProposalStatus::Pending,
            proposed_at_unix_secs,
            resolved_at_unix_secs: None,
            resolved_by: None,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CsRefundError {
    #[error("required CS refund proposal field {field} is empty")]
    EmptyField { field: &'static str },
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::{CsRefundError, CsRefundProposal, ProposalStatus};

    #[test]
    fn propose_initialises_in_pending_with_ulid_id() {
        let proposal = CsRefundProposal::propose(
            "order_1",
            "ops_alice",
            "wrong item delivered",
            1_700_000_000,
        )
        .expect("propose");
        assert_eq!(proposal.status, ProposalStatus::Pending);
        assert_eq!(proposal.proposal_id.len(), 26, "ULID");
        assert!(proposal.resolved_by.is_none());
        assert!(proposal.resolved_at_unix_secs.is_none());
    }

    #[test]
    fn propose_rejects_empty_required_fields() {
        for (order, proposer, reason, expected_field) in [
            ("", "alice", "reason", "order_id"),
            ("o", "", "reason", "proposed_by"),
            ("o", "alice", "   ", "reason"),
        ] {
            let Err(error) = CsRefundProposal::propose(order, proposer, reason, 0) else {
                panic!("empty {expected_field} must error");
            };
            match error {
                CsRefundError::EmptyField { field } => assert_eq!(field, expected_field),
            }
        }
    }

    #[test]
    fn terminal_status_detection_matches_spec() {
        assert!(ProposalStatus::Approved.is_terminal());
        assert!(ProposalStatus::Rejected.is_terminal());
        assert!(ProposalStatus::Cancelled.is_terminal());
        assert!(!ProposalStatus::Pending.is_terminal());
    }
}
