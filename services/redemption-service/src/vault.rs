//! Vault abstraction for `digital_code` fulfillment. SPEC-024 §3.4.
//!
//! A vault holds the pool of pre-allocated activation codes for digital
//! SKUs (third-party platform codes, gift card codes, beta keys, etc.).
//! The `digital_code` worker (`RedemptionService::run_digital_code_worker`)
//! claims one code per entitlement, attaches it to the entitlement payload,
//! posts the `entitlement_fulfillment` ledger txn, and advances state to
//! `Fulfilled`.
//!
//! P3.2 ships the trait + an in-memory implementation suitable for tests
//! and local dev. A real vault implementation (`HashiCorp` Vault, AWS
//! Secrets Manager, partner KMS) plugs in via the same trait without
//! touching orchestrator code.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
};

use async_trait::async_trait;
use thiserror::Error;

#[async_trait]
pub trait VaultClient: Send + Sync {
    /// Atomically claims one unused code from the pool for `sku_id` and
    /// returns it. Subsequent calls on the same `sku_id` must NOT return
    /// the same code — claiming is the equivalent of "use this once".
    ///
    /// # Errors
    ///
    /// Returns [`VaultError::Exhausted`] when no codes remain for the SKU.
    async fn claim_code(&self, sku_id: &str) -> Result<String, VaultError>;
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VaultError {
    /// No unused codes left for this SKU. Operationally: ops needs to
    /// re-seed the vault before more `digital_code` redemptions can be
    /// fulfilled.
    #[error("vault exhausted for sku {sku_id}: no unused codes remain")]
    Exhausted { sku_id: String },
    /// Storage backend unreachable / poisoned (in-memory mutex case here).
    #[error("vault storage unavailable")]
    StorageUnavailable,
}

/// In-memory vault. Pre-seed via [`Self::seed_codes`]; codes pop from the
/// pool on each claim. Intentionally simple: codes are returned in
/// insertion order (FIFO) so tests can predict the next claim.
#[derive(Default, Debug)]
pub struct InMemoryVault {
    inner: Mutex<InMemoryInner>,
}

#[derive(Default, Debug)]
struct InMemoryInner {
    /// `sku_id` → unused code pool (FIFO).
    pools: HashMap<String, Vec<String>>,
}

impl InMemoryVault {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes `codes` onto the pool for `sku_id`. Cumulative — subsequent
    /// calls extend the pool rather than replacing it.
    ///
    /// # Errors
    ///
    /// Returns [`VaultError::StorageUnavailable`] when the internal mutex
    /// is poisoned by a prior panic.
    pub fn seed_codes(&self, sku_id: &str, codes: Vec<String>) -> Result<(), VaultError> {
        let mut guard = self.lock()?;
        guard
            .pools
            .entry(sku_id.to_owned())
            .or_default()
            .extend(codes);
        Ok(())
    }

    /// Test helper: returns the number of unused codes remaining for `sku_id`.
    ///
    /// # Errors
    ///
    /// Returns [`VaultError::StorageUnavailable`] when the internal mutex
    /// is poisoned by a prior panic.
    pub fn remaining(&self, sku_id: &str) -> Result<usize, VaultError> {
        let guard = self.lock()?;
        Ok(guard.pools.get(sku_id).map_or(0, Vec::len))
    }

    fn lock(&self) -> Result<MutexGuard<'_, InMemoryInner>, VaultError> {
        self.inner
            .lock()
            .map_err(|_: PoisonError<_>| VaultError::StorageUnavailable)
    }
}

#[async_trait]
impl VaultClient for InMemoryVault {
    async fn claim_code(&self, sku_id: &str) -> Result<String, VaultError> {
        let mut guard = self.lock()?;
        let pool = guard
            .pools
            .get_mut(sku_id)
            .ok_or_else(|| VaultError::Exhausted {
                sku_id: sku_id.to_owned(),
            })?;
        if pool.is_empty() {
            return Err(VaultError::Exhausted {
                sku_id: sku_id.to_owned(),
            });
        }
        // FIFO: oldest code claimed first. drain(..1) is O(n) but pools are
        // typically small per claim; swap to VecDeque if profile shows hot.
        Ok(pool.remove(0))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::{InMemoryVault, VaultClient, VaultError};

    #[tokio::test]
    async fn claim_pops_codes_in_seed_order() {
        let vault = InMemoryVault::new();
        vault
            .seed_codes("sku_a", vec!["AAA-001".to_owned(), "AAA-002".to_owned()])
            .expect("seed");
        assert_eq!(vault.claim_code("sku_a").await.expect("claim 1"), "AAA-001");
        assert_eq!(vault.claim_code("sku_a").await.expect("claim 2"), "AAA-002");
        assert_eq!(vault.remaining("sku_a").expect("remaining"), 0);
    }

    #[tokio::test]
    async fn claim_returns_exhausted_when_pool_empty() {
        let vault = InMemoryVault::new();
        let Err(error) = vault.claim_code("sku_a").await else {
            panic!("empty pool must error");
        };
        assert_eq!(
            error,
            VaultError::Exhausted {
                sku_id: "sku_a".to_owned()
            }
        );
    }

    #[tokio::test]
    async fn seed_is_cumulative_across_calls() {
        let vault = InMemoryVault::new();
        vault
            .seed_codes("sku_a", vec!["X".to_owned()])
            .expect("seed 1");
        vault
            .seed_codes("sku_a", vec!["Y".to_owned()])
            .expect("seed 2");
        assert_eq!(vault.remaining("sku_a").expect("count"), 2);
        assert_eq!(vault.claim_code("sku_a").await.expect("claim 1"), "X");
        assert_eq!(vault.claim_code("sku_a").await.expect("claim 2"), "Y");
    }
}
