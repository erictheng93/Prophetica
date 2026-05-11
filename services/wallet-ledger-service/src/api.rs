use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderName, StatusCode},
    routing::{get, post},
};
use sdk::ledger::{LedgerAmount, LedgerEntryDraft};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ledger::{
    AuditLogDraft, LedgerService, LedgerStore, LedgerStoreError, PostTransactionDraft,
    PostTransactionError,
};

const IDEMPOTENCY_KEY_HEADER: HeaderName = HeaderName::from_static("idempotency-key");

#[derive(Debug, Clone, Deserialize)]
pub struct PostTransactionRequest {
    pub txn_type: String,
    pub idempotency_key: Option<String>,
    pub correlation_id: Option<String>,
    pub description: Option<String>,
    pub metadata: Option<Value>,
    pub entries: Vec<PostEntryRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManualPointsAdjustmentRequest {
    pub actor_user_id: String,
    pub actor_role: String,
    pub affiliate_id: Option<String>,
    pub target_user_id: String,
    pub direction: String,
    pub delta_minor_units: i64,
    pub reason_code: String,
    pub reason_text: String,
    pub correlation_id: Option<String>,
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
    pub audit_log_id: Option<String>,
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
            "/internal/ledger/manual-points-adjustments",
            post(post_manual_points_adjustment::<S>),
        )
        .route(
            "/internal/ledger/wallets/:user_id/balance",
            get(get_wallet_balance::<S>),
        )
        .route("/internal/ledger/entries", get(list_entries::<S>))
        .with_state(LedgerService::new(store))
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME, idempotency.key = tracing::field::Empty), skip(service, request))]
async fn post_transaction<S>(
    State(service): State<LedgerService<S>>,
    headers: HeaderMap,
    Json(request): Json<PostTransactionRequest>,
) -> Result<Json<PostTransactionResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: LedgerStore + Send + Sync,
{
    let idempotency_key = idempotency_key_from_headers(&headers)?;
    tracing::Span::current().record("idempotency.key", idempotency_key.as_str());
    let draft = request.try_into_draft(idempotency_key)?;
    let audit_log_id = draft
        .audit_log
        .as_ref()
        .map(|audit_log| audit_log.id.clone());
    let transaction = service.post_transaction(draft).await.map_err(api_error)?;

    Ok(Json(PostTransactionResponse {
        txn_id: transaction.id,
        idempotency_key: transaction.idempotency_key,
        replayed: transaction.replayed,
        audit_log_id,
    }))
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME, idempotency.key = tracing::field::Empty), skip(service, request))]
async fn post_manual_points_adjustment<S>(
    State(service): State<LedgerService<S>>,
    headers: HeaderMap,
    Json(request): Json<ManualPointsAdjustmentRequest>,
) -> Result<Json<PostTransactionResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: LedgerStore + Send + Sync,
{
    let idempotency_key = idempotency_key_from_headers(&headers)?;
    tracing::Span::current().record("idempotency.key", idempotency_key.as_str());
    let draft = request.try_into_draft(idempotency_key, &headers)?;
    let audit_log_id = draft
        .audit_log
        .as_ref()
        .map(|audit_log| audit_log.id.clone());
    let transaction = service.post_transaction(draft).await.map_err(api_error)?;

    Ok(Json(PostTransactionResponse {
        txn_id: transaction.id,
        idempotency_key: transaction.idempotency_key,
        replayed: transaction.replayed,
        audit_log_id,
    }))
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME, proph.user_id = %user_id), skip(service))]
async fn get_wallet_balance<S>(
    State(service): State<LedgerService<S>>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<WalletBalanceResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: LedgerStore + Send + Sync,
{
    audit_wallet_balance_read(&service, &headers, &user_id).await?;

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

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME), skip(service))]
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
    fn try_into_draft(
        self,
        idempotency_key: String,
    ) -> Result<PostTransactionDraft, (StatusCode, Json<ApiErrorResponse>)> {
        let entries = self
            .entries
            .into_iter()
            .map(PostEntryRequest::try_into_entry_draft)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(PostTransactionDraft {
            txn_type: self.txn_type,
            idempotency_key,
            correlation_id: self.correlation_id,
            description: self.description,
            metadata: self.metadata.unwrap_or_else(|| json!({})),
            audit_log: None,
            entries,
        })
    }
}

impl ManualPointsAdjustmentRequest {
    fn try_into_draft(
        self,
        idempotency_key: String,
        headers: &HeaderMap,
    ) -> Result<PostTransactionDraft, (StatusCode, Json<ApiErrorResponse>)> {
        let parts = self.try_into_parts(headers)?;
        let audit_log_id = format!("audit:{idempotency_key}");
        let metadata = parts.ledger_metadata(&audit_log_id);
        let audit_metadata = parts.audit_metadata(&audit_log_id, &idempotency_key, headers);
        let entries = parts.entries()?;

        Ok(PostTransactionDraft {
            txn_type: "manual_points_adjustment".to_owned(),
            idempotency_key,
            correlation_id: parts.correlation_id.clone(),
            description: Some(format!(
                "manual points adjustment {} {} minor units for {}",
                parts.direction, parts.delta_minor_units, parts.target_user_id
            )),
            metadata,
            audit_log: Some(AuditLogDraft {
                id: audit_log_id,
                action: "manual_points_adjustment".to_owned(),
                actor_user_id: parts.actor_user_id,
                actor_role: parts.actor_role,
                target_user_id: parts.target_user_id,
                correlation_id: parts.correlation_id,
                metadata: audit_metadata,
            }),
            entries,
        })
    }

    fn try_into_parts(
        self,
        headers: &HeaderMap,
    ) -> Result<ManualPointsAdjustmentDraftParts, (StatusCode, Json<ApiErrorResponse>)> {
        let actor_user_id = self.actor_user_id.trim().to_owned();
        let actor_role = self.actor_role.trim().to_ascii_lowercase();
        let affiliate_id = self.affiliate_id.map(|value| value.trim().to_owned());
        let target_user_id = self.target_user_id.trim().to_owned();
        let direction = self.direction.trim().to_owned();
        let reason_code = self.reason_code.trim().to_owned();
        let reason_text = self.reason_text.trim().to_owned();
        let correlation_id = self
            .correlation_id
            .or_else(|| header_value_any(headers, &["x-correlation-id", "x-request-id"]));

        if self.delta_minor_units <= 0 {
            return Err(validation_error(
                "delta_minor_units must be greater than zero".to_owned(),
            ));
        }
        if reason_code.is_empty() || reason_text.is_empty() {
            return Err(validation_error(
                "reason_code and reason_text are required".to_owned(),
            ));
        }
        if actor_user_id.is_empty() || target_user_id.is_empty() {
            return Err(validation_error(
                "actor_user_id and target_user_id are required".to_owned(),
            ));
        }
        if actor_role == "affiliate" && affiliate_id.as_deref().unwrap_or("").is_empty() {
            return Err(validation_error(
                "affiliate_id is required for affiliate adjustments".to_owned(),
            ));
        }

        Ok(ManualPointsAdjustmentDraftParts {
            actor_user_id,
            actor_role,
            affiliate_id,
            target_user_id,
            direction,
            delta_minor_units: self.delta_minor_units,
            reason_code,
            reason_text,
            correlation_id,
        })
    }
}

#[derive(Debug, Clone)]
struct ManualPointsAdjustmentDraftParts {
    actor_user_id: String,
    actor_role: String,
    affiliate_id: Option<String>,
    target_user_id: String,
    direction: String,
    delta_minor_units: i64,
    reason_code: String,
    reason_text: String,
    correlation_id: Option<String>,
}

impl ManualPointsAdjustmentDraftParts {
    fn ledger_metadata(&self, audit_log_id: &str) -> Value {
        json!({
            "actor_user_id": self.actor_user_id,
            "actor_role": self.actor_role,
            "affiliate_id": self.affiliate_id,
            "target_user_id": self.target_user_id,
            "adjustment_direction": self.direction,
            "delta_minor_units": self.delta_minor_units,
            "reason_code": self.reason_code,
            "reason_text": self.reason_text,
            "audit_log_id": audit_log_id,
        })
    }

    fn audit_metadata(
        &self,
        audit_log_id: &str,
        idempotency_key: &str,
        headers: &HeaderMap,
    ) -> Value {
        json!({
            "actor_user_id": self.actor_user_id,
            "actor_role": self.actor_role,
            "affiliate_id": self.affiliate_id,
            "target_user_id": self.target_user_id,
            "adjustment_direction": self.direction,
            "delta_minor_units": self.delta_minor_units,
            "reason_code": self.reason_code,
            "reason_text": self.reason_text,
            "idempotency_key": idempotency_key,
            "correlation_id": self.correlation_id,
            "audit_log_id": audit_log_id,
            "source_ip": header_value(headers, "x-forwarded-for"),
            "user_agent": header_value(headers, "user-agent"),
        })
    }

    fn entries(&self) -> Result<Vec<LedgerEntryDraft>, (StatusCode, Json<ApiErrorResponse>)> {
        let entries = match self.direction.as_str() {
            "grant" => vec![
                LedgerEntryDraft::new(
                    "Expense.ManualPointsGrant",
                    LedgerAmount::from_minor_units(self.delta_minor_units),
                ),
                LedgerEntryDraft::new(
                    format!("Player.Wallet.{}", self.target_user_id),
                    LedgerAmount::from_minor_units(-self.delta_minor_units),
                ),
            ],
            "deduct" => vec![
                LedgerEntryDraft::new(
                    format!("Player.Wallet.{}", self.target_user_id),
                    LedgerAmount::from_minor_units(self.delta_minor_units),
                ),
                LedgerEntryDraft::new(
                    "Expense.ManualPointsGrant",
                    LedgerAmount::from_minor_units(-self.delta_minor_units),
                ),
            ],
            _ => {
                return Err(validation_error(
                    "direction must be grant or deduct".to_owned(),
                ));
            }
        };

        Ok(entries)
    }
}

#[derive(Debug, Clone)]
struct AuditActorContext {
    actor_user_id: String,
    actor_role: String,
    affiliate_id: Option<String>,
    correlation_id: Option<String>,
    source_ip: Option<String>,
    user_agent: Option<String>,
}

async fn audit_wallet_balance_read<S>(
    service: &LedgerService<S>,
    headers: &HeaderMap,
    target_user_id: &str,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)>
where
    S: LedgerStore + Send + Sync,
{
    let Some(context) = audit_actor_context_from_headers(headers)? else {
        return Ok(());
    };
    let allowed = matches!(context.actor_role.as_str(), "admin" | "manager");
    let audit_log = wallet_balance_read_audit_log(target_user_id, &context, allowed);
    service
        .record_audit_log(audit_log)
        .await
        .map_err(store_error)?;

    if !allowed {
        return Err(forbidden_error(
            "only admin or manager can query member wallet balances".to_owned(),
        ));
    }

    Ok(())
}

fn audit_actor_context_from_headers(
    headers: &HeaderMap,
) -> Result<Option<AuditActorContext>, (StatusCode, Json<ApiErrorResponse>)> {
    let actor_user_id = header_value_any(headers, &["x-actor-user-id", "x-actor-id"]);
    let actor_role = header_value_any(headers, &["x-actor-role", "x-operator-role"]);
    if actor_user_id.is_none() && actor_role.is_none() {
        return Ok(None);
    }

    let Some(actor_user_id) = actor_user_id else {
        return Err(validation_error(
            "x-actor-id is required for audited balance reads".to_owned(),
        ));
    };
    let Some(actor_role) = actor_role else {
        return Err(validation_error(
            "x-operator-role is required for audited balance reads".to_owned(),
        ));
    };

    Ok(Some(AuditActorContext {
        actor_user_id,
        actor_role: actor_role.to_ascii_lowercase(),
        affiliate_id: header_value(headers, "x-affiliate-id"),
        correlation_id: header_value_any(headers, &["x-correlation-id", "x-request-id"]),
        source_ip: header_value(headers, "x-forwarded-for"),
        user_agent: header_value(headers, "user-agent"),
    }))
}

fn wallet_balance_read_audit_log(
    target_user_id: &str,
    context: &AuditActorContext,
    allowed: bool,
) -> AuditLogDraft {
    AuditLogDraft {
        id: wallet_balance_read_audit_id(target_user_id, context),
        action: "wallet_balance_read".to_owned(),
        actor_user_id: context.actor_user_id.clone(),
        actor_role: context.actor_role.clone(),
        target_user_id: target_user_id.to_owned(),
        correlation_id: context.correlation_id.clone(),
        metadata: json!({
            "actor_user_id": context.actor_user_id,
            "actor_role": context.actor_role,
            "affiliate_id": context.affiliate_id,
            "target_user_id": target_user_id,
            "operation": "wallet_balance_read",
            "allowed": allowed,
            "correlation_id": context.correlation_id,
            "source_ip": context.source_ip,
            "user_agent": context.user_agent,
        }),
    }
}

fn wallet_balance_read_audit_id(target_user_id: &str, context: &AuditActorContext) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let payload = json!({
        "action": "wallet_balance_read",
        "actor_user_id": context.actor_user_id,
        "actor_role": context.actor_role,
        "target_user_id": target_user_id,
        "correlation_id": context.correlation_id,
        "timestamp": timestamp,
    });
    let encoded = serde_json::to_vec(&payload).unwrap_or_default();
    format!(
        "audit:wallet_balance_read:{}",
        hex::encode(Sha256::digest(encoded))
    )
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
        PostTransactionError::InsufficientFunds { .. }
        | PostTransactionError::ManualAdjustmentInvalid { .. } => {
            validation_error(error.to_string())
        }
        PostTransactionError::IdempotencyConflict { .. } => (
            StatusCode::CONFLICT,
            Json(ApiErrorResponse {
                error: "idempotency_conflict",
                message: error.to_string(),
            }),
        ),
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

fn forbidden_error(message: String) -> (StatusCode, Json<ApiErrorResponse>) {
    (
        StatusCode::FORBIDDEN,
        Json(ApiErrorResponse {
            error: "forbidden",
            message,
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

fn idempotency_key_from_headers(
    headers: &HeaderMap,
) -> Result<String, (StatusCode, Json<ApiErrorResponse>)> {
    let Some(value) = headers.get(&IDEMPOTENCY_KEY_HEADER) else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "ERROR_CODE_IDEMPOTENCY_KEY_REQUIRED",
                message: "Idempotency-Key header is required for ledger writes".to_owned(),
            }),
        ));
    };

    let key = value.to_str().unwrap_or_default().trim();
    if key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "ERROR_CODE_IDEMPOTENCY_KEY_REQUIRED",
                message: "Idempotency-Key header must not be empty".to_owned(),
            }),
        ));
    }

    Ok(key.to_owned())
}

fn header_value_any(headers: &HeaderMap, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| header_value(headers, name))
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
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
        AccountBalance, AuditLogDraft, CommittedLedgerTransaction, ExpiredIdempotencyRecord,
        LedgerEntryView, LedgerStore, LedgerStoreError, PostTransactionDraft,
    };

    #[tokio::test]
    async fn post_transaction_requires_idempotency_key_header() {
        let store = InMemoryLedgerStore::default();
        let response = super::router(store.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/ledger/transactions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "txn_type": "deposit",
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
                        })
                        .to_string(),
                    ))
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("ERROR_CODE_IDEMPOTENCY_KEY_REQUIRED"));
        assert_eq!(store.commit_count(), 0);
    }

    #[tokio::test]
    async fn post_transaction_rejects_empty_idempotency_key_header() {
        let store = InMemoryLedgerStore::default();
        let response = super::router(store.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/internal/ledger/transactions")
                    .header("content-type", "application/json")
                    .header("Idempotency-Key", " ")
                    .body(Body::from(
                        json!({
                            "txn_type": "deposit",
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
                        })
                        .to_string(),
                    ))
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("ERROR_CODE_IDEMPOTENCY_KEY_REQUIRED"));
        assert_eq!(store.commit_count(), 0);
    }

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
    async fn manual_points_adjustment_affiliate_grant_commits_and_records_audit_log() {
        let store = InMemoryLedgerStore::default();
        let response = super::router(store.clone())
            .oneshot(json_post_to(
                "/internal/ledger/manual-points-adjustments",
                "manual-points:grant:001",
                &json!({
                    "actor_user_id": "affiliate-user-01",
                    "actor_role": "affiliate",
                    "affiliate_id": "aff-001",
                    "target_user_id": "member-001",
                    "direction": "grant",
                    "delta_minor_units": 1_500,
                    "reason_code": "affiliate_campaign",
                    "reason_text": "MVP campaign grant",
                    "correlation_id": "ticket-001"
                }),
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["txn_id"], json!(1));
        assert_eq!(body["idempotency_key"], json!("manual-points:grant:001"));
        assert_eq!(body["audit_log_id"], json!("audit:manual-points:grant:001"));
        assert_eq!(store.commit_count(), 1);
        assert_eq!(store.audit_count(), 1);
    }

    #[tokio::test]
    async fn manual_points_adjustment_rejects_affiliate_deduct() {
        let store = InMemoryLedgerStore::default();
        let response = super::router(store.clone())
            .oneshot(json_post_to(
                "/internal/ledger/manual-points-adjustments",
                "manual-points:deduct:affiliate",
                &json!({
                    "actor_user_id": "affiliate-user-01",
                    "actor_role": "affiliate",
                    "affiliate_id": "aff-001",
                    "target_user_id": "member-001",
                    "direction": "deduct",
                    "delta_minor_units": 500,
                    "reason_code": "affiliate_campaign",
                    "reason_text": "bad deduct"
                }),
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("validation_failed"));
        assert_eq!(store.commit_count(), 0);
        assert_eq!(store.audit_count(), 0);
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
    async fn get_wallet_balance_admin_read_records_audit_log() {
        let store = InMemoryLedgerStore::default();
        seed_entry(&store, "Player.Wallet.user_01", -10_000);

        let response = super::router(store.clone())
            .oneshot(empty_get_with_actor(
                "/internal/ledger/wallets/user_01/balance",
                "admin-001",
                "admin",
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(store.audit_count(), 1);
        let audit_log = store.latest_audit_log();
        assert_eq!(audit_log.action, "wallet_balance_read");
        assert_eq!(audit_log.actor_user_id, "admin-001");
        assert_eq!(audit_log.actor_role, "admin");
        assert_eq!(audit_log.target_user_id, "user_01");
        assert_eq!(audit_log.metadata["allowed"], json!(true));
    }

    #[tokio::test]
    async fn get_wallet_balance_affiliate_read_is_forbidden_and_audited() {
        let store = InMemoryLedgerStore::default();
        seed_entry(&store, "Player.Wallet.user_01", -10_000);

        let response = super::router(store.clone())
            .oneshot(empty_get_with_actor(
                "/internal/ledger/wallets/user_01/balance",
                "affiliate-user-001",
                "affiliate",
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(store.audit_count(), 1);
        let audit_log = store.latest_audit_log();
        assert_eq!(audit_log.action, "wallet_balance_read");
        assert_eq!(audit_log.actor_user_id, "affiliate-user-001");
        assert_eq!(audit_log.actor_role, "affiliate");
        assert_eq!(audit_log.target_user_id, "user_01");
        assert_eq!(audit_log.metadata["allowed"], json!(false));
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
        let idempotency_key = body
            .get("idempotency_key")
            .and_then(Value::as_str)
            .unwrap_or("test-idempotency-key");
        json_post_to("/internal/ledger/transactions", idempotency_key, body)
    }

    fn json_post_to(uri: &str, idempotency_key: &str, body: &Value) -> Request<Body> {
        match Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .header("Idempotency-Key", idempotency_key)
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

    fn empty_get_with_actor(uri: &str, actor_id: &str, actor_role: &str) -> Request<Body> {
        match Request::builder()
            .method("GET")
            .uri(uri)
            .header("x-actor-id", actor_id)
            .header("x-operator-role", actor_role)
            .header("x-request-id", "request-001")
            .header("user-agent", "wallet-ledger-test")
            .header("x-forwarded-for", "203.0.113.10")
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
        audit_logs: Arc<Mutex<Vec<AuditLogDraft>>>,
        next_id: Arc<AtomicI64>,
    }

    impl InMemoryLedgerStore {
        fn commit_count(&self) -> usize {
            match self.transactions.lock() {
                Ok(transactions) => transactions.len(),
                Err(error) => panic!("transactions lock should not be poisoned: {error}"),
            }
        }

        fn audit_count(&self) -> usize {
            match self.audit_logs.lock() {
                Ok(audit_logs) => audit_logs.len(),
                Err(error) => panic!("audit logs lock should not be poisoned: {error}"),
            }
        }

        fn latest_audit_log(&self) -> AuditLogDraft {
            match self.audit_logs.lock() {
                Ok(audit_logs) => audit_logs
                    .last()
                    .cloned()
                    .unwrap_or_else(|| panic!("audit log should exist")),
                Err(error) => panic!("audit logs lock should not be poisoned: {error}"),
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
                request_hash: crate::ledger::request_hash_for_draft(draft),
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
            if let Some(audit_log) = &draft.audit_log {
                match self.audit_logs.lock() {
                    Ok(mut audit_logs) => audit_logs.push(audit_log.clone()),
                    Err(error) => panic!("audit logs lock should not be poisoned: {error}"),
                }
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
            audit_log: &AuditLogDraft,
        ) -> Result<(), LedgerStoreError> {
            match self.audit_logs.lock() {
                Ok(mut audit_logs) => audit_logs.push(audit_log.clone()),
                Err(error) => panic!("audit logs lock should not be poisoned: {error}"),
            }
            Ok(())
        }

        async fn delete_expired_idempotency_records(
            &self,
        ) -> Result<Vec<ExpiredIdempotencyRecord>, LedgerStoreError> {
            Ok(Vec::new())
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
