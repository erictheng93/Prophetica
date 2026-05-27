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

use crate::{
    catalog::{CatalogItem, Sku, Stock},
    entitlement::Entitlement,
    order::RedemptionOrder,
};

#[async_trait]
pub trait RedemptionStore: Send + Sync {
    async fn upsert_catalog_item(&self, item: CatalogItem) -> Result<(), StoreError>;
    async fn upsert_sku(&self, sku: Sku) -> Result<(), StoreError>;
    async fn upsert_stock(&self, stock: Stock) -> Result<(), StoreError>;

    /// Persists an order. On idempotent replay (same `idempotency_key`),
    /// returns the previously persisted order without overwriting.
    async fn record_order(&self, order: RedemptionOrder) -> Result<RedemptionOrder, StoreError>;

    async fn get_order(&self, order_id: &str) -> Result<Option<RedemptionOrder>, StoreError>;

    async fn record_entitlement(&self, entitlement: Entitlement) -> Result<(), StoreError>;

    async fn list_entitlements_for_owner(
        &self,
        owner_user_id: &str,
    ) -> Result<Vec<Entitlement>, StoreError>;
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
    orders_by_idempotency: HashMap<String, String>,
    entitlements: HashMap<String, Entitlement>,
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

    async fn record_order(&self, order: RedemptionOrder) -> Result<RedemptionOrder, StoreError> {
        let mut guard = self.lock()?;
        if let Some(prior_id) = guard.orders_by_idempotency.get(&order.idempotency_key) {
            if let Some(prior) = guard.orders.get(prior_id) {
                return Ok(prior.clone());
            }
        }
        guard
            .orders_by_idempotency
            .insert(order.idempotency_key.clone(), order.order_id.clone());
        guard.orders.insert(order.order_id.clone(), order.clone());
        Ok(order)
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
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StoreError {
    /// The in-memory mutex was poisoned by a previous panic. Treat as a hard
    /// failure — callers should not silently swallow this.
    #[error("redemption store mutex poisoned (prior panic)")]
    Poisoned,
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
