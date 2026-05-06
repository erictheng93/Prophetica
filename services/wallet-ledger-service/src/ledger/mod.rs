use async_trait::async_trait;
use sdk::ledger::{LedgerEntryDraft, LedgerTransactionDraft, LedgerValidationError};
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;

pub const LEDGER_SERVICE_BOOTSTRAP_ONLY: bool = true;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostTransactionDraft {
    pub txn_type: String,
    pub idempotency_key: String,
    pub correlation_id: Option<String>,
    pub description: Option<String>,
    pub entries: Vec<LedgerEntryDraft>,
}

impl PostTransactionDraft {
    #[must_use]
    pub fn freeze_player_funds(
        user_id: &str,
        amount_minor_units: i64,
        idempotency_key: impl Into<String>,
    ) -> Self {
        Self {
            txn_type: "freeze".to_owned(),
            idempotency_key: idempotency_key.into(),
            correlation_id: None,
            description: Some("freeze player funds".to_owned()),
            entries: vec![
                LedgerEntryDraft::new(
                    format!("Player.Wallet.{user_id}"),
                    sdk::ledger::LedgerAmount::from_minor_units(amount_minor_units),
                ),
                LedgerEntryDraft::new(
                    format!("Player.Frozen.{user_id}"),
                    sdk::ledger::LedgerAmount::from_minor_units(-amount_minor_units),
                ),
            ],
        }
    }

    #[must_use]
    pub fn unfreeze_player_funds(
        user_id: &str,
        amount_minor_units: i64,
        idempotency_key: impl Into<String>,
    ) -> Self {
        Self {
            txn_type: "unfreeze".to_owned(),
            idempotency_key: idempotency_key.into(),
            correlation_id: None,
            description: Some("unfreeze player funds".to_owned()),
            entries: vec![
                LedgerEntryDraft::new(
                    format!("Player.Frozen.{user_id}"),
                    sdk::ledger::LedgerAmount::from_minor_units(amount_minor_units),
                ),
                LedgerEntryDraft::new(
                    format!("Player.Wallet.{user_id}"),
                    sdk::ledger::LedgerAmount::from_minor_units(-amount_minor_units),
                ),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedLedgerTransaction {
    pub id: i64,
    pub idempotency_key: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountBalance {
    pub account_code: String,
    pub balance_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntryView {
    pub txn_id: i64,
    pub account_code: String,
    pub direction: i16,
    pub amount_minor_units: i64,
    pub balance_after_minor_units: i64,
}

#[derive(Debug, Error)]
pub enum PostTransactionError {
    #[error(transparent)]
    Validation(#[from] LedgerValidationError),
    #[error(
        "insufficient funds for {account_code}: projected balance {balance_after_minor_units} minor units"
    )]
    InsufficientFunds {
        account_code: String,
        balance_after_minor_units: i64,
    },
    #[error(transparent)]
    Store(#[from] LedgerStoreError),
}

#[derive(Debug, Error)]
#[error("ledger store error: {message}")]
pub struct LedgerStoreError {
    message: String,
}

impl LedgerStoreError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn into_message(self) -> String {
        self.message
    }
}

impl From<sqlx::Error> for LedgerStoreError {
    fn from(value: sqlx::Error) -> Self {
        Self::new(value.to_string())
    }
}

#[async_trait]
pub trait LedgerStore {
    async fn find_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<CommittedLedgerTransaction>, LedgerStoreError>;

    async fn commit_new_transaction(
        &self,
        draft: &PostTransactionDraft,
    ) -> Result<CommittedLedgerTransaction, LedgerStoreError>;

    async fn get_account_balance(
        &self,
        account_code: &str,
    ) -> Result<AccountBalance, LedgerStoreError>;

    async fn list_entries(
        &self,
        account_code: &str,
        limit: i64,
    ) -> Result<Vec<LedgerEntryView>, LedgerStoreError>;
}

#[derive(Debug, Clone)]
pub struct LedgerService<S> {
    store: S,
}

impl<S> LedgerService<S> {
    #[must_use]
    pub const fn new(store: S) -> Self {
        Self { store }
    }
}

impl<S> LedgerService<S>
where
    S: LedgerStore + Send + Sync,
{
    /// Posts a ledger transaction with idempotency replay semantics.
    ///
    /// # Errors
    ///
    /// Returns [`PostTransactionError`] when validation fails or the backing store rejects the
    /// commit.
    pub async fn post_transaction(
        &self,
        draft: PostTransactionDraft,
    ) -> Result<CommittedLedgerTransaction, PostTransactionError> {
        let transaction = LedgerTransactionDraft::new(draft.entries.clone());
        transaction.validate_balanced()?;
        self.validate_player_liability_bounds(&draft).await?;

        if let Some(existing) = self
            .store
            .find_by_idempotency_key(&draft.idempotency_key)
            .await?
        {
            return Ok(CommittedLedgerTransaction {
                replayed: true,
                ..existing
            });
        }

        self.store
            .commit_new_transaction(&draft)
            .await
            .map_err(PostTransactionError::from)
    }

    async fn validate_player_liability_bounds(
        &self,
        draft: &PostTransactionDraft,
    ) -> Result<(), PostTransactionError> {
        if draft.txn_type == "chargeback" {
            return Ok(());
        }

        for entry in &draft.entries {
            if !is_player_liability_account(entry.account_code()) {
                continue;
            }

            let current = self
                .store
                .get_account_balance(entry.account_code())
                .await?
                .balance_minor_units;
            let balance_after = current + entry.amount().minor_units();

            if balance_after > 0 {
                return Err(PostTransactionError::InsufficientFunds {
                    account_code: entry.account_code().to_owned(),
                    balance_after_minor_units: balance_after,
                });
            }
        }

        Ok(())
    }

    /// Returns the current balance for an account code.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerStoreError`] when the backing store cannot be queried.
    pub async fn get_account_balance(
        &self,
        account_code: &str,
    ) -> Result<AccountBalance, LedgerStoreError> {
        self.store.get_account_balance(account_code).await
    }

    /// Lists recent entries for an account code.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerStoreError`] when the backing store cannot be queried.
    pub async fn list_entries(
        &self,
        account_code: &str,
        limit: i64,
    ) -> Result<Vec<LedgerEntryView>, LedgerStoreError> {
        self.store.list_entries(account_code, limit).await
    }
}

#[derive(Debug, Clone)]
pub struct PostgresLedgerStore {
    pool: PgPool,
}

impl PostgresLedgerStore {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LedgerStore for PostgresLedgerStore {
    async fn find_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<CommittedLedgerTransaction>, LedgerStoreError> {
        let row = sqlx::query_as::<_, (i64, String)>(
            "SELECT txn.id, txn.idempotency_key \
             FROM ledger_transactions txn \
             LEFT JOIN idempotency_records idem ON idem.txn_id = txn.id \
             WHERE idem.key = $1 OR txn.idempotency_key = $1",
        )
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(id, idempotency_key)| CommittedLedgerTransaction {
            id,
            idempotency_key,
            replayed: false,
        }))
    }

    async fn commit_new_transaction(
        &self,
        draft: &PostTransactionDraft,
    ) -> Result<CommittedLedgerTransaction, LedgerStoreError> {
        let mut tx = self.pool.begin().await?;
        let inserted = insert_ledger_transaction(&mut tx, draft).await?;

        let Some((txn_id, idempotency_key)) = inserted else {
            tx.rollback().await?;
            return self
                .find_by_idempotency_key(&draft.idempotency_key)
                .await?
                .map(|existing| CommittedLedgerTransaction {
                    replayed: true,
                    ..existing
                })
                .ok_or_else(|| {
                    LedgerStoreError::new(
                        "idempotency conflict did not return existing transaction",
                    )
                });
        };

        for entry in &draft.entries {
            let account_id = ensure_account(&mut tx, entry.account_code()).await?;
            let current_balance = account_balance(&mut tx, account_id).await?;
            let balance_after = current_balance + entry.amount().minor_units();
            sqlx::query(
                "INSERT INTO ledger_entries \
                 (txn_id, account_id, direction, amount, currency, balance_after) \
                 VALUES ($1, $2, $3, $4, 'PTS', $5)",
            )
            .bind(txn_id)
            .bind(account_id)
            .bind(entry.storage_direction())
            .bind(entry.storage_amount_minor_units())
            .bind(balance_after)
            .execute(&mut *tx)
            .await?;
        }

        persist_idempotency_record(&mut tx, txn_id, &idempotency_key).await?;
        tx.commit().await?;

        Ok(CommittedLedgerTransaction {
            id: txn_id,
            idempotency_key,
            replayed: false,
        })
    }

    async fn get_account_balance(
        &self,
        account_code: &str,
    ) -> Result<AccountBalance, LedgerStoreError> {
        let (balance,) = sqlx::query_as::<_, (i64,)>(
            "SELECT COALESCE(( \
                 SELECT SUM(e.amount * e.direction) \
                 FROM ledger_entries e \
                 JOIN ledger_accounts a ON a.id = e.account_id \
                 WHERE a.code = $1 \
             ), 0)::BIGINT",
        )
        .bind(account_code)
        .fetch_one(&self.pool)
        .await?;

        Ok(AccountBalance {
            account_code: account_code.to_owned(),
            balance_minor_units: balance,
        })
    }

    async fn list_entries(
        &self,
        account_code: &str,
        limit: i64,
    ) -> Result<Vec<LedgerEntryView>, LedgerStoreError> {
        let rows = sqlx::query_as::<_, (i64, String, i16, i64, i64)>(
            "SELECT e.txn_id, a.code, e.direction, e.amount, e.balance_after \
             FROM ledger_entries e \
             JOIN ledger_accounts a ON a.id = e.account_id \
             WHERE a.code = $1 \
             ORDER BY e.created_at DESC, e.id DESC \
             LIMIT $2",
        )
        .bind(account_code)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    txn_id,
                    account_code,
                    direction,
                    amount_minor_units,
                    balance_after_minor_units,
                )| LedgerEntryView {
                    txn_id,
                    account_code,
                    direction,
                    amount_minor_units,
                    balance_after_minor_units,
                },
            )
            .collect())
    }
}

async fn insert_ledger_transaction(
    tx: &mut Transaction<'_, Postgres>,
    draft: &PostTransactionDraft,
) -> Result<Option<(i64, String)>, sqlx::Error> {
    sqlx::query_as::<_, (i64, String)>(
        "INSERT INTO ledger_transactions \
         (txn_type, idempotency_key, correlation_id, description) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (idempotency_key) DO NOTHING \
         RETURNING id, idempotency_key",
    )
    .bind(&draft.txn_type)
    .bind(&draft.idempotency_key)
    .bind(draft.correlation_id.as_deref())
    .bind(draft.description.as_deref())
    .fetch_optional(&mut **tx)
    .await
}

async fn persist_idempotency_record(
    tx: &mut Transaction<'_, Postgres>,
    txn_id: i64,
    idempotency_key: &str,
) -> Result<(), sqlx::Error> {
    let response = json!({
        "txn_id": txn_id,
        "idempotency_key": idempotency_key,
        "replayed": false,
    });

    sqlx::query(
        "INSERT INTO idempotency_records (key, txn_id, response, status_code, expires_at) \
         VALUES ($1, $2, $3, 200, NOW() + INTERVAL '7 days') \
         ON CONFLICT (key) DO NOTHING",
    )
    .bind(idempotency_key)
    .bind(txn_id)
    .bind(response)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn ensure_account(
    tx: &mut Transaction<'_, Postgres>,
    account_code: &str,
) -> Result<i64, sqlx::Error> {
    if let Some((id,)) =
        sqlx::query_as::<_, (i64,)>("SELECT id FROM ledger_accounts WHERE code = $1")
            .bind(account_code)
            .fetch_optional(&mut **tx)
            .await?
    {
        return Ok(id);
    }

    let account_type = infer_account_type(account_code);
    let (id,) = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO ledger_accounts (code, type, currency) \
         VALUES ($1, $2, 'PTS') \
         ON CONFLICT (code) DO UPDATE SET code = EXCLUDED.code \
         RETURNING id",
    )
    .bind(account_code)
    .bind(account_type)
    .fetch_one(&mut **tx)
    .await?;

    Ok(id)
}

async fn account_balance(
    tx: &mut Transaction<'_, Postgres>,
    account_id: i64,
) -> Result<i64, sqlx::Error> {
    let (balance,) = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(SUM(amount * direction), 0)::BIGINT \
         FROM ledger_entries \
         WHERE account_id = $1",
    )
    .bind(account_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(balance)
}

fn infer_account_type(account_code: &str) -> &'static str {
    if account_code.starts_with("Bank.") {
        "asset"
    } else if account_code.starts_with("Revenue.") {
        "revenue"
    } else if account_code.starts_with("Expense.") {
        "expense"
    } else {
        "liability"
    }
}

fn is_player_liability_account(account_code: &str) -> bool {
    account_code.starts_with("Player.Wallet.")
        || account_code.starts_with("Player.Locked.")
        || account_code.starts_with("Player.Frozen.")
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    };

    use sdk::ledger::{LedgerAmount, LedgerEntryDraft};

    use super::{
        AccountBalance, CommittedLedgerTransaction, LedgerEntryView, LedgerService, LedgerStore,
        LedgerStoreError, PostTransactionDraft,
    };

    #[tokio::test]
    async fn replays_existing_transaction_for_same_idempotency_key() {
        let store = InMemoryLedgerStore::default();
        let service = LedgerService::new(store.clone());
        let draft = balanced_draft("deposit:gateway-001");

        let first = match service.post_transaction(draft.clone()).await {
            Ok(transaction) => transaction,
            Err(error) => panic!("first post should commit: {error}"),
        };
        let second = match service.post_transaction(draft).await {
            Ok(transaction) => transaction,
            Err(error) => panic!("second post should replay: {error}"),
        };

        assert!(!first.replayed);
        assert!(second.replayed);
        assert_eq!(first.id, second.id);
        assert_eq!(store.commit_count(), 1);
    }

    #[tokio::test]
    async fn repeated_idempotency_key_commits_only_once() {
        let store = InMemoryLedgerStore::default();
        let service = LedgerService::new(store.clone());
        let draft = balanced_draft("deposit:gateway-100x");

        for _ in 0..100 {
            let transaction = match service.post_transaction(draft.clone()).await {
                Ok(transaction) => transaction,
                Err(error) => panic!("post should commit or replay: {error}"),
            };

            assert_eq!(transaction.id, 1);
        }

        assert_eq!(store.commit_count(), 1);
    }

    #[tokio::test]
    async fn returns_account_balance_from_store() {
        let store = InMemoryLedgerStore::default();
        let service = LedgerService::new(store.clone());
        let draft = balanced_draft("deposit:balance-001");

        match service.post_transaction(draft).await {
            Ok(_) => {}
            Err(error) => panic!("post should commit: {error}"),
        }

        let balance = match service
            .get_account_balance("Player.Wallet.user_01")
            .await
        {
            Ok(balance) => balance,
            Err(error) => panic!("balance should load: {error}"),
        };

        assert_eq!(balance.balance_minor_units, -10_000);
    }

    #[tokio::test]
    async fn lists_entries_from_store() {
        let store = InMemoryLedgerStore::default();
        let service = LedgerService::new(store.clone());
        let draft = balanced_draft("deposit:entries-001");

        match service.post_transaction(draft).await {
            Ok(_) => {}
            Err(error) => panic!("post should commit: {error}"),
        }

        let entries = match service.list_entries("Bank.TrustAccount.TWD", 25).await {
            Ok(entries) => entries,
            Err(error) => panic!("entries should load: {error}"),
        };

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].amount_minor_units, 10_000);
    }

    #[tokio::test]
    async fn rejects_wallet_debit_that_exceeds_available_balance() {
        let store = InMemoryLedgerStore::default();
        let service = LedgerService::new(store.clone());
        let draft = PostTransactionDraft {
            txn_type: "order_lock".to_owned(),
            idempotency_key: "order:insufficient".to_owned(),
            correlation_id: None,
            description: None,
            entries: vec![
                LedgerEntryDraft::new(
                    "Player.Wallet.user_01",
                    LedgerAmount::from_minor_units(1),
                ),
                LedgerEntryDraft::new(
                    "Player.Locked.user_01",
                    LedgerAmount::from_minor_units(-1),
                ),
            ],
        };

        let error = match service.post_transaction(draft).await {
            Ok(_) => panic!("wallet debit should be rejected"),
            Err(error) => error,
        };

        assert_eq!(
            error.to_string(),
            "insufficient funds for Player.Wallet.user_01: projected balance 1 minor units"
        );
        assert_eq!(store.commit_count(), 0);
    }

    #[tokio::test]
    async fn chargeback_can_project_wallet_above_zero() {
        let store = InMemoryLedgerStore::default();
        let service = LedgerService::new(store.clone());
        let draft = PostTransactionDraft {
            txn_type: "chargeback".to_owned(),
            idempotency_key: "chargeback:exception".to_owned(),
            correlation_id: None,
            description: None,
            entries: vec![
                LedgerEntryDraft::new(
                    "Player.Wallet.user_01",
                    LedgerAmount::from_minor_units(1),
                ),
                LedgerEntryDraft::new(
                    "Bank.TrustAccount.TWD",
                    LedgerAmount::from_minor_units(-1),
                ),
            ],
        };

        match service.post_transaction(draft).await {
            Ok(transaction) => assert_eq!(transaction.id, 1),
            Err(error) => panic!("chargeback exception should commit: {error}"),
        }
    }

    #[test]
    fn freeze_player_funds_builds_balanced_wallet_to_frozen_draft() {
        let draft =
            PostTransactionDraft::freeze_player_funds("user_01", 10_000, "freeze:user_01:001");

        assert_eq!(draft.txn_type, "freeze");
        assert_eq!(draft.entries[0].account_code(), "Player.Wallet.user_01");
        assert_eq!(draft.entries[0].amount().minor_units(), 10_000);
        assert_eq!(draft.entries[1].account_code(), "Player.Frozen.user_01");
        assert_eq!(draft.entries[1].amount().minor_units(), -10_000);
    }

    #[test]
    fn unfreeze_player_funds_builds_balanced_frozen_to_wallet_draft() {
        let draft =
            PostTransactionDraft::unfreeze_player_funds("user_01", 10_000, "unfreeze:user_01:001");

        assert_eq!(draft.txn_type, "unfreeze");
        assert_eq!(draft.entries[0].account_code(), "Player.Frozen.user_01");
        assert_eq!(draft.entries[0].amount().minor_units(), 10_000);
        assert_eq!(draft.entries[1].account_code(), "Player.Wallet.user_01");
        assert_eq!(draft.entries[1].amount().minor_units(), -10_000);
    }

    #[tokio::test]
    async fn rejects_invalid_transaction_before_store_commit() {
        let store = InMemoryLedgerStore::default();
        let service = LedgerService::new(store.clone());
        let draft = PostTransactionDraft {
            txn_type: "deposit".to_owned(),
            idempotency_key: "deposit:bad".to_owned(),
            correlation_id: None,
            description: None,
            entries: vec![
                LedgerEntryDraft::new(
                    "Bank.TrustAccount.TWD",
                    LedgerAmount::from_minor_units(10_000),
                ),
                LedgerEntryDraft::new(
                    "Player.Wallet.user_01",
                    LedgerAmount::from_minor_units(-9_999),
                ),
            ],
        };

        let error = match service.post_transaction(draft).await {
            Ok(_) => panic!("imbalanced transaction should be rejected"),
            Err(error) => error,
        };

        assert_eq!(
            error.to_string(),
            "ledger transaction is imbalanced by 1 minor units"
        );
        assert_eq!(store.commit_count(), 0);
    }

    #[tokio::test]
    async fn postgres_store_persists_and_replays_by_idempotency_key() {
        let Some(database_url) = std::env::var("WALLET_LEDGER_TEST_DATABASE_URL").ok() else {
            return;
        };
        let pool = match sqlx::PgPool::connect(&database_url).await {
            Ok(pool) => pool,
            Err(error) => panic!("test database should be reachable: {error}"),
        };
        let store = super::PostgresLedgerStore::new(pool);
        let service = LedgerService::new(store);
        let draft = balanced_draft("deposit:postgres-001");

        let first = match service.post_transaction(draft.clone()).await {
            Ok(transaction) => transaction,
            Err(error) => panic!("first post should commit to postgres: {error}"),
        };
        let second = match service.post_transaction(draft).await {
            Ok(transaction) => transaction,
            Err(error) => panic!("second post should replay from postgres: {error}"),
        };

        assert!(!first.replayed);
        assert!(second.replayed);
        assert_eq!(first.id, second.id);
    }

    fn balanced_draft(idempotency_key: &str) -> PostTransactionDraft {
        PostTransactionDraft {
            txn_type: "deposit".to_owned(),
            idempotency_key: idempotency_key.to_owned(),
            correlation_id: Some("trace-001".to_owned()),
            description: Some("test deposit".to_owned()),
            entries: vec![
                LedgerEntryDraft::new("Bank.TrustAccount.TWD", LedgerAmount::from_points(100)),
                LedgerEntryDraft::new("Player.Wallet.user_01", LedgerAmount::from_points(-100)),
            ],
        }
    }

    #[derive(Clone, Default)]
    struct InMemoryLedgerStore {
        transactions: Arc<Mutex<Vec<CommittedLedgerTransaction>>>,
        entries: Arc<Mutex<Vec<LedgerEntryView>>>,
        next_id: Arc<AtomicI64>,
    }

    impl InMemoryLedgerStore {
        fn commit_count(&self) -> usize {
            match self.transactions.lock() {
                Ok(transactions) => transactions.len(),
                Err(error) => panic!("transactions lock should not be poisoned: {error}"),
            }
        }
    }

    #[async_trait::async_trait]
    impl LedgerStore for InMemoryLedgerStore {
        async fn find_by_idempotency_key(
            &self,
            idempotency_key: &str,
        ) -> Result<Option<CommittedLedgerTransaction>, LedgerStoreError> {
            let transactions = match self.transactions.lock() {
                Ok(transactions) => transactions,
                Err(error) => panic!("transactions lock should not be poisoned: {error}"),
            };
            Ok(transactions
                .iter()
                .find(|transaction| transaction.idempotency_key == idempotency_key)
                .cloned())
        }

        async fn commit_new_transaction(
            &self,
            draft: &PostTransactionDraft,
        ) -> Result<CommittedLedgerTransaction, LedgerStoreError> {
            let transaction = CommittedLedgerTransaction {
                id: self.next_id.fetch_add(1, Ordering::SeqCst) + 1,
                idempotency_key: draft.idempotency_key.clone(),
                replayed: false,
            };
            match self.transactions.lock() {
                Ok(mut transactions) => transactions.push(transaction.clone()),
                Err(error) => panic!("transactions lock should not be poisoned: {error}"),
            }
            match self.entries.lock() {
                Ok(mut entries) => {
                    for entry in &draft.entries {
                        let current_balance = entries
                            .iter()
                            .filter(|stored| stored.account_code == entry.account_code())
                            .map(|stored| {
                                i64::from(stored.direction) * stored.amount_minor_units
                            })
                            .sum::<i64>();
                        entries.push(LedgerEntryView {
                            txn_id: transaction.id,
                            account_code: entry.account_code().to_owned(),
                            direction: entry.storage_direction(),
                            amount_minor_units: entry.storage_amount_minor_units(),
                            balance_after_minor_units: current_balance
                                + entry.amount().minor_units(),
                        });
                    }
                }
                Err(error) => panic!("entries lock should not be poisoned: {error}"),
            }
            Ok(transaction)
        }

        async fn get_account_balance(
            &self,
            account_code: &str,
        ) -> Result<AccountBalance, LedgerStoreError> {
            let entries = match self.entries.lock() {
                Ok(entries) => entries,
                Err(error) => panic!("entries lock should not be poisoned: {error}"),
            };
            let balance = entries
                .iter()
                .filter(|entry| entry.account_code == account_code)
                .map(|entry| i64::from(entry.direction) * entry.amount_minor_units)
                .sum();

            Ok(AccountBalance {
                account_code: account_code.to_owned(),
                balance_minor_units: balance,
            })
        }

        async fn list_entries(
            &self,
            account_code: &str,
            limit: i64,
        ) -> Result<Vec<LedgerEntryView>, LedgerStoreError> {
            let entries = match self.entries.lock() {
                Ok(entries) => entries,
                Err(error) => panic!("entries lock should not be poisoned: {error}"),
            };
            let limit = usize::try_from(limit).unwrap_or(0);

            Ok(entries
                .iter()
                .rev()
                .filter(|entry| entry.account_code == account_code)
                .take(limit)
                .cloned()
                .collect())
        }
    }
}
