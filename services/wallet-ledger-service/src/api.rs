use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use sdk::ledger::{LedgerAmount, LedgerEntryDraft};
use serde::{Deserialize, Serialize};

use crate::ledger::{LedgerService, LedgerStore, PostTransactionDraft, PostTransactionError};

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
        PostTransactionError::Store(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                error: "store_error",
                message: error.to_string(),
            }),
        ),
    }
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
        CommittedLedgerTransaction, LedgerStore, LedgerStoreError, PostTransactionDraft,
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
            Ok(transaction)
        }
    }
}
