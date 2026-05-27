//! Ledger client abstraction for the redemption-service.
//!
//! SPEC-024 implementation plan §1 P3: the orchestrator
//! ([`crate::service::RedemptionService`]) writes two ledger transactions —
//! `redemption_purchase` and (for `digital_immediate` SKUs) `entitlement_fulfillment`.
//!
//! P3.1 ships the trait + an in-memory implementation suitable for unit tests
//! and local dev. P3.4 will replace the in-memory client with a real HTTP
//! client wrapping wallet-ledger-service. The trait shape is intentionally
//! identical to wallet-ledger-service's `PostTransactionRequest` /
//! `GetWalletBalanceResponse` JSON contract so the swap is a drop-in.

use std::{
    collections::HashMap,
    sync::{Mutex, MutexGuard, PoisonError},
};

use async_trait::async_trait;
use sdk::ledger::{
    LedgerAmount, LedgerEntryDraft, LedgerTransactionDraft, LedgerTxnKind, LedgerValidationError,
};
use serde_json::Value;
use thiserror::Error;

/// Inbound shape for [`LedgerClient::post_transaction`]. Mirrors the
/// `wallet-ledger-service` `PostTransactionRequest` JSON contract so that
/// swapping the in-memory client for an HTTP client never touches callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostTransactionRequest {
    pub txn_kind: LedgerTxnKind,
    /// `wallet-ledger-service`-side `txn_type` string (e.g. `"redemption_purchase"`).
    /// Carried explicitly so the request shape matches the existing JSON
    /// contract; the orchestrator constructs both this and `txn_kind` together.
    pub txn_type: String,
    pub idempotency_key: String,
    pub correlation_id: Option<String>,
    pub entries: Vec<LedgerEntryDraft>,
    pub metadata: HashMap<String, Value>,
}

/// Outbound shape for [`LedgerClient::post_transaction`]. `replayed = true`
/// indicates an idempotent retry — same `idempotency_key` already committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostTransactionResponse {
    pub txn_id: i64,
    pub idempotency_key: String,
    pub replayed: bool,
}

/// Outbound shape for [`LedgerClient::get_wallet_balance`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalletBalance {
    pub available_minor_units: i64,
    pub locked_minor_units: i64,
    pub frozen_minor_units: i64,
}

impl WalletBalance {
    /// True if the wallet can cover an outflow of `required_minor_units` from
    /// its available bucket. Locked / frozen funds are excluded by design —
    /// matches SPEC-002 §4.2.1.
    #[must_use]
    pub const fn covers(self, required_minor_units: i64) -> bool {
        self.available_minor_units >= required_minor_units
    }
}

#[async_trait]
pub trait LedgerClient: Send + Sync {
    /// Posts a balanced ledger transaction. Validates INV-1 / INV-6 / INV-7
    /// via the SDK before persisting (mirroring the wallet-ledger-service
    /// commit path). Idempotent on `request.idempotency_key`.
    async fn post_transaction(
        &self,
        request: PostTransactionRequest,
    ) -> Result<PostTransactionResponse, LedgerClientError>;

    /// Reads the wallet balance for a user. Returns zero-balanced
    /// [`WalletBalance`] for users with no recorded deposits.
    async fn get_wallet_balance(&self, user_id: &str) -> Result<WalletBalance, LedgerClientError>;
}

/// Errors raised by [`LedgerClient`] implementations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LedgerClientError {
    /// SDK pre-commit validation failed (INV-1 / INV-6 / INV-7).
    #[error("ledger draft rejected by invariant check: {0}")]
    Validation(#[from] LedgerValidationError),
    /// Storage backend unreachable / poisoned (in-memory mutex case here).
    #[error("ledger storage unavailable")]
    StorageUnavailable,
    /// HTTP / transport-layer failure (placeholder for P3.4 real client).
    #[error("ledger transport error: {0}")]
    Transport(String),
}

// ---------------------------------------------------------------------------
// InMemoryLedgerClient — for unit tests and local dev.
// ---------------------------------------------------------------------------

/// In-memory `LedgerClient`. Stores posted transactions and tracks running
/// per-account balances. Mirrors SPEC-002 INV-1 / INV-6 / INV-7 via the SDK
/// validator so test failures match production behaviour exactly.
#[derive(Default, Debug)]
pub struct InMemoryLedgerClient {
    inner: Mutex<InMemoryInner>,
}

#[derive(Default, Debug)]
struct InMemoryInner {
    /// `idempotency_key` → `txn_id`.
    posted_keys: HashMap<String, i64>,
    /// `account_code` → running balance in internal minor units. Signed:
    /// asset positive on debit, liability positive on credit (matches SDK
    /// convention so wallet balance = `-sum(Player.Wallet.user_id entries)`).
    balances: HashMap<String, i64>,
    /// Posted transactions in commit order (for test inspection).
    transactions: Vec<RecordedTransaction>,
    next_txn_id: i64,
}

/// Recorded transaction snapshot for test assertions.
#[derive(Debug, Clone)]
pub struct RecordedTransaction {
    pub txn_id: i64,
    pub txn_type: String,
    pub idempotency_key: String,
    pub metadata: HashMap<String, Value>,
    pub entries: Vec<LedgerEntryDraft>,
}

impl InMemoryLedgerClient {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> Result<MutexGuard<'_, InMemoryInner>, LedgerClientError> {
        self.inner
            .lock()
            .map_err(|_: PoisonError<_>| LedgerClientError::StorageUnavailable)
    }

    /// Test helper: pre-seed a deposit so a user has spendable balance without
    /// going through the orchestrator. Validates the deposit through the SDK
    /// to keep the seed honest.
    ///
    /// # Errors
    ///
    /// Propagates [`LedgerClientError`] from the underlying post.
    pub async fn seed_deposit(
        &self,
        user_id: &str,
        amount_minor_units: i64,
    ) -> Result<(), LedgerClientError> {
        let request = PostTransactionRequest {
            txn_kind: LedgerTxnKind::Deposit,
            txn_type: "deposit".to_owned(),
            idempotency_key: format!("seed-deposit:{user_id}:{amount_minor_units}"),
            correlation_id: None,
            entries: vec![
                LedgerEntryDraft::new(
                    "Bank.TrustAccount.TWD",
                    LedgerAmount::from_minor_units(amount_minor_units),
                ),
                LedgerEntryDraft::new(
                    format!("Player.Wallet.{user_id}"),
                    LedgerAmount::from_minor_units(-amount_minor_units),
                ),
            ],
            metadata: HashMap::new(),
        };
        self.post_transaction(request).await.map(|_| ())
    }

    /// Test helper: read the raw signed balance for an account. For
    /// `Player.Wallet.*` this returns the credit balance as a negative
    /// number (SDK convention); use [`Self::get_wallet_balance`] for the
    /// player-facing positive view.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerClientError::StorageUnavailable`] when the internal
    /// mutex is poisoned by a prior panic.
    pub fn account_balance(&self, account_code: &str) -> Result<i64, LedgerClientError> {
        let guard = self.lock()?;
        Ok(guard.balances.get(account_code).copied().unwrap_or(0))
    }

    /// Test helper: returns all posted transactions in commit order.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerClientError::StorageUnavailable`] when the internal
    /// mutex is poisoned by a prior panic.
    pub fn recorded_transactions(&self) -> Result<Vec<RecordedTransaction>, LedgerClientError> {
        let guard = self.lock()?;
        Ok(guard.transactions.clone())
    }
}

#[async_trait]
impl LedgerClient for InMemoryLedgerClient {
    async fn post_transaction(
        &self,
        request: PostTransactionRequest,
    ) -> Result<PostTransactionResponse, LedgerClientError> {
        // Validate INV-1 / INV-6 / INV-7 via the SDK before persisting.
        let draft = LedgerTransactionDraft::new(request.entries.iter().cloned())
            .with_kind(request.txn_kind);
        draft.validate()?;

        let mut guard = self.lock()?;

        if let Some(&existing) = guard.posted_keys.get(&request.idempotency_key) {
            return Ok(PostTransactionResponse {
                txn_id: existing,
                idempotency_key: request.idempotency_key,
                replayed: true,
            });
        }

        guard.next_txn_id += 1;
        let txn_id = guard.next_txn_id;

        for entry in &request.entries {
            *guard
                .balances
                .entry(entry.account_code().to_owned())
                .or_insert(0) += entry.amount().minor_units();
        }

        guard
            .posted_keys
            .insert(request.idempotency_key.clone(), txn_id);
        guard.transactions.push(RecordedTransaction {
            txn_id,
            txn_type: request.txn_type,
            idempotency_key: request.idempotency_key.clone(),
            metadata: request.metadata,
            entries: request.entries,
        });

        Ok(PostTransactionResponse {
            txn_id,
            idempotency_key: request.idempotency_key,
            replayed: false,
        })
    }

    async fn get_wallet_balance(&self, user_id: &str) -> Result<WalletBalance, LedgerClientError> {
        let guard = self.lock()?;
        let wallet_signed = guard
            .balances
            .get(&format!("Player.Wallet.{user_id}"))
            .copied()
            .unwrap_or(0);
        let locked_signed = guard
            .balances
            .get(&format!("Player.Locked.{user_id}"))
            .copied()
            .unwrap_or(0);
        let frozen_signed = guard
            .balances
            .get(&format!("Player.Frozen.{user_id}"))
            .copied()
            .unwrap_or(0);
        Ok(WalletBalance {
            // Liability accounts grow on credit (negative minor_units in SDK
            // convention); flip the sign for the player-facing balance view.
            available_minor_units: -wallet_signed,
            locked_minor_units: -locked_signed,
            frozen_minor_units: -frozen_signed,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::{InMemoryLedgerClient, LedgerClient, LedgerClientError, PostTransactionRequest};
    use sdk::ledger::{LedgerAmount, LedgerEntryDraft, LedgerTxnKind};
    use std::collections::HashMap;

    #[tokio::test]
    async fn seed_deposit_increases_player_wallet_balance() {
        let client = InMemoryLedgerClient::new();
        client.seed_deposit("user_42", 100_000).await.expect("seed");
        let balance = client.get_wallet_balance("user_42").await.expect("balance");
        assert_eq!(balance.available_minor_units, 100_000);
        assert!(balance.covers(50_000));
        assert!(!balance.covers(100_001));
    }

    #[tokio::test]
    async fn post_transaction_is_idempotent_on_repeated_key() {
        let client = InMemoryLedgerClient::new();
        client.seed_deposit("user_42", 100_000).await.expect("seed");

        let request = PostTransactionRequest {
            txn_kind: LedgerTxnKind::RedemptionPurchase,
            txn_type: "redemption_purchase".to_owned(),
            idempotency_key: "redemption:purchase:01HX".to_owned(),
            correlation_id: None,
            entries: vec![
                LedgerEntryDraft::new(
                    "Player.Wallet.user_42",
                    LedgerAmount::from_minor_units(5_000),
                ),
                LedgerEntryDraft::new(
                    "Liability.UnredeemedEntitlement.user_42",
                    LedgerAmount::from_minor_units(-5_000),
                ),
            ],
            metadata: HashMap::new(),
        };

        let first = client
            .post_transaction(request.clone())
            .await
            .expect("first");
        assert!(!first.replayed);
        let replay = client.post_transaction(request).await.expect("replay");
        assert!(replay.replayed);
        assert_eq!(first.txn_id, replay.txn_id);

        // Balance only moves once even though the same request posted twice.
        let balance = client.get_wallet_balance("user_42").await.expect("balance");
        assert_eq!(balance.available_minor_units, 95_000);
    }

    #[tokio::test]
    async fn post_transaction_rejects_cross_user_redemption() {
        // INV-7 enforced via SDK validate: Player.Wallet.alice ->
        // Liability.UnredeemedEntitlement.bob must fail at the SDK layer
        // before the in-memory store ever sees it.
        let client = InMemoryLedgerClient::new();
        client
            .seed_deposit("user_alice", 100_000)
            .await
            .expect("seed");

        let request = PostTransactionRequest {
            txn_kind: LedgerTxnKind::RedemptionPurchase,
            txn_type: "redemption_purchase".to_owned(),
            idempotency_key: "redemption:purchase:hostile".to_owned(),
            correlation_id: None,
            entries: vec![
                LedgerEntryDraft::new(
                    "Player.Wallet.user_alice",
                    LedgerAmount::from_minor_units(5_000),
                ),
                LedgerEntryDraft::new(
                    "Liability.UnredeemedEntitlement.user_bob",
                    LedgerAmount::from_minor_units(-5_000),
                ),
            ],
            metadata: HashMap::new(),
        };

        let error = match client.post_transaction(request).await {
            Ok(_) => panic!("cross-user redemption must be rejected by SDK INV-7"),
            Err(error) => error,
        };
        assert!(matches!(error, LedgerClientError::Validation(_)));
    }
}
