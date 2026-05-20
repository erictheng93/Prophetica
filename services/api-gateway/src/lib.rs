use std::{
    collections::HashMap,
    env,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex, OnceLock},
};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{FromRequestParts, Path, Query, State},
    http::{HeaderMap, StatusCode, header::HeaderName, request::Parts},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use ed25519_dalek::{Verifier, VerifyingKey};
use internal_kyc_auth::{
    INTERNAL_CALL_SIGNATURE_HEADER, INTERNAL_CALL_TIMESTAMP_HEADER, sign_internal_call_for_kyc,
};
use order_precheck::{OrderKind, OrderSide, required_minor_units};
use prometheus::{IntCounter, IntCounterVec, Opts};
use reqwest::Url;
use sdk::{
    config::ServiceConfig,
    feature_flags::{InMemoryFeatureFlagStore, PaymentsRealMoneyFlagReader, PgFeatureFlagStore},
    observability,
    postgres_migrations::{API_GATEWAY_MIGRATION_TABLE, run_postgres_migrations},
    service_stub,
};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, signal};

pub const SERVICE_NAME: &str = "api-gateway";
pub const DEFAULT_PORT: u16 = 8080;

pub mod admin_ops;
pub mod analytics_reporting;
pub mod drill_logs;
pub mod internal_call_auth;
pub mod internal_kyc_auth;
pub mod kill_switch;
pub mod order_precheck;
pub mod responsible_gaming;

const USER_ID_HEADER: HeaderName = HeaderName::from_static("x-user-id");
const TRANSITION_CALLER_HEADER: HeaderName = HeaderName::from_static("x-transition-caller");
const IDEMPOTENCY_KEY_HEADER: HeaderName = HeaderName::from_static("idempotency-key");
const DEFAULT_ORDER_FEE_BPS: u32 = 200;

pub const X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF: &str = "apps-web-dev-bff";
pub const X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON: &str = "2026-08-31";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XUserIdTransitionAllowlistEntry {
    pub caller: &'static str,
    pub method: &'static str,
    pub path_pattern: &'static str,
    pub expires_on: &'static str,
}

pub const X_USER_ID_TRANSITION_ALLOWLIST: &[XUserIdTransitionAllowlistEntry] = &[
    XUserIdTransitionAllowlistEntry {
        caller: X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
        method: "GET",
        path_pattern: "/v1/me/wallet/balance",
        expires_on: X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
    },
    XUserIdTransitionAllowlistEntry {
        caller: X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
        method: "GET",
        path_pattern: "/v1/ledger/entries",
        expires_on: X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
    },
    XUserIdTransitionAllowlistEntry {
        caller: X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
        method: "POST",
        path_pattern: "/v1/markets/:market_id/orders",
        expires_on: X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
    },
    XUserIdTransitionAllowlistEntry {
        caller: X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
        method: "GET",
        path_pattern: "/v1/account/affiliate",
        expires_on: X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
    },
    XUserIdTransitionAllowlistEntry {
        caller: X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
        method: "GET",
        path_pattern: "/v1/account/affiliate/referrals",
        expires_on: X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
    },
    XUserIdTransitionAllowlistEntry {
        caller: X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
        method: "GET",
        path_pattern: "/v1/account/affiliate/commission-summary",
        expires_on: X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
    },
    XUserIdTransitionAllowlistEntry {
        caller: X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
        method: "GET",
        path_pattern: "/v1/me/responsible-gaming/cooldown",
        expires_on: X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
    },
    XUserIdTransitionAllowlistEntry {
        caller: X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
        method: "POST",
        path_pattern: "/v1/me/responsible-gaming/cooldown",
        expires_on: X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
    },
    XUserIdTransitionAllowlistEntry {
        caller: X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
        method: "GET",
        path_pattern: "/v1/responsible-gaming/self-exclusion",
        expires_on: X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
    },
    XUserIdTransitionAllowlistEntry {
        caller: X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
        method: "POST",
        path_pattern: "/v1/responsible-gaming/self-exclusion",
        expires_on: X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
    },
];

fn gateway_auth_validation_failed_total() -> &'static IntCounterVec {
    static COUNTER: OnceLock<IntCounterVec> = OnceLock::new();
    COUNTER.get_or_init(|| {
        let counter = IntCounterVec::new(
            Opts::new(
                "gateway_auth_validation_failed_total",
                "Authentication validation failures rejected by the API gateway.",
            ),
            &["reason"],
        )
        .unwrap_or_else(|error| panic!("invalid auth validation metric definition: {error}"));
        let _ = prometheus::default_registry().register(Box::new(counter.clone()));
        counter
    })
}

fn gateway_principal_header_stripped_total() -> &'static IntCounter {
    static COUNTER: OnceLock<IntCounter> = OnceLock::new();
    COUNTER.get_or_init(|| {
        let counter = IntCounter::with_opts(Opts::new(
            "gateway_principal_header_stripped_total",
            "Inbound x-user-id principal headers ignored because a verified bearer principal was present.",
        ))
        .unwrap_or_else(|error| panic!("invalid principal header metric definition: {error}"));
        let _ = prometheus::default_registry().register(Box::new(counter.clone()));
        counter
    })
}

fn order_precheck_rejected_total() -> &'static IntCounterVec {
    static COUNTER: OnceLock<IntCounterVec> = OnceLock::new();
    COUNTER.get_or_init(|| {
        let counter = IntCounterVec::new(
            Opts::new(
                "order_precheck_rejected_total",
                "Order submissions rejected before reaching the matching engine.",
            ),
            &["reason"],
        )
        .unwrap_or_else(|error| panic!("invalid order precheck metric definition: {error}"));
        let _ = prometheus::default_registry().register(Box::new(counter.clone()));
        counter
    })
}

fn record_gateway_auth_validation_failed(reason: &'static str) {
    gateway_auth_validation_failed_total()
        .with_label_values(&[reason])
        .inc();
}

fn record_gateway_principal_header_stripped() {
    gateway_principal_header_stripped_total().inc();
}

fn record_order_precheck_rejected(reason: &'static str) {
    order_precheck_rejected_total()
        .with_label_values(&[reason])
        .inc();
}

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
pub struct AppStateWithEngine<W, E, K> {
    wallet_ledger: W,
    engine: E,
    kyc: K,
    market_fees: Arc<dyn MarketFeeReader + Send + Sync>,
    idempotency: IdempotencyStore,
}

impl<W, E, K> AppStateWithEngine<W, E, K> {
    #[must_use]
    pub fn new(wallet_ledger: W, engine: E, kyc: K) -> Self {
        Self::new_with_market_fees(wallet_ledger, engine, kyc, DefaultMarketFeeReader)
    }

    #[must_use]
    pub fn new_with_market_fees<M>(wallet_ledger: W, engine: E, kyc: K, market_fees: M) -> Self
    where
        M: MarketFeeReader + Send + Sync + 'static,
    {
        Self {
            wallet_ledger,
            engine,
            kyc,
            market_fees: Arc::new(market_fees),
            idempotency: IdempotencyStore::default(),
        }
    }
}

#[derive(Clone)]
pub struct AppStateWithServices<W, E, R, K> {
    wallet_ledger: W,
    engine: E,
    referral: R,
    kyc: K,
    market_fees: Arc<dyn MarketFeeReader + Send + Sync>,
    idempotency: IdempotencyStore,
}

impl<W, E, R, K> AppStateWithServices<W, E, R, K> {
    #[must_use]
    pub fn new(wallet_ledger: W, engine: E, referral: R, kyc: K) -> Self {
        Self::new_with_market_fees(wallet_ledger, engine, referral, kyc, DefaultMarketFeeReader)
    }

    #[must_use]
    pub fn new_with_market_fees<M>(
        wallet_ledger: W,
        engine: E,
        referral: R,
        kyc: K,
        market_fees: M,
    ) -> Self
    where
        M: MarketFeeReader + Send + Sync + 'static,
    {
        Self {
            wallet_ledger,
            engine,
            referral,
            kyc,
            market_fees: Arc::new(market_fees),
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

#[derive(Debug, Clone)]
pub struct AuthVerifier {
    keys: Arc<[AuthJwk]>,
    require_verified_principal: bool,
}

#[derive(Debug, Clone)]
struct AuthJwk {
    kid: Option<String>,
    key: VerifyingKey,
}

impl AuthVerifier {
    #[must_use]
    pub fn from_env() -> Self {
        let require_verified_principal = !cfg!(test)
            || env_flag("API_GATEWAY_REQUIRE_BEARER")
            || env_flag("API_GATEWAY_REQUIRE_VERIFIED_PRINCIPAL")
            || env::var("PROPHETICA_ENV").is_ok_and(|value| value == "production");
        let keys = env::var("API_GATEWAY_AUTH_JWKS_JSON")
            .ok()
            .and_then(|raw| Self::keys_from_jwks_json(&raw).ok())
            .unwrap_or_default();

        Self {
            keys: Arc::from(keys.into_boxed_slice()),
            require_verified_principal,
        }
    }

    /// Builds a verifier from a JWKS JSON document.
    ///
    /// # Errors
    ///
    /// Returns an error when the document is not valid JWKS JSON or contains
    /// unsupported / malformed keys.
    pub fn from_jwks_json(raw: &str, require_verified_principal: bool) -> Result<Self, String> {
        Ok(Self {
            keys: Arc::from(Self::keys_from_jwks_json(raw)?.into_boxed_slice()),
            require_verified_principal,
        })
    }

    fn verify_bearer(&self, token: &str, now_epoch_seconds: u64) -> Result<AuthContext, ()> {
        let mut parts = token.split('.');
        let header = parts.next().ok_or(())?;
        let payload = parts.next().ok_or(())?;
        let signature = parts.next().ok_or(())?;
        if parts.next().is_some() {
            return Err(());
        }

        let header_json: serde_json::Value =
            serde_json::from_slice(&base64_url_decode(header).ok_or(())?).map_err(|_| ())?;
        if header_json.get("alg").and_then(serde_json::Value::as_str) != Some("EdDSA") {
            return Err(());
        }
        let kid = header_json.get("kid").and_then(serde_json::Value::as_str);
        let signing_input = format!("{header}.{payload}");
        let signature_bytes: [u8; 64] = base64_url_decode(signature)
            .ok_or(())?
            .try_into()
            .map_err(|_| ())?;
        let signature = ed25519_dalek::Signature::from_bytes(&signature_bytes);
        let signature_valid = self
            .keys
            .iter()
            .filter(|key| kid.is_none_or(|kid| key.kid.as_deref() == Some(kid)))
            .any(|key| key.key.verify(signing_input.as_bytes(), &signature).is_ok());
        if !signature_valid {
            return Err(());
        }

        let payload_json: serde_json::Value =
            serde_json::from_slice(&base64_url_decode(payload).ok_or(())?).map_err(|_| ())?;
        let exp = payload_json
            .get("exp")
            .and_then(serde_json::Value::as_u64)
            .ok_or(())?;
        if exp.saturating_add(60) < now_epoch_seconds {
            return Err(());
        }
        let user_id = payload_json
            .get("sub")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or(())?;

        Ok(AuthContext {
            user_id: user_id.to_owned(),
        })
    }

    fn keys_from_jwks_json(raw: &str) -> Result<Vec<AuthJwk>, String> {
        let jwks: serde_json::Value =
            serde_json::from_str(raw).map_err(|error| format!("invalid JWKS JSON: {error}"))?;
        let keys = jwks
            .get("keys")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "JWKS must contain a keys array".to_owned())?;
        keys.iter()
            .map(|key| {
                if key.get("kty").and_then(serde_json::Value::as_str) != Some("OKP")
                    || key.get("crv").and_then(serde_json::Value::as_str) != Some("Ed25519")
                {
                    return Err("only OKP Ed25519 JWKS keys are supported".to_owned());
                }
                let x = key
                    .get("x")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "JWKS key is missing x coordinate".to_owned())?;
                let bytes: [u8; 32] = base64_url_decode(x)
                    .ok_or_else(|| "JWKS x coordinate is not base64url".to_owned())?
                    .try_into()
                    .map_err(|_| "JWKS x coordinate must be 32 bytes".to_owned())?;
                let verifying_key = VerifyingKey::from_bytes(&bytes)
                    .map_err(|error| format!("invalid Ed25519 public key: {error}"))?;
                Ok(AuthJwk {
                    kid: key
                        .get("kid")
                        .and_then(serde_json::Value::as_str)
                        .map(ToOwned::to_owned),
                    key: verifying_key,
                })
            })
            .collect()
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let verifier = parts
            .extensions
            .get::<AuthVerifier>()
            .cloned()
            .unwrap_or_else(AuthVerifier::from_env);
        if let Some(token) = bearer_token(&parts.headers) {
            let auth = verifier
                .verify_bearer(token, unix_now_seconds())
                .map_err(|()| {
                    record_gateway_auth_validation_failed("invalid_bearer");
                    ApiError::unauthorized("invalid bearer token")
                })?;
            if parts.headers.contains_key(&USER_ID_HEADER) {
                record_gateway_principal_header_stripped();
            }
            return Ok(auth);
        }
        if verifier.require_verified_principal {
            if x_user_id_transition_fallback_allowed(
                &parts.headers,
                parts.method.as_str(),
                parts.uri.path(),
            ) {
                return x_user_id_auth_context(&parts.headers);
            }
            record_gateway_auth_validation_failed("missing_bearer");
            return Err(ApiError::unauthorized("missing bearer token"));
        }

        x_user_id_auth_context(&parts.headers)
    }
}

fn x_user_id_auth_context(headers: &HeaderMap) -> Result<AuthContext, ApiError> {
    let Some(value) = headers.get(&USER_ID_HEADER) else {
        record_gateway_auth_validation_failed("missing_x_user_id");
        return Err(ApiError::unauthorized("missing x-user-id auth context"));
    };
    let user_id = value.to_str().map_err(|_| {
        record_gateway_auth_validation_failed("invalid_x_user_id");
        ApiError::unauthorized("invalid x-user-id auth context")
    })?;

    if user_id.is_empty() {
        record_gateway_auth_validation_failed("empty_x_user_id");
        return Err(ApiError::unauthorized("empty x-user-id auth context"));
    }

    Ok(AuthContext {
        user_id: user_id.to_owned(),
    })
}

fn x_user_id_transition_fallback_allowed(headers: &HeaderMap, method: &str, path: &str) -> bool {
    if is_production_env() {
        return false;
    }
    let Some(caller) = headers
        .get(&TRANSITION_CALLER_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    x_user_id_transition_fallback_allowed_for(caller, method, path)
}

fn x_user_id_transition_fallback_allowed_for(caller: &str, method: &str, path: &str) -> bool {
    X_USER_ID_TRANSITION_ALLOWLIST.iter().any(|entry| {
        entry.caller == caller
            && entry.method.eq_ignore_ascii_case(method)
            && path_matches_pattern(path, entry.path_pattern)
    })
}

fn path_matches_pattern(path: &str, pattern: &str) -> bool {
    let path_segments = path.trim_matches('/').split('/');
    let pattern_segments = pattern.trim_matches('/').split('/');
    path_segments
        .zip(pattern_segments)
        .try_fold(0usize, |count, (actual, expected)| {
            if expected.starts_with(':') || actual == expected {
                Some(count + 1)
            } else {
                None
            }
        })
        .is_some_and(|matched| {
            matched == path.trim_matches('/').split('/').count()
                && matched == pattern.trim_matches('/').split('/').count()
        })
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|value| !value.is_empty())
}

fn unix_now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn env_flag(name: &str) -> bool {
    env::var(name).is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes"))
}

fn base64_url_decode(value: &str) -> Option<Vec<u8>> {
    let mut bits = 0_u32;
    let mut bit_count = 0_u8;
    let mut out = Vec::new();
    for byte in value.bytes() {
        let six_bits = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            b'=' => continue,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(six_bits);
        bit_count += 6;
        if bit_count >= 8 {
            bit_count -= 8;
            out.push(((bits >> bit_count) & 0xff) as u8);
        }
    }
    Some(out)
}

#[derive(Debug, Serialize)]
pub struct ApiErrorBody {
    pub error: &'static str,
    pub message: String,
    #[serde(flatten)]
    pub details: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    error: &'static str,
    message: String,
    details: serde_json::Map<String, serde_json::Value>,
}

impl ApiError {
    fn new(status: StatusCode, error: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            error,
            message: message.into(),
            details: serde_json::Map::new(),
        }
    }

    fn with_detail(mut self, key: &'static str, value: serde_json::Value) -> Self {
        self.details.insert(key.to_owned(), value);
        self
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", message)
    }

    fn upstream(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_GATEWAY,
            "wallet_ledger_unavailable",
            message,
        )
    }

    fn bad_request(error: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, error, message)
    }

    fn conflict(error: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, error, message)
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
    }

    fn matching_engine(status: StatusCode, message: impl Into<String>) -> Self {
        Self::new(status, "matching_engine_error", message)
    }

    fn referral_service(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_GATEWAY,
            "referral_service_unavailable",
            message,
        )
    }

    fn market_service(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_GATEWAY,
            "market_service_unavailable",
            message,
        )
    }

    fn kyc_lookup_unavailable(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "ERROR_CODE_KYC_LOOKUP_UNAVAILABLE",
            message,
        )
    }

    fn kyc_tier_insufficient(current_tier: &str) -> Self {
        Self::new(
            StatusCode::FORBIDDEN,
            "ERROR_CODE_KYC_TIER_INSUFFICIENT",
            "KYC tier is below the minimum required for trading",
        )
        .with_detail("current_tier", serde_json::json!(current_tier))
        .with_detail("required_tier", serde_json::json!("TIER_1"))
        .with_detail("redirect_to", serde_json::json!("/kyc/tier-1"))
    }

    fn insufficient_funds(required_minor_units: i64, available_minor_units: i64) -> Self {
        Self::new(
            StatusCode::PAYMENT_REQUIRED,
            "ERROR_CODE_INSUFFICIENT_FUNDS",
            "Available wallet balance is below the required order cost",
        )
        .with_detail(
            "required_minor_units",
            serde_json::json!(required_minor_units),
        )
        .with_detail(
            "available_minor_units",
            serde_json::json!(available_minor_units),
        )
        .with_detail("redirect_to", serde_json::json!("/topup"))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiErrorBody {
                error: self.error,
                message: self.message,
                details: self.details,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentsRealMoneyStatusResponse {
    pub real_money_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KycTierResponse {
    pub tier: String,
}

#[derive(Debug, Clone)]
struct PaymentsRealMoneyState<R> {
    flags: R,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureReferralRequest {
    pub referral_code: String,
    pub referred_user_id: String,
    pub referred_at_day: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Market {
    pub id: String,
    pub title: String,
    pub brief: String,
    pub category: String,
    pub status: String,
    pub yes_price_cents: i32,
    pub yes_bid_cents: i32,
    pub yes_ask_cents: i32,
    #[serde(default)]
    pub fee_bps: Option<u32>,
    pub volume_24h_points: i64,
    pub volume_total_points: i64,
    pub opened_at: String,
    pub close_at: String,
    pub oracle_source: String,
    pub resolved_outcome: Option<String>,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketListResponse {
    pub markets: Vec<Market>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MarketListQuery {
    pub status: Option<String>,
    pub category: Option<String>,
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
    async fn capture_referral(
        &self,
        request: CaptureReferralRequest,
    ) -> Result<serde_json::Value, ApiError>;

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

    async fn list_admin_affiliates(&self) -> Result<serde_json::Value, ApiError>;

    async fn get_admin_affiliate_detail(
        &self,
        affiliate_id: &str,
    ) -> Result<serde_json::Value, ApiError>;

    async fn create_admin_affiliate(
        &self,
        request: serde_json::Value,
        context: AdminProxyContext,
    ) -> Result<serde_json::Value, ApiError>;

    async fn update_admin_affiliate(
        &self,
        affiliate_id: &str,
        request: serde_json::Value,
        context: AdminProxyContext,
    ) -> Result<serde_json::Value, ApiError>;

    async fn archive_admin_affiliate(
        &self,
        affiliate_id: &str,
        request: serde_json::Value,
        context: AdminProxyContext,
    ) -> Result<serde_json::Value, ApiError>;

    async fn list_commission_adjustment_reviews(&self) -> Result<serde_json::Value, ApiError>;

    async fn list_affiliate_payout_reviews(&self) -> Result<serde_json::Value, ApiError>;
}

#[async_trait]
pub trait MarketServiceClient {
    async fn list_markets(
        &self,
        status: Option<&str>,
        category: Option<&str>,
    ) -> Result<MarketListResponse, ApiError>;

    async fn get_market(&self, market_id: &str) -> Result<Market, ApiError>;
}

#[async_trait]
pub trait MarketFeeReader {
    async fn fee_bps_cap(&self, market_id: &str) -> Result<u32, ApiError>;
}

#[derive(Debug, Clone, Default)]
pub struct DefaultMarketFeeReader;

#[async_trait]
impl MarketFeeReader for DefaultMarketFeeReader {
    async fn fee_bps_cap(&self, _market_id: &str) -> Result<u32, ApiError> {
        Ok(DEFAULT_ORDER_FEE_BPS)
    }
}

#[async_trait]
pub trait KycTierClient {
    async fn get_kyc_tier(&self, user_id: &str) -> Result<String, ApiError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminProxyContext {
    pub actor_id: String,
    pub role: String,
    pub request_id: Option<String>,
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

#[derive(Debug, Clone)]
pub struct HttpMarketServiceClient {
    base_url: Url,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct HttpKycServiceClient {
    base_url: Url,
    client: reqwest::Client,
    internal_call_secret: String,
}

/// Test-only KYC tier client that approves every user. Production code MUST
/// use [`HttpKycServiceClient`]; gating with `#[cfg(test)]` keeps this out of
/// the release artifact so a mis-wired binary cannot accidentally bypass the
/// SPEC-005 §2.1 Tier 1 gate.
#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub struct AllowAllKycTierClient;

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

impl HttpMarketServiceClient {
    /// Creates an HTTP client for the internal market service.
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

    /// Creates a client from `MARKET_SERVICE_BASE_URL`, defaulting to localhost.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured URL is invalid.
    pub fn from_env() -> anyhow::Result<Self> {
        let base_url = env::var("MARKET_SERVICE_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8087".to_owned());
        Self::new(&base_url)
    }
}

impl HttpKycServiceClient {
    /// Creates an HTTP client for the internal KYC service.
    ///
    /// # Errors
    ///
    /// Returns an error when `base_url` is not a valid URL.
    pub fn new(base_url: &str, internal_call_secret: impl Into<String>) -> anyhow::Result<Self> {
        Ok(Self {
            base_url: Url::parse(base_url)?,
            client: reqwest::Client::new(),
            internal_call_secret: internal_call_secret.into(),
        })
    }

    /// Creates a client from `KYC_SERVICE_BASE_URL` + `INTERNAL_CALL_HMAC_SECRET`,
    /// defaulting to localhost.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured URL is invalid, or when the HMAC
    /// secret is missing in production.
    pub fn from_env() -> anyhow::Result<Self> {
        let base_url =
            env::var("KYC_SERVICE_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:8090".to_owned());
        let secret = match env::var("INTERNAL_CALL_HMAC_SECRET")
            .ok()
            .filter(|value| !value.trim().is_empty())
        {
            Some(value) => value,
            None if is_production_env() => {
                anyhow::bail!(
                    "INTERNAL_CALL_HMAC_SECRET is required for HttpKycServiceClient in production"
                );
            }
            None => "dev-analytics-internal-call-secret".to_owned(),
        };
        Self::new(&base_url, secret)
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
    async fn capture_referral(
        &self,
        request: CaptureReferralRequest,
    ) -> Result<serde_json::Value, ApiError> {
        let url = self
            .base_url
            .join("/internal/referrals/capture")
            .map_err(|error| ApiError::referral_service(error.to_string()))?;
        self.client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .json::<serde_json::Value>()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))
    }

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

    async fn list_admin_affiliates(&self) -> Result<serde_json::Value, ApiError> {
        let url = self
            .base_url
            .join("/internal/admin/affiliates")
            .map_err(|error| ApiError::referral_service(error.to_string()))?;
        self.client
            .get(url)
            .send()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .json::<serde_json::Value>()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))
    }

    async fn get_admin_affiliate_detail(
        &self,
        affiliate_id: &str,
    ) -> Result<serde_json::Value, ApiError> {
        let url = self
            .base_url
            .join(&format!("/internal/admin/affiliates/{affiliate_id}"))
            .map_err(|error| ApiError::referral_service(error.to_string()))?;
        self.client
            .get(url)
            .send()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .json::<serde_json::Value>()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))
    }

    async fn create_admin_affiliate(
        &self,
        request: serde_json::Value,
        context: AdminProxyContext,
    ) -> Result<serde_json::Value, ApiError> {
        let url = self
            .base_url
            .join("/internal/admin/affiliates")
            .map_err(|error| ApiError::referral_service(error.to_string()))?;
        self.client
            .post(url)
            .headers(admin_proxy_headers(context)?)
            .json(&request)
            .send()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .json::<serde_json::Value>()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))
    }

    async fn update_admin_affiliate(
        &self,
        affiliate_id: &str,
        request: serde_json::Value,
        context: AdminProxyContext,
    ) -> Result<serde_json::Value, ApiError> {
        let url = self
            .base_url
            .join(&format!("/internal/admin/affiliates/{affiliate_id}"))
            .map_err(|error| ApiError::referral_service(error.to_string()))?;
        self.client
            .patch(url)
            .headers(admin_proxy_headers(context)?)
            .json(&request)
            .send()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .json::<serde_json::Value>()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))
    }

    async fn archive_admin_affiliate(
        &self,
        affiliate_id: &str,
        request: serde_json::Value,
        context: AdminProxyContext,
    ) -> Result<serde_json::Value, ApiError> {
        let url = self
            .base_url
            .join(&format!("/internal/admin/affiliates/{affiliate_id}"))
            .map_err(|error| ApiError::referral_service(error.to_string()))?;
        self.client
            .delete(url)
            .headers(admin_proxy_headers(context)?)
            .json(&request)
            .send()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .json::<serde_json::Value>()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))
    }

    async fn list_commission_adjustment_reviews(&self) -> Result<serde_json::Value, ApiError> {
        let url = self
            .base_url
            .join("/internal/admin/affiliate-commission-adjustments")
            .map_err(|error| ApiError::referral_service(error.to_string()))?;
        self.client
            .get(url)
            .send()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .json::<serde_json::Value>()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))
    }

    async fn list_affiliate_payout_reviews(&self) -> Result<serde_json::Value, ApiError> {
        let url = self
            .base_url
            .join("/internal/admin/affiliate-payouts")
            .map_err(|error| ApiError::referral_service(error.to_string()))?;
        self.client
            .get(url)
            .send()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::referral_service(error.to_string()))?
            .json::<serde_json::Value>()
            .await
            .map_err(|error| ApiError::referral_service(error.to_string()))
    }
}

#[async_trait]
impl MarketServiceClient for HttpMarketServiceClient {
    async fn list_markets(
        &self,
        status: Option<&str>,
        category: Option<&str>,
    ) -> Result<MarketListResponse, ApiError> {
        let mut url = self
            .base_url
            .join("/internal/markets")
            .map_err(|error| ApiError::market_service(error.to_string()))?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(status) = status {
                query.append_pair("status", status);
            }
            if let Some(category) = category {
                query.append_pair("category", category);
            }
        }
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| ApiError::market_service(error.to_string()))?;

        response
            .error_for_status()
            .map_err(|error| ApiError::market_service(error.to_string()))?
            .json::<MarketListResponse>()
            .await
            .map_err(|error| ApiError::market_service(error.to_string()))
    }

    async fn get_market(&self, market_id: &str) -> Result<Market, ApiError> {
        let url = self
            .base_url
            .join(&format!("/internal/markets/{market_id}"))
            .map_err(|error| ApiError::market_service(error.to_string()))?;
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| ApiError::market_service(error.to_string()))?;

        response
            .error_for_status()
            .map_err(|error| ApiError::market_service(error.to_string()))?
            .json::<Market>()
            .await
            .map_err(|error| ApiError::market_service(error.to_string()))
    }
}

#[async_trait]
impl<T> MarketFeeReader for T
where
    T: MarketServiceClient + Send + Sync,
{
    async fn fee_bps_cap(&self, market_id: &str) -> Result<u32, ApiError> {
        let market = self.get_market(market_id).await?;
        Ok(market
            .fee_bps
            .unwrap_or(DEFAULT_ORDER_FEE_BPS)
            .max(DEFAULT_ORDER_FEE_BPS))
    }
}

#[async_trait]
impl KycTierClient for HttpKycServiceClient {
    async fn get_kyc_tier(&self, user_id: &str) -> Result<String, ApiError> {
        let path = format!("/internal/kyc/users/{user_id}/tier");
        let url = self
            .base_url
            .join(&path)
            .map_err(|error| ApiError::kyc_lookup_unavailable(error.to_string()))?;

        let timestamp = i64::try_from(unix_now_seconds()).unwrap_or(i64::MAX);
        let signature =
            sign_internal_call_for_kyc("GET", &path, b"", timestamp, &self.internal_call_secret);

        let response = self
            .client
            .get(url)
            .header(INTERNAL_CALL_TIMESTAMP_HEADER, timestamp.to_string())
            .header(INTERNAL_CALL_SIGNATURE_HEADER, signature)
            .send()
            .await
            .map_err(|error| ApiError::kyc_lookup_unavailable(error.to_string()))?;

        let body = response
            .error_for_status()
            .map_err(|error| ApiError::kyc_lookup_unavailable(error.to_string()))?
            .json::<KycTierResponse>()
            .await
            .map_err(|error| ApiError::kyc_lookup_unavailable(error.to_string()))?;

        Ok(body.tier)
    }
}

#[cfg(test)]
#[async_trait]
impl KycTierClient for AllowAllKycTierClient {
    async fn get_kyc_tier(&self, _user_id: &str) -> Result<String, ApiError> {
        Ok("TIER_1".to_owned())
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

fn admin_proxy_headers(context: AdminProxyContext) -> Result<reqwest::header::HeaderMap, ApiError> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "x-actor-id",
        context
            .actor_id
            .parse()
            .map_err(|error| ApiError::bad_request("invalid_admin_context", format!("{error}")))?,
    );
    headers.insert(
        "x-operator-role",
        context
            .role
            .parse()
            .map_err(|error| ApiError::bad_request("invalid_admin_context", format!("{error}")))?,
    );
    if let Some(request_id) = context.request_id {
        headers.insert(
            "x-request-id",
            request_id.parse().map_err(|error| {
                ApiError::bad_request("invalid_admin_context", format!("{error}"))
            })?,
        );
    }
    Ok(headers)
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
    router_with_payment_flags(wallet_ledger, InMemoryFeatureFlagStore::default())
}

pub fn router_with_payment_flags<C, F>(wallet_ledger: C, flags: F) -> Router
where
    C: WalletLedgerClient + Clone + Send + Sync + 'static,
    F: PaymentsRealMoneyFlagReader + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/v1/me/wallet/balance", get(get_my_wallet_balance::<C>))
        .route("/v1/ledger/entries", get(list_ledger_entries::<C>))
        .with_state(Arc::new(AppState::new(wallet_ledger)))
        .merge(payments_real_money_router(flags))
}

/// Test-only convenience: wires the router with [`AllowAllKycTierClient`].
/// Production must call [`router_with_engine_and_kyc`] (or one of the
/// `_and_kyc_*` variants) with [`HttpKycServiceClient`].
#[cfg(test)]
pub fn router_with_engine<W, E>(wallet_ledger: W, engine: E) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
{
    router_with_engine_and_kyc(wallet_ledger, engine, AllowAllKycTierClient)
}

pub fn router_with_engine_and_kyc<W, E, K>(wallet_ledger: W, engine: E, kyc: K) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
    K: KycTierClient + Clone + Send + Sync + 'static,
{
    router_with_engine_and_kyc_and_market_fees(wallet_ledger, engine, kyc, DefaultMarketFeeReader)
}

pub fn router_with_engine_and_kyc_and_market_fees<W, E, K, M>(
    wallet_ledger: W,
    engine: E,
    kyc: K,
    market_fees: M,
) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
    K: KycTierClient + Clone + Send + Sync + 'static,
    M: MarketFeeReader + Send + Sync + 'static,
{
    Router::new()
        .route(
            "/v1/me/wallet/balance",
            get(get_my_wallet_balance_with_engine::<W, E, K>),
        )
        .route(
            "/v1/ledger/entries",
            get(list_ledger_entries_with_engine::<W, E, K>),
        )
        .route(
            "/v1/markets/:market_id/orders",
            post(place_order::<W, E, K>),
        )
        .route("/v1/orders/:order_id", delete(cancel_order::<W, E, K>))
        .route(
            "/v1/markets/:market_id/book",
            get(get_book_snapshot::<W, E, K>),
        )
        .with_state(Arc::new(AppStateWithEngine::new_with_market_fees(
            wallet_ledger,
            engine,
            kyc,
            market_fees,
        )))
        .merge(payments_real_money_router(
            InMemoryFeatureFlagStore::default(),
        ))
}

/// Test-only convenience: wires the router with [`AllowAllKycTierClient`].
/// Production must call [`router_with_services_and_kyc`] (or one of the
/// `_and_kyc_*` variants) with [`HttpKycServiceClient`].
#[cfg(test)]
pub fn router_with_services<W, E, R>(wallet_ledger: W, engine: E, referral: R) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
    R: ReferralClient + Clone + Send + Sync + 'static,
{
    router_with_services_and_kyc(wallet_ledger, engine, referral, AllowAllKycTierClient)
}

pub fn router_with_services_and_kyc<W, E, R, K>(
    wallet_ledger: W,
    engine: E,
    referral: R,
    kyc: K,
) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
    R: ReferralClient + Clone + Send + Sync + 'static,
    K: KycTierClient + Clone + Send + Sync + 'static,
{
    router_with_services_and_kyc_and_payment_flags(
        wallet_ledger,
        engine,
        referral,
        kyc,
        InMemoryFeatureFlagStore::default(),
    )
}

pub fn router_with_services_and_kyc_and_payment_flags<W, E, R, K, F>(
    wallet_ledger: W,
    engine: E,
    referral: R,
    kyc: K,
    payment_flags: F,
) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
    R: ReferralClient + Clone + Send + Sync + 'static,
    K: KycTierClient + Clone + Send + Sync + 'static,
    F: PaymentsRealMoneyFlagReader + Clone + Send + Sync + 'static,
{
    router_with_services_and_kyc_and_payment_flags_and_market_fees(
        wallet_ledger,
        engine,
        referral,
        kyc,
        payment_flags,
        DefaultMarketFeeReader,
    )
}

pub fn router_with_services_and_kyc_and_payment_flags_and_market_fees<W, E, R, K, F, M>(
    wallet_ledger: W,
    engine: E,
    referral: R,
    kyc: K,
    payment_flags: F,
    market_fees: M,
) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
    R: ReferralClient + Clone + Send + Sync + 'static,
    K: KycTierClient + Clone + Send + Sync + 'static,
    F: PaymentsRealMoneyFlagReader + Clone + Send + Sync + 'static,
    M: MarketFeeReader + Send + Sync + 'static,
{
    let rg_router = responsible_gaming::responsible_gaming_router(
        wallet_ledger.clone(),
        responsible_gaming::InMemorySelfExclusionStore::default(),
        responsible_gaming::ResponsibleGamingClock::system(),
    );

    Router::new()
        .route(
            "/v1/me/wallet/balance",
            get(get_my_wallet_balance_with_services::<W, E, R, K>),
        )
        .route(
            "/v1/ledger/entries",
            get(list_ledger_entries_with_services::<W, E, R, K>),
        )
        .route(
            "/v1/markets/:market_id/orders",
            post(place_order_with_services::<W, E, R, K>),
        )
        .route(
            "/v1/orders/:order_id",
            delete(cancel_order_with_services::<W, E, R, K>),
        )
        .route(
            "/v1/markets/:market_id/book",
            get(get_book_snapshot_with_services::<W, E, R, K>),
        )
        .route(
            "/v1/account/affiliate",
            get(get_my_affiliate_dashboard::<W, E, R, K>),
        )
        .route(
            "/v1/account/affiliate/referrals",
            get(list_my_affiliate_referrals::<W, E, R, K>),
        )
        .route(
            "/v1/account/affiliate/commission-summary",
            get(get_my_affiliate_commission_summary::<W, E, R, K>),
        )
        .route(
            "/api/internal/referrals/capture",
            post(capture_referral::<W, E, R, K>),
        )
        .route(
            "/api/internal/admin/affiliates",
            get(list_admin_affiliates::<W, E, R, K>).post(create_admin_affiliate::<W, E, R, K>),
        )
        .route(
            "/api/internal/admin/affiliates/:affiliate_id",
            get(get_admin_affiliate_detail::<W, E, R, K>)
                .patch(update_admin_affiliate::<W, E, R, K>)
                .delete(archive_admin_affiliate::<W, E, R, K>),
        )
        .route(
            "/api/internal/admin/affiliate-commission-adjustments",
            get(list_commission_adjustment_reviews::<W, E, R, K>),
        )
        .route(
            "/api/internal/admin/affiliate-payouts",
            get(list_affiliate_payout_reviews::<W, E, R, K>),
        )
        .with_state(Arc::new(AppStateWithServices::new_with_market_fees(
            wallet_ledger,
            engine,
            referral,
            kyc,
            market_fees,
        )))
        .merge(rg_router)
        .merge(payments_real_money_router(payment_flags))
}

pub fn payments_real_money_router<R>(flags: R) -> Router
where
    R: PaymentsRealMoneyFlagReader + Clone + Send + Sync + 'static,
{
    Router::new()
        .route(
            "/v1/payments/real-money-status",
            get(get_payments_real_money_status::<R>),
        )
        .with_state(Arc::new(PaymentsRealMoneyState { flags }))
}

pub fn market_router<M>(market_service: M) -> Router
where
    M: MarketServiceClient + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/v1/markets", get(list_public_markets::<M>))
        .route("/v1/markets/:market_id", get(get_public_market::<M>))
        .with_state(Arc::new(market_service))
}

async fn list_public_markets<M>(
    State(market_service): State<Arc<M>>,
    Query(query): Query<MarketListQuery>,
) -> Result<Json<MarketListResponse>, ApiError>
where
    M: MarketServiceClient,
{
    market_service
        .list_markets(query.status.as_deref(), query.category.as_deref())
        .await
        .map(Json)
}

async fn get_public_market<M>(
    State(market_service): State<Arc<M>>,
    Path(market_id): Path<String>,
) -> Result<Json<Market>, ApiError>
where
    M: MarketServiceClient,
{
    market_service.get_market(&market_id).await.map(Json)
}

async fn get_payments_real_money_status<R>(
    State(state): State<Arc<PaymentsRealMoneyState<R>>>,
) -> Json<PaymentsRealMoneyStatusResponse>
where
    R: PaymentsRealMoneyFlagReader,
{
    Json(PaymentsRealMoneyStatusResponse {
        real_money_enabled: state.flags.payments_real_money_enabled(),
    })
}

async fn enforce_order_prechecks<W, K>(
    wallet_ledger: &W,
    kyc: &K,
    market_fees: &(dyn MarketFeeReader + Send + Sync),
    user_id: &str,
    market_id: &str,
    request: &PlaceOrderRequest,
) -> Result<(), ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let tier = kyc.get_kyc_tier(user_id).await.inspect_err(|_| {
        record_order_precheck_rejected("kyc_lookup_unavailable");
    })?;
    if !kyc_tier_allows_trading(&tier) {
        record_order_precheck_rejected("kyc_tier_insufficient");
        return Err(ApiError::kyc_tier_insufficient(&tier));
    }

    let order_kind = parse_order_kind(&request.order_type).inspect_err(|_| {
        record_order_precheck_rejected("invalid_order_type");
    })?;
    let side = parse_order_side(&request.side).inspect_err(|_| {
        record_order_precheck_rejected("invalid_order_side");
    })?;
    let fee_bps_cap = if matches!(side, OrderSide::Bid) {
        market_fees.fee_bps_cap(market_id).await?
    } else {
        DEFAULT_ORDER_FEE_BPS
    };
    let required_minor_units = required_minor_units(
        order_kind,
        side,
        i64::from(request.price_pts),
        request.quantity,
        fee_bps_cap,
    );
    if required_minor_units <= 0 {
        return Ok(());
    }

    let balance = wallet_ledger.get_wallet_balance(user_id).await?;
    if balance.available_minor_units < required_minor_units {
        record_order_precheck_rejected("insufficient_funds");
        return Err(ApiError::insufficient_funds(
            required_minor_units,
            balance.available_minor_units,
        ));
    }

    Ok(())
}

fn kyc_tier_allows_trading(tier: &str) -> bool {
    matches!(tier, "TIER_1" | "TIER_2" | "TIER_3")
}

fn parse_order_kind(value: &str) -> Result<OrderKind, ApiError> {
    if value.eq_ignore_ascii_case("LIMIT") {
        return Ok(OrderKind::Limit);
    }
    if value.eq_ignore_ascii_case("MARKET") {
        return Ok(OrderKind::Market);
    }
    Err(ApiError::bad_request(
        "invalid_order_type",
        "Order type must be LIMIT or MARKET",
    ))
}

fn parse_order_side(value: &str) -> Result<OrderSide, ApiError> {
    if value.eq_ignore_ascii_case("BUY") || value.eq_ignore_ascii_case("BID") {
        return Ok(OrderSide::Bid);
    }
    if value.eq_ignore_ascii_case("SELL") || value.eq_ignore_ascii_case("ASK") {
        return Ok(OrderSide::Ask);
    }
    Err(ApiError::bad_request(
        "invalid_order_side",
        "Order side must be BUY or SELL",
    ))
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

async fn get_my_wallet_balance_with_engine<W, E, K>(
    State(state): State<Arc<AppStateWithEngine<W, E, K>>>,
    auth: AuthContext,
) -> Result<Json<WalletBalance>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    K: KycTierClient + Send + Sync,
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

async fn list_ledger_entries_with_engine<W, E, K>(
    State(state): State<Arc<AppStateWithEngine<W, E, K>>>,
    _auth: AuthContext,
    Query(query): Query<LedgerEntriesQuery>,
) -> Result<Json<LedgerEntriesResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let entries = state
        .wallet_ledger
        .list_entries(&query.account_code, limit)
        .await?;
    Ok(Json(LedgerEntriesResponse { entries }))
}

#[tracing::instrument(fields(service.name = SERVICE_NAME, proph.user_id = %auth.user_id, proph.market_id = %market_id, latency.budget.api_order_submit_e2e = 0.030), skip(state, request))]
async fn place_order<W, E, K>(
    State(state): State<Arc<AppStateWithEngine<W, E, K>>>,
    auth: AuthContext,
    Path(market_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PlaceOrderRequest>,
) -> Result<Json<OrderActionResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    K: KycTierClient + Send + Sync,
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

    enforce_order_prechecks(
        &state.wallet_ledger,
        &state.kyc,
        state.market_fees.as_ref(),
        &auth.user_id,
        &market_id,
        &request,
    )
    .await?;

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
async fn cancel_order<W, E, K>(
    State(state): State<Arc<AppStateWithEngine<W, E, K>>>,
    auth: AuthContext,
    Path(order_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<OrderActionResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    K: KycTierClient + Send + Sync,
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
async fn get_book_snapshot<W, E, K>(
    State(state): State<Arc<AppStateWithEngine<W, E, K>>>,
    Path(market_id): Path<String>,
    Query(query): Query<BookSnapshotQuery>,
) -> Result<Json<BookSnapshotResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let depth = query.depth.unwrap_or(10).clamp(1, 100);
    let response = state.engine.get_book_snapshot(&market_id, depth).await?;
    Ok(Json(response))
}

async fn get_my_wallet_balance_with_services<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    auth: AuthContext,
) -> Result<Json<WalletBalance>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let balance = state
        .wallet_ledger
        .get_wallet_balance(&auth.user_id)
        .await?;
    Ok(Json(balance))
}

async fn list_ledger_entries_with_services<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    _auth: AuthContext,
    Query(query): Query<LedgerEntriesQuery>,
) -> Result<Json<LedgerEntriesResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let entries = state
        .wallet_ledger
        .list_entries(&query.account_code, limit)
        .await?;
    Ok(Json(LedgerEntriesResponse { entries }))
}

async fn place_order_with_services<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    auth: AuthContext,
    Path(market_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PlaceOrderRequest>,
) -> Result<Json<OrderActionResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
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

    enforce_order_prechecks(
        &state.wallet_ledger,
        &state.kyc,
        state.market_fees.as_ref(),
        &auth.user_id,
        &market_id,
        &request,
    )
    .await?;

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

async fn cancel_order_with_services<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    auth: AuthContext,
    Path(order_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<OrderActionResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
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

async fn get_book_snapshot_with_services<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    Path(market_id): Path<String>,
    Query(query): Query<BookSnapshotQuery>,
) -> Result<Json<BookSnapshotResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let depth = query.depth.unwrap_or(10).clamp(1, 100);
    let response = state.engine.get_book_snapshot(&market_id, depth).await?;
    Ok(Json(response))
}

async fn get_my_affiliate_dashboard<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    auth: AuthContext,
    Query(query): Query<AffiliateQuery>,
) -> Result<Json<AffiliateDashboardResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let response = state
        .referral
        .get_affiliate_dashboard(&auth.user_id, limit)
        .await?;
    Ok(Json(response))
}

async fn capture_referral<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    Json(request): Json<CaptureReferralRequest>,
) -> Result<Json<serde_json::Value>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let response = state.referral.capture_referral(request).await?;
    Ok(Json(response))
}

async fn list_my_affiliate_referrals<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    auth: AuthContext,
    Query(query): Query<AffiliateQuery>,
) -> Result<Json<Vec<AffiliateReferralListItem>>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let referrals = state
        .referral
        .list_affiliate_referrals(&auth.user_id, limit)
        .await?;
    Ok(Json(referrals))
}

async fn get_my_affiliate_commission_summary<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    auth: AuthContext,
) -> Result<Json<AffiliateCommissionSummary>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let summary = state
        .referral
        .get_affiliate_commission_summary(&auth.user_id)
        .await?;
    Ok(Json(summary))
}

async fn list_admin_affiliates<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
) -> Result<Json<serde_json::Value>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let affiliates = state.referral.list_admin_affiliates().await?;
    Ok(Json(affiliates))
}

async fn get_admin_affiliate_detail<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    Path(affiliate_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let affiliate = state
        .referral
        .get_admin_affiliate_detail(&affiliate_id)
        .await?;
    Ok(Json(affiliate))
}

async fn create_admin_affiliate<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    headers: HeaderMap,
    Json(request): Json<serde_json::Value>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let context = admin_proxy_context(&headers)?;
    let affiliate = state
        .referral
        .create_admin_affiliate(request, context)
        .await?;
    Ok((StatusCode::CREATED, Json(affiliate)))
}

async fn update_admin_affiliate<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    headers: HeaderMap,
    Path(affiliate_id): Path<String>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let context = admin_proxy_context(&headers)?;
    let affiliate = state
        .referral
        .update_admin_affiliate(&affiliate_id, request, context)
        .await?;
    Ok(Json(affiliate))
}

async fn archive_admin_affiliate<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    headers: HeaderMap,
    Path(affiliate_id): Path<String>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let context = admin_proxy_context(&headers)?;
    let affiliate = state
        .referral
        .archive_admin_affiliate(&affiliate_id, request, context)
        .await?;
    Ok(Json(affiliate))
}

fn admin_proxy_context(headers: &HeaderMap) -> Result<AdminProxyContext, ApiError> {
    let actor_id = header_string(headers, "x-actor-id")
        .ok_or_else(|| ApiError::unauthorized("missing x-actor-id admin context"))?;
    let role = header_string(headers, "x-operator-role")
        .ok_or_else(|| ApiError::unauthorized("missing x-operator-role admin context"))?;
    Ok(AdminProxyContext {
        actor_id,
        role,
        request_id: header_string(headers, "x-request-id"),
    })
}

fn header_string(headers: &HeaderMap, name: &'static str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

async fn list_commission_adjustment_reviews<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
) -> Result<Json<serde_json::Value>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let reviews = state.referral.list_commission_adjustment_reviews().await?;
    Ok(Json(reviews))
}

async fn list_affiliate_payout_reviews<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
) -> Result<Json<serde_json::Value>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let reviews = state.referral.list_affiliate_payout_reviews().await?;
    Ok(Json(reviews))
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
    let payments_setup = payments_real_money_flag_setup_from_env().await?;
    let payment_flags = payments_setup.flags.clone();
    if let Some(pg_store) = payments_setup.pg_store {
        let _poller_handle = spawn_payments_real_money_flag_poller(
            pg_store,
            payments_setup.flags,
            sdk::feature_flags::DEFAULT_PAYMENTS_REAL_MONEY_POLL_INTERVAL,
        );
    }
    let market_service = HttpMarketServiceClient::from_env()?;
    let app = service_stub::router(&config)
        .merge(
            router_with_services_and_kyc_and_payment_flags_and_market_fees(
                HttpWalletLedgerClient::from_env()?,
                HttpMatchingEngineClient::from_env()?,
                HttpReferralClient::from_env()?,
                HttpKycServiceClient::from_env()?,
                payment_flags,
                market_service.clone(),
            ),
        )
        .merge(market_router(market_service))
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

/// Boot-time flag wiring: loads the current `payments.real_money_enabled`
/// row into an [`InMemoryFeatureFlagStore`] and optionally returns a
/// [`PgFeatureFlagStore`] handle for a background poller.
///
/// When `FEATURE_FLAGS_DATABASE_URL` (or `DATABASE_URL`) is unset:
/// - In production, returns Err — boot must fail closed.
/// - In dev/test, returns the in-memory store with flag = false and no
///   pg_store (caller skips poller spawn).
struct PaymentsRealMoneyFlagSetup {
    flags: InMemoryFeatureFlagStore,
    pg_store: Option<PgFeatureFlagStore>,
}

async fn payments_real_money_flag_setup_from_env()
-> anyhow::Result<PaymentsRealMoneyFlagSetup> {
    let flags = InMemoryFeatureFlagStore::default();
    let Some(database_url) = feature_flags_database_url() else {
        if is_production_env() {
            anyhow::bail!("FEATURE_FLAGS_DATABASE_URL or DATABASE_URL is required in production")
        }
        return Ok(PaymentsRealMoneyFlagSetup {
            flags,
            pg_store: None,
        });
    };

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .map_err(|error| anyhow::anyhow!("failed to connect feature flag database: {error}"))?;
    let pg_store = PgFeatureFlagStore::new(pool);

    match pg_store.load_payments_real_money_enabled().await {
        Ok(enabled) => {
            flags.set_payments_real_money_enabled(enabled);
        }
        Err(error) if is_production_env() => {
            return Err(anyhow::anyhow!(
                "failed to load payments real money feature flag: {error}"
            ));
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load payments real money feature flag; defaulting disabled");
        }
    }

    Ok(PaymentsRealMoneyFlagSetup {
        flags,
        pg_store: Some(pg_store),
    })
}

/// Spawns a background task that polls the `payments.real_money_enabled`
/// row from Postgres at a fixed interval and propagates the value into
/// `flags`. Implements SPEC-023 plan §11 D-6: flag flips take effect
/// without restarting api-gateway. On query failure the task logs at WARN
/// and keeps the previous value (fail-safe; never flips on errored read).
fn spawn_payments_real_money_flag_poller(
    pg_store: PgFeatureFlagStore,
    flags: InMemoryFeatureFlagStore,
    interval: std::time::Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(interval);
        // First tick fires immediately by default; skip it because the boot
        // path already performed the initial load.
        tick.tick().await;
        loop {
            tick.tick().await;
            match pg_store.load_payments_real_money_enabled().await {
                Ok(enabled) => flags.set_payments_real_money_enabled(enabled),
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "payments.real_money_enabled poll failed; keeping previous value"
                    );
                }
            }
        }
    })
}

fn feature_flags_database_url() -> Option<String> {
    env::var("FEATURE_FLAGS_DATABASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("DATABASE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
}

fn is_production_env() -> bool {
    env::var("PROPHETICA_ENV").is_ok_and(|value| value == "production")
}

async fn admin_ops_router() -> anyhow::Result<Router> {
    let upstream = admin_ops::HttpAdminOpsStore::from_env()
        .map_err(|error| anyhow::anyhow!(error.message().to_owned()))?;
    let database_url = env::var("ADMIN_OPS_DATABASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("DATABASE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
        });
    match database_url {
        Some(database_url) => {
            let pool = sqlx::PgPool::connect(&database_url).await?;
            run_postgres_migrations(
                sqlx::migrate!("./migrations"),
                &pool,
                API_GATEWAY_MIGRATION_TABLE,
            )
            .await?;
            Ok(admin_ops::internal_admin_router(
                admin_ops::ProductionAdminOpsStore::new(
                    upstream,
                    admin_ops::PostgresCompensationProposalStore::new(pool),
                ),
            ))
        }
        None if is_production_env() => {
            anyhow::bail!("ADMIN_OPS_DATABASE_URL or DATABASE_URL is required in production")
        }
        None => Ok(admin_ops::internal_admin_router(upstream)),
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
        Extension,
        body::Body,
        http::{Request, StatusCode},
    };
    use ed25519_dalek::{Signer, SigningKey};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use super::{
        AdminProxyContext, AffiliateCommissionSummary, AffiliateDashboardResponse,
        AffiliateReferralListItem, ApiError, AuthVerifier, BookLevel, BookSnapshotResponse,
        CaptureReferralRequest, DEFAULT_ORDER_FEE_BPS, InMemoryFeatureFlagStore, KycTierClient,
        LedgerEntry, Market, MarketListResponse, MarketServiceClient, MatchingEngineClient,
        OrderActionResponse, PlaceOrderRequest, ReferralClient, SERVICE_NAME, WalletBalance,
        WalletLedgerClient, X_USER_ID_TRANSITION_ALLOWLIST,
        X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON, X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
        market_router, observability, router, router_with_engine, router_with_engine_and_kyc,
        router_with_engine_and_kyc_and_market_fees, router_with_payment_flags,
        router_with_services,
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
    async fn payments_real_money_status_is_public_and_disabled_by_default() {
        let app = router(InMemoryWalletLedgerClient::default());

        let response = app
            .oneshot(empty_get("/v1/payments/real-money-status"))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response_json(response).await,
            json!({ "real_money_enabled": false })
        );
    }

    #[tokio::test]
    async fn payments_real_money_status_uses_configured_flag_reader() {
        let flags = InMemoryFeatureFlagStore::default();
        flags.set_payments_real_money_enabled(true);
        let app = router_with_payment_flags(InMemoryWalletLedgerClient::default(), flags);

        let response = app
            .oneshot(empty_get("/v1/payments/real-money-status"))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response_json(response).await,
            json!({ "real_money_enabled": true })
        );
    }

    #[tokio::test]
    async fn public_markets_proxy_allows_anonymous_reads_and_forwards_filters() {
        let client = InMemoryMarketServiceClient::default();
        let app = market_router(client.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/markets?status=open&category=sports")
                    .header("x-user-id", "forged-user")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["markets"][0]["id"], json!("mkt-election"));
        assert_eq!(
            client.requests(),
            vec![(Some("open".to_owned()), Some("sports".to_owned()))]
        );
    }

    #[tokio::test]
    async fn public_market_detail_proxy_allows_anonymous_reads() {
        let client = InMemoryMarketServiceClient::default();
        let app = market_router(client.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/markets/mkt-election")
                    .header("x-user-id", "forged-user")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["id"], json!("mkt-election"));
        assert_eq!(client.detail_requests(), vec!["mkt-election".to_owned()]);
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
    async fn place_order_rejects_tier_zero_before_matching_engine() {
        let engine = InMemoryMatchingEngineClient::default();
        let kyc = InMemoryKycTierClient::with_tier("TIER_0");
        let app =
            router_with_engine_and_kyc(InMemoryWalletLedgerClient::default(), engine.clone(), kyc);

        let response = app
            .oneshot(order_post("idem-kyc-tier-zero", &order_request_body(3)))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("ERROR_CODE_KYC_TIER_INSUFFICIENT"));
        assert_eq!(body["required_tier"], json!("TIER_1"));
        assert_eq!(body["redirect_to"], json!("/kyc/tier-1"));
        assert_eq!(engine.place_requests(), Vec::new());
    }

    #[tokio::test]
    async fn place_order_fails_closed_when_kyc_lookup_is_unavailable() {
        let engine = InMemoryMatchingEngineClient::default();
        let kyc = InMemoryKycTierClient::unavailable();
        let app =
            router_with_engine_and_kyc(InMemoryWalletLedgerClient::default(), engine.clone(), kyc);

        let response = app
            .oneshot(order_post("idem-kyc-unavailable", &order_request_body(3)))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("ERROR_CODE_KYC_LOOKUP_UNAVAILABLE"));
        assert_eq!(engine.place_requests(), Vec::new());
    }

    #[tokio::test]
    async fn place_order_rejects_insufficient_funds_for_buy_before_matching_engine() {
        let wallet = InMemoryWalletLedgerClient::with_available_minor_units(100);
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine_and_kyc(
            wallet,
            engine.clone(),
            InMemoryKycTierClient::with_tier("TIER_1"),
        );

        let response = app
            .oneshot(order_post(
                "idem-insufficient-funds",
                &json!({
                    "side": "buy",
                    "outcome": "YES",
                    "type": "LIMIT",
                    "price_pts": 65,
                    "quantity": 10,
                    "time_in_force": "GTC"
                }),
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("ERROR_CODE_INSUFFICIENT_FUNDS"));
        assert_eq!(body["required_minor_units"], json!(663));
        assert_eq!(body["available_minor_units"], json!(100));
        assert_eq!(body["redirect_to"], json!("/topup"));
        assert_eq!(engine.place_requests(), Vec::new());
        let metrics = observability::prometheus_bootstrap_metrics(SERVICE_NAME);
        assert!(metrics.contains("order_precheck_rejected_total"));
        assert!(metrics.contains(r#"reason="insufficient_funds""#));
    }

    #[tokio::test]
    async fn place_order_uses_market_fee_bps_cap_for_buy_balance_precheck() {
        let wallet = InMemoryWalletLedgerClient::with_available_minor_units(665);
        let engine = InMemoryMatchingEngineClient::default();
        let market_service = InMemoryMarketServiceClient::with_fee_bps(300);
        let app = router_with_engine_and_kyc_and_market_fees(
            wallet,
            engine.clone(),
            InMemoryKycTierClient::with_tier("TIER_1"),
            market_service.clone(),
        );

        let response = app
            .oneshot(order_post(
                "idem-market-fee-bps",
                &json!({
                    "side": "buy",
                    "outcome": "YES",
                    "type": "LIMIT",
                    "price_pts": 65,
                    "quantity": 10,
                    "time_in_force": "GTC"
                }),
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("ERROR_CODE_INSUFFICIENT_FUNDS"));
        assert_eq!(body["required_minor_units"], json!(669));
        assert_eq!(body["available_minor_units"], json!(665));
        assert_eq!(body["redirect_to"], json!("/topup"));
        assert_eq!(market_service.detail_requests(), vec!["mkt-election"]);
        assert_eq!(engine.place_requests(), Vec::new());
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
    async fn strict_auth_rejects_forged_user_id_without_bearer() {
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine(InMemoryWalletLedgerClient::default(), engine.clone())
            .layer(test_auth_extension());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/markets/mkt-election/orders")
                    .header("x-user-id", "forged_user")
                    .header("idempotency-key", "idem-forged-no-bearer")
                    .header("content-type", "application/json")
                    .body(Body::from(order_request_body(3).to_string()))
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(engine.place_requests(), Vec::new());
        let metrics = observability::prometheus_bootstrap_metrics(SERVICE_NAME);
        assert!(metrics.contains("gateway_auth_validation_failed_total"));
        assert!(metrics.contains(r#"reason="missing_bearer""#));
    }

    #[test]
    fn x_user_id_transition_allowlist_is_explicit_and_time_limited() {
        let actual = X_USER_ID_TRANSITION_ALLOWLIST
            .iter()
            .map(|entry| {
                (
                    entry.caller,
                    entry.method,
                    entry.path_pattern,
                    entry.expires_on,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            actual,
            vec![
                (
                    X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
                    "GET",
                    "/v1/me/wallet/balance",
                    X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
                ),
                (
                    X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
                    "GET",
                    "/v1/ledger/entries",
                    X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
                ),
                (
                    X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
                    "POST",
                    "/v1/markets/:market_id/orders",
                    X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
                ),
                (
                    X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
                    "GET",
                    "/v1/account/affiliate",
                    X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
                ),
                (
                    X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
                    "GET",
                    "/v1/account/affiliate/referrals",
                    X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
                ),
                (
                    X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
                    "GET",
                    "/v1/account/affiliate/commission-summary",
                    X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
                ),
                (
                    X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
                    "GET",
                    "/v1/me/responsible-gaming/cooldown",
                    X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
                ),
                (
                    X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
                    "POST",
                    "/v1/me/responsible-gaming/cooldown",
                    X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
                ),
                (
                    X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
                    "GET",
                    "/v1/responsible-gaming/self-exclusion",
                    X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
                ),
                (
                    X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
                    "POST",
                    "/v1/responsible-gaming/self-exclusion",
                    X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON,
                ),
            ]
        );
        assert!(actual.iter().all(|(_, _, path, expires_on)| {
            !path.contains('*') && *expires_on <= "2026-08-31"
        }));
    }

    #[test]
    fn x_user_id_transition_allowlist_matches_only_named_caller_routes() {
        assert!(super::x_user_id_transition_fallback_allowed_for(
            X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
            "POST",
            "/v1/markets/mkt-election/orders",
        ));
        assert!(!super::x_user_id_transition_fallback_allowed_for(
            "unknown-caller",
            "POST",
            "/v1/markets/mkt-election/orders",
        ));
        assert!(!super::x_user_id_transition_fallback_allowed_for(
            X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
            "DELETE",
            "/v1/orders/ord_123",
        ));
    }

    #[tokio::test]
    async fn strict_auth_uses_verified_bearer_subject_not_forged_header() {
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine(InMemoryWalletLedgerClient::default(), engine.clone())
            .layer(test_auth_extension());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/markets/mkt-election/orders")
                    .header(
                        "authorization",
                        format!("Bearer {}", test_jwt("token_user")),
                    )
                    .header("x-user-id", "forged_user")
                    .header("idempotency-key", "idem-bearer-over-header")
                    .header("content-type", "application/json")
                    .body(Body::from(order_request_body(3).to_string()))
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            engine.place_requests(),
            vec![(
                "token_user".to_owned(),
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
        let metrics = observability::prometheus_bootstrap_metrics(SERVICE_NAME);
        assert!(metrics.contains("gateway_principal_header_stripped_total"));
    }

    #[tokio::test]
    async fn strict_auth_rejects_expired_bearer() {
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine(InMemoryWalletLedgerClient::default(), engine.clone())
            .layer(test_auth_extension());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/markets/mkt-election/orders")
                    .header(
                        "authorization",
                        format!("Bearer {}", expired_test_jwt("token_user")),
                    )
                    .header("idempotency-key", "idem-expired-bearer")
                    .header("content-type", "application/json")
                    .body(Body::from(order_request_body(3).to_string()))
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(engine.place_requests(), Vec::new());
    }

    #[tokio::test]
    async fn strict_auth_rejects_bearer_signed_by_unknown_key() {
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine(InMemoryWalletLedgerClient::default(), engine.clone())
            .layer(test_auth_extension());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/markets/mkt-election/orders")
                    .header(
                        "authorization",
                        format!(
                            "Bearer {}",
                            test_jwt_with_seed("token_user", [8; 32], 1_900_000_000)
                        ),
                    )
                    .header("idempotency-key", "idem-unknown-key")
                    .header("content-type", "application/json")
                    .body(Body::from(order_request_body(3).to_string()))
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(engine.place_requests(), Vec::new());
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
    async fn internal_referral_capture_proxies_to_referral_service() {
        let referral = InMemoryReferralClient::default();
        let app = router_with_services(
            InMemoryWalletLedgerClient::default(),
            InMemoryMatchingEngineClient::default(),
            referral.clone(),
        );

        let response = app
            .oneshot(admin_json_request(
                "POST",
                "/api/internal/referrals/capture",
                &json!({
                    "referral_code": "AFF-100",
                    "referred_user_id": "MEM-900",
                    "referred_at_day": 20_000
                }),
            ))
            .await
            .expect("capture should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            referral.capture_requests(),
            vec![CaptureReferralRequest {
                referral_code: "AFF-100".to_owned(),
                referred_user_id: "MEM-900".to_owned(),
                referred_at_day: Some(20_000),
            }]
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

    #[tokio::test]
    async fn affiliate_admin_review_queues_proxy_referral_service() {
        let app = router_with_services(
            InMemoryWalletLedgerClient::default(),
            InMemoryMatchingEngineClient::default(),
            InMemoryReferralClient::default(),
        );

        let commission_reviews = app
            .clone()
            .oneshot(empty_get(
                "/api/internal/admin/affiliate-commission-adjustments",
            ))
            .await
            .expect("request should be handled");
        assert_eq!(commission_reviews.status(), StatusCode::OK);
        let body = response_json(commission_reviews).await;
        assert_eq!(body[0]["id"], json!("COM-2026-0514-001"));
        assert_eq!(body[0]["affiliateId"], json!("AFF-001"));

        let payout_reviews = app
            .oneshot(empty_get("/api/internal/admin/affiliate-payouts"))
            .await
            .expect("request should be handled");
        assert_eq!(payout_reviews.status(), StatusCode::OK);
        let body = response_json(payout_reviews).await;
        assert_eq!(body[0]["id"], json!("PAY-2026-0514-001"));
        assert_eq!(body[0]["status"], json!("pending"));
    }

    #[tokio::test]
    async fn affiliate_admin_crud_proxies_actor_context() {
        let app = router_with_services(
            InMemoryWalletLedgerClient::default(),
            InMemoryMatchingEngineClient::default(),
            InMemoryReferralClient::default(),
        );

        let created = app
            .clone()
            .oneshot(admin_json_request(
                "POST",
                "/api/internal/admin/affiliates",
                &json!({
                    "id": "AFF-200",
                    "displayName": "Orion Partners",
                    "reason": "onboarding",
                    "authorizationBasis": "SPEC-011"
                }),
            ))
            .await
            .expect("create should be handled");
        assert_eq!(created.status(), StatusCode::CREATED);
        let body = response_json(created).await;
        assert_eq!(body["affiliate"]["id"], "AFF-200");
        assert_eq!(body["auditCorrelationId"], "audit:affiliate-create:admin-1");

        let updated = app
            .clone()
            .oneshot(admin_json_request(
                "PATCH",
                "/api/internal/admin/affiliates/AFF-200",
                &json!({
                    "commissionRateBps": 700,
                    "reason": "rate adjustment",
                    "authorizationBasis": "SPEC-011"
                }),
            ))
            .await
            .expect("update should be handled");
        assert_eq!(updated.status(), StatusCode::OK);
        assert_eq!(response_json(updated).await["affiliate"]["id"], "AFF-200");

        let archived = app
            .oneshot(admin_json_request(
                "DELETE",
                "/api/internal/admin/affiliates/AFF-200",
                &json!({
                    "reason": "offboarding",
                    "authorizationBasis": "termination notice"
                }),
            ))
            .await
            .expect("delete should be handled");
        assert_eq!(archived.status(), StatusCode::OK);
        assert_eq!(
            response_json(archived).await["affiliate"]["status"],
            "archived"
        );
    }

    fn empty_get(uri: &str) -> Request<Body> {
        Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap_or_else(|error| panic!("request should build: {error}"))
    }

    fn admin_json_request(method: &str, uri: &str, body: &Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("x-actor-id", "admin-1")
            .header("x-operator-role", "admin")
            .header("x-request-id", "req-1")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
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

    fn test_jwks_json() -> String {
        let key = test_signing_key();
        json!({
            "keys": [{
                "kty": "OKP",
                "crv": "Ed25519",
                "kid": "test-key",
                "alg": "EdDSA",
                "use": "sig",
                "x": base64_url_encode(&key.verifying_key().to_bytes())
            }]
        })
        .to_string()
    }

    fn test_auth_extension() -> Extension<AuthVerifier> {
        Extension(
            AuthVerifier::from_jwks_json(&test_jwks_json(), true)
                .unwrap_or_else(|error| panic!("test JWKS should parse: {error}")),
        )
    }

    fn test_jwt(subject: &str) -> String {
        test_jwt_with_seed(subject, [7; 32], 1_900_000_000)
    }

    fn expired_test_jwt(subject: &str) -> String {
        test_jwt_with_seed(subject, [7; 32], 1)
    }

    fn test_jwt_with_seed(subject: &str, seed: [u8; 32], exp: u64) -> String {
        let key = SigningKey::from_bytes(&seed);
        let header = base64_url_encode(
            json!({
                "alg": "EdDSA",
                "kid": "test-key",
                "typ": "JWT"
            })
            .to_string()
            .as_bytes(),
        );
        let payload = base64_url_encode(
            json!({
                "sub": subject,
                "role": "user",
                "iat": 1_800_000_000_u64,
                "exp": exp
            })
            .to_string()
            .as_bytes(),
        );
        let signing_input = format!("{header}.{payload}");
        let signature = key.sign(signing_input.as_bytes());
        format!(
            "{signing_input}.{}",
            base64_url_encode(&signature.to_bytes())
        )
    }

    fn test_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7; 32])
    }

    fn base64_url_encode(bytes: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
        let mut index = 0;
        while index < bytes.len() {
            let b0 = bytes[index];
            let b1 = bytes.get(index + 1).copied();
            let b2 = bytes.get(index + 2).copied();
            out.push(TABLE[(b0 >> 2) as usize] as char);
            out.push(
                TABLE[(((b0 & 0b0000_0011) << 4) | (b1.unwrap_or_default() >> 4)) as usize] as char,
            );
            if let Some(b1) = b1 {
                out.push(
                    TABLE[(((b1 & 0b0000_1111) << 2) | (b2.unwrap_or_default() >> 6)) as usize]
                        as char,
                );
            }
            if let Some(b2) = b2 {
                out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
            }
            index += 3;
        }
        out
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
        available_minor_units: Option<i64>,
    }

    #[derive(Clone, Default)]
    struct InMemoryMatchingEngineClient {
        placed_orders: Arc<Mutex<Vec<(String, String, PlaceOrderRequest)>>>,
        canceled_orders: Arc<Mutex<Vec<(String, String)>>>,
        snapshot_reads: Arc<Mutex<Vec<(String, usize)>>>,
    }

    #[derive(Clone, Default)]
    struct InMemoryReferralClient {
        captures: Arc<Mutex<Vec<CaptureReferralRequest>>>,
        dashboards: Arc<Mutex<Vec<(String, usize)>>>,
        referral_lists: Arc<Mutex<Vec<(String, usize)>>>,
        summaries: Arc<Mutex<Vec<String>>>,
    }

    type MarketListRequests = Arc<Mutex<Vec<(Option<String>, Option<String>)>>>;

    #[derive(Clone, Default)]
    struct InMemoryMarketServiceClient {
        requests: MarketListRequests,
        detail_requests: Arc<Mutex<Vec<String>>>,
        fee_bps: Option<u32>,
    }

    #[derive(Clone)]
    struct InMemoryKycTierClient {
        tier: Option<String>,
    }

    impl Default for InMemoryKycTierClient {
        fn default() -> Self {
            Self {
                tier: Some("TIER_1".to_owned()),
            }
        }
    }

    impl InMemoryKycTierClient {
        fn with_tier(tier: &str) -> Self {
            Self {
                tier: Some(tier.to_owned()),
            }
        }

        fn unavailable() -> Self {
            Self { tier: None }
        }
    }

    impl InMemoryMarketServiceClient {
        fn with_fee_bps(fee_bps: u32) -> Self {
            Self {
                fee_bps: Some(fee_bps),
                ..Self::default()
            }
        }

        fn requests(&self) -> Vec<(Option<String>, Option<String>)> {
            self.requests
                .lock()
                .unwrap_or_else(|error| panic!("market requests lock should not poison: {error}"))
                .clone()
        }

        fn detail_requests(&self) -> Vec<String> {
            self.detail_requests
                .lock()
                .unwrap_or_else(|error| {
                    panic!("market detail requests lock should not poison: {error}")
                })
                .clone()
        }
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
        fn with_available_minor_units(available_minor_units: i64) -> Self {
            Self {
                available_minor_units: Some(available_minor_units),
                ..Self::default()
            }
        }

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
        fn capture_requests(&self) -> Vec<CaptureReferralRequest> {
            self.captures
                .lock()
                .unwrap_or_else(|error| panic!("capture requests lock should not poison: {error}"))
                .clone()
        }

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
    impl MarketServiceClient for InMemoryMarketServiceClient {
        async fn list_markets(
            &self,
            status: Option<&str>,
            category: Option<&str>,
        ) -> Result<MarketListResponse, ApiError> {
            self.requests
                .lock()
                .unwrap_or_else(|error| panic!("market requests lock should not poison: {error}"))
                .push((status.map(str::to_owned), category.map(str::to_owned)));

            Ok(MarketListResponse {
                markets: vec![self.market()],
            })
        }

        async fn get_market(&self, market_id: &str) -> Result<Market, ApiError> {
            self.detail_requests
                .lock()
                .unwrap_or_else(|error| {
                    panic!("market detail requests lock should not poison: {error}")
                })
                .push(market_id.to_owned());

            Ok(self.market())
        }
    }

    impl InMemoryMarketServiceClient {
        fn market(&self) -> Market {
            Market {
                fee_bps: Some(self.fee_bps.unwrap_or(DEFAULT_ORDER_FEE_BPS)),
                ..test_market()
            }
        }
    }

    fn test_market() -> Market {
        Market {
            id: "mkt-election".to_owned(),
            title: "Election market".to_owned(),
            brief: "Election outcome".to_owned(),
            category: "sports".to_owned(),
            status: "open".to_owned(),
            yes_price_cents: 52,
            yes_bid_cents: 51,
            yes_ask_cents: 53,
            fee_bps: Some(200),
            volume_24h_points: 1_200,
            volume_total_points: 12_000,
            opened_at: "2026-05-20T00:00:00Z".to_owned(),
            close_at: "2026-06-01T00:00:00Z".to_owned(),
            oracle_source: "official".to_owned(),
            resolved_outcome: None,
            resolved_at: None,
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
                available_minor_units: self.available_minor_units.unwrap_or(-10_000),
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
    impl KycTierClient for InMemoryKycTierClient {
        async fn get_kyc_tier(&self, _user_id: &str) -> Result<String, ApiError> {
            self.tier
                .clone()
                .ok_or_else(|| ApiError::kyc_lookup_unavailable("KYC service unavailable"))
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
        async fn capture_referral(
            &self,
            request: CaptureReferralRequest,
        ) -> Result<serde_json::Value, ApiError> {
            self.captures
                .lock()
                .unwrap_or_else(|error| panic!("capture requests lock should not poison: {error}"))
                .push(request.clone());
            Ok(serde_json::json!({
                "referrer_id": "user_aff_100",
                "referred_id": request.referred_user_id,
                "referral_code": request.referral_code,
                "captured": true
            }))
        }

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

        async fn list_admin_affiliates(&self) -> Result<serde_json::Value, ApiError> {
            Ok(serde_json::json!([{
                "id": "AFF-001",
                "displayName": "Crescent Partners"
            }]))
        }

        async fn get_admin_affiliate_detail(
            &self,
            affiliate_id: &str,
        ) -> Result<serde_json::Value, ApiError> {
            Ok(serde_json::json!({
                "id": affiliate_id,
                "displayName": "Crescent Partners"
            }))
        }

        async fn create_admin_affiliate(
            &self,
            request: serde_json::Value,
            context: AdminProxyContext,
        ) -> Result<serde_json::Value, ApiError> {
            Ok(serde_json::json!({
                "affiliate": request,
                "auditCorrelationId": format!("audit:affiliate-create:{}", context.actor_id)
            }))
        }

        async fn update_admin_affiliate(
            &self,
            affiliate_id: &str,
            request: serde_json::Value,
            context: AdminProxyContext,
        ) -> Result<serde_json::Value, ApiError> {
            Ok(serde_json::json!({
                "affiliate": {
                    "id": affiliate_id,
                    "patch": request
                },
                "auditCorrelationId": format!("audit:affiliate-update:{}", context.actor_id)
            }))
        }

        async fn archive_admin_affiliate(
            &self,
            affiliate_id: &str,
            _request: serde_json::Value,
            context: AdminProxyContext,
        ) -> Result<serde_json::Value, ApiError> {
            Ok(serde_json::json!({
                "affiliate": {
                    "id": affiliate_id,
                    "status": "archived"
                },
                "auditCorrelationId": format!("audit:affiliate-archive:{}", context.actor_id)
            }))
        }

        async fn list_commission_adjustment_reviews(&self) -> Result<serde_json::Value, ApiError> {
            Ok(serde_json::json!([{
                "id": "COM-2026-0514-001",
                "affiliateId": "AFF-001",
                "adjustmentType": "credit",
                "amountMinor": 42000,
                "currency": "USDT",
                "reason": "活動轉換補發",
                "status": "pending",
                "requestedBy": "admin_ops",
                "requestedAt": "2026-05-14 09:20"
            }]))
        }

        async fn list_affiliate_payout_reviews(&self) -> Result<serde_json::Value, ApiError> {
            Ok(serde_json::json!([{
                "id": "PAY-2026-0514-001",
                "affiliateId": "AFF-001",
                "amountMinor": 128_400,
                "currency": "USDT",
                "requestedAt": "2026-05-14 10:05",
                "status": "pending"
            }]))
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
