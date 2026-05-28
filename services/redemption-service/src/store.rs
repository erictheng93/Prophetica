//! Persistence boundary for the redemption-service domain. SPEC-024 §3 / P2.
//!
//! P2 ships an in-memory implementation suitable for unit tests; the
//! Postgres-backed implementation arrives with P3 / P4 once the fulfillment
//! and refund workflows need transactional access. Keeping the trait stable
//! now lets later phases swap implementations without disturbing the API
//! layer.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
};

use async_trait::async_trait;
use thiserror::Error;

use serde_json::Value;

use crate::{
    catalog::{CatalogItem, FulfillmentMode, Sku, Stock},
    entitlement::{Entitlement, EntitlementState},
    order::{OrderState, RedemptionOrder},
};

#[async_trait]
pub trait RedemptionStore: Send + Sync {
    async fn upsert_catalog_item(&self, item: CatalogItem) -> Result<(), StoreError>;
    async fn upsert_sku(&self, sku: Sku) -> Result<(), StoreError>;
    async fn upsert_stock(&self, stock: Stock) -> Result<(), StoreError>;

    /// SKU lookup used by the orchestrator (`RedemptionService::submit_order`)
    /// to resolve price + fulfillment mode at submit time. Returns `None`
    /// when the SKU is not in the catalog so the orchestrator can return a
    /// 404 instead of leaking storage internals.
    async fn get_sku(&self, sku_id: &str) -> Result<Option<Sku>, StoreError>;

    /// Persists an order. On idempotent replay (same `idempotency_key`),
    /// returns the previously persisted order without overwriting.
    async fn record_order(&self, order: RedemptionOrder) -> Result<RedemptionOrder, StoreError>;

    /// Atomically updates an existing order's state. Returns
    /// [`StoreError::OrderNotFound`] if the `order_id` is unknown. Required
    /// by the orchestrator to advance Submitted -> Accepted -> Committed ->
    /// Fulfilled without losing concurrent writes.
    async fn update_order_state(
        &self,
        order_id: &str,
        new_state: OrderState,
    ) -> Result<(), StoreError>;

    async fn get_order(&self, order_id: &str) -> Result<Option<RedemptionOrder>, StoreError>;

    async fn record_entitlement(&self, entitlement: Entitlement) -> Result<(), StoreError>;

    /// Atomically updates an existing entitlement's state. Returns
    /// [`StoreError::EntitlementNotFound`] if the `entitlement_id` is unknown.
    async fn update_entitlement_state(
        &self,
        entitlement_id: &str,
        new_state: EntitlementState,
    ) -> Result<(), StoreError>;

    async fn get_entitlement_for_order(
        &self,
        order_id: &str,
    ) -> Result<Option<Entitlement>, StoreError>;

    async fn list_entitlements_for_owner(
        &self,
        owner_user_id: &str,
    ) -> Result<Vec<Entitlement>, StoreError>;

    /// Returns up to `limit` entitlements currently in [`EntitlementState::Issued`]
    /// whose backing SKU has fulfillment mode `mode`. Used by the P3.2 /
    /// P3.3 fulfillment workers to find pending work. PG implementation
    /// (P3.4) will JOIN entitlements → skus on `sku_id` to filter
    /// efficiently; the in-memory impl iterates and looks up per-entitlement.
    async fn list_entitlements_pending_fulfillment(
        &self,
        mode: FulfillmentMode,
        limit: usize,
    ) -> Result<Vec<Entitlement>, StoreError>;

    /// Atomically writes `payload` to an entitlement. Used by fulfillment
    /// workers to attach mode-specific delivery data (the activation code
    /// for `digital_code`, the booking ID for `service_async`, etc.).
    /// Returns [`StoreError::EntitlementNotFound`] if the `entitlement_id`
    /// is unknown.
    async fn set_entitlement_payload(
        &self,
        entitlement_id: &str,
        payload: Value,
    ) -> Result<(), StoreError>;
}

/// In-memory `RedemptionStore`. Intentional: no `unwrap` on the mutex; we
/// surface poisoning as [`StoreError::Poisoned`] so callers can decide how to
/// react. Workspace clippy lints (`unwrap_used`) forbid the lazier path.
#[derive(Default, Debug)]
pub struct InMemoryRedemptionStore {
    inner: Mutex<InMemoryInner>,
}

#[derive(Default, Debug)]
struct InMemoryInner {
    catalog_items: HashMap<String, CatalogItem>,
    skus: HashMap<String, Sku>,
    stock: HashMap<String, Stock>,
    orders: HashMap<String, RedemptionOrder>,
    /// Maps the user-scoped idempotency key (see [`idempotency_storage_key`])
    /// to the canonical `order_id`. Keying by `(user_id, idempotency_key)`
    /// instead of `idempotency_key` alone prevents cross-user collisions
    /// that would leak another user's order on replay (codex P3.1 P1 #3).
    orders_by_idempotency: HashMap<String, String>,
    entitlements: HashMap<String, Entitlement>,
}

/// Composite key for idempotent order lookup. Scoping by user prevents
/// user B from receiving user A's order by reusing user A's
/// `idempotency_key` — the prior failure mode flagged by codex on P3.1.
///
/// The separator is ASCII unit separator (U+001F), a control character that
/// can never appear in a user-supplied string in this codebase, so it
/// disambiguates `(user, key)` boundaries without escaping. If product
/// constraints ever loosen to allow control characters in IDs, swap for a
/// tuple `HashMap<(String, String), String>`.
fn idempotency_storage_key(user_id: &str, idempotency_key: &str) -> String {
    format!("{user_id}\u{1f}{idempotency_key}")
}

impl InMemoryRedemptionStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> Result<MutexGuard<'_, InMemoryInner>, StoreError> {
        self.inner
            .lock()
            .map_err(|_: PoisonError<_>| StoreError::Poisoned)
    }
}

#[async_trait]
impl RedemptionStore for InMemoryRedemptionStore {
    async fn upsert_catalog_item(&self, item: CatalogItem) -> Result<(), StoreError> {
        let mut guard = self.lock()?;
        guard.catalog_items.insert(item.item_id.clone(), item);
        Ok(())
    }

    async fn upsert_sku(&self, sku: Sku) -> Result<(), StoreError> {
        let mut guard = self.lock()?;
        guard.skus.insert(sku.sku_id.clone(), sku);
        Ok(())
    }

    async fn upsert_stock(&self, stock: Stock) -> Result<(), StoreError> {
        let mut guard = self.lock()?;
        guard.stock.insert(stock.sku_id.clone(), stock);
        Ok(())
    }

    async fn get_sku(&self, sku_id: &str) -> Result<Option<Sku>, StoreError> {
        let guard = self.lock()?;
        Ok(guard.skus.get(sku_id).cloned())
    }

    async fn record_order(&self, order: RedemptionOrder) -> Result<RedemptionOrder, StoreError> {
        let mut guard = self.lock()?;
        let storage_key = idempotency_storage_key(&order.user_id, &order.idempotency_key);
        if let Some(prior_id) = guard.orders_by_idempotency.get(&storage_key) {
            if let Some(prior) = guard.orders.get(prior_id) {
                return Ok(prior.clone());
            }
        }
        guard
            .orders_by_idempotency
            .insert(storage_key, order.order_id.clone());
        guard.orders.insert(order.order_id.clone(), order.clone());
        Ok(order)
    }

    async fn update_order_state(
        &self,
        order_id: &str,
        new_state: OrderState,
    ) -> Result<(), StoreError> {
        let mut guard = self.lock()?;
        let Some(order) = guard.orders.get_mut(order_id) else {
            return Err(StoreError::OrderNotFound {
                order_id: order_id.to_owned(),
            });
        };
        order.state = new_state;
        Ok(())
    }

    async fn get_order(&self, order_id: &str) -> Result<Option<RedemptionOrder>, StoreError> {
        let guard = self.lock()?;
        Ok(guard.orders.get(order_id).cloned())
    }

    async fn record_entitlement(&self, entitlement: Entitlement) -> Result<(), StoreError> {
        let mut guard = self.lock()?;
        guard
            .entitlements
            .insert(entitlement.entitlement_id.clone(), entitlement);
        Ok(())
    }

    async fn update_entitlement_state(
        &self,
        entitlement_id: &str,
        new_state: EntitlementState,
    ) -> Result<(), StoreError> {
        let mut guard = self.lock()?;
        let Some(entitlement) = guard.entitlements.get_mut(entitlement_id) else {
            return Err(StoreError::EntitlementNotFound {
                entitlement_id: entitlement_id.to_owned(),
            });
        };
        entitlement.state = new_state;
        Ok(())
    }

    async fn get_entitlement_for_order(
        &self,
        order_id: &str,
    ) -> Result<Option<Entitlement>, StoreError> {
        let guard = self.lock()?;
        Ok(guard
            .entitlements
            .values()
            .find(|e| e.order_id == order_id)
            .cloned())
    }

    async fn list_entitlements_for_owner(
        &self,
        owner_user_id: &str,
    ) -> Result<Vec<Entitlement>, StoreError> {
        let guard = self.lock()?;
        Ok(guard
            .entitlements
            .values()
            .filter(|e| e.owner_user_id == owner_user_id)
            .cloned()
            .collect())
    }

    async fn list_entitlements_pending_fulfillment(
        &self,
        mode: FulfillmentMode,
        limit: usize,
    ) -> Result<Vec<Entitlement>, StoreError> {
        let guard = self.lock()?;
        let mut hits: Vec<Entitlement> = guard
            .entitlements
            .values()
            .filter(|e| e.state == EntitlementState::Issued)
            .filter(|e| {
                guard
                    .skus
                    .get(&e.sku_id)
                    .is_some_and(|sku| sku.fulfillment_mode == mode)
            })
            .cloned()
            .collect();
        // Stable ordering by entitlement_id so worker behaviour is
        // deterministic in tests (ULIDs sort by creation time).
        hits.sort_by(|a, b| a.entitlement_id.cmp(&b.entitlement_id));
        hits.truncate(limit);
        Ok(hits)
    }

    async fn set_entitlement_payload(
        &self,
        entitlement_id: &str,
        payload: Value,
    ) -> Result<(), StoreError> {
        let mut guard = self.lock()?;
        let Some(entitlement) = guard.entitlements.get_mut(entitlement_id) else {
            return Err(StoreError::EntitlementNotFound {
                entitlement_id: entitlement_id.to_owned(),
            });
        };
        entitlement.fulfillment_payload = Some(payload);
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StoreError {
    /// The in-memory mutex was poisoned by a previous panic. Treat as a hard
    /// failure — callers should not silently swallow this.
    #[error("redemption store mutex poisoned (prior panic)")]
    Poisoned,
    /// `update_order_state` was called for an order that does not exist.
    #[error("redemption order not found: {order_id}")]
    OrderNotFound { order_id: String },
    /// `update_entitlement_state` was called for an entitlement that does not
    /// exist.
    #[error("entitlement not found: {entitlement_id}")]
    EntitlementNotFound { entitlement_id: String },
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::{InMemoryRedemptionStore, RedemptionStore};
    use crate::{entitlement::Entitlement, order::RedemptionOrder};

    #[tokio::test]
    async fn record_order_returns_prior_on_idempotent_replay() {
        let store = InMemoryRedemptionStore::new();
        let order_a = RedemptionOrder::submit("user_42", "sku_a", 100, "k-1").expect("submit");

        let first = store.record_order(order_a.clone()).await.expect("first");
        assert_eq!(first.order_id, order_a.order_id);

        // Replay with same idempotency_key, even if other fields differ — must
        // return the prior order_id (idempotency contract).
        let order_b = RedemptionOrder::submit("user_42", "sku_a", 100, "k-1").expect("submit");
        let replay = store.record_order(order_b).await.expect("replay");
        assert_eq!(replay.order_id, order_a.order_id);
    }

    #[tokio::test]
    async fn record_order_isolates_idempotency_keys_per_user() {
        // codex P3.1 P1 #3 regression guard: user B reusing user A's
        // idempotency_key must NOT return user A's order. Both must persist
        // as independent orders with distinct order_ids.
        let store = InMemoryRedemptionStore::new();
        let alice = RedemptionOrder::submit("user_alice", "sku_a", 100, "shared-key")
            .expect("alice submit");
        let bob =
            RedemptionOrder::submit("user_bob", "sku_a", 100, "shared-key").expect("bob submit");

        let alice_stored = store
            .record_order(alice.clone())
            .await
            .expect("alice store");
        let bob_stored = store.record_order(bob.clone()).await.expect("bob store");

        assert_eq!(alice_stored.order_id, alice.order_id);
        assert_eq!(alice_stored.user_id, "user_alice");
        assert_eq!(bob_stored.order_id, bob.order_id);
        assert_eq!(bob_stored.user_id, "user_bob");
        assert_ne!(
            alice_stored.order_id, bob_stored.order_id,
            "shared idempotency_key across users must produce distinct order_ids",
        );

        // Replay for each user resolves to their own order, not the other's.
        let alice_replay = RedemptionOrder::submit("user_alice", "sku_a", 100, "shared-key")
            .expect("alice replay submit");
        let resolved = store
            .record_order(alice_replay)
            .await
            .expect("alice replay store");
        assert_eq!(resolved.order_id, alice.order_id);
    }

    #[tokio::test]
    async fn list_entitlements_filters_by_owner() {
        let store = InMemoryRedemptionStore::new();
        let alice = Entitlement::issue("order_1", "user_alice", "sku_a").expect("alice");
        let bob = Entitlement::issue("order_2", "user_bob", "sku_b").expect("bob");
        store
            .record_entitlement(alice.clone())
            .await
            .expect("store alice");
        store.record_entitlement(bob).await.expect("store bob");

        let alice_only = store
            .list_entitlements_for_owner("user_alice")
            .await
            .expect("list");
        assert_eq!(alice_only.len(), 1);
        assert_eq!(alice_only[0].entitlement_id, alice.entitlement_id);
    }
}
