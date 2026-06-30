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
use serde::{Deserialize, Serialize};
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
    /// HTTP / transport-layer failure raised by [`HttpLedgerClient`].
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

// ---------------------------------------------------------------------------
// HttpLedgerClient — production wrapper around wallet-ledger-service HTTP.
// SPEC-024 implementation plan §1 P3.4a.
// ---------------------------------------------------------------------------

/// HTTP-backed `LedgerClient` that targets wallet-ledger-service's
/// `/internal/ledger/*` endpoints. The JSON contract is mirrored exactly by
/// [`HttpPostTransactionRequest`] / [`HttpPostEntry`] /
/// [`HttpWalletBalanceResponse`] so swapping the in-memory client for this
/// one in the binary is a one-line change.
pub struct HttpLedgerClient {
    client: reqwest::Client,
    base_url: String,
}

impl HttpLedgerClient {
    /// `base_url` should be the wallet-ledger-service root (e.g.
    /// `http://wallet-ledger-service:8086`). The client builds
    /// `{base_url}/internal/ledger/*` from this.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerClientError::Transport`] when the underlying
    /// `reqwest::Client::builder` fails to construct (rare — typically
    /// only on TLS misconfiguration).
    pub fn new(base_url: impl Into<String>) -> Result<Self, LedgerClientError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|err| LedgerClientError::Transport(err.to_string()))?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
        })
    }
}

/// JSON shape mirroring wallet-ledger-service `api::PostTransactionRequest`.
/// Kept private (not part of the crate API) — callers go through the
/// [`LedgerClient`] trait.
#[derive(Debug, Serialize)]
struct HttpPostTransactionRequest<'a> {
    txn_type: &'a str,
    idempotency_key: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    correlation_id: Option<&'a str>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    metadata: &'a HashMap<String, Value>,
    entries: Vec<HttpPostEntry<'a>>,
}

#[derive(Debug, Serialize)]
struct HttpPostEntry<'a> {
    account_code: &'a str,
    direction: HttpEntryDirection,
    amount_minor_units: i64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum HttpEntryDirection {
    Debit,
    Credit,
}

#[derive(Debug, Deserialize)]
struct HttpPostTransactionResponse {
    txn_id: i64,
    idempotency_key: String,
    replayed: bool,
}

#[derive(Debug, Deserialize)]
#[allow(clippy::struct_field_names)] // mirrors wallet-ledger-service JSON contract verbatim
struct HttpWalletBalanceResponse {
    available_minor_units: i64,
    locked_minor_units: i64,
    frozen_minor_units: i64,
}

impl<'a> From<&'a LedgerEntryDraft> for HttpPostEntry<'a> {
    fn from(draft: &'a LedgerEntryDraft) -> Self {
        // SDK convention: positive minor_units = debit; negative = credit.
        // The HTTP contract expects a separate `direction` field with the
        // amount always positive.
        let minor_units = draft.amount().minor_units();
        Self {
            account_code: draft.account_code(),
            direction: if minor_units >= 0 {
                HttpEntryDirection::Debit
            } else {
                HttpEntryDirection::Credit
            },
            amount_minor_units: minor_units.abs(),
        }
    }
}

#[async_trait]
impl LedgerClient for HttpLedgerClient {
    async fn post_transaction(
        &self,
        request: PostTransactionRequest,
    ) -> Result<PostTransactionResponse, LedgerClientError> {
        let body = HttpPostTransactionRequest {
            txn_type: &request.txn_type,
            idempotency_key: &request.idempotency_key,
            correlation_id: request.correlation_id.as_deref(),
            metadata: &request.metadata,
            entries: request.entries.iter().map(HttpPostEntry::from).collect(),
        };

        let url = format!("{}/internal/ledger/transactions", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|err| LedgerClientError::Transport(err.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(LedgerClientError::Transport(format!(
                "ledger POST {url} returned HTTP {status}: {body}"
            )));
        }

        let parsed: HttpPostTransactionResponse = response
            .json()
            .await
            .map_err(|err| LedgerClientError::Transport(err.to_string()))?;
        Ok(PostTransactionResponse {
            txn_id: parsed.txn_id,
            idempotency_key: parsed.idempotency_key,
            replayed: parsed.replayed,
        })
    }

    async fn get_wallet_balance(&self, user_id: &str) -> Result<WalletBalance, LedgerClientError> {
        // URL-encode would be safer, but wallet-ledger-service path is
        // `:user_id` with internal-only callers controlling the value. If
        // user_id ever leaks unicode, swap for percent-encoding here.
        let url = format!(
            "{}/internal/ledger/wallets/{user_id}/balance",
            self.base_url
        );
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|err| LedgerClientError::Transport(err.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(LedgerClientError::Transport(format!(
                "ledger GET {url} returned HTTP {status}: {body}"
            )));
        }

        let parsed: HttpWalletBalanceResponse = response
            .json()
            .await
            .map_err(|err| LedgerClientError::Transport(err.to_string()))?;
        Ok(WalletBalance {
            available_minor_units: parsed.available_minor_units,
            locked_minor_units: parsed.locked_minor_units,
            frozen_minor_units: parsed.frozen_minor_units,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::{
        HttpEntryDirection, HttpPostEntry, InMemoryLedgerClient, LedgerClient, LedgerClientError,
        PostTransactionRequest,
    };
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

    #[test]
    fn http_post_entry_maps_positive_amount_to_debit() {
        let draft =
            LedgerEntryDraft::new("Player.Wallet.user_42", LedgerAmount::from_minor_units(500));
        let entry = HttpPostEntry::from(&draft);
        assert_eq!(entry.account_code, "Player.Wallet.user_42");
        assert!(matches!(entry.direction, HttpEntryDirection::Debit));
        assert_eq!(entry.amount_minor_units, 500);
    }

    #[test]
    fn http_post_entry_maps_negative_amount_to_credit_with_positive_amount_field() {
        let draft = LedgerEntryDraft::new(
            "Liability.UnredeemedEntitlement.user_42",
            LedgerAmount::from_minor_units(-500),
        );
        let entry = HttpPostEntry::from(&draft);
        assert_eq!(
            entry.account_code,
            "Liability.UnredeemedEntitlement.user_42"
        );
        assert!(matches!(entry.direction, HttpEntryDirection::Credit));
        assert_eq!(
            entry.amount_minor_units, 500,
            "HTTP contract carries positive amount + separate direction field"
        );
    }

    #[test]
    fn http_post_entry_zero_maps_to_debit_with_zero_amount() {
        // 0 minor_units is filtered earlier by SDK INV-1 (zero amount entries
        // rejected at validate). The HTTP mapping treats 0 as debit so the
        // server-side validator returns a deterministic error, not a panic.
        let draft = LedgerEntryDraft::new("Revenue.TradingFee", LedgerAmount::from_minor_units(0));
        let entry = HttpPostEntry::from(&draft);
        assert!(matches!(entry.direction, HttpEntryDirection::Debit));
        assert_eq!(entry.amount_minor_units, 0);
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

        let Err(error) = client.post_transaction(request).await else {
            panic!("cross-user redemption must be rejected by SDK INV-7");
        };
        assert!(matches!(error, LedgerClientError::Validation(_)));
    }
}
