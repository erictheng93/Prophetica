use std::{
    collections::HashMap,
    env,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{FromRequestParts, Path, Query, State},
    http::{HeaderMap, StatusCode, header::HeaderName, request::Parts},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use reqwest::Url;
use sdk::{config::ServiceConfig, observability, service_stub};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, signal};

pub const SERVICE_NAME: &str = "api-gateway";
pub const DEFAULT_PORT: u16 = 8080;

pub mod admin_ops;
pub mod analytics_reporting;
pub mod drill_logs;
pub mod kill_switch;
pub mod responsible_gaming;

const USER_ID_HEADER: HeaderName = HeaderName::from_static("x-user-id");
const IDEMPOTENCY_KEY_HEADER: HeaderName = HeaderName::from_static("idempotency-key");

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

#[derive(Clone)]
pub struct AppStateWithEngine<W, E> {
    wallet_ledger: W,
    engine: E,
    idempotency: IdempotencyStore,
}

impl<W, E> AppStateWithEngine<W, E> {
    #[must_use]
    pub fn new(wallet_ledger: W, engine: E) -> Self {
        Self {
            wallet_ledger,
            engine,
            idempotency: IdempotencyStore::default(),
        }
    }
}

#[derive(Clone)]
pub struct AppStateWithServices<W, E, R> {
    wallet_ledger: W,
    engine: E,
    referral: R,
    idempotency: IdempotencyStore,
}

impl<W, E, R> AppStateWithServices<W, E, R> {
    #[must_use]
    pub fn new(wallet_ledger: W, engine: E, referral: R) -> Self {
        Self {
            wallet_ledger,
            engine,
            referral,
            idempotency: IdempotencyStore::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct IdempotencyStore {
    records: Arc<Mutex<HashMap<IdempotencyScope, IdempotencyRecord>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IdempotencyScope {
    user_id: String,
    method: &'static str,
    route: &'static str,
    key: String,
}

#[derive(Debug, Clone)]
struct IdempotencyRecord {
    request_hash: u64,
    response: OrderActionResponse,
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

    fn bad_request(error: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            error,
            message: message.into(),
        }
    }

    fn conflict(error: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            error,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: "internal_error",
            message: message.into(),
        }
    }

    fn matching_engine(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            error: "matching_engine_error",
            message: message.into(),
        }
    }

    fn referral_service(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            error: "referral_service_unavailable",
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceOrderRequest {
    pub side: String,
    pub outcome: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub price_pts: i32,
    pub quantity: i64,
    pub time_in_force: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderActionResponse {
    pub order_id: String,
    pub status: String,
    pub filled_qty: i64,
    pub avg_price_pts: i32,
    pub remaining_qty: i64,
    pub fee_pts: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookLevel {
    pub price_pts: u64,
    pub qty: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookSnapshotResponse {
    pub market_id: String,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BookSnapshotQuery {
    pub depth: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffiliateReferralListItem {
    pub referred_id: String,
    pub masked_username: String,
    pub referred_at_day: i64,
    pub status: String,
    pub cumulative_commission_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffiliateCommissionSummary {
    pub successful_referrals_count: u32,
    pub signup_bonus_minor_units: i64,
    pub trade_commission_minor_units: i64,
    pub total_rewards_minor_units: i64,
    pub last_30d_commission_minor_units: i64,
    pub last_12m_commission_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffiliateDashboardResponse {
    pub user_id: String,
    pub referral_code: String,
    pub share_url: String,
    pub summary: AffiliateCommissionSummary,
    pub referrals: Vec<AffiliateReferralListItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AffiliateQuery {
    pub limit: Option<usize>,
}

#[async_trait]
pub trait WalletLedgerClient {
    async fn get_wallet_balance(&self, user_id: &str) -> Result<WalletBalance, ApiError>;

    async fn list_entries(
        &self,
        account_code: &str,
        limit: i64,
    ) -> Result<Vec<LedgerEntry>, ApiError>;

    async fn freeze_player_funds(
        &self,
        user_id: &str,
        amount_minor_units: i64,
        idempotency_key: &str,
    ) -> Result<(), ApiError>;
}

#[async_trait]
pub trait MatchingEngineClient {
    async fn place_order(
        &self,
        user_id: &str,
        market_id: &str,
        request: &PlaceOrderRequest,
    ) -> Result<OrderActionResponse, ApiError>;

    async fn cancel_order(
        &self,
        user_id: &str,
        order_id: &str,
    ) -> Result<OrderActionResponse, ApiError>;

    async fn get_book_snapshot(
        &self,
        market_id: &str,
        depth: usize,
    ) -> Result<BookSnapshotResponse, ApiError>;
}

#[async_trait]
pub trait ReferralClient {
    async fn get_affiliate_dashboard(
        &self,
        user_id: &str,
        limit: usize,
    ) -> Result<AffiliateDashboardResponse, ApiError>;

    async fn list_affiliate_referrals(
        &self,
        user_id: &str,
        limit: usize,
    ) -> Result<Vec<AffiliateReferralListItem>, ApiError>;

    async fn get_affiliate_commission_summary(
        &self,
        user_id: &str,
    ) -> Result<AffiliateCommissionSummary, ApiError>;
}

#[derive(Debug, Clone)]
pub struct HttpWalletLedgerClient {
    base_url: Url,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct HttpMatchingEngineClient {
    base_url: Url,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct HttpReferralClient {
    base_url: Url,
    client: reqwest::Client,
}

impl HttpMatchingEngineClient {
    /// Creates an HTTP client for the internal matching-engine service.
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

    /// Creates a client from `MATCHING_ENGINE_BASE_URL`, defaulting to localhost.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured URL is invalid.
    pub fn from_env() -> anyhow::Result<Self> {
        let base_url = env::var("MATCHING_ENGINE_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8088".to_owned());
        Self::new(&base_url)
    }
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

impl HttpReferralClient {
    /// Creates an HTTP client for the internal referral service.
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

    /// Creates a client from `REFERRAL_SERVICE_BASE_URL`, defaulting to localhost.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured URL is invalid.
    pub fn from_env() -> anyhow::Result<Self> {
        let base_url = env::var("REFERRAL_SERVICE_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8092".to_owned());
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

    async fn freeze_player_funds(
        &self,
        user_id: &str,
        amount_minor_units: i64,
        idempotency_key: &str,
    ) -> Result<(), ApiError> {
        if amount_minor_units <= 0 {
            return Ok(());
        }

        let url = self
            .base_url
            .join("/internal/ledger/transactions")
            .map_err(|error| ApiError::upstream(error.to_string()))?;
        let body = FreezePlayerFundsRequest {
            txn_type: "freeze",
            correlation_id: Some(idempotency_key),
            description: Some("self-exclusion freeze player funds"),
            metadata: serde_json::json!({
                "reason": "self_exclusion",
                "triggered_by": user_id,
                "ref_id": idempotency_key,
            }),
            entries: vec![
                FreezePlayerFundsEntry {
                    account_code: format!("Player.Wallet.{user_id}"),
                    direction: "debit",
                    amount_minor_units,
                },
                FreezePlayerFundsEntry {
                    account_code: format!("Player.Frozen.{user_id}"),
                    direction: "credit",
                    amount_minor_units,
                },
            ],
        };

        self.client
            .post(url)
            .header(IDEMPOTENCY_KEY_HEADER.as_str(), idempotency_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| ApiError::upstream(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::upstream(error.to_string()))?;

        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct FreezePlayerFundsRequest<'a> {
    txn_type: &'static str,
    correlation_id: Option<&'a str>,
    description: Option<&'static str>,
    metadata: serde_json::Value,
    entries: Vec<FreezePlayerFundsEntry>,
}

#[derive(Debug, Serialize)]
struct FreezePlayerFundsEntry {
    account_code: String,
    direction: &'static str,
    amount_minor_units: i64,
}

#[async_trait]
impl MatchingEngineClient for HttpMatchingEngineClient {
    async fn place_order(
        &self,
        user_id: &str,
        market_id: &str,
        request: &PlaceOrderRequest,
    ) -> Result<OrderActionResponse, ApiError> {
        let url = self
            .base_url
            .join(&format!("/api/v1/markets/{market_id}/orders"))
            .map_err(|error| {
                ApiError::matching_engine(StatusCode::BAD_GATEWAY, error.to_string())
            })?;
        let response = self
            .client
            .post(url)
            .header(USER_ID_HEADER.as_str(), user_id)
            .json(request)
            .send()
            .await
            .map_err(|error| {
                ApiError::matching_engine(StatusCode::BAD_GATEWAY, error.to_string())
            })?;

        decode_engine_response(response).await
    }

    async fn cancel_order(
        &self,
        user_id: &str,
        order_id: &str,
    ) -> Result<OrderActionResponse, ApiError> {
        let url = self
            .base_url
            .join(&format!("/api/v1/orders/{order_id}"))
            .map_err(|error| {
                ApiError::matching_engine(StatusCode::BAD_GATEWAY, error.to_string())
            })?;
        let response = self
            .client
            .delete(url)
            .header(USER_ID_HEADER.as_str(), user_id)
            .send()
            .await
            .map_err(|error| {
                ApiError::matching_engine(StatusCode::BAD_GATEWAY, error.to_string())
            })?;

        decode_engine_response(response).await
    }

    async fn get_book_snapshot(
        &self,
        market_id: &str,
        depth: usize,
    ) -> Result<BookSnapshotResponse, ApiError> {
        let url = self
            .base_url
            .join(&format!("/api/v1/markets/{market_id}/book"))
            .map_err(|error| {
                ApiError::matching_engine(StatusCode::BAD_GATEWAY, error.to_string())
            })?;
        let response = self
            .client
            .get(url)
            .query(&[("depth", depth.to_string())])
            .send()
            .await
            .map_err(|error| {
                ApiError::matching_engine(StatusCode::BAD_GATEWAY, error.to_string())
            })?;

        decode_engine_response(response).await
    }
}

#[async_trait]
impl ReferralClient for HttpReferralClient {
    async fn get_affiliate_dashboard(
        &self,
        user_id: &str,
        limit: usize,
    ) -> Result<AffiliateDashboardResponse, ApiError> {
        let url = self
            .base_url
            .join(&format!(
                "/internal/referrals/users/{user_id}/affiliate-dashboard"
            ))
            .map_err(|error| ApiError::referral_service(error.to_string()))?;
        self.client
            .get(url)
            .query(&[("limit", limit.to_string())])
            .send()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .json::<AffiliateDashboardResponse>()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))
    }

    async fn list_affiliate_referrals(
        &self,
        user_id: &str,
        limit: usize,
    ) -> Result<Vec<AffiliateReferralListItem>, ApiError> {
        let url = self
            .base_url
            .join(&format!("/internal/referrals/users/{user_id}/referrals"))
            .map_err(|error| ApiError::referral_service(error.to_string()))?;
        self.client
            .get(url)
            .query(&[("limit", limit.to_string())])
            .send()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .json::<Vec<AffiliateReferralListItem>>()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))
    }

    async fn get_affiliate_commission_summary(
        &self,
        user_id: &str,
    ) -> Result<AffiliateCommissionSummary, ApiError> {
        let url = self
            .base_url
            .join(&format!(
                "/internal/referrals/users/{user_id}/commission-summary"
            ))
            .map_err(|error| ApiError::referral_service(error.to_string()))?;
        self.client
            .get(url)
            .send()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .json::<AffiliateCommissionSummary>()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))
    }
}

async fn decode_engine_response<T>(response: reqwest::Response) -> Result<T, ApiError>
where
    T: serde::de::DeserializeOwned,
{
    let status = response.status();
    if !status.is_success() {
        let message = response
            .text()
            .await
            .unwrap_or_else(|error| error.to_string());
        return Err(ApiError::matching_engine(status, message));
    }

    response
        .json::<T>()
        .await
        .map_err(|error| ApiError::matching_engine(StatusCode::BAD_GATEWAY, error.to_string()))
}

impl IdempotencyStore {
    fn find(&self, scope: &IdempotencyScope) -> Result<Option<IdempotencyRecord>, ApiError> {
        self.records
            .lock()
            .map_err(|_| ApiError::internal("idempotency store lock poisoned"))
            .map(|records| records.get(scope).cloned())
    }

    fn insert(&self, scope: IdempotencyScope, record: IdempotencyRecord) -> Result<(), ApiError> {
        self.records
            .lock()
            .map_err(|_| ApiError::internal("idempotency store lock poisoned"))?
            .insert(scope, record);
        Ok(())
    }
}

fn idempotency_scope(
    headers: &HeaderMap,
    user_id: &str,
    method: &'static str,
    route: &'static str,
) -> Result<IdempotencyScope, ApiError> {
    let Some(value) = headers.get(&IDEMPOTENCY_KEY_HEADER) else {
        return Err(ApiError::bad_request(
            "idempotency_key_required",
            "Idempotency-Key header is required for mutating trading requests",
        ));
    };
    let key = value
        .to_str()
        .map_err(|_| {
            ApiError::bad_request(
                "idempotency_key_required",
                "Idempotency-Key header must be valid ASCII",
            )
        })?
        .trim();
    if key.is_empty() {
        return Err(ApiError::bad_request(
            "idempotency_key_required",
            "Idempotency-Key header must not be empty",
        ));
    }

    Ok(IdempotencyScope {
        user_id: user_id.to_owned(),
        method,
        route,
        key: key.to_owned(),
    })
}

fn order_request_hash(user_id: &str, market_id: &str, request: &PlaceOrderRequest) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "POST".hash(&mut hasher);
    "/v1/markets/:market_id/orders".hash(&mut hasher);
    user_id.hash(&mut hasher);
    market_id.hash(&mut hasher);
    request.side.hash(&mut hasher);
    request.outcome.hash(&mut hasher);
    request.order_type.hash(&mut hasher);
    request.price_pts.hash(&mut hasher);
    request.quantity.hash(&mut hasher);
    request.time_in_force.hash(&mut hasher);
    hasher.finish()
}

fn cancel_request_hash(user_id: &str, order_id: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "DELETE".hash(&mut hasher);
    "/v1/orders/:order_id".hash(&mut hasher);
    user_id.hash(&mut hasher);
    order_id.hash(&mut hasher);
    hasher.finish()
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

pub fn router_with_engine<W, E>(wallet_ledger: W, engine: E) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
{
    Router::new()
        .route(
            "/v1/me/wallet/balance",
            get(get_my_wallet_balance_with_engine::<W, E>),
        )
        .route(
            "/v1/ledger/entries",
            get(list_ledger_entries_with_engine::<W, E>),
        )
        .route("/v1/markets/:market_id/orders", post(place_order::<W, E>))
        .route("/v1/orders/:order_id", delete(cancel_order::<W, E>))
        .route(
            "/v1/markets/:market_id/book",
            get(get_book_snapshot::<W, E>),
        )
        .with_state(Arc::new(AppStateWithEngine::new(wallet_ledger, engine)))
}

pub fn router_with_services<W, E, R>(wallet_ledger: W, engine: E, referral: R) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
    R: ReferralClient + Clone + Send + Sync + 'static,
{
    let rg_router = responsible_gaming::responsible_gaming_router(
        wallet_ledger.clone(),
        responsible_gaming::InMemorySelfExclusionStore::default(),
        responsible_gaming::ResponsibleGamingClock::system(),
    );

    Router::new()
        .route(
            "/v1/me/wallet/balance",
            get(get_my_wallet_balance_with_services::<W, E, R>),
        )
        .route(
            "/v1/ledger/entries",
            get(list_ledger_entries_with_services::<W, E, R>),
        )
        .route(
            "/v1/markets/:market_id/orders",
            post(place_order_with_services::<W, E, R>),
        )
        .route(
            "/v1/orders/:order_id",
            delete(cancel_order_with_services::<W, E, R>),
        )
        .route(
            "/v1/markets/:market_id/book",
            get(get_book_snapshot_with_services::<W, E, R>),
        )
        .route(
            "/v1/account/affiliate",
            get(get_my_affiliate_dashboard::<W, E, R>),
        )
        .route(
            "/v1/account/affiliate/referrals",
            get(list_my_affiliate_referrals::<W, E, R>),
        )
        .route(
            "/v1/account/affiliate/commission-summary",
            get(get_my_affiliate_commission_summary::<W, E, R>),
        )
        .with_state(Arc::new(AppStateWithServices::new(
            wallet_ledger,
            engine,
            referral,
        )))
        .merge(rg_router)
}

#[tracing::instrument(fields(service.name = SERVICE_NAME, proph.user_id = %auth.user_id), skip(state))]
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

async fn get_my_wallet_balance_with_engine<W, E>(
    State(state): State<Arc<AppStateWithEngine<W, E>>>,
    auth: AuthContext,
) -> Result<Json<WalletBalance>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
{
    let balance = state
        .wallet_ledger
        .get_wallet_balance(&auth.user_id)
        .await?;
    Ok(Json(balance))
}

#[tracing::instrument(fields(service.name = SERVICE_NAME, proph.user_id = %auth.user_id), skip(state))]
async fn list_ledger_entries<C>(
    State(state): State<Arc<AppState<C>>>,
    auth: AuthContext,
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

async fn list_ledger_entries_with_engine<W, E>(
    State(state): State<Arc<AppStateWithEngine<W, E>>>,
    _auth: AuthContext,
    Query(query): Query<LedgerEntriesQuery>,
) -> Result<Json<LedgerEntriesResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
{
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let entries = state
        .wallet_ledger
        .list_entries(&query.account_code, limit)
        .await?;
    Ok(Json(LedgerEntriesResponse { entries }))
}

#[tracing::instrument(fields(service.name = SERVICE_NAME, proph.user_id = %auth.user_id, proph.market_id = %market_id, latency.budget.api_order_submit_e2e = 0.030), skip(state, request))]
async fn place_order<W, E>(
    State(state): State<Arc<AppStateWithEngine<W, E>>>,
    auth: AuthContext,
    Path(market_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PlaceOrderRequest>,
) -> Result<Json<OrderActionResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
{
    let scope = idempotency_scope(
        &headers,
        &auth.user_id,
        "POST",
        "/v1/markets/:market_id/orders",
    )?;
    let request_hash = order_request_hash(&auth.user_id, &market_id, &request);
    if let Some(record) = state.idempotency.find(&scope)? {
        if record.request_hash == request_hash {
            return Ok(Json(record.response));
        }
        return Err(ApiError::conflict(
            "idempotency_conflict",
            "Idempotency-Key was already used with a different request",
        ));
    }

    let response = state
        .engine
        .place_order(&auth.user_id, &market_id, &request)
        .await?;
    state.idempotency.insert(
        scope,
        IdempotencyRecord {
            request_hash,
            response: response.clone(),
        },
    )?;
    Ok(Json(response))
}

#[tracing::instrument(fields(service.name = SERVICE_NAME, proph.user_id = %auth.user_id), skip(state))]
async fn cancel_order<W, E>(
    State(state): State<Arc<AppStateWithEngine<W, E>>>,
    auth: AuthContext,
    Path(order_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<OrderActionResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
{
    let scope = idempotency_scope(&headers, &auth.user_id, "DELETE", "/v1/orders/:order_id")?;
    let request_hash = cancel_request_hash(&auth.user_id, &order_id);
    if let Some(record) = state.idempotency.find(&scope)? {
        if record.request_hash == request_hash {
            return Ok(Json(record.response));
        }
        return Err(ApiError::conflict(
            "idempotency_conflict",
            "Idempotency-Key was already used with a different request",
        ));
    }

    let response = state.engine.cancel_order(&auth.user_id, &order_id).await?;
    state.idempotency.insert(
        scope,
        IdempotencyRecord {
            request_hash,
            response: response.clone(),
        },
    )?;
    Ok(Json(response))
}

#[tracing::instrument(fields(service.name = SERVICE_NAME, proph.market_id = %market_id), skip(state))]
async fn get_book_snapshot<W, E>(
    State(state): State<Arc<AppStateWithEngine<W, E>>>,
    Path(market_id): Path<String>,
    Query(query): Query<BookSnapshotQuery>,
) -> Result<Json<BookSnapshotResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
{
    let depth = query.depth.unwrap_or(10).clamp(1, 100);
    let response = state.engine.get_book_snapshot(&market_id, depth).await?;
    Ok(Json(response))
}

async fn get_my_wallet_balance_with_services<W, E, R>(
    State(state): State<Arc<AppStateWithServices<W, E, R>>>,
    auth: AuthContext,
) -> Result<Json<WalletBalance>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
{
    let balance = state
        .wallet_ledger
        .get_wallet_balance(&auth.user_id)
        .await?;
    Ok(Json(balance))
}

async fn list_ledger_entries_with_services<W, E, R>(
    State(state): State<Arc<AppStateWithServices<W, E, R>>>,
    _auth: AuthContext,
    Query(query): Query<LedgerEntriesQuery>,
) -> Result<Json<LedgerEntriesResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
{
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let entries = state
        .wallet_ledger
        .list_entries(&query.account_code, limit)
        .await?;
    Ok(Json(LedgerEntriesResponse { entries }))
}

async fn place_order_with_services<W, E, R>(
    State(state): State<Arc<AppStateWithServices<W, E, R>>>,
    auth: AuthContext,
    Path(market_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PlaceOrderRequest>,
) -> Result<Json<OrderActionResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
{
    let scope = idempotency_scope(
        &headers,
        &auth.user_id,
        "POST",
        "/v1/markets/:market_id/orders",
    )?;
    let request_hash = order_request_hash(&auth.user_id, &market_id, &request);
    if let Some(record) = state.idempotency.find(&scope)? {
        if record.request_hash == request_hash {
            return Ok(Json(record.response));
        }
        return Err(ApiError::conflict(
            "idempotency_conflict",
            "Idempotency-Key was already used with a different request",
        ));
    }

    let response = state
        .engine
        .place_order(&auth.user_id, &market_id, &request)
        .await?;
    state.idempotency.insert(
        scope,
        IdempotencyRecord {
            request_hash,
            response: response.clone(),
        },
    )?;
    Ok(Json(response))
}

async fn cancel_order_with_services<W, E, R>(
    State(state): State<Arc<AppStateWithServices<W, E, R>>>,
    auth: AuthContext,
    Path(order_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<OrderActionResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
{
    let scope = idempotency_scope(&headers, &auth.user_id, "DELETE", "/v1/orders/:order_id")?;
    let request_hash = cancel_request_hash(&auth.user_id, &order_id);
    if let Some(record) = state.idempotency.find(&scope)? {
        if record.request_hash == request_hash {
            return Ok(Json(record.response));
        }
        return Err(ApiError::conflict(
            "idempotency_conflict",
            "Idempotency-Key was already used with a different request",
        ));
    }

    let response = state.engine.cancel_order(&auth.user_id, &order_id).await?;
    state.idempotency.insert(
        scope,
        IdempotencyRecord {
            request_hash,
            response: response.clone(),
        },
    )?;
    Ok(Json(response))
}

async fn get_book_snapshot_with_services<W, E, R>(
    State(state): State<Arc<AppStateWithServices<W, E, R>>>,
    Path(market_id): Path<String>,
    Query(query): Query<BookSnapshotQuery>,
) -> Result<Json<BookSnapshotResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
{
    let depth = query.depth.unwrap_or(10).clamp(1, 100);
    let response = state.engine.get_book_snapshot(&market_id, depth).await?;
    Ok(Json(response))
}

async fn get_my_affiliate_dashboard<W, E, R>(
    State(state): State<Arc<AppStateWithServices<W, E, R>>>,
    auth: AuthContext,
    Query(query): Query<AffiliateQuery>,
) -> Result<Json<AffiliateDashboardResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
{
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let response = state
        .referral
        .get_affiliate_dashboard(&auth.user_id, limit)
        .await?;
    Ok(Json(response))
}

async fn list_my_affiliate_referrals<W, E, R>(
    State(state): State<Arc<AppStateWithServices<W, E, R>>>,
    auth: AuthContext,
    Query(query): Query<AffiliateQuery>,
) -> Result<Json<Vec<AffiliateReferralListItem>>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
{
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let referrals = state
        .referral
        .list_affiliate_referrals(&auth.user_id, limit)
        .await?;
    Ok(Json(referrals))
}

async fn get_my_affiliate_commission_summary<W, E, R>(
    State(state): State<Arc<AppStateWithServices<W, E, R>>>,
    auth: AuthContext,
) -> Result<Json<AffiliateCommissionSummary>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
{
    let summary = state
        .referral
        .get_affiliate_commission_summary(&auth.user_id)
        .await?;
    Ok(Json(summary))
}

/// Runs the API gateway HTTP server.
///
/// # Errors
///
/// Returns an error when configuration loading, socket binding, or serving fails.
#[tracing::instrument(fields(service.name = SERVICE_NAME), skip_all)]
pub async fn run_http() -> anyhow::Result<()> {
    observability::init_json_tracing(SERVICE_NAME);
    let config = ServiceConfig::from_env(SERVICE_NAME, DEFAULT_PORT)?;
    let app = service_stub::router(&config)
        .merge(router_with_services(
            HttpWalletLedgerClient::from_env()?,
            HttpMatchingEngineClient::from_env()?,
            HttpReferralClient::from_env()?,
        ))
        .merge(responsible_gaming::cooldown_router(
            responsible_gaming::InMemoryCooldownStore::default(),
            responsible_gaming::ResponsibleGamingClock::system(),
        ))
        .merge(
            analytics_reporting::AnalyticsProxyConfig::from_env().map_or_else(
                |_| analytics_reporting::unconfigured_router(),
                analytics_reporting::proxy_router,
            ),
        )
        .merge(admin_ops_router().await?);
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "api gateway listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn admin_ops_router() -> anyhow::Result<Router> {
    let upstream = admin_ops::HttpAdminOpsStore::from_env()
        .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;
    let database_url = env::var("ADMIN_OPS_DATABASE_URL").or_else(|_| env::var("DATABASE_URL"));
    match database_url {
        Ok(database_url) => {
            let pool = sqlx::PgPool::connect(&database_url).await?;
            sqlx::migrate!("./migrations").run(&pool).await?;
            Ok(admin_ops::internal_admin_router(
                admin_ops::ProductionAdminOpsStore::new(
                    upstream,
                    admin_ops::PostgresCompensationProposalStore::new(pool),
                ),
            ))
        }
        Err(_) if env::var("PROPHETICA_ENV").is_ok_and(|value| value == "production") => {
            anyhow::bail!("ADMIN_OPS_DATABASE_URL or DATABASE_URL is required in production")
        }
        Err(_) => Ok(admin_ops::internal_admin_router(upstream)),
    }
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

    use super::{
        AffiliateCommissionSummary, AffiliateDashboardResponse, AffiliateReferralListItem,
        ApiError, BookLevel, BookSnapshotResponse, LedgerEntry, MatchingEngineClient,
        OrderActionResponse, PlaceOrderRequest, ReferralClient, WalletBalance, WalletLedgerClient,
        router, router_with_engine, router_with_services,
    };

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

    #[tokio::test]
    async fn place_order_requires_idempotency_key() {
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine(InMemoryWalletLedgerClient::default(), engine.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/markets/mkt-election/orders")
                    .header("x-user-id", "10")
                    .header("content-type", "application/json")
                    .body(Body::from(order_request_body(3).to_string()))
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("idempotency_key_required"));
        assert_eq!(engine.place_requests(), Vec::new());
    }

    #[tokio::test]
    async fn place_order_replays_same_idempotency_key_without_second_engine_call() {
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine(InMemoryWalletLedgerClient::default(), engine.clone());
        let request_body = order_request_body(3);

        let first = app
            .clone()
            .oneshot(order_post("idem-order-001", &request_body))
            .await
            .expect("first request should be handled");
        let second = app
            .oneshot(order_post("idem-order-001", &request_body))
            .await
            .expect("second request should be handled");

        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(second.status(), StatusCode::OK);
        let body = response_json(second).await;
        assert_eq!(body["order_id"], json!("ord_01HZ0000000000000000000000"));
        assert_eq!(engine.place_requests().len(), 1);
    }

    #[tokio::test]
    async fn place_order_rejects_idempotency_key_reuse_with_different_payload() {
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine(InMemoryWalletLedgerClient::default(), engine.clone());

        let first = app
            .clone()
            .oneshot(order_post("idem-order-002", &order_request_body(3)))
            .await
            .expect("first request should be handled");
        let second = app
            .oneshot(order_post("idem-order-002", &order_request_body(4)))
            .await
            .expect("second request should be handled");

        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(second.status(), StatusCode::CONFLICT);
        let body = response_json(second).await;
        assert_eq!(body["error"], json!("idempotency_conflict"));
        assert_eq!(engine.place_requests().len(), 1);
    }

    #[tokio::test]
    async fn cancel_order_requires_idempotency_key() {
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine(InMemoryWalletLedgerClient::default(), engine.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/orders/ord_01HZ0000000000000000000000")
                    .header("x-user-id", "10")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("idempotency_key_required"));
        assert_eq!(engine.cancel_requests(), Vec::new());
    }

    #[tokio::test]
    async fn cancel_order_replays_same_idempotency_key_without_second_engine_call() {
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine(InMemoryWalletLedgerClient::default(), engine.clone());

        let first = app
            .clone()
            .oneshot(cancel_delete(
                "idem-cancel-001",
                "ord_01HZ0000000000000000000000",
            ))
            .await
            .expect("first request should be handled");
        let second = app
            .oneshot(cancel_delete(
                "idem-cancel-001",
                "ord_01HZ0000000000000000000000",
            ))
            .await
            .expect("second request should be handled");

        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(second.status(), StatusCode::OK);
        let body = response_json(second).await;
        assert_eq!(body["status"], json!("CANCELLED"));
        assert_eq!(engine.cancel_requests().len(), 1);
    }

    #[tokio::test]
    async fn place_order_forwards_authenticated_user_to_matching_engine() {
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine(InMemoryWalletLedgerClient::default(), engine.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/markets/mkt-election/orders")
                    .header("x-user-id", "10")
                    .header("idempotency-key", "idem-order-forward")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "side": "sell",
                            "outcome": "YES",
                            "type": "LIMIT",
                            "price_pts": 55,
                            "quantity": 3,
                            "time_in_force": "GTC"
                        })
                        .to_string(),
                    ))
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["order_id"], json!("ord_01HZ0000000000000000000000"));
        assert_eq!(
            engine.place_requests(),
            vec![(
                "10".to_owned(),
                "mkt-election".to_owned(),
                PlaceOrderRequest {
                    side: "sell".to_owned(),
                    outcome: "YES".to_owned(),
                    order_type: "LIMIT".to_owned(),
                    price_pts: 55,
                    quantity: 3,
                    time_in_force: "GTC".to_owned(),
                },
            )]
        );
    }

    #[tokio::test]
    async fn cancel_order_forwards_authenticated_user_to_matching_engine() {
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine(InMemoryWalletLedgerClient::default(), engine.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/orders/ord_01HZ0000000000000000000000")
                    .header("x-user-id", "10")
                    .header("idempotency-key", "idem-cancel-forward")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["status"], json!("CANCELLED"));
        assert_eq!(
            engine.cancel_requests(),
            vec![("10".to_owned(), "ord_01HZ0000000000000000000000".to_owned())]
        );
    }

    #[tokio::test]
    async fn book_snapshot_proxy_clamps_depth_for_matching_engine() {
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine(InMemoryWalletLedgerClient::default(), engine.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/markets/mkt-election/book?depth=999")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["asks"], json!([{ "price_pts": 55, "qty": 3 }]));
        assert_eq!(
            engine.snapshot_requests(),
            vec![("mkt-election".to_owned(), 100)]
        );
    }

    #[tokio::test]
    async fn affiliate_dashboard_forwards_authenticated_user_and_clamps_limit() {
        let referral = InMemoryReferralClient::default();
        let app = router_with_services(
            InMemoryWalletLedgerClient::default(),
            InMemoryMatchingEngineClient::default(),
            referral.clone(),
        );
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/account/affiliate?limit=999")
                    .header("x-user-id", "user_referrer")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["referral_code"], json!("a3f9c12be801_X9PK"));
        assert_eq!(
            referral.dashboard_requests(),
            vec![("user_referrer".to_owned(), 200)]
        );
    }

    #[tokio::test]
    async fn affiliate_referrals_and_commission_summary_proxy_authenticated_user() {
        let referral = InMemoryReferralClient::default();
        let app = router_with_services(
            InMemoryWalletLedgerClient::default(),
            InMemoryMatchingEngineClient::default(),
            referral.clone(),
        );

        let referrals = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/account/affiliate/referrals?limit=2")
                    .header("x-user-id", "user_referrer")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");
        assert_eq!(referrals.status(), StatusCode::OK);
        assert_eq!(
            response_json(referrals).await.as_array().map(Vec::len),
            Some(1)
        );

        let summary = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/account/affiliate/commission-summary")
                    .header("x-user-id", "user_referrer")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");
        assert_eq!(summary.status(), StatusCode::OK);
        assert_eq!(
            response_json(summary).await["total_rewards_minor_units"],
            json!(42_000)
        );
        assert_eq!(
            referral.referral_requests(),
            vec![("user_referrer".to_owned(), 2)]
        );
        assert_eq!(
            referral.summary_requests(),
            vec!["user_referrer".to_owned()]
        );
    }

    fn empty_get(uri: &str) -> Request<Body> {
        Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap_or_else(|error| panic!("request should build: {error}"))
    }

    fn order_request_body(quantity: i64) -> Value {
        json!({
            "side": "sell",
            "outcome": "YES",
            "type": "LIMIT",
            "price_pts": 55,
            "quantity": quantity,
            "time_in_force": "GTC"
        })
    }

    fn order_post(idempotency_key: &str, body: &Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri("/v1/markets/mkt-election/orders")
            .header("x-user-id", "10")
            .header("idempotency-key", idempotency_key)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap_or_else(|error| panic!("request should build: {error}"))
    }

    fn cancel_delete(idempotency_key: &str, order_id: &str) -> Request<Body> {
        Request::builder()
            .method("DELETE")
            .uri(format!("/v1/orders/{order_id}"))
            .header("x-user-id", "10")
            .header("idempotency-key", idempotency_key)
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

    #[derive(Clone, Default)]
    struct InMemoryMatchingEngineClient {
        placed_orders: Arc<Mutex<Vec<(String, String, PlaceOrderRequest)>>>,
        canceled_orders: Arc<Mutex<Vec<(String, String)>>>,
        snapshot_reads: Arc<Mutex<Vec<(String, usize)>>>,
    }

    #[derive(Clone, Default)]
    struct InMemoryReferralClient {
        dashboards: Arc<Mutex<Vec<(String, usize)>>>,
        referral_lists: Arc<Mutex<Vec<(String, usize)>>>,
        summaries: Arc<Mutex<Vec<String>>>,
    }

    impl InMemoryMatchingEngineClient {
        fn place_requests(&self) -> Vec<(String, String, PlaceOrderRequest)> {
            self.placed_orders
                .lock()
                .unwrap_or_else(|error| panic!("place requests lock should not poison: {error}"))
                .clone()
        }

        fn cancel_requests(&self) -> Vec<(String, String)> {
            self.canceled_orders
                .lock()
                .unwrap_or_else(|error| panic!("cancel requests lock should not poison: {error}"))
                .clone()
        }

        fn snapshot_requests(&self) -> Vec<(String, usize)> {
            self.snapshot_reads
                .lock()
                .unwrap_or_else(|error| panic!("snapshot requests lock should not poison: {error}"))
                .clone()
        }
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

    impl InMemoryReferralClient {
        fn dashboard_requests(&self) -> Vec<(String, usize)> {
            self.dashboards
                .lock()
                .unwrap_or_else(|error| panic!("dashboard reads lock should not poison: {error}"))
                .clone()
        }

        fn referral_requests(&self) -> Vec<(String, usize)> {
            self.referral_lists
                .lock()
                .unwrap_or_else(|error| panic!("referral reads lock should not poison: {error}"))
                .clone()
        }

        fn summary_requests(&self) -> Vec<String> {
            self.summaries
                .lock()
                .unwrap_or_else(|error| panic!("summary reads lock should not poison: {error}"))
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

        async fn freeze_player_funds(
            &self,
            _user_id: &str,
            _amount_minor_units: i64,
            _idempotency_key: &str,
        ) -> Result<(), ApiError> {
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl MatchingEngineClient for InMemoryMatchingEngineClient {
        async fn place_order(
            &self,
            user_id: &str,
            market_id: &str,
            request: &PlaceOrderRequest,
        ) -> Result<OrderActionResponse, ApiError> {
            self.placed_orders
                .lock()
                .unwrap_or_else(|error| panic!("place requests lock should not poison: {error}"))
                .push((user_id.to_owned(), market_id.to_owned(), request.clone()));

            Ok(OrderActionResponse {
                order_id: "ord_01HZ0000000000000000000000".to_owned(),
                status: "ACTIVE".to_owned(),
                filled_qty: 0,
                avg_price_pts: 0,
                remaining_qty: 3,
                fee_pts: 0,
            })
        }

        async fn cancel_order(
            &self,
            user_id: &str,
            order_id: &str,
        ) -> Result<OrderActionResponse, ApiError> {
            self.canceled_orders
                .lock()
                .unwrap_or_else(|error| panic!("cancel requests lock should not poison: {error}"))
                .push((user_id.to_owned(), order_id.to_owned()));

            Ok(OrderActionResponse {
                order_id: order_id.to_owned(),
                status: "CANCELLED".to_owned(),
                filled_qty: 0,
                avg_price_pts: 0,
                remaining_qty: 0,
                fee_pts: 0,
            })
        }

        async fn get_book_snapshot(
            &self,
            market_id: &str,
            depth: usize,
        ) -> Result<BookSnapshotResponse, ApiError> {
            self.snapshot_reads
                .lock()
                .unwrap_or_else(|error| panic!("snapshot requests lock should not poison: {error}"))
                .push((market_id.to_owned(), depth));

            Ok(BookSnapshotResponse {
                market_id: market_id.to_owned(),
                bids: Vec::new(),
                asks: vec![BookLevel {
                    price_pts: 55,
                    qty: 3,
                }],
            })
        }
    }

    #[async_trait::async_trait]
    impl ReferralClient for InMemoryReferralClient {
        async fn get_affiliate_dashboard(
            &self,
            user_id: &str,
            limit: usize,
        ) -> Result<AffiliateDashboardResponse, ApiError> {
            self.dashboards
                .lock()
                .unwrap_or_else(|error| panic!("dashboard reads lock should not poison: {error}"))
                .push((user_id.to_owned(), limit));
            Ok(AffiliateDashboardResponse {
                user_id: user_id.to_owned(),
                referral_code: "a3f9c12be801_X9PK".to_owned(),
                share_url: "https://proph.net/?ref=a3f9c12be801_X9PK".to_owned(),
                summary: sample_affiliate_summary(),
                referrals: vec![sample_affiliate_referral()],
            })
        }

        async fn list_affiliate_referrals(
            &self,
            user_id: &str,
            limit: usize,
        ) -> Result<Vec<AffiliateReferralListItem>, ApiError> {
            self.referral_lists
                .lock()
                .unwrap_or_else(|error| panic!("referral reads lock should not poison: {error}"))
                .push((user_id.to_owned(), limit));
            Ok(vec![sample_affiliate_referral()])
        }

        async fn get_affiliate_commission_summary(
            &self,
            user_id: &str,
        ) -> Result<AffiliateCommissionSummary, ApiError> {
            self.summaries
                .lock()
                .unwrap_or_else(|error| panic!("summary reads lock should not poison: {error}"))
                .push(user_id.to_owned());
            Ok(sample_affiliate_summary())
        }
    }

    fn sample_affiliate_referral() -> AffiliateReferralListItem {
        AffiliateReferralListItem {
            referred_id: "user_referred_1".to_owned(),
            masked_username: "ali*e".to_owned(),
            referred_at_day: 10,
            status: "qualified".to_owned(),
            cumulative_commission_minor_units: 30_000,
        }
    }

    fn sample_affiliate_summary() -> AffiliateCommissionSummary {
        AffiliateCommissionSummary {
            successful_referrals_count: 2,
            signup_bonus_minor_units: 20_000,
            trade_commission_minor_units: 22_000,
            total_rewards_minor_units: 42_000,
            last_30d_commission_minor_units: 12_000,
            last_12m_commission_minor_units: 42_000,
        }
    }
}
