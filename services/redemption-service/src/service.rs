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
        OrderError, OrderState, RedemptionOrder, ledger_idempotency_key_for_fulfillment,
        ledger_idempotency_key_for_purchase,
    },
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
    pub processed: usize,
    pub vault_exhausted: usize,
    pub ledger_failed: usize,
    pub store_failed: usize,
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
    #[error("sku vanished from catalog mid-fulfillment for order {order_id}")]
    SkuVanished { order_id: String },
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
    // P3.2 digital_code worker tests
    // -------------------------------------------------------------------

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
