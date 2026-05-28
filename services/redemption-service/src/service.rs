//! `RedemptionService` — the orchestrator that ties catalog lookups, the
//! order / entitlement state machines, and ledger writes into a single
//! submit-to-fulfill workflow.
//!
//! SPEC-024 implementation plan §1 P3 / §3.2 / §3.4. P3.1 ships the full
//! happy path for `DigitalImmediate` SKUs (submit → Accepted → Committed →
//! Fulfilled, two ledger transactions back-to-back) plus the partial path
//! for other modes (stops at `Committed`, entitlement `Issued`). The async
//! worker for `DigitalCode` / `ServiceAsync` arrives in P3.2 / P3.3 by
//! consuming the `Issued` state from this same store.
//!
//! Trade-offs the SDK callers should know about:
//!
//! 1. **Self-healing forward drive (codex P3.1 P1 #1 + #2 follow-up)**:
//!    `submit_order` reloads the order's current state on every call and
//!    drives it forward to the terminal state. Idempotent ledger writes
//!    (deterministic `ledger_idempotency_key_for_*(order_id)`) make this
//!    safe to repeat — a crash between ledger commit and order state
//!    advance is healed by the next retry, no recovery sweep needed.
//! 2. **Replay vs. first-submission** differ only in the response's
//!    `replayed` flag and the `record_order` short-circuit. Both paths
//!    drive forward through `drive_to_completion`, so a replay against an
//!    order stuck at Accepted (ledger ran, state didn't advance) actually
//!    finishes the job and returns the correct final view.
//! 3. **Terminal states are sticky**: `Refunded` / `Expired` orders never
//!    re-enter the forward drive on replay — they return the current view
//!    as-is, matching SPEC-024 §3.2.
//! 4. **INV-7** (cross-user) is enforced by the [`crate::ledger::LedgerClient`]
//!    layer through the SDK validator; the orchestrator never builds a
//!    cross-user draft on its own. Hostile callers attempting to forge a
//!    request with one `user_id` and a different wallet account get rejected
//!    at the ledger boundary.

use std::{collections::HashMap, sync::Arc};

use sdk::ledger::{LedgerAmount, LedgerEntryDraft, LedgerTxnKind, LedgerValidationError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{
    catalog::{FulfillmentMode, Sku},
    entitlement::{Entitlement, EntitlementError, EntitlementState},
    ledger::{LedgerClient, LedgerClientError, PostTransactionRequest},
    order::{
        OrderError, OrderState, RedemptionOrder, ledger_idempotency_key_for_expiry,
        ledger_idempotency_key_for_fulfillment, ledger_idempotency_key_for_purchase,
        ledger_idempotency_key_for_refund,
    },
    partner::{PartnerAdapter, PartnerError, PartnerStatus},
    store::{RedemptionStore, StoreError},
    vault::{VaultClient, VaultError},
};

/// Inbound submit-order request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitOrderRequest {
    pub user_id: String,
    pub sku_id: String,
    pub idempotency_key: String,
    #[serde(default)]
    pub correlation_id: Option<String>,
}

/// Summary of one fulfillment worker pass. Per-entitlement failures are
/// counted by category instead of aborting the whole batch — operators can
/// alert on `vault_exhausted` vs. `ledger_failed` differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct WorkerStats {
    /// Entitlements that reached `Fulfilled` this pass.
    pub processed: usize,
    /// Vault ran out of unused codes (`digital_code` worker only).
    pub vault_exhausted: usize,
    /// Partner accepted the dispatch but service is not yet complete
    /// (`service_async` worker only).
    pub partner_pending: usize,
    /// Partner reported a terminal failure (`service_async` worker only).
    /// Operational cleanup (refund) is out of worker scope.
    pub partner_failed: usize,
    /// Ledger rejected the fulfillment txn (INV-* violation or transport).
    pub ledger_failed: usize,
    /// Store could not persist a state update.
    pub store_failed: usize,
}

/// Internal classification of a single `service_async` worker step.
/// Promoted to [`WorkerStats`] counters by the calling loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServiceAsyncOutcome {
    /// Entitlement reached `Fulfilled` this pass.
    Processed,
    /// Partner dispatched but not yet complete; try again later.
    PartnerPending,
    /// Partner reported terminal failure; operational cleanup required.
    PartnerFailed,
}

/// Outbound order projection returned to API callers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderView {
    pub order_id: String,
    pub state: OrderState,
    pub user_id: String,
    pub sku_id: String,
    pub price_minor_units: i64,
    /// Set once the order has reached `Committed` or later. Absent for
    /// orders still in `Submitted` / `Accepted`.
    pub entitlement_id: Option<String>,
    pub idempotency_key: String,
    /// `true` when this response came from an idempotent replay
    /// (same `idempotency_key` already processed).
    pub replayed: bool,
}

/// Errors surfaced by the orchestrator.
#[derive(Debug, Error)]
pub enum RedemptionError {
    #[error("required field {field} is empty")]
    EmptyField { field: &'static str },
    #[error("sku not found: {sku_id}")]
    SkuNotFound { sku_id: String },
    #[error(
        "insufficient balance: required {required_minor_units}, available {available_minor_units}"
    )]
    InsufficientBalance {
        required_minor_units: i64,
        available_minor_units: i64,
    },
    #[error("redemption order build failed: {0}")]
    OrderBuild(#[from] OrderError),
    #[error("entitlement build failed: {0}")]
    EntitlementBuild(#[from] EntitlementError),
    #[error("ledger rejected redemption: {0}")]
    LedgerRejected(#[from] LedgerClientError),
    #[error("redemption store unavailable: {0}")]
    StoreUnavailable(#[from] StoreError),
    #[error("vault rejected fulfillment: {0}")]
    VaultRejected(#[from] VaultError),
    #[error("partner rejected fulfillment: {0}")]
    PartnerRejected(#[from] PartnerError),
    #[error("sku vanished from catalog mid-fulfillment for order {order_id}")]
    SkuVanished { order_id: String },
    /// Refund / cancel / expire was attempted on an order whose state does
    /// not allow the operation. SPEC-024 §3.2 transitions:
    /// - cancel-window refund: only from `Committed`
    /// - CS-driven refund: only from `Fulfilled`
    /// - expire: only from `Committed`
    #[error(
        "order {order_id} is in state {state:?}; {operation} requires one of {allowed_states:?}"
    )]
    OrderStateNotAllowed {
        order_id: String,
        operation: &'static str,
        state: OrderState,
        allowed_states: &'static [OrderState],
    },
    /// Order id supplied to cancel / expire was not in the store.
    #[error("order {order_id} not found")]
    OrderNotFound { order_id: String },
    /// Cancel-window deadline (SPEC-024 §3.2 / §7) elapsed —
    /// `now > committed_at + sku.cancel_window_seconds`. The only path back
    /// to the player wallet is now the CS-driven refund.
    #[error(
        "cancel window expired for order {order_id}: deadline was {deadline_unix_secs}, now is {now_unix_secs}"
    )]
    CancelWindowExpired {
        order_id: String,
        deadline_unix_secs: i64,
        now_unix_secs: i64,
    },
    /// `cancel_order` was called against an order whose `committed_at`
    /// timestamp was never recorded. Indicates a code bug — every Committed
    /// order should have `record_commit_timestamps` invoked.
    #[error("order {order_id} is Committed but missing committed_at timestamp")]
    MissingCommitTimestamp { order_id: String },
    /// Defensive: a SPEC-024 state transition that should always be legal was
    /// rejected — points to either a code bug here or a concurrent write
    /// that advanced the order between our load and update.
    #[error("unexpected internal state for order {order_id}: {message}")]
    Internal { order_id: String, message: String },
}

impl RedemptionError {
    #[must_use]
    pub fn from_ledger_validation(error: LedgerValidationError) -> Self {
        Self::LedgerRejected(LedgerClientError::Validation(error))
    }
}

/// Orchestrator. Cheap to clone (everything is `Arc`-wrapped) so it can be
/// the axum `Router` state.
#[derive(Clone)]
pub struct RedemptionService {
    store: Arc<dyn RedemptionStore>,
    ledger: Arc<dyn LedgerClient>,
}

impl RedemptionService {
    #[must_use]
    pub fn new(store: Arc<dyn RedemptionStore>, ledger: Arc<dyn LedgerClient>) -> Self {
        Self { store, ledger }
    }

    /// Submits a redemption order, then drives it forward to whatever
    /// terminal state its `fulfillment_mode` allows. Safe to retry —
    /// idempotent ledger keys + state-aware forward drive make the operation
    /// self-healing across crashes.
    ///
    /// Full path for `DigitalImmediate` SKUs (terminal = `Fulfilled`).
    /// Partial path (terminal = `Committed`, entitlement `Issued`) for the
    /// other fulfillment modes — the matching P3.2 / P3.3 worker advances
    /// them later.
    ///
    /// # Errors
    ///
    /// See [`RedemptionError`] variants.
    pub async fn submit_order(
        &self,
        request: SubmitOrderRequest,
    ) -> Result<OrderView, RedemptionError> {
        validate_submit_request(&request)?;

        let sku = self.lookup_sku(&request.sku_id).await?;

        let draft = RedemptionOrder::submit(
            request.user_id.clone(),
            sku.sku_id.clone(),
            sku.price_minor_units,
            request.idempotency_key.clone(),
        )?;
        let draft_order_id = draft.order_id.clone();
        let recorded = self.store.record_order(draft).await?;
        let is_replay = recorded.order_id != draft_order_id;

        self.drive_to_completion(recorded, &sku, request.correlation_id.as_deref(), is_replay)
            .await
    }

    /// Forward-drives an order from its current persisted state toward the
    /// terminal state allowed by the SKU's fulfillment mode. Each ledger
    /// write uses a deterministic idempotency key
    /// (`ledger_idempotency_key_for_*(order_id)`) so re-entering this loop
    /// after a crash is safe — the ledger client returns `replayed: true`
    /// and balances do not move twice.
    ///
    /// Heals every partial-failure state codex P3.1 flagged:
    /// - `Submitted` with a still-pending balance check → re-checks balance
    /// - `Accepted` but ledger purchase already committed → re-posts
    ///   (idempotent on the ledger key), advances to `Committed`
    /// - `Committed` but entitlement record missing → issues entitlement
    /// - `DigitalImmediate` `Fulfilled` but entitlement still `Issued` →
    ///   updates entitlement state to `Fulfilled` (the P1 #2 divergence path)
    /// - `Refunded` / `Expired` → returns current view as-is (terminal, sticky)
    async fn drive_to_completion(
        &self,
        mut order: RedemptionOrder,
        sku: &Sku,
        correlation_id: Option<&str>,
        was_replay: bool,
    ) -> Result<OrderView, RedemptionError> {
        if matches!(order.state, OrderState::Refunded | OrderState::Expired) {
            return self.build_view(order, None, was_replay).await;
        }

        if matches!(order.state, OrderState::Submitted) {
            let balance = self.ledger.get_wallet_balance(&order.user_id).await?;
            if !balance.covers(sku.price_minor_units) {
                return Err(RedemptionError::InsufficientBalance {
                    required_minor_units: sku.price_minor_units,
                    available_minor_units: balance.available_minor_units,
                });
            }
            self.advance_order(&order.order_id, OrderState::Accepted)
                .await?;
            order.state = OrderState::Accepted;
        }

        if matches!(order.state, OrderState::Accepted) {
            let audit_log_id = generate_audit_log_id();
            self.post_redemption_purchase(&order, sku, &audit_log_id, correlation_id)
                .await?;
            self.advance_order(&order.order_id, OrderState::Committed)
                .await?;
            order.state = OrderState::Committed;
            // Stamp commit + expiry timestamps so the cancel-window deadline
            // (P5.1) and the expiry sweep (P5.2) have something to read.
            // Reload is idempotent: replay against an already-stamped order
            // overwrites with the same values (computed from a fresh clock,
            // which differs across retries — see doc note below for the
            // intentional choice to NOT re-stamp on replay).
            if order.committed_at_unix_secs.is_none() {
                let committed_at = current_unix_secs();
                let expires_at =
                    committed_at.saturating_add(i64::from(sku.entitlement_ttl_days) * 86_400);
                self.store
                    .record_commit_timestamps(&order.order_id, committed_at, expires_at)
                    .await?;
                order.committed_at_unix_secs = Some(committed_at);
                order.expires_at_unix_secs = Some(expires_at);
            }
        }

        let entitlement = self.ensure_entitlement(&order).await?;
        let entitlement_id = entitlement.entitlement_id.clone();
        let mut entitlement_state = entitlement.state;

        if matches!(sku.fulfillment_mode, FulfillmentMode::DigitalImmediate) {
            if matches!(order.state, OrderState::Committed) {
                self.post_entitlement_fulfillment(&order, sku, &entitlement_id)
                    .await?;
                self.advance_order(&order.order_id, OrderState::Fulfilled)
                    .await?;
                order.state = OrderState::Fulfilled;
            }
            if !matches!(entitlement_state, EntitlementState::Fulfilled) {
                self.heal_entitlement_to_fulfilled(&order.order_id, &entitlement_id)
                    .await?;
                entitlement_state = EntitlementState::Fulfilled;
            }
        }

        let _ = entitlement_state; // future use; suppresses unused-binding warning
        self.build_view(order, Some(entitlement_id), was_replay)
            .await
    }

    /// Loads an order view by id. Returns `None` when the order is unknown.
    ///
    /// # Errors
    ///
    /// Propagates [`StoreError`] when the underlying store is unavailable.
    pub async fn get_order(&self, order_id: &str) -> Result<Option<OrderView>, RedemptionError> {
        let Some(order) = self.store.get_order(order_id).await? else {
            return Ok(None);
        };
        let entitlement = self.store.get_entitlement_for_order(order_id).await?;
        Ok(Some(OrderView {
            order_id: order.order_id,
            state: order.state,
            user_id: order.user_id,
            sku_id: order.sku_id,
            price_minor_units: order.price_minor_units,
            entitlement_id: entitlement.map(|e| e.entitlement_id),
            idempotency_key: order.idempotency_key,
            replayed: false,
        }))
    }

    /// Lists all entitlements owned by `user_id`. Strict owner filter — never
    /// returns another user's entitlements (SPEC-024 §3.3 ownership rule).
    ///
    /// # Errors
    ///
    /// Propagates [`StoreError`] when the underlying store is unavailable.
    pub async fn list_entitlements(
        &self,
        user_id: &str,
    ) -> Result<Vec<Entitlement>, RedemptionError> {
        let entitlements = self.store.list_entitlements_for_owner(user_id).await?;
        Ok(entitlements)
    }

    async fn lookup_sku(&self, sku_id: &str) -> Result<Sku, RedemptionError> {
        self.store
            .get_sku(sku_id)
            .await?
            .ok_or_else(|| RedemptionError::SkuNotFound {
                sku_id: sku_id.to_owned(),
            })
    }

    async fn build_view(
        &self,
        order: RedemptionOrder,
        known_entitlement_id: Option<String>,
        replayed: bool,
    ) -> Result<OrderView, RedemptionError> {
        let entitlement_id = match known_entitlement_id {
            Some(id) => Some(id),
            None => self
                .store
                .get_entitlement_for_order(&order.order_id)
                .await?
                .map(|e| e.entitlement_id),
        };
        Ok(OrderView {
            order_id: order.order_id,
            state: order.state,
            user_id: order.user_id,
            sku_id: order.sku_id,
            price_minor_units: order.price_minor_units,
            entitlement_id,
            idempotency_key: order.idempotency_key,
            replayed,
        })
    }

    async fn advance_order(
        &self,
        order_id: &str,
        new_state: OrderState,
    ) -> Result<(), RedemptionError> {
        self.store.update_order_state(order_id, new_state).await?;
        Ok(())
    }

    /// Returns the entitlement for `order`, issuing one if none exists yet.
    /// Idempotent: if a prior `submit_order` call crashed between order
    /// commit and entitlement record, the replay creates it now.
    async fn ensure_entitlement(
        &self,
        order: &RedemptionOrder,
    ) -> Result<Entitlement, RedemptionError> {
        if let Some(existing) = self
            .store
            .get_entitlement_for_order(&order.order_id)
            .await?
        {
            return Ok(existing);
        }
        let issued = Entitlement::issue(
            order.order_id.clone(),
            order.user_id.clone(),
            order.sku_id.clone(),
        )?;
        self.store.record_entitlement(issued.clone()).await?;
        Ok(issued)
    }

    /// Heals the codex P3.1 P1 #2 divergence: order advanced to `Fulfilled`
    /// but entitlement still `Issued`. Idempotent — no-op when the
    /// entitlement is already `Fulfilled`.
    async fn heal_entitlement_to_fulfilled(
        &self,
        order_id: &str,
        entitlement_id: &str,
    ) -> Result<(), RedemptionError> {
        self.store
            .update_entitlement_state(entitlement_id, EntitlementState::Fulfilled)
            .await
            .map_err(|err| match err {
                StoreError::EntitlementNotFound { entitlement_id } => RedemptionError::Internal {
                    order_id: order_id.to_owned(),
                    message: format!(
                        "entitlement {entitlement_id} vanished while healing to Fulfilled"
                    ),
                },
                other => RedemptionError::StoreUnavailable(other),
            })?;
        Ok(())
    }

    async fn post_redemption_purchase(
        &self,
        order: &RedemptionOrder,
        sku: &Sku,
        audit_log_id: &str,
        correlation_id: Option<&str>,
    ) -> Result<(), RedemptionError> {
        let request = PostTransactionRequest {
            txn_kind: LedgerTxnKind::RedemptionPurchase,
            txn_type: "redemption_purchase".to_owned(),
            idempotency_key: ledger_idempotency_key_for_purchase(&order.order_id),
            correlation_id: correlation_id.map(str::to_owned),
            entries: vec![
                LedgerEntryDraft::new(
                    format!("Player.Wallet.{}", order.user_id),
                    LedgerAmount::from_minor_units(sku.price_minor_units),
                ),
                LedgerEntryDraft::new(
                    format!("Liability.UnredeemedEntitlement.{}", order.user_id),
                    LedgerAmount::from_minor_units(-sku.price_minor_units),
                ),
            ],
            metadata: redemption_purchase_metadata(order, sku, audit_log_id),
        };
        self.ledger.post_transaction(request).await?;
        Ok(())
    }

    /// SPEC-024 §3.2 / §7 cancel-window refund. Allowed only when:
    ///
    /// 1. order state is [`OrderState::Committed`] (Fulfilled goes via
    ///    [`Self::refund_fulfilled_order_cs`]), and
    /// 2. `now_unix_secs ≤ order.committed_at + sku.cancel_window_seconds`
    ///    — i.e. the cancel window has not closed yet.
    ///
    /// Callers are responsible for sourcing `now_unix_secs`. HTTP handlers
    /// use [`current_unix_secs`]; tests pass deterministic values so a
    /// suite can both (a) exercise the in-window happy path with a recent
    /// clock and (b) exercise the deadline-elapsed branch by passing a
    /// timestamp past `committed_at + cancel_window`.
    ///
    /// Ledger: `redemption_refund` txn moves the value from
    /// `Liability.UnredeemedEntitlement.{user}` back to `Player.Wallet.{user}`,
    /// keyed on `ledger_idempotency_key_for_refund(order_id)`.
    ///
    /// # Errors
    ///
    /// - [`RedemptionError::OrderNotFound`] when `order_id` is unknown.
    /// - [`RedemptionError::OrderStateNotAllowed`] when the order is not
    ///   `Committed`.
    /// - [`RedemptionError::CancelWindowExpired`] when the deadline has passed.
    /// - [`RedemptionError::MissingCommitTimestamp`] when the order is
    ///   `Committed` but no `committed_at` was recorded (code bug — every
    ///   Committed order should have had `record_commit_timestamps` invoked).
    /// - Propagates ledger / store errors.
    pub async fn cancel_order(
        &self,
        order_id: &str,
        now_unix_secs: i64,
    ) -> Result<OrderView, RedemptionError> {
        let order = self.store.get_order(order_id).await?.ok_or_else(|| {
            RedemptionError::OrderNotFound {
                order_id: order_id.to_owned(),
            }
        })?;

        if !matches!(order.state, OrderState::Committed) {
            return Err(RedemptionError::OrderStateNotAllowed {
                order_id: order.order_id.clone(),
                operation: "cancel_order",
                state: order.state,
                allowed_states: &[OrderState::Committed],
            });
        }

        let sku = self.store.get_sku(&order.sku_id).await?.ok_or_else(|| {
            RedemptionError::SkuVanished {
                order_id: order.order_id.clone(),
            }
        })?;

        let committed_at = order.committed_at_unix_secs.ok_or_else(|| {
            RedemptionError::MissingCommitTimestamp {
                order_id: order.order_id.clone(),
            }
        })?;
        let deadline_unix_secs = committed_at.saturating_add(i64::from(sku.cancel_window_seconds));
        if now_unix_secs > deadline_unix_secs {
            return Err(RedemptionError::CancelWindowExpired {
                order_id: order.order_id.clone(),
                deadline_unix_secs,
                now_unix_secs,
            });
        }

        self.post_redemption_refund(&order, &sku).await?;
        self.advance_order(&order.order_id, OrderState::Refunded)
            .await?;
        if let Some(entitlement) = self
            .store
            .get_entitlement_for_order(&order.order_id)
            .await?
        {
            self.store
                .update_entitlement_state(&entitlement.entitlement_id, EntitlementState::Refunded)
                .await?;
        }
        let refunded_order = RedemptionOrder {
            state: OrderState::Refunded,
            ..order
        };
        self.build_view(refunded_order, None, false).await
    }

    /// SPEC-024 §3.2 CS-driven refund from `Fulfilled`. P4 skeleton: the
    /// 4-eye approval workflow (SPEC-012) is NOT wired here — this method
    /// assumes the caller has already cleared CS approval. The orchestrator
    /// posts the refund ledger entry and advances state.
    ///
    /// # Errors
    ///
    /// - [`RedemptionError::OrderStateNotAllowed`] when the order is not
    ///   `Fulfilled`.
    /// - Propagates ledger / store errors.
    pub async fn refund_fulfilled_order_cs(
        &self,
        order_id: &str,
    ) -> Result<OrderView, RedemptionError> {
        let order = self.store.get_order(order_id).await?.ok_or_else(|| {
            RedemptionError::OrderNotFound {
                order_id: order_id.to_owned(),
            }
        })?;
        if !matches!(order.state, OrderState::Fulfilled) {
            return Err(RedemptionError::OrderStateNotAllowed {
                order_id: order.order_id.clone(),
                operation: "refund_fulfilled_order_cs",
                state: order.state,
                allowed_states: &[OrderState::Fulfilled],
            });
        }
        let sku = self.store.get_sku(&order.sku_id).await?.ok_or_else(|| {
            RedemptionError::SkuVanished {
                order_id: order.order_id.clone(),
            }
        })?;
        self.post_redemption_refund(&order, &sku).await?;
        self.advance_order(&order.order_id, OrderState::Refunded)
            .await?;
        if let Some(entitlement) = self
            .store
            .get_entitlement_for_order(&order.order_id)
            .await?
        {
            self.store
                .update_entitlement_state(&entitlement.entitlement_id, EntitlementState::Refunded)
                .await?;
        }
        let refunded_order = RedemptionOrder {
            state: OrderState::Refunded,
            ..order
        };
        self.build_view(refunded_order, None, false).await
    }

    /// SPEC-024 §3.2 / §7 expiry sweep. Lists up to `limit` `Committed`
    /// orders whose `expires_at_unix_secs <= now_unix_secs` and calls
    /// [`Self::expire_order`] on each. Per-order failures (ledger reject,
    /// missing SKU, etc.) increment the appropriate counter in the returned
    /// [`WorkerStats`] but do not abort the batch.
    ///
    /// Production caller is the `redemption-expiry-sweeper` binary; it
    /// loops or runs one-shot on a cron schedule, passing
    /// [`current_unix_secs`] as `now_unix_secs`.
    ///
    /// # Errors
    ///
    /// Returns [`RedemptionError::StoreUnavailable`] only when the initial
    /// `list_orders_pending_expiry` call fails; individual `expire_order`
    /// failures are encoded in [`WorkerStats`].
    pub async fn run_expiry_sweep(
        &self,
        now_unix_secs: i64,
        limit: usize,
    ) -> Result<WorkerStats, RedemptionError> {
        let pending = self
            .store
            .list_orders_pending_expiry(now_unix_secs, limit)
            .await?;

        let mut stats = WorkerStats::default();
        for order in pending {
            match self.expire_order(&order.order_id).await {
                Ok(_) => stats.processed += 1,
                Err(RedemptionError::LedgerRejected(_)) => stats.ledger_failed += 1,
                Err(_) => stats.store_failed += 1,
            }
        }
        Ok(stats)
    }

    /// SPEC-024 §3.2 / §7 expiry. Marks a single `Committed` order as
    /// expired and posts the breakage ledger entry that recognises the
    /// unused entitlement as `Revenue.EntitlementBreakage`. Caller is
    /// responsible for the "is this expired now?" decision — the production
    /// expiry sweep (which queries `WHERE state='committed' AND expires_at < NOW()`)
    /// invokes this method per expired order id.
    ///
    /// # Errors
    ///
    /// - [`RedemptionError::OrderNotFound`] when `order_id` is unknown.
    /// - [`RedemptionError::OrderStateNotAllowed`] when the order is not
    ///   `Committed` (Fulfilled orders cannot expire — happy-path terminal).
    /// - Propagates ledger / store errors.
    pub async fn expire_order(&self, order_id: &str) -> Result<OrderView, RedemptionError> {
        let order = self.store.get_order(order_id).await?.ok_or_else(|| {
            RedemptionError::OrderNotFound {
                order_id: order_id.to_owned(),
            }
        })?;
        if !matches!(order.state, OrderState::Committed) {
            return Err(RedemptionError::OrderStateNotAllowed {
                order_id: order.order_id.clone(),
                operation: "expire_order",
                state: order.state,
                allowed_states: &[OrderState::Committed],
            });
        }
        self.post_entitlement_expiry(&order).await?;
        self.advance_order(&order.order_id, OrderState::Expired)
            .await?;
        if let Some(entitlement) = self
            .store
            .get_entitlement_for_order(&order.order_id)
            .await?
        {
            self.store
                .update_entitlement_state(&entitlement.entitlement_id, EntitlementState::Expired)
                .await?;
        }
        let expired_order = RedemptionOrder {
            state: OrderState::Expired,
            ..order
        };
        self.build_view(expired_order, None, false).await
    }

    async fn post_redemption_refund(
        &self,
        order: &RedemptionOrder,
        sku: &Sku,
    ) -> Result<(), RedemptionError> {
        let request = PostTransactionRequest {
            txn_kind: LedgerTxnKind::RedemptionRefund,
            txn_type: "redemption_refund".to_owned(),
            idempotency_key: ledger_idempotency_key_for_refund(&order.order_id),
            correlation_id: None,
            entries: vec![
                LedgerEntryDraft::new(
                    format!("Liability.UnredeemedEntitlement.{}", order.user_id),
                    LedgerAmount::from_minor_units(sku.price_minor_units),
                ),
                LedgerEntryDraft::new(
                    format!("Player.Wallet.{}", order.user_id),
                    LedgerAmount::from_minor_units(-sku.price_minor_units),
                ),
            ],
            metadata: HashMap::from([(
                "order_id".to_owned(),
                Value::String(order.order_id.clone()),
            )]),
        };
        self.ledger.post_transaction(request).await?;
        Ok(())
    }

    async fn post_entitlement_expiry(
        &self,
        order: &RedemptionOrder,
    ) -> Result<(), RedemptionError> {
        let request = PostTransactionRequest {
            txn_kind: LedgerTxnKind::EntitlementExpiry,
            txn_type: "entitlement_expiry".to_owned(),
            idempotency_key: ledger_idempotency_key_for_expiry(&order.order_id),
            correlation_id: None,
            entries: vec![
                LedgerEntryDraft::new(
                    format!("Liability.UnredeemedEntitlement.{}", order.user_id),
                    LedgerAmount::from_minor_units(order.price_minor_units),
                ),
                LedgerEntryDraft::new(
                    "Revenue.EntitlementBreakage".to_owned(),
                    LedgerAmount::from_minor_units(-order.price_minor_units),
                ),
            ],
            metadata: HashMap::from([(
                "order_id".to_owned(),
                Value::String(order.order_id.clone()),
            )]),
        };
        self.ledger.post_transaction(request).await?;
        Ok(())
    }

    /// SPEC-024 §3.4 `digital_code` worker pass. Lists up to `limit`
    /// entitlements in [`EntitlementState::Issued`] for `DigitalCode` SKUs,
    /// claims a code from `vault`, attaches it to the entitlement payload,
    /// posts the `entitlement_fulfillment` ledger transaction, and advances
    /// both order and entitlement to `Fulfilled`.
    ///
    /// Per-entitlement failures are recorded in [`WorkerStats`] and the
    /// worker continues to the next entitlement — one stuck record never
    /// blocks the rest of the batch.
    ///
    /// Crash-safe by design (see also the `submit_order` doc-comment on
    /// self-healing):
    /// - If [`VaultClient::claim_code`] succeeds but ledger / store updates
    ///   fail, the next pass detects `fulfillment_payload.is_some()` and
    ///   reuses the existing code instead of claiming a fresh one. This
    ///   prevents the "lost code on retry" failure mode.
    /// - `entitlement_fulfillment` ledger writes use
    ///   `ledger_idempotency_key_for_fulfillment(order_id)` so duplicate
    ///   posts are squashed by the ledger client.
    ///
    /// # Errors
    ///
    /// Returns [`RedemptionError::StoreUnavailable`] only when listing
    /// pending work fails. Individual entitlement failures are reflected in
    /// the returned [`WorkerStats`], not as `Err`.
    pub async fn run_digital_code_worker(
        &self,
        vault: &dyn VaultClient,
        limit: usize,
    ) -> Result<WorkerStats, RedemptionError> {
        let pending = self
            .store
            .list_entitlements_pending_fulfillment(FulfillmentMode::DigitalCode, limit)
            .await?;

        let mut stats = WorkerStats::default();
        for entitlement in pending {
            match self
                .fulfill_digital_code_entitlement(&entitlement, vault)
                .await
            {
                Ok(()) => stats.processed += 1,
                Err(RedemptionError::VaultRejected(_)) => stats.vault_exhausted += 1,
                Err(RedemptionError::LedgerRejected(_)) => stats.ledger_failed += 1,
                Err(_) => stats.store_failed += 1,
            }
        }
        Ok(stats)
    }

    /// SPEC-024 §3.4 `service_async` worker pass. Lists up to `limit`
    /// entitlements in [`EntitlementState::Issued`] for `ServiceAsync` SKUs
    /// and drives each one through the two-phase partner workflow:
    ///
    /// 1. **Dispatch** (if `fulfillment_payload.is_none()`): call
    ///    [`PartnerAdapter::dispatch`], persist the returned `booking_id`
    ///    onto the entitlement payload.
    /// 2. **Status poll**: call [`PartnerAdapter::status(booking_id)`].
    ///    On `Completed` → post `entitlement_fulfillment` ledger txn,
    ///    advance order + entitlement to `Fulfilled`.
    ///    On `Pending` → record `partner_pending` in stats, leave for the
    ///    next pass.
    ///    On `Failed` → record `partner_failed`; refund is operational
    ///    work (P4), not the worker's responsibility.
    ///
    /// Crash-safety mirrors `run_digital_code_worker`: never re-dispatch
    /// an entitlement that already has a `booking_id` payload, never re-post
    /// the ledger fulfillment without ledger-side idempotency squashing it.
    ///
    /// # Errors
    ///
    /// Returns [`RedemptionError::StoreUnavailable`] only when listing
    /// pending work fails; individual entitlement outcomes are encoded in
    /// the returned [`WorkerStats`].
    pub async fn run_service_async_worker(
        &self,
        adapter: &dyn PartnerAdapter,
        limit: usize,
    ) -> Result<WorkerStats, RedemptionError> {
        let pending = self
            .store
            .list_entitlements_pending_fulfillment(FulfillmentMode::ServiceAsync, limit)
            .await?;

        let mut stats = WorkerStats::default();
        for entitlement in pending {
            match self
                .process_service_async_entitlement(&entitlement, adapter)
                .await
            {
                Ok(ServiceAsyncOutcome::Processed) => stats.processed += 1,
                Ok(ServiceAsyncOutcome::PartnerPending) => stats.partner_pending += 1,
                Ok(ServiceAsyncOutcome::PartnerFailed)
                | Err(RedemptionError::PartnerRejected(_)) => stats.partner_failed += 1,
                Err(RedemptionError::LedgerRejected(_)) => stats.ledger_failed += 1,
                Err(_) => stats.store_failed += 1,
            }
        }
        Ok(stats)
    }

    async fn process_service_async_entitlement(
        &self,
        entitlement: &Entitlement,
        adapter: &dyn PartnerAdapter,
    ) -> Result<ServiceAsyncOutcome, RedemptionError> {
        // Step 1: ensure a partner booking exists (dispatch only once).
        let existing_booking_id = entitlement
            .fulfillment_payload
            .as_ref()
            .and_then(|p| p.get("booking_id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let booking_id = if let Some(existing) = existing_booking_id {
            existing
        } else {
            let booking = adapter.dispatch(entitlement).await?;
            let mut payload = serde_json::Map::new();
            payload.insert(
                "booking_id".to_owned(),
                Value::String(booking.booking_id.clone()),
            );
            if let Some(meta) = booking.metadata {
                payload.insert("partner_metadata".to_owned(), meta);
            }
            self.store
                .set_entitlement_payload(&entitlement.entitlement_id, Value::Object(payload))
                .await?;
            booking.booking_id
        };

        // Step 2: ask the partner what state the booking is in.
        match adapter.status(&booking_id).await? {
            PartnerStatus::Pending => return Ok(ServiceAsyncOutcome::PartnerPending),
            PartnerStatus::Failed { .. } => return Ok(ServiceAsyncOutcome::PartnerFailed),
            PartnerStatus::Completed { metadata } => {
                if let Some(meta) = metadata {
                    self.merge_partner_completion_metadata(entitlement, meta)
                        .await?;
                }
            }
        }

        // Step 3: post the fulfillment ledger txn and advance state.
        let order = self
            .store
            .get_order(&entitlement.order_id)
            .await?
            .ok_or_else(|| RedemptionError::Internal {
                order_id: entitlement.order_id.clone(),
                message: "order missing during service_async fulfillment".to_owned(),
            })?;
        let sku = self
            .store
            .get_sku(&entitlement.sku_id)
            .await?
            .ok_or_else(|| RedemptionError::SkuVanished {
                order_id: order.order_id.clone(),
            })?;

        self.post_entitlement_fulfillment(&order, &sku, &entitlement.entitlement_id)
            .await?;

        if matches!(order.state, OrderState::Committed) {
            self.advance_order(&order.order_id, OrderState::Fulfilled)
                .await?;
        }
        self.heal_entitlement_to_fulfilled(&order.order_id, &entitlement.entitlement_id)
            .await?;
        Ok(ServiceAsyncOutcome::Processed)
    }

    /// Merges partner-side completion metadata (transcript link, attendance
    /// proof, etc.) into the entitlement payload. Preserves the existing
    /// `booking_id` (which is the worker's only reliable handle back to the
    /// partner on subsequent retries).
    async fn merge_partner_completion_metadata(
        &self,
        entitlement: &Entitlement,
        completion_metadata: Value,
    ) -> Result<(), RedemptionError> {
        let mut merged = entitlement
            .fulfillment_payload
            .as_ref()
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        merged.insert("completion".to_owned(), completion_metadata);
        self.store
            .set_entitlement_payload(&entitlement.entitlement_id, Value::Object(merged))
            .await?;
        Ok(())
    }

    async fn fulfill_digital_code_entitlement(
        &self,
        entitlement: &Entitlement,
        vault: &dyn VaultClient,
    ) -> Result<(), RedemptionError> {
        // Step 1: get-or-claim the activation code. Reusing an existing
        // payload across retries is what makes the worker crash-safe — a
        // prior pass that claimed a code but then failed to post the ledger
        // txn left the code on the entitlement record; we MUST NOT claim a
        // fresh code or it leaks one per retry.
        if entitlement.fulfillment_payload.is_none() {
            let code = vault.claim_code(&entitlement.sku_id).await?;
            let payload = serde_json::json!({ "code": code });
            self.store
                .set_entitlement_payload(&entitlement.entitlement_id, payload)
                .await?;
        }

        let order = self
            .store
            .get_order(&entitlement.order_id)
            .await?
            .ok_or_else(|| RedemptionError::Internal {
                order_id: entitlement.order_id.clone(),
                message: "order missing during digital_code fulfillment".to_owned(),
            })?;
        let sku = self
            .store
            .get_sku(&entitlement.sku_id)
            .await?
            .ok_or_else(|| RedemptionError::SkuVanished {
                order_id: order.order_id.clone(),
            })?;

        self.post_entitlement_fulfillment(&order, &sku, &entitlement.entitlement_id)
            .await?;

        // Advance order BEFORE entitlement: the order state machine drives
        // the `submit_order` self-healing forward-drive (Committed → Fulfilled).
        // If the entitlement advance fails, the next worker pass + a
        // subsequent `submit_order` replay (which runs `heal_entitlement_to_fulfilled`)
        // catches up.
        if matches!(order.state, OrderState::Committed) {
            self.advance_order(&order.order_id, OrderState::Fulfilled)
                .await?;
        }
        self.heal_entitlement_to_fulfilled(&order.order_id, &entitlement.entitlement_id)
            .await?;
        Ok(())
    }

    async fn post_entitlement_fulfillment(
        &self,
        order: &RedemptionOrder,
        sku: &Sku,
        entitlement_id: &str,
    ) -> Result<(), RedemptionError> {
        let revenue_account = revenue_account_for_mode(sku.fulfillment_mode);
        let request = PostTransactionRequest {
            txn_kind: LedgerTxnKind::EntitlementFulfillment,
            txn_type: "entitlement_fulfillment".to_owned(),
            idempotency_key: ledger_idempotency_key_for_fulfillment(&order.order_id),
            correlation_id: None,
            entries: vec![
                LedgerEntryDraft::new(
                    format!("Liability.UnredeemedEntitlement.{}", order.user_id),
                    LedgerAmount::from_minor_units(sku.price_minor_units),
                ),
                LedgerEntryDraft::new(
                    revenue_account.to_owned(),
                    LedgerAmount::from_minor_units(-sku.price_minor_units),
                ),
            ],
            metadata: HashMap::from([
                ("order_id".to_owned(), Value::String(order.order_id.clone())),
                (
                    "entitlement_id".to_owned(),
                    Value::String(entitlement_id.to_owned()),
                ),
            ]),
        };
        self.ledger.post_transaction(request).await?;
        Ok(())
    }
}

fn validate_submit_request(request: &SubmitOrderRequest) -> Result<(), RedemptionError> {
    if request.user_id.is_empty() {
        return Err(RedemptionError::EmptyField { field: "user_id" });
    }
    if request.sku_id.is_empty() {
        return Err(RedemptionError::EmptyField { field: "sku_id" });
    }
    if request.idempotency_key.is_empty() {
        return Err(RedemptionError::EmptyField {
            field: "idempotency_key",
        });
    }
    Ok(())
}

fn redemption_purchase_metadata(
    order: &RedemptionOrder,
    sku: &Sku,
    audit_log_id: &str,
) -> HashMap<String, Value> {
    HashMap::from([
        ("order_id".to_owned(), Value::String(order.order_id.clone())),
        ("sku_id".to_owned(), Value::String(sku.sku_id.clone())),
        (
            "audit_log_id".to_owned(),
            Value::String(audit_log_id.to_owned()),
        ),
    ])
}

/// Picks the Revenue.VirtualGoodsSales sub-account for the fulfillment mode.
/// Digital flows land in `.digital`; service flows in `.service`. Matches
/// the pre-seeded chart-of-accounts in
/// `services/wallet-ledger-service/migrations/202605270001_redemption_chart_of_accounts_seed.sql`.
fn revenue_account_for_mode(mode: FulfillmentMode) -> &'static str {
    match mode {
        FulfillmentMode::DigitalImmediate | FulfillmentMode::DigitalCode => {
            "Revenue.VirtualGoodsSales.digital"
        }
        FulfillmentMode::ServiceAsync => "Revenue.VirtualGoodsSales.service",
    }
}

fn generate_audit_log_id() -> String {
    // ULID; satisfies INV-6 (g) v1.1 format check (^[A-Za-z0-9_:-]{8,160}$).
    ulid::Ulid::new().to_string()
}

/// Current wall-clock as Unix epoch seconds. Centralised so the cancel
/// window + expiry sweep paths agree on the source of "now", and so that
/// tests can swap in `cancel_order_at(..., explicit_now_secs)` for
/// deterministic checks. Production callers (HTTP handlers, expiry binary)
/// just read from this helper indirectly via [`current_unix_secs`].
#[must_use]
pub fn current_unix_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |dur| i64::try_from(dur.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::{RedemptionError, RedemptionService, SubmitOrderRequest};
    use crate::{
        catalog::{CatalogItem, CatalogItemKind, FulfillmentMode, Sku, Stock, StockKind},
        entitlement::EntitlementState,
        ledger::{InMemoryLedgerClient, LedgerClient},
        order::OrderState,
        store::{InMemoryRedemptionStore, RedemptionStore},
    };
    use std::sync::Arc;

    async fn build_service_with_sku(
        sku_id: &str,
        price: i64,
        mode: FulfillmentMode,
    ) -> (
        RedemptionService,
        Arc<InMemoryRedemptionStore>,
        Arc<InMemoryLedgerClient>,
    ) {
        let store = Arc::new(InMemoryRedemptionStore::new());
        let ledger = Arc::new(InMemoryLedgerClient::new());
        let service = RedemptionService::new(store.clone() as Arc<_>, ledger.clone() as Arc<_>);

        let item = CatalogItem::new("item_badge", CatalogItemKind::Digital, "cosmetic", "Badge")
            .expect("item");
        let sku = Sku::new(sku_id, &item.item_id, price, mode, 30).expect("sku");
        let stock = Stock {
            sku_id: sku_id.to_owned(),
            kind: StockKind::Unlimited,
            available_qty: 0,
            reservation_ttl_seconds: 300,
        };
        store.upsert_catalog_item(item).await.expect("item upsert");
        store.upsert_sku(sku).await.expect("sku upsert");
        store.upsert_stock(stock).await.expect("stock upsert");

        (service, store, ledger)
    }

    fn submit_request(user_id: &str, sku_id: &str, key: &str) -> SubmitOrderRequest {
        SubmitOrderRequest {
            user_id: user_id.to_owned(),
            sku_id: sku_id.to_owned(),
            idempotency_key: key.to_owned(),
            correlation_id: None,
        }
    }

    #[tokio::test]
    async fn digital_immediate_happy_path_ends_fulfilled() {
        let (service, store, ledger) =
            build_service_with_sku("sku_a", 500, FulfillmentMode::DigitalImmediate).await;
        ledger.seed_deposit("user_42", 100_000).await.expect("seed");

        let view = service
            .submit_order(submit_request("user_42", "sku_a", "client-key-1"))
            .await
            .expect("submit");
        assert_eq!(view.state, OrderState::Fulfilled);
        assert!(!view.replayed);
        assert!(view.entitlement_id.is_some());

        let entitlements = store
            .list_entitlements_for_owner("user_42")
            .await
            .expect("list");
        assert_eq!(entitlements.len(), 1);
        assert_eq!(entitlements[0].state, EntitlementState::Fulfilled);

        let balance = ledger.get_wallet_balance("user_42").await.expect("balance");
        assert_eq!(balance.available_minor_units, 99_500);
        assert_eq!(
            ledger
                .account_balance("Revenue.VirtualGoodsSales.digital")
                .expect("rev"),
            -500,
            "revenue stays credit-positive in SDK sign convention"
        );
    }

    #[tokio::test]
    async fn digital_code_stops_at_committed_with_entitlement_issued() {
        let (service, _store, ledger) =
            build_service_with_sku("sku_b", 750, FulfillmentMode::DigitalCode).await;
        ledger.seed_deposit("user_42", 5_000).await.expect("seed");

        let view = service
            .submit_order(submit_request("user_42", "sku_b", "client-key-2"))
            .await
            .expect("submit");
        assert_eq!(view.state, OrderState::Committed);
        assert!(view.entitlement_id.is_some());

        // No fulfillment ledger txn yet for non-immediate modes.
        let posted = ledger.recorded_transactions().expect("recorded");
        let fulfillment_count = posted
            .iter()
            .filter(|t| t.txn_type == "entitlement_fulfillment")
            .count();
        assert_eq!(fulfillment_count, 0);
    }

    #[tokio::test]
    async fn idempotent_replay_returns_prior_order_view_without_double_ledger_post() {
        let (service, _store, ledger) =
            build_service_with_sku("sku_a", 500, FulfillmentMode::DigitalImmediate).await;
        ledger.seed_deposit("user_42", 100_000).await.expect("seed");

        let first = service
            .submit_order(submit_request("user_42", "sku_a", "shared-key"))
            .await
            .expect("first");
        let replay = service
            .submit_order(submit_request("user_42", "sku_a", "shared-key"))
            .await
            .expect("replay");

        assert_eq!(first.order_id, replay.order_id);
        assert_eq!(first.entitlement_id, replay.entitlement_id);
        assert!(replay.replayed);

        // Balance only decremented once even though submit_order ran twice.
        let balance = ledger.get_wallet_balance("user_42").await.expect("balance");
        assert_eq!(balance.available_minor_units, 99_500);
    }

    #[tokio::test]
    async fn unknown_sku_yields_sku_not_found() {
        let store = Arc::new(InMemoryRedemptionStore::new());
        let ledger = Arc::new(InMemoryLedgerClient::new());
        let service = RedemptionService::new(store.clone() as Arc<_>, ledger.clone() as Arc<_>);

        let error = match service
            .submit_order(submit_request("user_42", "nonexistent", "k"))
            .await
        {
            Ok(_) => panic!("unknown sku should be rejected"),
            Err(error) => error,
        };
        assert!(matches!(error, RedemptionError::SkuNotFound { .. }));
    }

    #[tokio::test]
    async fn insufficient_balance_aborts_before_ledger_write() {
        let (service, _store, ledger) =
            build_service_with_sku("sku_a", 500, FulfillmentMode::DigitalImmediate).await;
        // No deposit; user has 0 balance.

        let error = match service
            .submit_order(submit_request("user_42", "sku_a", "k"))
            .await
        {
            Ok(_) => panic!("zero balance should be rejected"),
            Err(error) => error,
        };
        assert!(matches!(error, RedemptionError::InsufficientBalance { .. }));

        // No redemption_purchase ledger txn should have been posted.
        let posted = ledger.recorded_transactions().expect("recorded");
        assert!(
            posted.iter().all(|t| t.txn_type != "redemption_purchase"),
            "no redemption_purchase should be posted when balance check fails"
        );
    }

    #[tokio::test]
    async fn submit_request_rejects_empty_user_id_and_idempotency_key() {
        let (service, _store, _ledger) =
            build_service_with_sku("sku_a", 500, FulfillmentMode::DigitalImmediate).await;
        for (user, sku, key, expected_field) in [
            ("", "sku_a", "k", "user_id"),
            ("u", "", "k", "sku_id"),
            ("u", "sku_a", "", "idempotency_key"),
        ] {
            let error = match service.submit_order(submit_request(user, sku, key)).await {
                Ok(_) => panic!("empty field must error"),
                Err(error) => error,
            };
            match error {
                RedemptionError::EmptyField { field } => assert_eq!(field, expected_field),
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn purchase_metadata_carries_order_sku_and_audit_ulid() {
        let (service, _store, ledger) =
            build_service_with_sku("sku_a", 500, FulfillmentMode::DigitalImmediate).await;
        ledger.seed_deposit("user_42", 1_000).await.expect("seed");

        let view = service
            .submit_order(submit_request("user_42", "sku_a", "k"))
            .await
            .expect("submit");

        let posted = ledger.recorded_transactions().expect("recorded");
        let purchase = posted
            .iter()
            .find(|t| t.txn_type == "redemption_purchase")
            .expect("purchase posted");
        assert_eq!(
            purchase.metadata.get("order_id").and_then(|v| v.as_str()),
            Some(view.order_id.as_str())
        );
        assert_eq!(
            purchase.metadata.get("sku_id").and_then(|v| v.as_str()),
            Some("sku_a")
        );
        let audit_id = purchase
            .metadata
            .get("audit_log_id")
            .and_then(|v| v.as_str())
            .expect("audit_log_id present");
        // 26-char ULID satisfies INV-6 (g) v1.1 format check.
        assert_eq!(audit_id.len(), 26);
    }

    #[tokio::test]
    async fn get_order_returns_view_with_entitlement_after_commit() {
        let (service, _store, ledger) =
            build_service_with_sku("sku_a", 500, FulfillmentMode::DigitalImmediate).await;
        ledger.seed_deposit("user_42", 1_000).await.expect("seed");

        let view = service
            .submit_order(submit_request("user_42", "sku_a", "k"))
            .await
            .expect("submit");
        let loaded = service
            .get_order(&view.order_id)
            .await
            .expect("get")
            .expect("present");
        assert_eq!(loaded.entitlement_id, view.entitlement_id);
        assert_eq!(loaded.state, OrderState::Fulfilled);
    }

    #[tokio::test]
    async fn replay_self_heals_from_accepted_state_drives_to_fulfilled() {
        // codex P3.1 P1 #1 regression guard: simulate the crash window where
        // the ledger purchase committed but order state never advanced past
        // Accepted. The retry must finish the job and return Fulfilled, NOT
        // the stale Accepted view.
        let (service, store, ledger) =
            build_service_with_sku("sku_a", 500, FulfillmentMode::DigitalImmediate).await;
        ledger.seed_deposit("user_42", 100_000).await.expect("seed");

        let first = service
            .submit_order(submit_request("user_42", "sku_a", "k"))
            .await
            .expect("first");
        assert_eq!(first.state, OrderState::Fulfilled);

        // Force state backwards to simulate mid-orchestration crash (in
        // production the store would simply still hold the pre-crash state).
        store
            .update_order_state(&first.order_id, OrderState::Accepted)
            .await
            .expect("rewind to Accepted");

        let replay = service
            .submit_order(submit_request("user_42", "sku_a", "k"))
            .await
            .expect("replay");
        assert_eq!(replay.order_id, first.order_id);
        assert_eq!(replay.state, OrderState::Fulfilled);
        assert!(replay.replayed);

        // Ledger balance only decremented once — the ledger client's
        // idempotency layer (same ledger_idempotency_key_for_purchase)
        // squashes the retry.
        let balance = ledger.get_wallet_balance("user_42").await.expect("balance");
        assert_eq!(balance.available_minor_units, 99_500);
    }

    #[tokio::test]
    async fn replay_self_heals_fulfilled_order_with_issued_entitlement() {
        // codex P3.1 P1 #2 regression guard: order=Fulfilled but
        // entitlement=Issued (the fulfill_immediately partial-failure
        // window). Replay must drive entitlement to Fulfilled.
        let (service, store, ledger) =
            build_service_with_sku("sku_a", 500, FulfillmentMode::DigitalImmediate).await;
        ledger.seed_deposit("user_42", 100_000).await.expect("seed");

        let first = service
            .submit_order(submit_request("user_42", "sku_a", "k"))
            .await
            .expect("first");
        let entitlement_id = first
            .entitlement_id
            .clone()
            .expect("DigitalImmediate must issue entitlement");

        // Simulate the partial-failure window: store layer regresses
        // entitlement to Issued (store doesn't validate transitions —
        // intentional, see `update_entitlement_state` docs).
        store
            .update_entitlement_state(&entitlement_id, EntitlementState::Issued)
            .await
            .expect("rewind entitlement");

        let _ = service
            .submit_order(submit_request("user_42", "sku_a", "k"))
            .await
            .expect("replay");

        let healed = store
            .get_entitlement_for_order(&first.order_id)
            .await
            .expect("get")
            .expect("entitlement present");
        assert_eq!(
            healed.state,
            EntitlementState::Fulfilled,
            "replay must self-heal entitlement to Fulfilled"
        );
    }

    // -------------------------------------------------------------------
    // P4 refund / expiry / breakage tests
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn cancel_order_from_committed_refunds_and_restores_balance() {
        // Use DigitalCode so submit_order stops at Committed (Fulfilled
        // can't be cancelled by the cancel-window path).
        let (service, store, ledger) =
            build_service_with_sku("code_a", 1_000, FulfillmentMode::DigitalCode).await;
        ledger.seed_deposit("user_42", 10_000).await.expect("seed");
        let view = service
            .submit_order(submit_request("user_42", "code_a", "k"))
            .await
            .expect("submit");
        assert_eq!(view.state, OrderState::Committed);
        let balance_after_commit = ledger
            .get_wallet_balance("user_42")
            .await
            .expect("balance")
            .available_minor_units;
        assert_eq!(balance_after_commit, 9_000);

        let refunded = service
            .cancel_order(&view.order_id, current_unix_secs())
            .await
            .expect("cancel");
        assert_eq!(refunded.state, OrderState::Refunded);

        // Wallet balance restored.
        let balance_after_refund = ledger
            .get_wallet_balance("user_42")
            .await
            .expect("balance")
            .available_minor_units;
        assert_eq!(balance_after_refund, 10_000);

        // Entitlement moved to Refunded.
        let healed = store
            .get_entitlement_for_order(&view.order_id)
            .await
            .expect("get")
            .expect("present");
        assert_eq!(healed.state, EntitlementState::Refunded);

        // Ledger has redemption_refund txn.
        let posted = ledger.recorded_transactions().expect("recorded");
        assert!(posted.iter().any(|t| t.txn_type == "redemption_refund"));
    }

    #[tokio::test]
    async fn cancel_order_rejects_non_committed_state() {
        let (service, _store, ledger) =
            build_service_with_sku("sku_a", 500, FulfillmentMode::DigitalImmediate).await;
        ledger.seed_deposit("user_42", 1_000).await.expect("seed");
        let view = service
            .submit_order(submit_request("user_42", "sku_a", "k"))
            .await
            .expect("submit");
        assert_eq!(view.state, OrderState::Fulfilled);
        let error = match service
            .cancel_order(&view.order_id, current_unix_secs())
            .await
        {
            Ok(_) => panic!("cancel_order on Fulfilled must error"),
            Err(error) => error,
        };
        match error {
            RedemptionError::OrderStateNotAllowed {
                operation,
                state,
                allowed_states,
                ..
            } => {
                assert_eq!(operation, "cancel_order");
                assert_eq!(state, OrderState::Fulfilled);
                assert_eq!(allowed_states, &[OrderState::Committed]);
            }
            other => panic!("expected OrderStateNotAllowed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn cancel_order_idempotent_replay_does_not_double_refund() {
        let (service, _store, ledger) =
            build_service_with_sku("code_a", 1_000, FulfillmentMode::DigitalCode).await;
        ledger.seed_deposit("user_42", 10_000).await.expect("seed");
        let view = service
            .submit_order(submit_request("user_42", "code_a", "k"))
            .await
            .expect("submit");

        let first = service
            .cancel_order(&view.order_id, current_unix_secs())
            .await
            .expect("cancel 1");
        assert_eq!(first.state, OrderState::Refunded);

        // Second cancel fails — already terminal.
        let err = match service
            .cancel_order(&view.order_id, current_unix_secs())
            .await
        {
            Ok(_) => panic!("second cancel on Refunded must error"),
            Err(error) => error,
        };
        assert!(matches!(err, RedemptionError::OrderStateNotAllowed { .. }));

        // Ledger only saw ONE redemption_refund — no double-refund.
        let posted = ledger.recorded_transactions().expect("recorded");
        let refund_count = posted
            .iter()
            .filter(|t| t.txn_type == "redemption_refund")
            .count();
        assert_eq!(refund_count, 1, "exactly one refund posted");
        let balance = ledger.get_wallet_balance("user_42").await.expect("balance");
        assert_eq!(balance.available_minor_units, 10_000);
    }

    #[tokio::test]
    async fn cancel_order_rejects_when_window_has_elapsed() {
        // SKU with 60-second cancel window. submit_order stamps
        // committed_at = SystemTime::now() unix secs. Passing
        // committed_at + 61 as `now` puts the call past the deadline.
        let store = Arc::new(InMemoryRedemptionStore::new());
        let ledger = Arc::new(InMemoryLedgerClient::new());
        let item = CatalogItem::new(
            "item_w",
            CatalogItemKind::Digital,
            "cosmetic",
            "Window-test SKU",
        )
        .expect("item");
        let sku = Sku::new(
            "sku_window",
            "item_w",
            500,
            FulfillmentMode::DigitalCode,
            30,
        )
        .expect("sku")
        .with_cancel_window(60)
        .expect("60s window");
        let stock = Stock {
            sku_id: "sku_window".to_owned(),
            kind: StockKind::Unlimited,
            available_qty: 0,
            reservation_ttl_seconds: 300,
        };
        store.upsert_catalog_item(item).await.expect("item");
        store.upsert_sku(sku).await.expect("sku");
        store.upsert_stock(stock).await.expect("stock");
        ledger.seed_deposit("user_42", 10_000).await.expect("seed");
        let service = RedemptionService::new(store.clone() as Arc<_>, ledger.clone() as Arc<_>);

        let view = service
            .submit_order(submit_request("user_42", "sku_window", "k"))
            .await
            .expect("submit");
        assert_eq!(view.state, OrderState::Committed);

        let order = store
            .get_order(&view.order_id)
            .await
            .expect("get")
            .expect("present");
        let committed_at = order
            .committed_at_unix_secs
            .expect("submit must stamp committed_at");

        // Now within window: succeeds.
        let now_within = committed_at + 30;
        let _ = service
            .cancel_order(&view.order_id, now_within)
            .await
            .expect("within-window cancel must succeed");

        // Submit a second order, then attempt cancel past the deadline.
        let view2 = service
            .submit_order(submit_request("user_42", "sku_window", "k2"))
            .await
            .expect("submit 2");
        let order2 = store
            .get_order(&view2.order_id)
            .await
            .expect("get")
            .expect("present");
        let committed_at2 = order2
            .committed_at_unix_secs
            .expect("submit must stamp committed_at");
        let now_past = committed_at2 + 61;
        let error = match service.cancel_order(&view2.order_id, now_past).await {
            Ok(_) => panic!("past-deadline cancel must be rejected"),
            Err(error) => error,
        };
        match error {
            RedemptionError::CancelWindowExpired {
                deadline_unix_secs,
                now_unix_secs,
                ..
            } => {
                assert_eq!(deadline_unix_secs, committed_at2 + 60);
                assert_eq!(now_unix_secs, now_past);
            }
            other => panic!("expected CancelWindowExpired, got {other:?}"),
        }

        // Order must remain Committed (no state change on rejection).
        let after = store
            .get_order(&view2.order_id)
            .await
            .expect("get")
            .expect("present");
        assert_eq!(after.state, OrderState::Committed);
    }

    #[tokio::test]
    async fn cs_refund_from_fulfilled_moves_to_refunded_and_restores_balance() {
        let (service, _store, ledger) =
            build_service_with_sku("sku_a", 500, FulfillmentMode::DigitalImmediate).await;
        ledger.seed_deposit("user_42", 1_000).await.expect("seed");
        let view = service
            .submit_order(submit_request("user_42", "sku_a", "k"))
            .await
            .expect("submit");
        assert_eq!(view.state, OrderState::Fulfilled);

        let refunded = service
            .refund_fulfilled_order_cs(&view.order_id)
            .await
            .expect("cs refund");
        assert_eq!(refunded.state, OrderState::Refunded);

        let balance = ledger.get_wallet_balance("user_42").await.expect("balance");
        assert_eq!(balance.available_minor_units, 1_000);
    }

    #[tokio::test]
    async fn cs_refund_rejects_non_fulfilled_state() {
        let (service, _store, ledger) =
            build_service_with_sku("code_a", 500, FulfillmentMode::DigitalCode).await;
        ledger.seed_deposit("user_42", 1_000).await.expect("seed");
        let view = service
            .submit_order(submit_request("user_42", "code_a", "k"))
            .await
            .expect("submit");
        assert_eq!(view.state, OrderState::Committed);

        let err = match service.refund_fulfilled_order_cs(&view.order_id).await {
            Ok(_) => panic!("cs refund on Committed must error"),
            Err(error) => error,
        };
        assert!(matches!(err, RedemptionError::OrderStateNotAllowed { .. }));
    }

    #[tokio::test]
    async fn expire_order_from_committed_writes_breakage_and_advances() {
        let (service, store, ledger) =
            build_service_with_sku("code_a", 1_500, FulfillmentMode::DigitalCode).await;
        ledger.seed_deposit("user_42", 10_000).await.expect("seed");
        let view = service
            .submit_order(submit_request("user_42", "code_a", "k"))
            .await
            .expect("submit");

        let expired = service.expire_order(&view.order_id).await.expect("expire");
        assert_eq!(expired.state, OrderState::Expired);

        let entitlement = store
            .get_entitlement_for_order(&view.order_id)
            .await
            .expect("get")
            .expect("present");
        assert_eq!(entitlement.state, EntitlementState::Expired);

        // Ledger: liability cleared into Revenue.EntitlementBreakage.
        let posted = ledger.recorded_transactions().expect("recorded");
        assert!(posted.iter().any(|t| t.txn_type == "entitlement_expiry"));
        let breakage = ledger
            .account_balance("Revenue.EntitlementBreakage")
            .expect("balance");
        assert_eq!(breakage, -1_500, "credit-positive in SDK convention");

        // Wallet NOT refunded (this is breakage, not refund).
        let wallet = ledger.get_wallet_balance("user_42").await.expect("balance");
        assert_eq!(wallet.available_minor_units, 10_000 - 1_500);
    }

    #[tokio::test]
    async fn run_expiry_sweep_processes_overdue_orders_and_leaves_fresh_ones() {
        let (service, store, ledger) =
            build_service_with_sku("code_a", 1_000, FulfillmentMode::DigitalCode).await;
        ledger.seed_deposit("user_42", 100_000).await.expect("seed");

        // Two orders submitted at "now"; both have expires_at =
        // committed_at + 30 days. We'll pass a `now` past both, then again
        // past only one, to verify selectivity.
        let view_a = service
            .submit_order(submit_request("user_42", "code_a", "k1"))
            .await
            .expect("submit a");
        let view_b = service
            .submit_order(submit_request("user_42", "code_a", "k2"))
            .await
            .expect("submit b");

        let order_a = store
            .get_order(&view_a.order_id)
            .await
            .expect("get")
            .expect("present");
        let order_b = store
            .get_order(&view_b.order_id)
            .await
            .expect("get")
            .expect("present");
        let exp_a = order_a.expires_at_unix_secs.expect("expires_at a");
        let exp_b = order_b.expires_at_unix_secs.expect("expires_at b");
        assert_eq!(exp_a, exp_b, "same SKU TTL → same expires_at");

        // Pass `now` past both deadlines: sweep should process both.
        let stats = service
            .run_expiry_sweep(exp_a + 1, 10)
            .await
            .expect("sweep");
        assert_eq!(stats.processed, 2);

        // Re-running with the same `now` finds nothing (both are Expired,
        // not Committed). Sweep is idempotent.
        let again = service
            .run_expiry_sweep(exp_a + 1, 10)
            .await
            .expect("sweep 2");
        assert_eq!(again.processed, 0);

        // Both orders now Expired.
        for order_id in [&view_a.order_id, &view_b.order_id] {
            let order = store
                .get_order(order_id)
                .await
                .expect("get")
                .expect("present");
            assert_eq!(order.state, OrderState::Expired);
        }
        // Revenue.EntitlementBreakage credited 1_000 × 2.
        let breakage = ledger
            .account_balance("Revenue.EntitlementBreakage")
            .expect("balance");
        assert_eq!(breakage, -2_000);
    }

    #[tokio::test]
    async fn run_expiry_sweep_respects_limit_and_processes_oldest_first() {
        let (service, store, ledger) =
            build_service_with_sku("code_a", 500, FulfillmentMode::DigitalCode).await;
        ledger.seed_deposit("user_42", 100_000).await.expect("seed");

        let view_a = service
            .submit_order(submit_request("user_42", "code_a", "k1"))
            .await
            .expect("a");
        let view_b = service
            .submit_order(submit_request("user_42", "code_a", "k2"))
            .await
            .expect("b");
        let view_c = service
            .submit_order(submit_request("user_42", "code_a", "k3"))
            .await
            .expect("c");

        // Pass limit=2 so only 2 of 3 get processed this pass.
        let now = current_unix_secs() + 86_400 * 31;
        let stats = service
            .run_expiry_sweep(now, 2)
            .await
            .expect("sweep limit=2");
        assert_eq!(stats.processed, 2);

        // One order remains Committed; the other two are Expired. All three
        // share the same expires_at (same SKU + back-to-back submits), so
        // we don't assert specific identity — just count.
        let mut final_states: Vec<OrderState> = Vec::with_capacity(3);
        for view in [&view_a, &view_b, &view_c] {
            let order = store
                .get_order(&view.order_id)
                .await
                .expect("store")
                .expect("present");
            final_states.push(order.state);
        }
        let committed_count = final_states
            .iter()
            .filter(|s| matches!(s, OrderState::Committed))
            .count();
        let expired_count = final_states
            .iter()
            .filter(|s| matches!(s, OrderState::Expired))
            .count();
        assert_eq!(committed_count, 1);
        assert_eq!(expired_count, 2);
    }

    #[tokio::test]
    async fn expire_order_rejects_fulfilled_and_refunded() {
        let (service, _store, ledger) =
            build_service_with_sku("sku_a", 500, FulfillmentMode::DigitalImmediate).await;
        ledger.seed_deposit("user_42", 5_000).await.expect("seed");

        // Try to expire a Fulfilled order — refuse.
        let view = service
            .submit_order(submit_request("user_42", "sku_a", "k1"))
            .await
            .expect("submit 1");
        let err = match service.expire_order(&view.order_id).await {
            Ok(_) => panic!("expire on Fulfilled must error"),
            Err(error) => error,
        };
        assert!(matches!(err, RedemptionError::OrderStateNotAllowed { .. }));

        // Try to expire a non-existent order — OrderNotFound.
        let err = match service.expire_order("01HX-DOES-NOT-EXIST").await {
            Ok(_) => panic!("unknown order must error"),
            Err(error) => error,
        };
        assert!(matches!(err, RedemptionError::OrderNotFound { .. }));
    }

    // -------------------------------------------------------------------
    // P3.2 digital_code worker tests
    // -------------------------------------------------------------------

    use super::current_unix_secs;
    use crate::partner::InMemoryPartnerAdapter;
    use crate::vault::{InMemoryVault, VaultClient};

    #[tokio::test]
    async fn digital_code_worker_drives_issued_to_fulfilled() {
        let (service, store, ledger) =
            build_service_with_sku("code_a", 250, FulfillmentMode::DigitalCode).await;
        ledger.seed_deposit("user_42", 10_000).await.expect("seed");
        let vault = InMemoryVault::new();
        vault
            .seed_codes("code_a", vec!["AAA-001".to_owned(), "AAA-002".to_owned()])
            .expect("seed codes");

        // submit_order stops at Committed for non-DigitalImmediate modes.
        let view = service
            .submit_order(submit_request("user_42", "code_a", "k1"))
            .await
            .expect("submit");
        assert_eq!(view.state, OrderState::Committed);
        assert!(view.entitlement_id.is_some(), "entitlement issued");
        let pending = store
            .list_entitlements_pending_fulfillment(FulfillmentMode::DigitalCode, 10)
            .await
            .expect("list");
        assert_eq!(pending.len(), 1, "should have 1 pending digital_code");

        // Worker pass picks it up and fulfills.
        let stats = service
            .run_digital_code_worker(&vault, 10)
            .await
            .expect("worker");
        assert_eq!(stats.processed, 1);
        assert_eq!(stats.vault_exhausted, 0);
        assert_eq!(stats.ledger_failed, 0);

        // Order + entitlement both Fulfilled, payload contains code.
        let healed = store
            .get_entitlement_for_order(&view.order_id)
            .await
            .expect("get")
            .expect("present");
        assert_eq!(healed.state, EntitlementState::Fulfilled);
        let payload = healed
            .fulfillment_payload
            .as_ref()
            .expect("payload populated");
        assert_eq!(
            payload.get("code").and_then(|v| v.as_str()),
            Some("AAA-001")
        );
        assert_eq!(vault.remaining("code_a").expect("remaining"), 1);

        // Ledger has purchase + fulfillment for this order.
        let posted = ledger.recorded_transactions().expect("recorded");
        let kinds: Vec<&str> = posted.iter().map(|t| t.txn_type.as_str()).collect();
        assert!(kinds.contains(&"redemption_purchase"));
        assert!(kinds.contains(&"entitlement_fulfillment"));
    }

    #[tokio::test]
    async fn digital_code_worker_reuses_payload_on_retry_does_not_double_claim() {
        // P3.2 crash-safety guard: if a prior worker pass claimed a code but
        // failed to post the ledger / advance state, the entitlement holds
        // the payload at state=Issued. A retry MUST reuse that code instead
        // of claiming a fresh one.
        let (service, store, ledger) =
            build_service_with_sku("code_a", 250, FulfillmentMode::DigitalCode).await;
        ledger.seed_deposit("user_42", 10_000).await.expect("seed");
        let vault = InMemoryVault::new();
        vault
            .seed_codes("code_a", vec!["ONLY-001".to_owned()])
            .expect("seed");

        let view = service
            .submit_order(submit_request("user_42", "code_a", "k1"))
            .await
            .expect("submit");
        let entitlement_id = view.entitlement_id.expect("issued");

        // Simulate "claimed code but failed to post ledger" by directly
        // setting the payload + leaving state at Issued. Vault should now be
        // empty (the one code was 'consumed' by the prior failed attempt).
        let code = vault.claim_code("code_a").await.expect("manual claim");
        assert_eq!(vault.remaining("code_a").expect("remaining"), 0);
        store
            .set_entitlement_payload(&entitlement_id, serde_json::json!({ "code": code.clone() }))
            .await
            .expect("set payload");

        // Retry: vault is exhausted, but worker should NOT call claim_code
        // because payload is already present. Must succeed.
        let stats = service
            .run_digital_code_worker(&vault, 10)
            .await
            .expect("worker retry");
        assert_eq!(
            stats.processed, 1,
            "retry must reuse existing payload without re-claiming"
        );

        // Final payload still holds the original code.
        let healed = store
            .get_entitlement_for_order(&view.order_id)
            .await
            .expect("get")
            .expect("present");
        assert_eq!(
            healed
                .fulfillment_payload
                .as_ref()
                .and_then(|v| v.get("code"))
                .and_then(|v| v.as_str()),
            Some(code.as_str())
        );
        assert_eq!(healed.state, EntitlementState::Fulfilled);
    }

    #[tokio::test]
    async fn digital_code_worker_records_vault_exhaustion_without_aborting_batch() {
        let (service, store, ledger) =
            build_service_with_sku("code_a", 250, FulfillmentMode::DigitalCode).await;
        ledger.seed_deposit("user_42", 100_000).await.expect("seed");
        ledger
            .seed_deposit("user_99", 100_000)
            .await
            .expect("seed 2");
        let vault = InMemoryVault::new();
        // Only one code available; we'll create two pending entitlements.
        vault
            .seed_codes("code_a", vec!["LUCKY-001".to_owned()])
            .expect("seed");

        service
            .submit_order(submit_request("user_42", "code_a", "k1"))
            .await
            .expect("submit 1");
        service
            .submit_order(submit_request("user_99", "code_a", "k2"))
            .await
            .expect("submit 2");

        let stats = service
            .run_digital_code_worker(&vault, 10)
            .await
            .expect("worker");
        assert_eq!(stats.processed, 1, "one entitlement claims the only code");
        assert_eq!(
            stats.vault_exhausted, 1,
            "the second is recorded but doesn't abort the batch"
        );

        // The stuck entitlement remains Issued with no payload — operators
        // can re-seed the vault and re-run the worker to clear it.
        let remaining_issued = store
            .list_entitlements_pending_fulfillment(FulfillmentMode::DigitalCode, 10)
            .await
            .expect("list");
        assert_eq!(remaining_issued.len(), 1);
        assert!(remaining_issued[0].fulfillment_payload.is_none());
    }

    // -------------------------------------------------------------------
    // P3.3 service_async worker tests
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn service_async_worker_first_pass_dispatches_and_marks_pending() {
        let (service, store, ledger) =
            build_service_with_sku("svc_a", 1_000, FulfillmentMode::ServiceAsync).await;
        ledger.seed_deposit("user_42", 10_000).await.expect("seed");
        let adapter = InMemoryPartnerAdapter::new();

        let view = service
            .submit_order(submit_request("user_42", "svc_a", "k1"))
            .await
            .expect("submit");
        assert_eq!(view.state, OrderState::Committed);
        let entitlement_id = view.entitlement_id.expect("issued");

        // First worker pass: partner accepts but state stays Pending.
        let stats = service
            .run_service_async_worker(&adapter, 10)
            .await
            .expect("worker pass 1");
        assert_eq!(stats.processed, 0);
        assert_eq!(stats.partner_pending, 1);

        // Entitlement now carries the booking_id; order/entitlement
        // unchanged.
        let dispatched = store
            .get_entitlement_for_order(&view.order_id)
            .await
            .expect("get")
            .expect("present");
        assert_eq!(dispatched.state, EntitlementState::Issued);
        let booking_id = dispatched
            .fulfillment_payload
            .as_ref()
            .and_then(|p| p.get("booking_id"))
            .and_then(|v| v.as_str())
            .expect("booking_id present")
            .to_owned();
        assert_eq!(booking_id.len(), 26, "ULID booking_id");

        // Confirm no fulfillment ledger txn yet.
        let posted = ledger.recorded_transactions().expect("recorded");
        assert!(
            posted
                .iter()
                .all(|t| t.txn_type != "entitlement_fulfillment"),
            "no fulfillment posted while partner still Pending"
        );

        // Mark completed and rerun: this pass advances to Fulfilled.
        adapter
            .mark_completed(
                &booking_id,
                Some(serde_json::json!({"transcript": "https://...t"})),
            )
            .expect("mark");
        let stats = service
            .run_service_async_worker(&adapter, 10)
            .await
            .expect("worker pass 2");
        assert_eq!(stats.processed, 1);
        assert_eq!(stats.partner_pending, 0);

        let healed = store
            .get_entitlement_for_order(&view.order_id)
            .await
            .expect("get")
            .expect("present");
        assert_eq!(healed.state, EntitlementState::Fulfilled);
        // Payload merges completion metadata while preserving booking_id.
        let payload = healed.fulfillment_payload.as_ref().expect("payload");
        assert_eq!(
            payload.get("booking_id").and_then(|v| v.as_str()),
            Some(booking_id.as_str())
        );
        assert!(
            payload.get("completion").is_some(),
            "completion metadata merged into payload"
        );

        let order = store
            .get_order(&view.order_id)
            .await
            .expect("get order")
            .expect("present");
        assert_eq!(order.state, OrderState::Fulfilled);
        let _ = entitlement_id;
    }

    #[tokio::test]
    async fn service_async_worker_does_not_redispatch_on_retry() {
        // P3.3 crash-safety: a prior worker pass dispatched (booking_id
        // stored on payload) but failed before posting fulfillment. The
        // next pass MUST reuse the booking_id, not call dispatch again.
        let (service, _store, ledger) =
            build_service_with_sku("svc_a", 1_000, FulfillmentMode::ServiceAsync).await;
        ledger.seed_deposit("user_42", 10_000).await.expect("seed");
        let adapter = InMemoryPartnerAdapter::new();

        service
            .submit_order(submit_request("user_42", "svc_a", "k1"))
            .await
            .expect("submit");

        // First pass: dispatch — booking_id stored.
        let _ = service
            .run_service_async_worker(&adapter, 10)
            .await
            .expect("pass 1");

        // Snapshot bookings on partner side. Subsequent worker passes must
        // not create new bookings even though entitlement is still Issued.
        let pass2 = service
            .run_service_async_worker(&adapter, 10)
            .await
            .expect("pass 2");
        let pass3 = service
            .run_service_async_worker(&adapter, 10)
            .await
            .expect("pass 3");

        // All subsequent passes count as partner_pending (booking exists,
        // partner status still Pending). NO additional dispatches happen,
        // so we only ever have 1 booking_id.
        assert_eq!(pass2.partner_pending, 1);
        assert_eq!(pass3.partner_pending, 1);
        assert_eq!(pass2.processed, 0);
        assert_eq!(pass3.processed, 0);
    }

    #[tokio::test]
    async fn service_async_worker_records_partner_failure_without_fulfilling() {
        let (service, store, ledger) =
            build_service_with_sku("svc_a", 1_000, FulfillmentMode::ServiceAsync).await;
        ledger.seed_deposit("user_42", 10_000).await.expect("seed");
        let adapter = InMemoryPartnerAdapter::new();

        service
            .submit_order(submit_request("user_42", "svc_a", "k1"))
            .await
            .expect("submit");

        // Dispatch first.
        let stats = service
            .run_service_async_worker(&adapter, 10)
            .await
            .expect("dispatch pass");
        assert_eq!(stats.partner_pending, 1);

        // Partner fails the booking; subsequent worker pass records that
        // outcome but does NOT post the fulfillment ledger txn or advance
        // state.
        let booking_id = adapter_only_booking_id(&adapter);
        adapter
            .mark_failed(&booking_id, "partner_capacity".to_owned())
            .expect("mark failed");

        let stats = service
            .run_service_async_worker(&adapter, 10)
            .await
            .expect("failure pass");
        assert_eq!(stats.processed, 0);
        assert_eq!(stats.partner_failed, 1);

        // Entitlement remains Issued — refund path is P4.
        let stuck = store
            .list_entitlements_pending_fulfillment(FulfillmentMode::ServiceAsync, 10)
            .await
            .expect("list");
        assert_eq!(stuck.len(), 1);
        assert_eq!(stuck[0].state, EntitlementState::Issued);

        let posted = ledger.recorded_transactions().expect("recorded");
        assert!(
            posted
                .iter()
                .all(|t| t.txn_type != "entitlement_fulfillment"),
            "no fulfillment ledger txn on partner failure"
        );
    }

    fn adapter_only_booking_id(adapter: &InMemoryPartnerAdapter) -> String {
        let ids = adapter.booking_ids().expect("booking_ids");
        assert_eq!(ids.len(), 1, "expected exactly one dispatched booking");
        ids.into_iter().next().expect("first")
    }

    #[tokio::test]
    async fn list_entitlements_isolates_users() {
        let (service, _store, ledger) =
            build_service_with_sku("sku_a", 500, FulfillmentMode::DigitalImmediate).await;
        ledger
            .seed_deposit("alice", 1_000)
            .await
            .expect("seed alice");
        ledger.seed_deposit("bob", 1_000).await.expect("seed bob");
        service
            .submit_order(submit_request("alice", "sku_a", "alice-key"))
            .await
            .expect("alice");
        service
            .submit_order(submit_request("bob", "sku_a", "bob-key"))
            .await
            .expect("bob");

        let alice_only = service.list_entitlements("alice").await.expect("list");
        assert_eq!(alice_only.len(), 1);
        assert_eq!(alice_only[0].owner_user_id, "alice");
    }
}
