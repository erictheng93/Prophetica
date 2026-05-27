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
//! 1. **Two-phase semantics**: the orchestrator persists the order first,
//!    then writes the ledger, then advances order state. If the process
//!    crashes between ledger commit and state update, the ledger is the
//!    source of truth — a recovery sweep can match on the
//!    `ledger_idempotency_key_for_purchase(order_id)` key and re-advance
//!    the order. We accept this eventual consistency for P3.1; full
//!    transactional coupling waits for the PG-backed store.
//! 2. **Idempotency replay** returns the prior `OrderView` *unchanged* — we
//!    do not re-validate balances or re-post ledger entries on retry,
//!    matching SPEC-002 §6.1 idempotent contract.
//! 3. **INV-7** (cross-user) is enforced by the [`crate::ledger::LedgerClient`]
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

    /// Submits a redemption order. Full path for `DigitalImmediate` SKUs;
    /// partial path (stops at `Committed`, entitlement `Issued`) for the
    /// other fulfillment modes.
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

        if is_replay {
            return self.build_replay_view(recorded).await;
        }

        let balance = self.ledger.get_wallet_balance(&request.user_id).await?;
        if !balance.covers(sku.price_minor_units) {
            return Err(RedemptionError::InsufficientBalance {
                required_minor_units: sku.price_minor_units,
                available_minor_units: balance.available_minor_units,
            });
        }

        self.advance_order(&recorded.order_id, OrderState::Accepted)
            .await?;

        let audit_log_id = generate_audit_log_id();
        self.post_redemption_purchase(
            &recorded,
            &sku,
            &audit_log_id,
            request.correlation_id.as_deref(),
        )
        .await?;

        self.advance_order(&recorded.order_id, OrderState::Committed)
            .await?;

        let entitlement = Entitlement::issue(
            recorded.order_id.clone(),
            recorded.user_id.clone(),
            recorded.sku_id.clone(),
        )?;
        let entitlement_id = entitlement.entitlement_id.clone();
        self.store.record_entitlement(entitlement).await?;

        let final_state = if matches!(sku.fulfillment_mode, FulfillmentMode::DigitalImmediate) {
            self.fulfill_immediately(&recorded, &sku, &entitlement_id)
                .await?;
            OrderState::Fulfilled
        } else {
            OrderState::Committed
        };

        Ok(OrderView {
            order_id: recorded.order_id,
            state: final_state,
            user_id: recorded.user_id,
            sku_id: recorded.sku_id,
            price_minor_units: recorded.price_minor_units,
            entitlement_id: Some(entitlement_id),
            idempotency_key: recorded.idempotency_key,
            replayed: false,
        })
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

    async fn build_replay_view(
        &self,
        recorded: RedemptionOrder,
    ) -> Result<OrderView, RedemptionError> {
        let entitlement = self
            .store
            .get_entitlement_for_order(&recorded.order_id)
            .await?;
        Ok(OrderView {
            order_id: recorded.order_id,
            state: recorded.state,
            user_id: recorded.user_id,
            sku_id: recorded.sku_id,
            price_minor_units: recorded.price_minor_units,
            entitlement_id: entitlement.map(|e| e.entitlement_id),
            idempotency_key: recorded.idempotency_key,
            replayed: true,
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

    async fn fulfill_immediately(
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

        self.advance_order(&order.order_id, OrderState::Fulfilled)
            .await?;
        self.store
            .update_entitlement_state(entitlement_id, EntitlementState::Fulfilled)
            .await
            .map_err(|err| match err {
                StoreError::EntitlementNotFound { entitlement_id } => RedemptionError::Internal {
                    order_id: order.order_id.clone(),
                    message: format!("entitlement {entitlement_id} vanished mid-fulfill"),
                },
                other => RedemptionError::StoreUnavailable(other),
            })?;
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
