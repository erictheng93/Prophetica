//! SPEC-024 redemption / entitlement service.
//!
//! Implements the catalog, redemption-order, and entitlement domain on top of
//! the SPEC-002 ledger (via `sdk::ledger`). Boundaries:
//!
//! - **In scope (P2 — this crate, today):** domain types, state machines,
//!   idempotency-key construction, in-memory repository, stub HTTP routes.
//! - **Out of scope (P3 / P4):** fulfillment workers, cancel/refund window,
//!   expiry sweep + breakage. Stub routes return 501 with a pointer to the
//!   plan section that owns the work.
//!
//! See [SPEC-024](../../../../specs/SPEC-024-redemption-entitlement.html) and
//! its [implementation plan](../../../../docs/implementation-plans/SPEC-024-redemption-entitlement-implementation-plan.html)
//! for cross-cutting design.

pub const SERVICE_NAME: &str = "redemption-service";

/// Default HTTP bind port. Sits after `analytics-reporting-service` (8094).
pub const DEFAULT_PORT: u16 = 8095;

pub mod api;
pub mod catalog;
pub mod entitlement;
pub mod order;
pub mod store;
