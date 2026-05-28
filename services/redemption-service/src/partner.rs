//! Partner adapter abstraction for `service_async` fulfillment. SPEC-024 §3.4.
//!
//! Unlike `digital_code` (a single atomic claim from a vault), service-style
//! fulfillment is two-phase:
//!
//! 1. **Dispatch** — the orchestrator hands the entitlement to a partner
//!    (consultation booking system, ticketing platform, white-glove
//!    concierge, etc.) and receives a partner-side reference (`booking_id`).
//!    The booking is recorded on the entitlement payload so the worker
//!    never re-dispatches on retry.
//! 2. **Completion polling** — the worker periodically asks the partner
//!    `status(booking_id)`; on `Completed`, posts the
//!    `entitlement_fulfillment` ledger txn and advances state to `Fulfilled`.
//!
//! P3.3 ships the trait + an in-memory implementation suitable for tests
//! and local dev. The real adapters (e.g. webhook-driven, partner-specific
//! REST clients) plug in via the same trait without touching orchestrator
//! code.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
};

use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

use crate::entitlement::Entitlement;

#[async_trait]
pub trait PartnerAdapter: Send + Sync {
    /// Submits the entitlement to the partner. Returns a partner-side
    /// reference the worker records on the entitlement payload so that
    /// later [`Self::status`] calls can address the same booking.
    ///
    /// Must be safe to call once per entitlement — the orchestrator never
    /// re-dispatches an entitlement that already has a payload.
    ///
    /// # Errors
    ///
    /// Returns [`PartnerError`] when the partner is unreachable or rejects
    /// the request outright (per-business reasons).
    async fn dispatch(&self, entitlement: &Entitlement) -> Result<PartnerBooking, PartnerError>;

    /// Queries the partner for the current state of a booking. Idempotent.
    ///
    /// # Errors
    ///
    /// Returns [`PartnerError`] on transport / authentication failures.
    /// Business-level outcomes (pending, completed, failed) are encoded in
    /// [`PartnerStatus`], NOT raised as errors.
    async fn status(&self, booking_id: &str) -> Result<PartnerStatus, PartnerError>;
}

/// Acknowledgement returned by [`PartnerAdapter::dispatch`]. The `metadata`
/// field carries any partner-specific context the orchestrator should
/// surface back to the player (booking confirmation, scheduled slot, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartnerBooking {
    pub booking_id: String,
    pub metadata: Option<Value>,
}

/// Snapshot of a booking's progress, returned by [`PartnerAdapter::status`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartnerStatus {
    /// Partner has accepted the booking but the service has not yet
    /// concluded — the worker should poll again later.
    Pending,
    /// Service rendered. `metadata` carries any partner-side completion
    /// data (transcript link, attendance proof, etc.) that the orchestrator
    /// merges into the entitlement payload.
    Completed { metadata: Option<Value> },
    /// Partner reported a terminal failure — the worker should NOT retry.
    /// Cleanup (refund or compensation) is operational work, not worker
    /// responsibility.
    Failed { reason: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PartnerError {
    #[error("partner unreachable: {0}")]
    Unreachable(String),
    #[error("partner rejected request: {0}")]
    Rejected(String),
    #[error("partner storage unavailable")]
    StorageUnavailable,
    /// No such booking on the partner side — almost always a code bug here
    /// (calling status before dispatch).
    #[error("partner reports unknown booking_id {booking_id}")]
    UnknownBooking { booking_id: String },
}

// ---------------------------------------------------------------------------
// InMemoryPartnerAdapter — for unit tests and local dev.
// ---------------------------------------------------------------------------

/// In-memory partner adapter. Auto-generates a ULID booking on dispatch;
/// state starts at `Pending` and can be advanced via [`Self::mark_completed`]
/// / [`Self::mark_failed`] from tests.
#[derive(Default, Debug)]
pub struct InMemoryPartnerAdapter {
    inner: Mutex<InMemoryInner>,
}

#[derive(Default, Debug)]
struct InMemoryInner {
    /// `booking_id` → current state.
    bookings: HashMap<String, PartnerStatus>,
}

impl InMemoryPartnerAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Test helper: mark a booking completed. Subsequent
    /// [`PartnerAdapter::status`] calls return `Completed { metadata }`.
    ///
    /// # Errors
    ///
    /// Returns [`PartnerError::StorageUnavailable`] when the internal mutex
    /// is poisoned, or [`PartnerError::UnknownBooking`] when the booking
    /// was never dispatched.
    pub fn mark_completed(
        &self,
        booking_id: &str,
        metadata: Option<Value>,
    ) -> Result<(), PartnerError> {
        let mut guard = self.lock()?;
        if !guard.bookings.contains_key(booking_id) {
            return Err(PartnerError::UnknownBooking {
                booking_id: booking_id.to_owned(),
            });
        }
        guard
            .bookings
            .insert(booking_id.to_owned(), PartnerStatus::Completed { metadata });
        Ok(())
    }

    /// Test helper: mark a booking failed. Subsequent status calls return
    /// `Failed { reason }`.
    ///
    /// # Errors
    ///
    /// Returns [`PartnerError::StorageUnavailable`] when the internal mutex
    /// is poisoned, or [`PartnerError::UnknownBooking`] when the booking
    /// was never dispatched.
    pub fn mark_failed(&self, booking_id: &str, reason: String) -> Result<(), PartnerError> {
        let mut guard = self.lock()?;
        if !guard.bookings.contains_key(booking_id) {
            return Err(PartnerError::UnknownBooking {
                booking_id: booking_id.to_owned(),
            });
        }
        guard
            .bookings
            .insert(booking_id.to_owned(), PartnerStatus::Failed { reason });
        Ok(())
    }

    /// Test helper: returns all booking IDs currently tracked, sorted by
    /// the natural order of their ULIDs (creation time).
    ///
    /// # Errors
    ///
    /// Returns [`PartnerError::StorageUnavailable`] when the internal mutex
    /// is poisoned by a prior panic.
    pub fn booking_ids(&self) -> Result<Vec<String>, PartnerError> {
        let guard = self.lock()?;
        let mut ids: Vec<String> = guard.bookings.keys().cloned().collect();
        ids.sort();
        Ok(ids)
    }

    fn lock(&self) -> Result<MutexGuard<'_, InMemoryInner>, PartnerError> {
        self.inner
            .lock()
            .map_err(|_: PoisonError<_>| PartnerError::StorageUnavailable)
    }
}

#[async_trait]
impl PartnerAdapter for InMemoryPartnerAdapter {
    async fn dispatch(&self, _entitlement: &Entitlement) -> Result<PartnerBooking, PartnerError> {
        let booking_id = ulid::Ulid::new().to_string();
        let mut guard = self.lock()?;
        guard
            .bookings
            .insert(booking_id.clone(), PartnerStatus::Pending);
        Ok(PartnerBooking {
            booking_id,
            metadata: None,
        })
    }

    async fn status(&self, booking_id: &str) -> Result<PartnerStatus, PartnerError> {
        let guard = self.lock()?;
        guard
            .bookings
            .get(booking_id)
            .cloned()
            .ok_or_else(|| PartnerError::UnknownBooking {
                booking_id: booking_id.to_owned(),
            })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::{InMemoryPartnerAdapter, PartnerAdapter, PartnerError, PartnerStatus};
    use crate::entitlement::Entitlement;

    fn test_entitlement() -> Entitlement {
        Entitlement::issue("order_1", "user_42", "sku_a").expect("issue")
    }

    #[tokio::test]
    async fn dispatch_returns_pending_booking() {
        let adapter = InMemoryPartnerAdapter::new();
        let ent = test_entitlement();
        let booking = adapter.dispatch(&ent).await.expect("dispatch");
        assert_eq!(booking.booking_id.len(), 26, "ULID booking_id");
        let status = adapter.status(&booking.booking_id).await.expect("status");
        assert_eq!(status, PartnerStatus::Pending);
    }

    #[tokio::test]
    async fn mark_completed_transitions_status() {
        let adapter = InMemoryPartnerAdapter::new();
        let ent = test_entitlement();
        let booking = adapter.dispatch(&ent).await.expect("dispatch");
        adapter
            .mark_completed(&booking.booking_id, None)
            .expect("mark");
        assert_eq!(
            adapter.status(&booking.booking_id).await.expect("status"),
            PartnerStatus::Completed { metadata: None }
        );
    }

    #[tokio::test]
    async fn status_for_unknown_booking_returns_unknown_error() {
        let adapter = InMemoryPartnerAdapter::new();
        let error = match adapter.status("ghost-booking").await {
            Ok(_) => panic!("unknown booking must error"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            PartnerError::UnknownBooking {
                booking_id: "ghost-booking".to_owned()
            }
        );
    }
}
