use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use sdk::ledger::{LedgerAmount, LedgerEntryDraft};
use serde::{Deserialize, Serialize};

use crate::ledger::{
    LedgerService, LedgerStore, LedgerStoreError, PostTransactionDraft, PostTransactionError,
};

#[derive(Debug, Clone, Deserialize)]
pub struct PostTransactionRequest {
    pub txn_type: String,
    pub idempotency_key: String,
    pub correlation_id: Option<String>,
    pub description: Option<String>,
    pub entries: Vec<PostEntryRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostEntryRequest {
    pub account_code: String,
    pub direction: EntryDirectionRequest,
    pub amount_minor_units: i64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryDirectionRequest {
    Debit,
    Credit,
}

#[derive(Debug, Serialize)]
pub struct PostTransactionResponse {
    pub txn_id: i64,
    pub idempotency_key: String,
    pub replayed: bool,
}

#[derive(Debug, Serialize)]
pub struct WalletBalanceResponse {
    pub user_id: String,
    pub available_minor_units: i64,
    pub locked_minor_units: i64,
    pub frozen_minor_units: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListEntriesQuery {
    pub account_code: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ListEntriesResponse {
    pub entries: Vec<EntryResponse>,
}

#[derive(Debug, Serialize)]
pub struct EntryResponse {
    pub txn_id: i64,
    pub account_code: String,
    pub direction: &'static str,
    pub amount_minor_units: i64,
    pub balance_after_minor_units: i64,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub error: &'static str,
    pub message: String,
}

pub fn router<S>(store: S) -> Router
where
    S: LedgerStore + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/internal/ledger/transactions", post(post_transaction::<S>))
        .route(
            "/internal/ledger/wallets/:user_id/balance",
            get(get_wallet_balance::<S>),
        )
        .route("/internal/ledger/entries", get(list_entries::<S>))
        .with_state(LedgerService::new(store))
}

async fn post_transaction<S>(
    State(service): State<LedgerService<S>>,
    Json(request): Json<PostTransactionRequest>,
) -> Result<Json<PostTransactionResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: LedgerStore + Send + Sync,
{
    let draft = request.try_into_draft()?;
    let transaction = service.post_transaction(draft).await.map_err(api_error)?;

    Ok(Json(PostTransactionResponse {
        txn_id: transaction.id,
        idempotency_key: transaction.idempotency_key,
        replayed: transaction.replayed,
    }))
}

async fn get_wallet_balance<S>(
    State(service): State<LedgerService<S>>,
    Path(user_id): Path<String>,
) -> Result<Json<WalletBalanceResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: LedgerStore + Send + Sync,
{
    let available = service
        .get_account_balance(&format!("Player.Wallet.{user_id}"))
        .await
        .map_err(store_error)?;
    let locked = service
        .get_account_balance(&format!("Player.Locked.{user_id}"))
        .await
        .map_err(store_error)?;
    let frozen = service
        .get_account_balance(&format!("Player.Frozen.{user_id}"))
        .await
        .map_err(store_error)?;

    Ok(Json(WalletBalanceResponse {
        user_id,
        available_minor_units: available.balance_minor_units,
        locked_minor_units: locked.balance_minor_units,
        frozen_minor_units: frozen.balance_minor_units,
    }))
}

async fn list_entries<S>(
    State(service): State<LedgerService<S>>,
    Query(query): Query<ListEntriesQuery>,
) -> Result<Json<ListEntriesResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: LedgerStore + Send + Sync,
{
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let entries = service
        .list_entries(&query.account_code, limit)
        .await
        .map_err(store_error)?
        .into_iter()
        .map(|entry| EntryResponse {
            txn_id: entry.txn_id,
            account_code: entry.account_code,
            direction: if entry.direction == 1 {
                "debit"
            } else {
                "credit"
            },
            amount_minor_units: entry.amount_minor_units,
            balance_after_minor_units: entry.balance_after_minor_units,
        })
        .collect();

    Ok(Json(ListEntriesResponse { entries }))
}

impl PostTransactionRequest {
    fn try_into_draft(self) -> Result<PostTransactionDraft, (StatusCode, Json<ApiErrorResponse>)> {
        let entries = self
            .entries
            .into_iter()
            .map(PostEntryRequest::try_into_entry_draft)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(PostTransactionDraft {
            txn_type: self.txn_type,
            idempotency_key: self.idempotency_key,
            correlation_id: self.correlation_id,
            description: self.description,
            entries,
        })
    }
}

impl PostEntryRequest {
    fn try_into_entry_draft(
        self,
    ) -> Result<LedgerEntryDraft, (StatusCode, Json<ApiErrorResponse>)> {
        if self.amount_minor_units <= 0 {
            return Err(validation_error(format!(
                "entry amount must be positive for account {}",
                self.account_code
            )));
        }

        let signed_amount = match self.direction {
            EntryDirectionRequest::Debit => self.amount_minor_units,
            EntryDirectionRequest::Credit => -self.amount_minor_units,
        };

        Ok(LedgerEntryDraft::new(
            self.account_code,
            LedgerAmount::from_minor_units(signed_amount),
        ))
    }
}

fn api_error(error: PostTransactionError) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        PostTransactionError::Validation(error) => validation_error(error.to_string()),
        PostTransactionError::InsufficientFunds { .. } => validation_error(error.to_string()),
        PostTransactionError::Store(error) => store_error(error),
    }
}

fn store_error(error: LedgerStoreError) -> (StatusCode, Json<ApiErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiErrorResponse {
            error: "store_error",
            message: error.into_message(),
        }),
    )
}

fn validation_error(message: String) -> (StatusCode, Json<ApiErrorResponse>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(ApiErrorResponse {
            error: "validation_failed",
            message,
        }),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    };

    use async_trait::async_trait;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use crate::ledger::{
        AccountBalance, CommittedLedgerTransaction, LedgerEntryView, LedgerStore,
        LedgerStoreError, PostTransactionDraft,
    };

    #[tokio::test]
    async fn post_transaction_commits_balanced_request() {
        let store = InMemoryLedgerStore::default();
        let app = super::router(store.clone());
        let response = app
            .oneshot(json_post(&json!({
                "txn_type": "deposit",
                "idempotency_key": "gateway:txn-001",
                "correlation_id": "trace-001",
                "description": "test deposit",
                "entries": [
                    {
                        "account_code": "Bank.TrustAccount.TWD",
                        "direction": "debit",
                        "amount_minor_units": 10_000
                    },
                    {
                        "account_code": "Player.Wallet.user_01",
                        "direction": "credit",
                        "amount_minor_units": 10_000
                    }
                ]
            })))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["txn_id"], json!(1));
        assert_eq!(body["idempotency_key"], json!("gateway:txn-001"));
        assert_eq!(body["replayed"], json!(false));
        assert_eq!(store.commit_count(), 1);
    }

    #[tokio::test]
    async fn post_transaction_replays_same_idempotency_key() {
        let store = InMemoryLedgerStore::default();
        let request_body = json!({
            "txn_type": "deposit",
            "idempotency_key": "gateway:txn-002",
            "entries": [
                {
                    "account_code": "Bank.TrustAccount.TWD",
                    "direction": "debit",
                    "amount_minor_units": 10_000
                },
                {
                    "account_code": "Player.Wallet.user_01",
                    "direction": "credit",
                    "amount_minor_units": 10_000
                }
            ]
        });

        let first = super::router(store.clone())
            .oneshot(json_post(&request_body))
            .await
            .expect("first request should be handled");
        let second = super::router(store.clone())
            .oneshot(json_post(&request_body))
            .await
            .expect("second request should be handled");

        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(second.status(), StatusCode::OK);
        let body = response_json(second).await;
        assert_eq!(body["txn_id"], json!(1));
        assert_eq!(body["replayed"], json!(true));
        assert_eq!(store.commit_count(), 1);
    }

    #[tokio::test]
    async fn post_transaction_replays_one_hundred_retries() {
        let store = InMemoryLedgerStore::default();
        let request_body = json!({
            "txn_type": "deposit",
            "idempotency_key": "gateway:txn-100x",
            "entries": [
                {
                    "account_code": "Bank.TrustAccount.TWD",
                    "direction": "debit",
                    "amount_minor_units": 10_000
                },
                {
                    "account_code": "Player.Wallet.user_01",
                    "direction": "credit",
                    "amount_minor_units": 10_000
                }
            ]
        });

        for attempt in 0..100 {
            let response = super::router(store.clone())
                .oneshot(json_post(&request_body))
                .await
                .expect("retry request should be handled");

            assert_eq!(response.status(), StatusCode::OK);
            let body = response_json(response).await;
            assert_eq!(body["txn_id"], json!(1));
            assert_eq!(body["replayed"], json!(attempt > 0));
        }

        assert_eq!(store.commit_count(), 1);
    }

    #[tokio::test]
    async fn get_wallet_balance_returns_available_locked_and_frozen_accounts() {
        let store = InMemoryLedgerStore::default();
        seed_entry(&store, "Player.Wallet.user_01", -10_000);
        seed_entry(&store, "Player.Locked.user_01", -2_500);
        seed_entry(&store, "Player.Frozen.user_01", -1_000);

        let response = super::router(store)
            .oneshot(empty_get("/internal/ledger/wallets/user_01/balance"))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["available_minor_units"], json!(-10_000));
        assert_eq!(body["locked_minor_units"], json!(-2_500));
        assert_eq!(body["frozen_minor_units"], json!(-1_000));
    }

    #[tokio::test]
    async fn list_entries_returns_recent_entries_for_account() {
        let store = InMemoryLedgerStore::default();
        seed_entry(&store, "Player.Wallet.user_01", -10_000);
        seed_entry(&store, "Player.Wallet.user_01", 2_500);
        seed_entry(&store, "Player.Locked.user_01", -2_500);

        let response = super::router(store)
            .oneshot(empty_get(
                "/internal/ledger/entries?account_code=Player.Wallet.user_01&limit=10",
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["entries"].as_array().map(Vec::len), Some(2));
        assert_eq!(body["entries"][0]["amount_minor_units"], json!(2_500));
        assert_eq!(body["entries"][0]["direction"], json!("debit"));
    }

    #[tokio::test]
    async fn post_transaction_rejects_imbalanced_request() {
        let store = InMemoryLedgerStore::default();
        let response = super::router(store.clone())
            .oneshot(json_post(&json!({
                "txn_type": "deposit",
                "idempotency_key": "gateway:txn-bad",
                "entries": [
                    {
                        "account_code": "Bank.TrustAccount.TWD",
                        "direction": "debit",
                        "amount_minor_units": 10_000
                    },
                    {
                        "account_code": "Player.Wallet.user_01",
                        "direction": "credit",
                        "amount_minor_units": 9_999
                    }
                ]
            })))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("validation_failed"));
        assert_eq!(store.commit_count(), 0);
    }

    fn json_post(body: &Value) -> Request<Body> {
        match Request::builder()
            .method("POST")
            .uri("/internal/ledger/transactions")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
        {
            Ok(request) => request,
            Err(error) => panic!("request should build: {error}"),
        }
    }

    fn empty_get(uri: &str) -> Request<Body> {
        match Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
        {
            Ok(request) => request,
            Err(error) => panic!("request should build: {error}"),
        }
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let collected = match response.into_body().collect().await {
            Ok(body) => body,
            Err(error) => panic!("response body should collect: {error}"),
        };
        match serde_json::from_slice(&collected.to_bytes()) {
            Ok(body) => body,
            Err(error) => panic!("response should be json: {error}"),
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

    #[async_trait]
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

    fn seed_entry(store: &InMemoryLedgerStore, account_code: &str, signed_amount: i64) {
        let direction = if signed_amount >= 0 { 1 } else { -1 };
        let amount_minor_units = signed_amount.abs();
        match store.entries.lock() {
            Ok(mut entries) => entries.push(LedgerEntryView {
                txn_id: 1,
                account_code: account_code.to_owned(),
                direction,
                amount_minor_units,
                balance_after_minor_units: signed_amount,
            }),
            Err(error) => panic!("entries lock should not be poisoned: {error}"),
        }
    }
}
