use async_trait::async_trait;
use sdk::ledger::{LedgerEntryDraft, LedgerTransactionDraft, LedgerValidationError};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;

pub const LEDGER_SERVICE_BOOTSTRAP_ONLY: bool = true;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostTransactionDraft {
    pub txn_type: String,
    pub idempotency_key: String,
    pub correlation_id: Option<String>,
    pub description: Option<String>,
    pub metadata: Value,
    pub audit_log: Option<AuditLogDraft>,
    pub entries: Vec<LedgerEntryDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLogDraft {
    pub id: String,
    pub action: String,
    pub actor_user_id: String,
    pub actor_role: String,
    pub target_user_id: String,
    pub correlation_id: Option<String>,
    pub metadata: Value,
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
            metadata: json!({}),
            audit_log: None,
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
            metadata: json!({}),
            audit_log: None,
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
    pub request_hash: String,
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

#[must_use]
pub fn webhook_idempotency_key(gateway_transaction_id: &str) -> String {
    format!("gateway:{gateway_transaction_id}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpiredIdempotencyRecord {
    pub key: String,
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
    #[error("idempotency key {idempotency_key} was already used with a different request")]
    IdempotencyConflict { idempotency_key: String },
    #[error("manual points adjustment invalid: {message}")]
    ManualAdjustmentInvalid { message: String },
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

    async fn record_audit_log(&self, audit_log: &AuditLogDraft) -> Result<(), LedgerStoreError>;

    async fn delete_expired_idempotency_records(
        &self,
    ) -> Result<Vec<ExpiredIdempotencyRecord>, LedgerStoreError>;
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
        validate_manual_points_adjustment(&draft)?;
        // Map txn_type 字串 → 典型 LedgerTxnKind enum，讓 INV-6 知道是否
        // 允許 Player.Wallet → Player.Wallet（限 trade / settle）。
        // 未知 txn_type 落入 LedgerTxnKind::Other（最嚴格 INV-6 模式）。
        let kind = crate::map_txn_type_to_kind(&draft.txn_type);
        let transaction = LedgerTransactionDraft::new(draft.entries.clone()).with_kind(kind);
        // validate() 包含 INV-1（借貸代數和）+ INV-6（Player.Wallet outflow whitelist）
        transaction.validate()?;
        self.validate_player_liability_bounds(&draft).await?;
        let request_hash = request_hash_for_draft(&draft);

        if let Some(existing) = self
            .store
            .find_by_idempotency_key(&draft.idempotency_key)
            .await?
        {
            if existing.request_hash != request_hash {
                return Err(PostTransactionError::IdempotencyConflict {
                    idempotency_key: draft.idempotency_key,
                });
            }
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

    /// Finds a previously committed ledger transaction by idempotency key.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerStoreError`] when the backing store lookup fails.
    pub async fn find_transaction_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<CommittedLedgerTransaction>, LedgerStoreError> {
        self.store.find_by_idempotency_key(idempotency_key).await
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

    /// Records an operation audit log that is not tied to a new ledger transaction.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerStoreError`] when the backing store cannot persist the audit log.
    pub async fn record_audit_log(&self, audit_log: AuditLogDraft) -> Result<(), LedgerStoreError> {
        self.store.record_audit_log(&audit_log).await
    }

    /// Deletes expired idempotency records for the daily retention cleanup.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerStoreError`] when the backing store cannot run the cleanup query.
    pub async fn cleanup_expired_idempotency_records(
        &self,
    ) -> Result<Vec<ExpiredIdempotencyRecord>, LedgerStoreError> {
        self.store.delete_expired_idempotency_records().await
    }
}

pub(crate) fn request_hash_for_draft(draft: &PostTransactionDraft) -> String {
    let entries = draft
        .entries
        .iter()
        .map(|entry| {
            json!({
                "account_code": entry.account_code(),
                "minor_units": entry.amount().minor_units(),
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "txn_type": &draft.txn_type,
        "correlation_id": &draft.correlation_id,
        "description": &draft.description,
        "metadata": &draft.metadata,
        "entries": entries,
    });
    let encoded = serde_json::to_vec(&payload).unwrap_or_default();
    hex::encode(Sha256::digest(encoded))
}

fn validate_manual_points_adjustment(
    draft: &PostTransactionDraft,
) -> Result<(), PostTransactionError> {
    if draft.txn_type != "manual_points_adjustment" {
        return Ok(());
    }

    let role = metadata_str(&draft.metadata, "actor_role")?;
    let direction = metadata_str(&draft.metadata, "adjustment_direction")?;
    let target_user_id = metadata_str(&draft.metadata, "target_user_id")?;
    require_metadata(&draft.metadata, "reason_code")?;
    require_metadata(&draft.metadata, "reason_text")?;
    require_metadata(&draft.metadata, "audit_log_id")?;

    if role == "affiliate"
        && draft
            .metadata
            .get("affiliate_id")
            .and_then(Value::as_str)
            .is_none()
    {
        return manual_adjustment_error("affiliate_id is required for affiliate grants");
    }

    match direction {
        "grant" => {
            if !matches!(role, "admin" | "manager" | "affiliate") {
                return manual_adjustment_error("actor_role is not allowed to grant points");
            }
            require_manual_entries(draft, target_user_id, false)?;
        }
        "deduct" => {
            if !matches!(role, "admin" | "manager") {
                return manual_adjustment_error("only admin or manager can deduct points");
            }
            require_manual_entries(draft, target_user_id, true)?;
        }
        _ => return manual_adjustment_error("adjustment_direction must be grant or deduct"),
    }

    if draft.audit_log.is_none() {
        return manual_adjustment_error("audit_log is required");
    }

    Ok(())
}

fn metadata_str<'a>(metadata: &'a Value, key: &str) -> Result<&'a str, PostTransactionError> {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| PostTransactionError::ManualAdjustmentInvalid {
            message: format!("{key} is required"),
        })
}

fn require_metadata(metadata: &Value, key: &str) -> Result<(), PostTransactionError> {
    metadata_str(metadata, key).map(|_| ())
}

fn manual_adjustment_error<T>(message: &str) -> Result<T, PostTransactionError> {
    Err(PostTransactionError::ManualAdjustmentInvalid {
        message: message.to_owned(),
    })
}

fn require_manual_entries(
    draft: &PostTransactionDraft,
    target_user_id: &str,
    deduct: bool,
) -> Result<(), PostTransactionError> {
    if draft.entries.len() != 2 {
        return manual_adjustment_error("manual adjustment must contain exactly two entries");
    }
    let wallet_code = format!("Player.Wallet.{target_user_id}");
    let wallet_amount =
        if deduct {
            draft.entries.iter().find(|entry| {
                entry.account_code() == wallet_code && entry.amount().minor_units() > 0
            })
        } else {
            draft.entries.iter().find(|entry| {
                entry.account_code() == wallet_code && entry.amount().minor_units() < 0
            })
        };
    let expense_amount = if deduct {
        draft.entries.iter().find(|entry| {
            entry.account_code() == "Expense.ManualPointsGrant" && entry.amount().minor_units() < 0
        })
    } else {
        draft.entries.iter().find(|entry| {
            entry.account_code() == "Expense.ManualPointsGrant" && entry.amount().minor_units() > 0
        })
    };

    let Some(wallet_entry) = wallet_amount else {
        return manual_adjustment_error("manual adjustment wallet entry has invalid direction");
    };
    let Some(expense_entry) = expense_amount else {
        return manual_adjustment_error("manual adjustment expense entry has invalid direction");
    };
    if wallet_entry.amount().minor_units().abs() != expense_entry.amount().minor_units().abs() {
        return manual_adjustment_error("manual adjustment entries must use the same amount");
    }
    Ok(())
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
        let row = sqlx::query_as::<_, (i64, String, String)>(
            "SELECT txn.id, txn.idempotency_key, COALESCE(idem.request_hash, '') \
             FROM ledger_transactions txn \
             LEFT JOIN idempotency_records idem ON idem.txn_id = txn.id \
             WHERE idem.key = $1 OR txn.idempotency_key = $1",
        )
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(
            |(id, idempotency_key, request_hash)| CommittedLedgerTransaction {
                id,
                idempotency_key,
                request_hash,
                replayed: false,
            },
        ))
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

        let account_codes = sorted_unique_account_codes(draft);
        lock_account_codes(&mut tx, &account_codes).await?;

        let mut account_ids = Vec::with_capacity(draft.entries.len());
        for entry in &draft.entries {
            account_ids.push(ensure_account(&mut tx, entry.account_code()).await?);
        }
        lock_accounts(&mut tx, &account_ids).await?;
        validate_locked_player_liability_bounds(&mut tx, draft, &account_ids).await?;

        for (entry, account_id) in draft.entries.iter().zip(account_ids) {
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

        if let Some(audit_log) = &draft.audit_log {
            persist_audit_log(&mut tx, audit_log).await?;
        }
        persist_idempotency_record(
            &mut tx,
            txn_id,
            &idempotency_key,
            &request_hash_for_draft(draft),
        )
        .await?;
        tx.commit().await?;

        Ok(CommittedLedgerTransaction {
            id: txn_id,
            idempotency_key,
            request_hash: request_hash_for_draft(draft),
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

    async fn record_audit_log(&self, audit_log: &AuditLogDraft) -> Result<(), LedgerStoreError> {
        sqlx::query(
            "INSERT INTO audit_logs \
             (id, action, actor_user_id, actor_role, target_user_id, correlation_id, metadata) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(&audit_log.id)
        .bind(&audit_log.action)
        .bind(&audit_log.actor_user_id)
        .bind(&audit_log.actor_role)
        .bind(&audit_log.target_user_id)
        .bind(audit_log.correlation_id.as_deref())
        .bind(&audit_log.metadata)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete_expired_idempotency_records(
        &self,
    ) -> Result<Vec<ExpiredIdempotencyRecord>, LedgerStoreError> {
        let rows = sqlx::query_as::<_, (String,)>(
            "DELETE FROM idempotency_records \
             WHERE expires_at <= NOW() \
             RETURNING key",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(key,)| ExpiredIdempotencyRecord { key })
            .collect())
    }
}

async fn insert_ledger_transaction(
    tx: &mut Transaction<'_, Postgres>,
    draft: &PostTransactionDraft,
) -> Result<Option<(i64, String)>, sqlx::Error> {
    sqlx::query_as::<_, (i64, String)>(
        "INSERT INTO ledger_transactions \
         (txn_type, idempotency_key, correlation_id, description, metadata) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (idempotency_key) DO NOTHING \
         RETURNING id, idempotency_key",
    )
    .bind(&draft.txn_type)
    .bind(&draft.idempotency_key)
    .bind(draft.correlation_id.as_deref())
    .bind(draft.description.as_deref())
    .bind(&draft.metadata)
    .fetch_optional(&mut **tx)
    .await
}

async fn persist_audit_log(
    tx: &mut Transaction<'_, Postgres>,
    audit_log: &AuditLogDraft,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_logs \
         (id, action, actor_user_id, actor_role, target_user_id, correlation_id, metadata) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(&audit_log.id)
    .bind(&audit_log.action)
    .bind(&audit_log.actor_user_id)
    .bind(&audit_log.actor_role)
    .bind(&audit_log.target_user_id)
    .bind(audit_log.correlation_id.as_deref())
    .bind(&audit_log.metadata)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn persist_idempotency_record(
    tx: &mut Transaction<'_, Postgres>,
    txn_id: i64,
    idempotency_key: &str,
    request_hash: &str,
) -> Result<(), sqlx::Error> {
    let response = json!({
        "txn_id": txn_id,
        "idempotency_key": idempotency_key,
        "replayed": false,
    });

    sqlx::query(
        "INSERT INTO idempotency_records (key, txn_id, request_hash, response, status_code, expires_at) \
         VALUES ($1, $2, $3, $4, 200, NOW() + INTERVAL '7 days') \
         ON CONFLICT (key) DO NOTHING",
    )
    .bind(idempotency_key)
    .bind(txn_id)
    .bind(request_hash)
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

fn sorted_unique_account_codes(draft: &PostTransactionDraft) -> Vec<&str> {
    let mut account_codes = draft
        .entries
        .iter()
        .map(sdk::ledger::LedgerEntryDraft::account_code)
        .collect::<Vec<_>>();
    account_codes.sort_unstable();
    account_codes.dedup();
    account_codes
}

async fn lock_account_codes(
    tx: &mut Transaction<'_, Postgres>,
    account_codes: &[&str],
) -> Result<(), sqlx::Error> {
    for account_code in account_codes {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(account_code)
            .execute(&mut **tx)
            .await?;
    }

    Ok(())
}

async fn lock_accounts(
    tx: &mut Transaction<'_, Postgres>,
    account_ids: &[i64],
) -> Result<(), sqlx::Error> {
    let mut sorted_ids = account_ids.to_vec();
    sorted_ids.sort_unstable();
    sorted_ids.dedup();

    for account_id in sorted_ids {
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(account_id)
            .execute(&mut **tx)
            .await?;
    }

    Ok(())
}

async fn validate_locked_player_liability_bounds(
    tx: &mut Transaction<'_, Postgres>,
    draft: &PostTransactionDraft,
    account_ids: &[i64],
) -> Result<(), LedgerStoreError> {
    if draft.txn_type == "chargeback" {
        return Ok(());
    }

    for (entry, account_id) in draft.entries.iter().zip(account_ids) {
        if !is_player_liability_account(entry.account_code()) {
            continue;
        }

        let current_balance = account_balance(tx, *account_id).await?;
        let balance_after = current_balance + entry.amount().minor_units();
        if balance_after > 0 {
            return Err(LedgerStoreError::new(format!(
                "insufficient funds for {}: projected balance {} minor units",
                entry.account_code(),
                balance_after
            )));
        }
    }

    Ok(())
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
    use std::time::{SystemTime, UNIX_EPOCH};

    use proptest::prelude::*;
    use sdk::ledger::{LedgerAmount, LedgerEntryDraft};
    use serde_json::json;
    use tokio::task::JoinSet;

    use super::{
        AccountBalance, CommittedLedgerTransaction, ExpiredIdempotencyRecord, LedgerEntryView,
        LedgerService, LedgerStore, LedgerStoreError, PostTransactionDraft, PostTransactionError,
        webhook_idempotency_key,
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
    async fn rejects_idempotency_key_reuse_with_different_payload() {
        let store = InMemoryLedgerStore::default();
        let service = LedgerService::new(store.clone());
        let first = balanced_draft_for_user("user_01", "deposit:conflict-001", 10_000);
        let second = balanced_draft_for_user("user_01", "deposit:conflict-001", 20_000);

        match service.post_transaction(first).await {
            Ok(_) => {}
            Err(error) => panic!("first post should commit: {error}"),
        }

        let error = match service.post_transaction(second).await {
            Ok(_) => panic!("different payload with same idempotency key must fail"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            PostTransactionError::IdempotencyConflict { .. }
        ));
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

    proptest! {
        #[test]
        fn same_idempotency_key_replayed_up_to_one_hundred_times_commits_once(
            attempts in 1_usize..=100
        ) {
            let runtime = tokio::runtime::Runtime::new()
                .unwrap_or_else(|error| panic!("runtime should start: {error}"));
            runtime.block_on(async move {
                let store = InMemoryLedgerStore::default();
                let service = LedgerService::new(store.clone());
                let draft = balanced_draft("deposit:property-100x");

                for _ in 0..attempts {
                    if let Err(error) = service.post_transaction(draft.clone()).await {
                        panic!("post should commit or replay: {error}");
                    }
                }

                prop_assert_eq!(store.commit_count(), 1);
                Ok(())
            })?;
        }
    }

    #[tokio::test]
    async fn cleanup_expired_idempotency_records_deletes_expired_keys() {
        let store = InMemoryLedgerStore::default();
        store.seed_expired_idempotency_key("expired-key");
        let service = LedgerService::new(store.clone());

        let deleted = match service.cleanup_expired_idempotency_records().await {
            Ok(deleted) => deleted,
            Err(error) => panic!("cleanup should succeed: {error}"),
        };

        assert_eq!(
            deleted,
            vec![ExpiredIdempotencyRecord {
                key: "expired-key".to_owned(),
            }]
        );
        assert_eq!(store.expired_idempotency_key_count(), 0);
    }

    #[test]
    fn webhook_idempotency_uses_gateway_transaction_id() {
        assert_eq!(
            webhook_idempotency_key("txn_01HX"),
            "gateway:txn_01HX".to_owned()
        );
    }

    #[test]
    fn ledger_entries_schema_is_monthly_partition_ready() {
        let migration = include_str!("../../migrations/202605050001_create_ledger.sql");

        assert!(
            migration.contains("PARTITION BY RANGE (created_at)"),
            "SPEC-002 requires ledger_entries to be partitioned by created_at from day 1"
        );
    }

    #[test]
    fn inv6_migration_allows_manual_adjustment_contra_expense_only() {
        let migration = include_str!("../../migrations/202605070004_inv6_outflow_whitelist.sql");

        assert!(migration.contains("Expense.ManualPointsGrant"));
        assert!(migration.contains("manual_points_adjustment"));
        assert!(migration.contains("adjustment_direction"));
        assert!(migration.contains("actor_role"));
    }

    #[test]
    fn redemption_chart_of_accounts_seed_includes_required_kinds() {
        let migration =
            include_str!("../../migrations/202605270001_redemption_chart_of_accounts_seed.sql");

        assert!(migration.contains("Revenue.VirtualGoodsSales.digital"));
        assert!(migration.contains("Revenue.VirtualGoodsSales.service"));
        assert!(migration.contains("Revenue.EntitlementBreakage"));
        assert!(
            migration.contains("ON CONFLICT (code) DO NOTHING"),
            "seed must be idempotent so reruns do not duplicate accounts"
        );
    }

    #[test]
    fn inv6_redemption_carveout_requires_purchase_metadata() {
        let migration = include_str!("../../migrations/202605270002_inv6_redemption_carveout.sql");

        assert!(migration.contains("Liability.UnredeemedEntitlement.%"));
        assert!(migration.contains("redemption_purchase"));
        assert!(
            migration.contains("order_id") && migration.contains("sku_id"),
            "carve-out must require order_id and sku_id metadata to avoid empty-context outflows"
        );
        assert!(
            migration.contains("audit_log_id"),
            "redemption outflows must always carry an audit_log_id"
        );
    }

    #[test]
    fn inv7_trigger_covers_all_redemption_family_txn_types() {
        let migration = include_str!("../../migrations/202605270003_inv7_redemption_same_user.sql");

        assert!(migration.contains("redemption_purchase"));
        assert!(migration.contains("entitlement_fulfillment"));
        assert!(migration.contains("redemption_refund"));
        assert!(migration.contains("entitlement_expiry"));
        assert!(
            migration.contains("array_length(v_user_ids, 1) > 1"),
            "INV-7 must reject when more than one distinct user_id appears"
        );
        assert!(
            migration.contains("DEFERRABLE INITIALLY DEFERRED"),
            "INV-7 trigger must align with INV-1 / INV-6 deferred semantics"
        );
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

        let balance = match service.get_account_balance("Player.Wallet.user_01").await {
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
            metadata: json!({}),
            audit_log: None,
            entries: vec![
                LedgerEntryDraft::new("Player.Wallet.user_01", LedgerAmount::from_minor_units(1)),
                LedgerEntryDraft::new("Player.Locked.user_01", LedgerAmount::from_minor_units(-1)),
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
            metadata: json!({}),
            audit_log: None,
            entries: vec![
                LedgerEntryDraft::new("Player.Wallet.user_01", LedgerAmount::from_minor_units(1)),
                LedgerEntryDraft::new("Bank.TrustAccount.TWD", LedgerAmount::from_minor_units(-1)),
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
            metadata: json!({}),
            audit_log: None,
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
        let Some(pool) = postgres_test_pool().await else {
            return;
        };
        let store = super::PostgresLedgerStore::new(pool);
        let service = LedgerService::new(store);
        let test_id = unique_test_id("postgres-replay");
        let draft = balanced_draft_for_user(
            &format!("user_{test_id}"),
            &format!("deposit:{test_id}:postgres-001"),
            10_000,
        );

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

    #[tokio::test]
    async fn postgres_migration_trigger_rejects_imbalanced_commit() {
        let Some(pool) = postgres_test_pool().await else {
            return;
        };
        let test_id = unique_test_id("postgres-trigger");
        let mut tx = match pool.begin().await {
            Ok(tx) => tx,
            Err(error) => panic!("test transaction should start: {error}"),
        };

        let (txn_id,) = match sqlx::query_as::<_, (i64,)>(
            "INSERT INTO ledger_transactions (txn_type, idempotency_key) \
             VALUES ('deposit', $1) \
             RETURNING id",
        )
        .bind(format!("deposit:{test_id}:imbalanced"))
        .fetch_one(&mut *tx)
        .await
        {
            Ok(row) => row,
            Err(error) => panic!("test transaction row should insert: {error}"),
        };
        let bank_id = ensure_test_account(&mut tx, &format!("Bank.Test.{test_id}"), "asset").await;
        let wallet_id =
            ensure_test_account(&mut tx, &format!("Player.Wallet.{test_id}"), "liability").await;

        for (account_id, direction, amount) in
            [(bank_id, 1_i16, 10_000_i64), (wallet_id, -1, 9_999)]
        {
            match sqlx::query(
                "INSERT INTO ledger_entries \
                 (txn_id, account_id, direction, amount, currency, balance_after) \
                 VALUES ($1, $2, $3, $4, 'PTS', $5)",
            )
            .bind(txn_id)
            .bind(account_id)
            .bind(direction)
            .bind(amount)
            .bind(i64::from(direction) * amount)
            .execute(&mut *tx)
            .await
            {
                Ok(_) => {}
                Err(error) => {
                    panic!("imbalanced entries should insert before deferred commit: {error}");
                }
            }
        }

        let error = match tx.commit().await {
            Ok(()) => panic!("deferred trigger should reject imbalanced commit"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("Ledger imbalance for txn"),
            "unexpected trigger error: {error}"
        );
    }

    #[tokio::test]
    async fn postgres_same_idempotency_key_creates_one_txn_and_record() {
        let Some(pool) = postgres_test_pool().await else {
            return;
        };
        let store = super::PostgresLedgerStore::new(pool.clone());
        let service = LedgerService::new(store);
        let test_id = unique_test_id("postgres-idempo-count");
        let idempotency_key = format!("deposit:{test_id}:one-row");
        let draft = balanced_draft_for_user(&format!("user_{test_id}"), &idempotency_key, 10_000);

        for _ in 0..10 {
            match service.post_transaction(draft.clone()).await {
                Ok(_) => {}
                Err(error) => panic!("post should commit or replay: {error}"),
            }
        }

        let (txn_count,) = match sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM ledger_transactions WHERE idempotency_key = $1",
        )
        .bind(&idempotency_key)
        .fetch_one(&pool)
        .await
        {
            Ok(row) => row,
            Err(error) => panic!("transaction count should query: {error}"),
        };
        let (record_count,) = match sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM idempotency_records WHERE key = $1",
        )
        .bind(&idempotency_key)
        .fetch_one(&pool)
        .await
        {
            Ok(row) => row,
            Err(error) => panic!("idempotency count should query: {error}"),
        };

        assert_eq!(txn_count, 1);
        assert_eq!(record_count, 1);
    }

    #[tokio::test]
    async fn postgres_ledger_entries_are_immutable() {
        let Some(pool) = postgres_test_pool().await else {
            return;
        };
        let store = super::PostgresLedgerStore::new(pool.clone());
        let service = LedgerService::new(store);
        let test_id = unique_test_id("postgres-immutable");
        let draft = balanced_draft_for_user(
            &format!("user_{test_id}"),
            &format!("deposit:{test_id}:immutable"),
            10_000,
        );

        let transaction = match service.post_transaction(draft).await {
            Ok(transaction) => transaction,
            Err(error) => panic!("post should commit: {error}"),
        };

        let update_error = match sqlx::query(
            "UPDATE ledger_entries SET balance_after = balance_after WHERE txn_id = $1",
        )
        .bind(transaction.id)
        .execute(&pool)
        .await
        {
            Ok(_) => panic!("ledger entry update should be rejected"),
            Err(error) => error,
        };
        let delete_error = match sqlx::query("DELETE FROM ledger_entries WHERE txn_id = $1")
            .bind(transaction.id)
            .execute(&pool)
            .await
        {
            Ok(_) => panic!("ledger entry delete should be rejected"),
            Err(error) => error,
        };

        assert!(
            update_error
                .to_string()
                .contains("ledger_entries are immutable"),
            "unexpected update error: {update_error}"
        );
        assert!(
            delete_error
                .to_string()
                .contains("ledger_entries are immutable"),
            "unexpected delete error: {delete_error}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn postgres_concurrent_deposits_keep_balances_and_balance_after_consistent() {
        let Some(pool) = postgres_test_pool().await else {
            return;
        };
        let test_id = unique_test_id("postgres-concurrency");
        let user_id = format!("concurrent_{test_id}");
        let wallet_code = format!("Player.Wallet.{user_id}");
        let bank_code = format!("Bank.TrustAccount.{test_id}");
        let deposit_count = 1_000_i64;
        let deposit_amount = 10_000_i64;

        let mut tasks = JoinSet::new();
        for index in 0..deposit_count {
            let service = LedgerService::new(super::PostgresLedgerStore::new(pool.clone()));
            let user_id = user_id.clone();
            let bank_code = bank_code.clone();
            let idempotency_key = format!("deposit:{test_id}:{index}");
            tasks.spawn(async move {
                let draft = PostTransactionDraft {
                    txn_type: "deposit".to_owned(),
                    idempotency_key,
                    correlation_id: None,
                    description: Some("concurrent deposit test".to_owned()),
                    metadata: json!({}),
                    audit_log: None,
                    entries: vec![
                        LedgerEntryDraft::new(
                            bank_code,
                            LedgerAmount::from_minor_units(deposit_amount),
                        ),
                        LedgerEntryDraft::new(
                            format!("Player.Wallet.{user_id}"),
                            LedgerAmount::from_minor_units(-deposit_amount),
                        ),
                    ],
                };
                service.post_transaction(draft).await
            });
        }

        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => panic!("deposit should commit: {error}"),
                Err(error) => panic!("deposit task should join: {error}"),
            }
        }

        let wallet_balance = current_balance(&pool, &wallet_code).await;
        let bank_balance = current_balance(&pool, &bank_code).await;
        let transaction_count = transaction_count(&pool, &format!("deposit:{test_id}:%")).await;

        assert_eq!(transaction_count, deposit_count);
        assert_eq!(wallet_balance, -(deposit_count * deposit_amount));
        assert_eq!(bank_balance, deposit_count * deposit_amount);

        let wallet_balance_after_values =
            balance_after_values(&pool, &wallet_code, deposit_count).await;
        let bank_balance_after_values =
            balance_after_values(&pool, &bank_code, deposit_count).await;
        let expected_wallet = (1..=deposit_count)
            .map(|index| -(index * deposit_amount))
            .collect::<Vec<_>>();
        let expected_bank = (1..=deposit_count)
            .map(|index| index * deposit_amount)
            .collect::<Vec<_>>();

        assert_eq!(wallet_balance_after_values, expected_wallet);
        assert_eq!(bank_balance_after_values, expected_bank);
    }

    fn balanced_draft(idempotency_key: &str) -> PostTransactionDraft {
        balanced_draft_for_user("user_01", idempotency_key, 10_000)
    }

    fn balanced_draft_for_user(
        user_id: &str,
        idempotency_key: &str,
        amount_minor_units: i64,
    ) -> PostTransactionDraft {
        PostTransactionDraft {
            txn_type: "deposit".to_owned(),
            idempotency_key: idempotency_key.to_owned(),
            correlation_id: Some("trace-001".to_owned()),
            description: Some("test deposit".to_owned()),
            metadata: json!({}),
            audit_log: None,
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
        }
    }

    async fn postgres_test_pool() -> Option<sqlx::PgPool> {
        let database_url = std::env::var("WALLET_LEDGER_TEST_DATABASE_URL").ok()?;
        let pool = match sqlx::PgPool::connect(&database_url).await {
            Ok(pool) => pool,
            Err(error) => panic!("test database should be reachable: {error}"),
        };
        match sqlx::migrate!("./migrations").run(&pool).await {
            Ok(()) => {}
            Err(error) => panic!("migrations should run against test database: {error}"),
        }

        Some(pool)
    }

    fn unique_test_id(prefix: &str) -> String {
        let nanos = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(error) => panic!("system time should be after unix epoch: {error}"),
        };
        format!("{prefix}-{nanos}")
    }

    async fn ensure_test_account(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        account_code: &str,
        account_type: &str,
    ) -> i64 {
        let (id,) = match sqlx::query_as::<_, (i64,)>(
            "INSERT INTO ledger_accounts (code, type, currency) \
             VALUES ($1, $2, 'PTS') \
             ON CONFLICT (code) DO UPDATE SET code = EXCLUDED.code \
             RETURNING id",
        )
        .bind(account_code)
        .bind(account_type)
        .fetch_one(&mut **tx)
        .await
        {
            Ok(row) => row,
            Err(error) => panic!("test account should upsert: {error}"),
        };
        id
    }

    async fn current_balance(pool: &sqlx::PgPool, account_code: &str) -> i64 {
        let (balance,) = match sqlx::query_as::<_, (i64,)>(
            "SELECT COALESCE(SUM(e.amount * e.direction), 0)::BIGINT \
             FROM ledger_entries e \
             JOIN ledger_accounts a ON a.id = e.account_id \
             WHERE a.code = $1",
        )
        .bind(account_code)
        .fetch_one(pool)
        .await
        {
            Ok(row) => row,
            Err(error) => panic!("balance should query: {error}"),
        };
        balance
    }

    async fn transaction_count(pool: &sqlx::PgPool, pattern: &str) -> i64 {
        let (count,) = match sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM ledger_transactions WHERE idempotency_key LIKE $1",
        )
        .bind(pattern)
        .fetch_one(pool)
        .await
        {
            Ok(row) => row,
            Err(error) => panic!("transaction count should query: {error}"),
        };
        count
    }

    async fn balance_after_values(
        pool: &sqlx::PgPool,
        account_code: &str,
        expected_count: i64,
    ) -> Vec<i64> {
        let rows = match sqlx::query_as::<_, (i64,)>(
            "SELECT e.balance_after \
             FROM ledger_entries e \
             JOIN ledger_accounts a ON a.id = e.account_id \
             WHERE a.code = $1 \
             ORDER BY ABS(e.balance_after)",
        )
        .bind(account_code)
        .fetch_all(pool)
        .await
        {
            Ok(rows) => rows,
            Err(error) => panic!("balance_after values should query: {error}"),
        };
        let expected_len = match usize::try_from(expected_count) {
            Ok(value) => value,
            Err(error) => panic!("expected count should fit usize: {error}"),
        };
        assert_eq!(rows.len(), expected_len);
        rows.into_iter().map(|(value,)| value).collect()
    }

    #[derive(Clone, Default)]
    struct InMemoryLedgerStore {
        transactions: Arc<Mutex<Vec<CommittedLedgerTransaction>>>,
        entries: Arc<Mutex<Vec<LedgerEntryView>>>,
        expired_idempotency_keys: Arc<Mutex<Vec<String>>>,
        next_id: Arc<AtomicI64>,
    }

    impl InMemoryLedgerStore {
        fn commit_count(&self) -> usize {
            match self.transactions.lock() {
                Ok(transactions) => transactions.len(),
                Err(error) => panic!("transactions lock should not be poisoned: {error}"),
            }
        }

        fn seed_expired_idempotency_key(&self, key: &str) {
            match self.expired_idempotency_keys.lock() {
                Ok(mut keys) => keys.push(key.to_owned()),
                Err(error) => panic!("expired keys lock should not be poisoned: {error}"),
            }
        }

        fn expired_idempotency_key_count(&self) -> usize {
            match self.expired_idempotency_keys.lock() {
                Ok(keys) => keys.len(),
                Err(error) => panic!("expired keys lock should not be poisoned: {error}"),
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
                request_hash: super::request_hash_for_draft(draft),
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
                            .map(|stored| i64::from(stored.direction) * stored.amount_minor_units)
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

        async fn record_audit_log(
            &self,
            _audit_log: &super::AuditLogDraft,
        ) -> Result<(), LedgerStoreError> {
            Ok(())
        }

        async fn delete_expired_idempotency_records(
            &self,
        ) -> Result<Vec<ExpiredIdempotencyRecord>, LedgerStoreError> {
            let keys = match self.expired_idempotency_keys.lock() {
                Ok(mut keys) => std::mem::take(&mut *keys),
                Err(error) => panic!("expired keys lock should not be poisoned: {error}"),
            };

            Ok(keys
                .into_iter()
                .map(|key| ExpiredIdempotencyRecord { key })
                .collect())
        }
    }
}
