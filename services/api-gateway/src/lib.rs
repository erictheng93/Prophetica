use std::{env, sync::Arc};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{FromRequestParts, Query, State},
    http::{StatusCode, header::HeaderName, request::Parts},
    response::{IntoResponse, Response},
    routing::get,
};
use reqwest::Url;
use sdk::{config::ServiceConfig, observability, service_stub};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, signal};

pub const SERVICE_NAME: &str = "api-gateway";
pub const DEFAULT_PORT: u16 = 8080;

const USER_ID_HEADER: HeaderName = HeaderName::from_static("x-user-id");

#[derive(Clone)]
pub struct AppState<C> {
    wallet_ledger: C,
}

impl<C> AppState<C> {
    #[must_use]
    pub const fn new(wallet_ledger: C) -> Self {
        Self { wallet_ledger }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthContext {
    pub user_id: String,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Some(value) = parts.headers.get(&USER_ID_HEADER) else {
            return Err(ApiError::unauthorized("missing x-user-id auth context"));
        };
        let user_id = value
            .to_str()
            .map_err(|_| ApiError::unauthorized("invalid x-user-id auth context"))?;

        if user_id.is_empty() {
            return Err(ApiError::unauthorized("empty x-user-id auth context"));
        }

        Ok(Self {
            user_id: user_id.to_owned(),
        })
    }
}

#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    pub error: &'static str,
    pub message: String,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    error: &'static str,
    message: String,
}

impl ApiError {
    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            error: "unauthorized",
            message: message.into(),
        }
    }

    fn upstream(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            error: "wallet_ledger_unavailable",
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiErrorBody {
                error: self.error,
                message: self.message,
            }),
        )
            .into_response()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletBalance {
    pub user_id: String,
    pub available_minor_units: i64,
    pub locked_minor_units: i64,
    pub frozen_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub txn_id: i64,
    pub account_code: String,
    pub direction: String,
    pub amount_minor_units: i64,
    pub balance_after_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LedgerEntriesQuery {
    pub account_code: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntriesResponse {
    pub entries: Vec<LedgerEntry>,
}

#[async_trait]
pub trait WalletLedgerClient {
    async fn get_wallet_balance(&self, user_id: &str) -> Result<WalletBalance, ApiError>;

    async fn list_entries(
        &self,
        account_code: &str,
        limit: i64,
    ) -> Result<Vec<LedgerEntry>, ApiError>;
}

#[derive(Debug, Clone)]
pub struct HttpWalletLedgerClient {
    base_url: Url,
    client: reqwest::Client,
}

impl HttpWalletLedgerClient {
    /// Creates an HTTP client for the internal wallet-ledger service.
    ///
    /// # Errors
    ///
    /// Returns an error when `base_url` is not a valid URL.
    pub fn new(base_url: &str) -> anyhow::Result<Self> {
        Ok(Self {
            base_url: Url::parse(base_url)?,
            client: reqwest::Client::new(),
        })
    }

    /// Creates a client from `WALLET_LEDGER_BASE_URL`, defaulting to localhost.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured URL is invalid.
    pub fn from_env() -> anyhow::Result<Self> {
        let base_url = env::var("WALLET_LEDGER_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8086".to_owned());
        Self::new(&base_url)
    }
}

#[async_trait]
impl WalletLedgerClient for HttpWalletLedgerClient {
    async fn get_wallet_balance(&self, user_id: &str) -> Result<WalletBalance, ApiError> {
        let url = self
            .base_url
            .join(&format!("/internal/ledger/wallets/{user_id}/balance"))
            .map_err(|error| ApiError::upstream(error.to_string()))?;
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| ApiError::upstream(error.to_string()))?;

        response
            .error_for_status()
            .map_err(|error| ApiError::upstream(error.to_string()))?
            .json::<WalletBalance>()
            .await
            .map_err(|error| ApiError::upstream(error.to_string()))
    }

    async fn list_entries(
        &self,
        account_code: &str,
        limit: i64,
    ) -> Result<Vec<LedgerEntry>, ApiError> {
        let url = self
            .base_url
            .join("/internal/ledger/entries")
            .map_err(|error| ApiError::upstream(error.to_string()))?;
        let response = self
            .client
            .get(url)
            .query(&[
                ("account_code", account_code),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await
            .map_err(|error| ApiError::upstream(error.to_string()))?;
        let body = response
            .error_for_status()
            .map_err(|error| ApiError::upstream(error.to_string()))?
            .json::<LedgerEntriesResponse>()
            .await
            .map_err(|error| ApiError::upstream(error.to_string()))?;

        Ok(body.entries)
    }
}

pub fn router<C>(wallet_ledger: C) -> Router
where
    C: WalletLedgerClient + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/v1/me/wallet/balance", get(get_my_wallet_balance::<C>))
        .route("/v1/ledger/entries", get(list_ledger_entries::<C>))
        .with_state(Arc::new(AppState::new(wallet_ledger)))
}

async fn get_my_wallet_balance<C>(
    State(state): State<Arc<AppState<C>>>,
    auth: AuthContext,
) -> Result<Json<WalletBalance>, ApiError>
where
    C: WalletLedgerClient + Send + Sync,
{
    let balance = state
        .wallet_ledger
        .get_wallet_balance(&auth.user_id)
        .await?;
    Ok(Json(balance))
}

async fn list_ledger_entries<C>(
    State(state): State<Arc<AppState<C>>>,
    _auth: AuthContext,
    Query(query): Query<LedgerEntriesQuery>,
) -> Result<Json<LedgerEntriesResponse>, ApiError>
where
    C: WalletLedgerClient + Send + Sync,
{
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let entries = state
        .wallet_ledger
        .list_entries(&query.account_code, limit)
        .await?;
    Ok(Json(LedgerEntriesResponse { entries }))
}

/// Runs the API gateway HTTP server.
///
/// # Errors
///
/// Returns an error when configuration loading, socket binding, or serving fails.
pub async fn run_http() -> anyhow::Result<()> {
    observability::init_json_tracing(SERVICE_NAME);
    let config = ServiceConfig::from_env(SERVICE_NAME, DEFAULT_PORT)?;
    let app = service_stub::router(&config).merge(router(HttpWalletLedgerClient::from_env()?));
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "api gateway listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        tracing::warn!(%error, "failed to install ctrl-c handler");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::{ApiError, LedgerEntry, WalletBalance, WalletLedgerClient, router};

    #[tokio::test]
    async fn wallet_balance_requires_auth_context() {
        let app = router(InMemoryWalletLedgerClient::default());
        let response = app
            .oneshot(empty_get("/v1/me/wallet/balance"))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("unauthorized"));
    }

    #[tokio::test]
    async fn wallet_balance_uses_authenticated_user_id() {
        let client = InMemoryWalletLedgerClient::default();
        let app = router(client.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/me/wallet/balance")
                    .header("x-user-id", "user_01")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["user_id"], json!("user_01"));
        assert_eq!(body["available_minor_units"], json!(-10_000));
        assert_eq!(client.requested_user_ids(), vec!["user_01".to_owned()]);
    }

    #[tokio::test]
    async fn ledger_entries_proxy_clamps_limit() {
        let client = InMemoryWalletLedgerClient::default();
        let app = router(client.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/ledger/entries?account_code=Player.Wallet.user_01&limit=999")
                    .header("x-user-id", "admin_01")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["entries"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            client.entry_requests(),
            vec![("Player.Wallet.user_01".to_owned(), 500)]
        );
    }

    fn empty_get(uri: &str) -> Request<Body> {
        Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap_or_else(|error| panic!("request should build: {error}"))
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let collected = response
            .into_body()
            .collect()
            .await
            .unwrap_or_else(|error| panic!("response body should collect: {error}"));
        serde_json::from_slice(&collected.to_bytes())
            .unwrap_or_else(|error| panic!("response should be json: {error}"))
    }

    #[derive(Clone, Default)]
    struct InMemoryWalletLedgerClient {
        requested_user_ids: Arc<Mutex<Vec<String>>>,
        entry_requests: Arc<Mutex<Vec<(String, i64)>>>,
    }

    impl InMemoryWalletLedgerClient {
        fn requested_user_ids(&self) -> Vec<String> {
            self.requested_user_ids
                .lock()
                .unwrap_or_else(|error| panic!("requested users lock should not poison: {error}"))
                .clone()
        }

        fn entry_requests(&self) -> Vec<(String, i64)> {
            self.entry_requests
                .lock()
                .unwrap_or_else(|error| panic!("entry requests lock should not poison: {error}"))
                .clone()
        }
    }

    #[async_trait::async_trait]
    impl WalletLedgerClient for InMemoryWalletLedgerClient {
        async fn get_wallet_balance(&self, user_id: &str) -> Result<WalletBalance, ApiError> {
            self.requested_user_ids
                .lock()
                .unwrap_or_else(|error| panic!("requested users lock should not poison: {error}"))
                .push(user_id.to_owned());

            Ok(WalletBalance {
                user_id: user_id.to_owned(),
                available_minor_units: -10_000,
                locked_minor_units: -2_500,
                frozen_minor_units: -1_000,
            })
        }

        async fn list_entries(
            &self,
            account_code: &str,
            limit: i64,
        ) -> Result<Vec<LedgerEntry>, ApiError> {
            self.entry_requests
                .lock()
                .unwrap_or_else(|error| panic!("entry requests lock should not poison: {error}"))
                .push((account_code.to_owned(), limit));

            Ok(vec![LedgerEntry {
                txn_id: 1,
                account_code: account_code.to_owned(),
                direction: "credit".to_owned(),
                amount_minor_units: 10_000,
                balance_after_minor_units: -10_000,
            }])
        }
    }
}
