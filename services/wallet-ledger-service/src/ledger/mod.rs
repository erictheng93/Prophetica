use async_trait::async_trait;
use sdk::ledger::{LedgerEntryDraft, LedgerTransactionDraft, LedgerValidationError};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedLedgerTransaction {
    pub id: i64,
    pub idempotency_key: String,
    pub replayed: bool,
}

#[derive(Debug, Error)]
pub enum PostTransactionError {
    #[error(transparent)]
    Validation(#[from] LedgerValidationError),
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
            "SELECT id, idempotency_key FROM ledger_transactions WHERE idempotency_key = $1",
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

        tx.commit().await?;

        Ok(CommittedLedgerTransaction {
            id: txn_id,
            idempotency_key,
            replayed: false,
        })
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

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    };

    use sdk::ledger::{LedgerAmount, LedgerEntryDraft};

    use super::{
        CommittedLedgerTransaction, LedgerService, LedgerStore, LedgerStoreError,
        PostTransactionDraft,
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
            Ok(transaction)
        }
    }
}
