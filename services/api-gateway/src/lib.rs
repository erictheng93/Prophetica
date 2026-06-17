use std::{
    collections::HashMap,
    env,
    fmt::Write as _,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

use async_trait::async_trait;
use axum::{
    Extension, Json, Router,
    extract::{FromRequestParts, Path, Query, State},
    http::{HeaderMap, StatusCode, header::HeaderName, request::Parts},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use ed25519_dalek::{Verifier, VerifyingKey};
use internal_kyc_auth::{
    INTERNAL_CALL_SIGNATURE_HEADER, INTERNAL_CALL_TIMESTAMP_HEADER, sign_internal_call_for_kyc,
};
use order_precheck::{
    OrderKind, OrderSide, buy_order_lock_minor_units, curve_taker_fee_bps,
    curve_taker_fee_minor_units, required_minor_units,
};
use prometheus::{IntCounter, IntCounterVec, Opts};
use redemption_service::{
    cs_refund::CsRefundProposal,
    entitlement::Entitlement,
    service::{OrderView, SubmitOrderRequest},
};
use reqwest::Url;
use sdk::{
    config::ServiceConfig,
    feature_flags::{InMemoryFeatureFlagStore, PaymentsRealMoneyFlagReader, PgFeatureFlagStore},
    ledger::DEFAULT_TAKER_FEE_CURVE_BPS,
    observability,
    postgres_migrations::{API_GATEWAY_MIGRATION_TABLE, run_postgres_migrations},
    service_stub,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
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
const DEFAULT_KYC_SERVICE_BASE_URL: &str = "http://127.0.0.1:8085";
const DEFAULT_POSITION_SERVICE_BASE_URL: &str = "http://127.0.0.1:8089";
const DEFAULT_REDEMPTION_SERVICE_BASE_URL: &str = "http://127.0.0.1:8095";
const DEFAULT_TRADE_FILL_SAGA_RETRY_INTERVAL: Duration = Duration::from_secs(5);
const TRADE_FILL_SAGA_BATCH_LIMIT: i64 = 50;

pub const X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF: &str = "apps-web-dev-bff";
pub const X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON: &str = "2026-08-31";

fn resolve_kyc_service_base_url(
    kyc_service_base_url: Option<&str>,
    kyc_service_url: Option<&str>,
) -> String {
    kyc_service_base_url
        .filter(|value| !value.trim().is_empty())
        .or_else(|| kyc_service_url.filter(|value| !value.trim().is_empty()))
        .unwrap_or(DEFAULT_KYC_SERVICE_BASE_URL)
        .trim_end_matches('/')
        .to_owned()
}

pub(crate) fn kyc_service_base_url_from_env() -> String {
    resolve_kyc_service_base_url(
        env::var("KYC_SERVICE_BASE_URL").ok().as_deref(),
        env::var("KYC_SERVICE_URL").ok().as_deref(),
    )
}

fn resolve_redemption_service_base_url(
    redemption_service_base_url: Option<&str>,
    redemption_service_url: Option<&str>,
) -> String {
    redemption_service_base_url
        .filter(|value| !value.trim().is_empty())
        .or_else(|| redemption_service_url.filter(|value| !value.trim().is_empty()))
        .unwrap_or(DEFAULT_REDEMPTION_SERVICE_BASE_URL)
        .trim_end_matches('/')
        .to_owned()
}

pub(crate) fn redemption_service_base_url_from_env() -> String {
    resolve_redemption_service_base_url(
        env::var("REDEMPTION_SERVICE_BASE_URL").ok().as_deref(),
        env::var("REDEMPTION_SERVICE_URL").ok().as_deref(),
    )
}

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
    position_projection: Arc<dyn PositionProjectionClient + Send + Sync>,
    idempotency: IdempotencyStore,
    order_owners: Arc<dyn OrderOwnerRepository>,
    trade_sagas: Arc<dyn TradeFillSagaStore>,
}

#[derive(Clone)]
struct TradeFillStores {
    order_owners: Arc<dyn OrderOwnerRepository>,
    trade_sagas: Arc<dyn TradeFillSagaStore>,
}

impl TradeFillStores {
    fn in_memory() -> Self {
        Self {
            order_owners: Arc::new(OrderOwnerStore::default()),
            trade_sagas: Arc::new(InMemoryTradeFillSagaStore::default()),
        }
    }
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
        Self::new_with_market_fees_and_position_projection(
            wallet_ledger,
            engine,
            kyc,
            market_fees,
            NoopPositionProjectionClient,
        )
    }

    #[must_use]
    pub fn new_with_market_fees_and_position_projection<M, P>(
        wallet_ledger: W,
        engine: E,
        kyc: K,
        market_fees: M,
        position_projection: P,
    ) -> Self
    where
        M: MarketFeeReader + Send + Sync + 'static,
        P: PositionProjectionClient + Send + Sync + 'static,
    {
        Self::new_with_market_fees_position_projection_and_trade_stores(
            wallet_ledger,
            engine,
            kyc,
            market_fees,
            position_projection,
            TradeFillStores::in_memory(),
        )
    }

    fn new_with_market_fees_position_projection_and_trade_stores<M, P>(
        wallet_ledger: W,
        engine: E,
        kyc: K,
        market_fees: M,
        position_projection: P,
        trade_stores: TradeFillStores,
    ) -> Self
    where
        M: MarketFeeReader + Send + Sync + 'static,
        P: PositionProjectionClient + Send + Sync + 'static,
    {
        Self {
            wallet_ledger,
            engine,
            kyc,
            market_fees: Arc::new(market_fees),
            position_projection: Arc::new(position_projection),
            idempotency: IdempotencyStore::default(),
            order_owners: trade_stores.order_owners,
            trade_sagas: trade_stores.trade_sagas,
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
    position_projection: Arc<dyn PositionProjectionClient + Send + Sync>,
    idempotency: IdempotencyStore,
    order_owners: Arc<dyn OrderOwnerRepository>,
    trade_sagas: Arc<dyn TradeFillSagaStore>,
    affiliate_credentials: Arc<dyn AffiliateCredentialClient>,
    affiliate_account_sagas: Arc<dyn AffiliateAccountSagaStore>,
    gateway_denied_audits: Arc<dyn GatewayDeniedAuditStore>,
}

#[derive(Clone)]
struct AffiliateAdminStores {
    affiliate_credentials: Arc<dyn AffiliateCredentialClient>,
    affiliate_account_sagas: Arc<dyn AffiliateAccountSagaStore>,
    gateway_denied_audits: Arc<dyn GatewayDeniedAuditStore>,
}

impl AffiliateAdminStores {
    fn in_memory_noop() -> Self {
        Self {
            affiliate_credentials: Arc::new(NoopAffiliateCredentialClient),
            affiliate_account_sagas: Arc::new(InMemoryAffiliateAccountSagaStore::default()),
            gateway_denied_audits: Arc::new(InMemoryGatewayDeniedAuditStore::default()),
        }
    }
}

#[derive(Clone)]
struct GatewayStateStores {
    trade: TradeFillStores,
    affiliate_admin: AffiliateAdminStores,
}

impl GatewayStateStores {
    fn in_memory_noop() -> Self {
        Self {
            trade: TradeFillStores::in_memory(),
            affiliate_admin: AffiliateAdminStores::in_memory_noop(),
        }
    }
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
        Self::new_with_market_fees_and_position_projection(
            wallet_ledger,
            engine,
            referral,
            kyc,
            market_fees,
            NoopPositionProjectionClient,
        )
    }

    #[must_use]
    pub fn new_with_market_fees_and_position_projection<M, P>(
        wallet_ledger: W,
        engine: E,
        referral: R,
        kyc: K,
        market_fees: M,
        position_projection: P,
    ) -> Self
    where
        M: MarketFeeReader + Send + Sync + 'static,
        P: PositionProjectionClient + Send + Sync + 'static,
    {
        Self::new_with_market_fees_position_projection_and_trade_stores(
            wallet_ledger,
            engine,
            referral,
            kyc,
            market_fees,
            position_projection,
            GatewayStateStores::in_memory_noop(),
        )
    }

    fn new_with_market_fees_position_projection_and_trade_stores<M, P>(
        wallet_ledger: W,
        engine: E,
        referral: R,
        kyc: K,
        market_fees: M,
        position_projection: P,
        stores: GatewayStateStores,
    ) -> Self
    where
        M: MarketFeeReader + Send + Sync + 'static,
        P: PositionProjectionClient + Send + Sync + 'static,
    {
        Self {
            wallet_ledger,
            engine,
            referral,
            kyc,
            market_fees: Arc::new(market_fees),
            position_projection: Arc::new(position_projection),
            idempotency: IdempotencyStore::default(),
            order_owners: stores.trade.order_owners,
            trade_sagas: stores.trade.trade_sagas,
            affiliate_credentials: stores.affiliate_admin.affiliate_credentials,
            affiliate_account_sagas: stores.affiliate_admin.affiliate_account_sagas,
            gateway_denied_audits: stores.affiliate_admin.gateway_denied_audits,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct IdempotencyStore {
    records: Arc<Mutex<HashMap<IdempotencyScope, IdempotencyRecord>>>,
}

#[derive(Debug, Clone, Default)]
struct OrderOwnerStore {
    owners: Arc<Mutex<HashMap<String, String>>>,
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
struct OrderOwnerRecord {
    order: String,
    user: String,
    market: String,
    matching_user: String,
}

#[async_trait]
trait OrderOwnerRepository: Send + Sync {
    async fn record_owner(&self, owner: OrderOwnerRecord) -> Result<(), ApiError>;

    async fn find_owner(&self, order_id: &str) -> Result<Option<String>, ApiError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TradeFillSagaStatus {
    Pending,
    Running,
    Retryable,
    Completed,
}

impl TradeFillSagaStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Retryable => "retryable",
            Self::Completed => "completed",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "retryable" => Self::Retryable,
            "completed" => Self::Completed,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TradeFillSaga {
    fill_key: String,
    market_id: String,
    fill_id: String,
    taker_user_id: String,
    maker_user_id: String,
    buyer_user_id: String,
    seller_user_id: String,
    taker_order_id: String,
    maker_order_id: String,
    taker_side: String,
    maker_side: String,
    outcome: String,
    price_pts: i32,
    quantity: i64,
    fee_curve_bps: u32,
    reservation_txn_id: Option<i64>,
    ledger_txn_id: Option<i64>,
    settlement_idempotency_key: String,
    taker_projection_idempotency_key: String,
    maker_projection_idempotency_key: String,
    status: TradeFillSagaStatus,
    attempts: i32,
    settlement_posted: bool,
    taker_projected: bool,
    maker_projected: bool,
}

#[async_trait]
trait TradeFillSagaStore: Send + Sync {
    async fn enqueue(&self, saga: TradeFillSaga) -> Result<(), ApiError>;

    async fn claim_due(&self, limit: i64) -> Result<Vec<TradeFillSaga>, ApiError>;

    async fn mark_settlement_posted(
        &self,
        fill_key: &str,
        ledger_txn_id: Option<i64>,
    ) -> Result<(), ApiError>;

    async fn mark_taker_projected(&self, fill_key: &str) -> Result<(), ApiError>;

    async fn mark_maker_projected(&self, fill_key: &str) -> Result<(), ApiError>;

    async fn mark_completed(&self, fill_key: &str) -> Result<(), ApiError>;

    async fn mark_retryable(&self, fill_key: &str, error: &str) -> Result<(), ApiError>;
}

#[derive(Debug, Clone)]
struct PostgresOrderOwnerStore {
    pool: sqlx::PgPool,
}

#[derive(Debug, Clone)]
struct PostgresTradeFillSagaStore {
    pool: sqlx::PgPool,
}

#[derive(Debug, Clone, Default)]
struct InMemoryTradeFillSagaStore {
    sagas: Arc<Mutex<HashMap<String, TradeFillSaga>>>,
}

impl PostgresOrderOwnerStore {
    const fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

impl PostgresTradeFillSagaStore {
    const fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
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
        // SPEC-023 plan §11 D-1 / Phase 0 review I-3: when the JWKS holds more
        // than one key (e.g. during rotation), a token without `kid` would
        // otherwise be tried against every key, letting a single leaked key
        // forge tokens that other keys would reject. Require an explicit kid
        // whenever the keyset is non-singleton.
        if kid.is_none() && self.keys.len() > 1 {
            return Err(());
        }
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

    fn not_found(error: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, error, message)
    }

    fn conflict(error: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, error, message)
    }

    fn forbidden(error: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, error, message)
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

    fn redemption_service(message: impl Into<String>) -> Self {
        Self::redemption_service_status(StatusCode::BAD_GATEWAY, message)
    }

    fn market_service(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_GATEWAY,
            "market_service_unavailable",
            message,
        )
    }

    fn redemption_service_status(status: StatusCode, message: impl Into<String>) -> Self {
        Self::new(status, "redemption_service_unavailable", message)
    }

    fn position_service(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_GATEWAY,
            "position_service_unavailable",
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

    fn kyc_tier_insufficient(current_tier: KycTier) -> Self {
        let tier_str = match serde_json::to_value(current_tier) {
            Ok(value) => value,
            Err(_) => serde_json::Value::Null,
        };
        Self::new(
            StatusCode::FORBIDDEN,
            "ERROR_CODE_KYC_TIER_INSUFFICIENT",
            "KYC tier is below the minimum required for trading",
        )
        .with_detail("current_tier", tier_str)
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
pub struct Position {
    pub id: String,
    pub user_id: String,
    pub market_id: String,
    pub outcome: String,
    pub qty: i64,
    pub avg_price_pts: i32,
    pub locked_amount: i64,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub settle_txn_id: Option<i64>,
    pub realized_pnl_points: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionListResponse {
    pub positions: Vec<Position>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PositionListQuery {
    pub status: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_curve_bps: Option<u32>,
}

impl PlaceOrderRequest {
    fn with_fee_curve_bps(&self, fee_curve_bps: u32) -> Self {
        let mut request = self.clone();
        request.fee_curve_bps = Some(fee_curve_bps);
        request
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderActionResponse {
    pub order_id: String,
    pub status: String,
    pub filled_qty: i64,
    pub avg_price_pts: i32,
    pub remaining_qty: i64,
    pub fee_pts: i64,
    #[serde(default)]
    pub fills: Vec<OrderFill>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderFill {
    pub fill_id: String,
    pub market_id: String,
    pub seq_no: u64,
    pub price_pts: i32,
    pub quantity: i64,
    pub taker_order_id: String,
    pub maker_order_id: String,
    pub taker_user_id: u64,
    pub maker_user_id: u64,
    pub taker_side: String,
    pub maker_side: String,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerPostResult {
    pub txn_id: i64,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeSettlementRequest {
    pub market_id: String,
    pub fill_id: String,
    pub buyer_user_id: String,
    pub seller_user_id: String,
    pub taker_user_id: String,
    pub maker_user_id: String,
    pub price_pts: i32,
    pub quantity: i64,
    pub taker_fee_bps: u32,
    pub maker_fee_bps: u32,
    pub taker_fee_minor_units: i64,
    pub maker_fee_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionFillProjectionRequest {
    pub user_id: String,
    pub market_id: String,
    pub outcome: String,
    pub side: String,
    pub quantity: i64,
    pub price_pts: i32,
    pub order_id: String,
    pub fill_id: String,
    pub filled_at: Option<String>,
    pub ledger_txn_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderPrecheckOutcome {
    pub required_minor_units: i64,
    pub lock_minor_units: i64,
    pub fee_curve_bps: u32,
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

/// KYC tier as exchanged with kyc-service. Wire format mirrors
/// `kyc_service::kyc::KycTier`; using an enum here keeps the gateway from
/// silently failing closed on a kyc-service rename and prevents subtle bugs
/// from string-typo drift (SPEC-023 plan v0.1.3 follow-up S-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KycTier {
    #[serde(rename = "TIER_0")]
    Tier0,
    #[serde(rename = "TIER_1")]
    Tier1,
    #[serde(rename = "TIER_2")]
    Tier2,
    #[serde(rename = "TIER_3")]
    Tier3,
}

impl KycTier {
    /// Returns true when the tier satisfies the SPEC-005 §2.1 trading gate.
    #[must_use]
    pub const fn allows_trading(self) -> bool {
        matches!(self, Self::Tier1 | Self::Tier2 | Self::Tier3)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KycTierResponse {
    pub tier: KycTier,
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

    async fn reserve_order_funds(
        &self,
        user_id: &str,
        market_id: &str,
        amount_minor_units: i64,
        idempotency_key: &str,
    ) -> Result<Option<LedgerPostResult>, ApiError> {
        let _ = (user_id, market_id, amount_minor_units, idempotency_key);
        Ok(None)
    }

    async fn release_order_funds(
        &self,
        user_id: &str,
        market_id: &str,
        amount_minor_units: i64,
        idempotency_key: &str,
    ) -> Result<(), ApiError> {
        let _ = (user_id, market_id, amount_minor_units, idempotency_key);
        Ok(())
    }

    async fn settle_trade_fill(
        &self,
        request: &TradeSettlementRequest,
        idempotency_key: &str,
    ) -> Result<Option<LedgerPostResult>, ApiError> {
        let _ = (request, idempotency_key);
        Ok(None)
    }
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

    async fn list_admin_affiliates(
        &self,
        context: AdminProxyContext,
    ) -> Result<serde_json::Value, ApiError>;

    async fn get_admin_affiliate_detail(
        &self,
        affiliate_id: &str,
        context: AdminProxyContext,
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
pub(crate) trait RedemptionClient {
    async fn submit_order(
        &self,
        user_id: &str,
        request: SubmitRedemptionOrderRequest,
    ) -> Result<(StatusCode, OrderView), ApiError>;

    async fn get_order(&self, order_id: &str) -> Result<Option<OrderView>, ApiError>;

    async fn cancel_order(&self, order_id: &str) -> Result<OrderView, ApiError>;

    async fn list_entitlements(&self, user_id: &str) -> Result<Vec<Entitlement>, ApiError>;

    async fn propose_cs_refund(
        &self,
        order_id: &str,
        user_id: &str,
        reason: &str,
    ) -> Result<(StatusCode, CsRefundProposal), ApiError>;

    async fn approve_cs_refund(
        &self,
        proposal_id: &str,
        approver_user_id: &str,
    ) -> Result<(StatusCode, OrderView), ApiError>;
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SubmitRedemptionOrderRequest {
    sku_id: String,
    idempotency_key: String,
    correlation_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProposeRedemptionRefundRequest {
    reason: String,
}

#[derive(Debug, Clone)]
struct RedemptionState<R> {
    client: R,
}

impl<R> RedemptionState<R> {
    const fn new(client: R) -> Self {
        Self { client }
    }
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
pub trait PositionServiceClient {
    async fn list_positions(
        &self,
        user_id: &str,
        status: Option<&str>,
    ) -> Result<PositionListResponse, ApiError>;
}

#[async_trait]
pub trait PositionProjectionClient {
    async fn project_fill(
        &self,
        request: PositionFillProjectionRequest,
        idempotency_key: &str,
    ) -> Result<(), ApiError>;
}

#[derive(Debug, Clone, Default)]
pub struct NoopPositionProjectionClient;

#[async_trait]
impl PositionProjectionClient for NoopPositionProjectionClient {
    async fn project_fill(
        &self,
        request: PositionFillProjectionRequest,
        idempotency_key: &str,
    ) -> Result<(), ApiError> {
        let _ = (request, idempotency_key);
        Ok(())
    }
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
        Ok(DEFAULT_TAKER_FEE_CURVE_BPS)
    }
}

#[async_trait]
pub trait KycTierClient {
    async fn get_kyc_tier(&self, user_id: &str) -> Result<KycTier, ApiError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminProxyContext {
    pub actor_id: String,
    pub role: String,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffiliateAccountCreateRequest {
    pub user_id: String,
    pub affiliate_id: String,
    pub email: String,
    pub phone: String,
    pub password: String,
    pub display_name: String,
    pub managing_manager_id: Option<String>,
    pub commission_rate_bps: Option<i32>,
    pub tier: Option<i16>,
    pub reason: String,
    pub authorization_basis: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffiliateAccountSagaResponse {
    pub idempotency_key: String,
    pub status: String,
    pub target_user_id: String,
    pub affiliate_id: String,
    pub actor_id: String,
    pub actor_role: String,
    pub last_error: Option<String>,
    pub affiliate: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffiliateCredentialCreateRequest {
    pub user_id: String,
    pub display_name: String,
    pub email: String,
    pub phone: String,
    pub password: String,
    pub affiliate_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffiliateAccountSagaRecord {
    pub idempotency_key: String,
    pub request_hash: String,
    pub actor_id: String,
    pub actor_role: String,
    pub target_user_id: String,
    pub affiliate_id: String,
    pub status: String,
    pub last_error: Option<String>,
    pub affiliate: Option<serde_json::Value>,
}

#[async_trait]
pub trait AffiliateCredentialClient: Send + Sync {
    async fn create_affiliate_credential(
        &self,
        request: AffiliateCredentialCreateRequest,
        context: AdminProxyContext,
    ) -> Result<(), ApiError>;

    async fn verify_active_manager(
        &self,
        manager_id: &str,
        context: AdminProxyContext,
    ) -> Result<(), ApiError>;
}

#[async_trait]
pub trait AffiliateAccountSagaStore: Send + Sync {
    async fn get(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<AffiliateAccountSagaRecord>, ApiError>;

    async fn put(&self, record: AffiliateAccountSagaRecord) -> Result<(), ApiError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayDeniedAuditRecord {
    pub correlation_id: String,
    pub domain: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub action: String,
    pub actor_id: String,
    pub actor_role: String,
    pub reason_or_error_code: String,
    pub attempted_payload: serde_json::Value,
    pub idempotency_key: Option<String>,
    pub request_id: Option<String>,
}

#[async_trait]
pub trait GatewayDeniedAuditStore: Send + Sync {
    async fn record_denied(&self, record: GatewayDeniedAuditRecord) -> Result<(), ApiError>;
}

#[derive(Debug, Clone, Default)]
pub struct NoopAffiliateCredentialClient;

#[async_trait]
impl AffiliateCredentialClient for NoopAffiliateCredentialClient {
    async fn create_affiliate_credential(
        &self,
        _request: AffiliateCredentialCreateRequest,
        _context: AdminProxyContext,
    ) -> Result<(), ApiError> {
        Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            "user_service_unavailable",
            "affiliate credential client is not configured",
        ))
    }

    async fn verify_active_manager(
        &self,
        _manager_id: &str,
        _context: AdminProxyContext,
    ) -> Result<(), ApiError> {
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAffiliateAccountSagaStore {
    records: Arc<Mutex<HashMap<String, AffiliateAccountSagaRecord>>>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryGatewayDeniedAuditStore {
    records: Arc<Mutex<Vec<GatewayDeniedAuditRecord>>>,
}

#[derive(Debug, Clone)]
pub struct PostgresAffiliateAccountSagaStore {
    pool: sqlx::PgPool,
}

#[derive(Debug, Clone)]
pub struct PostgresGatewayDeniedAuditStore {
    pool: sqlx::PgPool,
}

impl PostgresAffiliateAccountSagaStore {
    const fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

impl PostgresGatewayDeniedAuditStore {
    const fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AffiliateAccountSagaStore for InMemoryAffiliateAccountSagaStore {
    async fn get(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<AffiliateAccountSagaRecord>, ApiError> {
        self.records
            .lock()
            .map_err(|_| ApiError::internal("affiliate saga store lock poisoned"))
            .map(|records| records.get(idempotency_key).cloned())
    }

    async fn put(&self, record: AffiliateAccountSagaRecord) -> Result<(), ApiError> {
        self.records
            .lock()
            .map_err(|_| ApiError::internal("affiliate saga store lock poisoned"))?
            .insert(record.idempotency_key.clone(), record);
        Ok(())
    }
}

#[async_trait]
impl GatewayDeniedAuditStore for InMemoryGatewayDeniedAuditStore {
    async fn record_denied(&self, record: GatewayDeniedAuditRecord) -> Result<(), ApiError> {
        self.records
            .lock()
            .map_err(|_| ApiError::internal("gateway denied audit store lock poisoned"))?
            .push(record);
        Ok(())
    }
}

impl InMemoryGatewayDeniedAuditStore {
    #[cfg(test)]
    fn records(&self) -> Vec<GatewayDeniedAuditRecord> {
        self.records
            .lock()
            .map_or_else(|_| Vec::new(), |records| records.clone())
    }
}

#[async_trait]
impl AffiliateAccountSagaStore for PostgresAffiliateAccountSagaStore {
    async fn get(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<AffiliateAccountSagaRecord>, ApiError> {
        let row = sqlx::query(
            r"
            SELECT idempotency_key, request_hash, actor_id, actor_role, target_user_id,
                   affiliate_id, status, last_error, response_payload
            FROM api_gateway_affiliate_account_sagas
            WHERE idempotency_key = $1
            ",
        )
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
        row.map(|row| {
            Ok(AffiliateAccountSagaRecord {
                idempotency_key: row.try_get("idempotency_key").map_err(|error| {
                    ApiError::internal(format!("decode affiliate saga idempotency_key: {error}"))
                })?,
                request_hash: row.try_get("request_hash").map_err(|error| {
                    ApiError::internal(format!("decode affiliate saga request_hash: {error}"))
                })?,
                actor_id: row.try_get("actor_id").map_err(|error| {
                    ApiError::internal(format!("decode affiliate saga actor_id: {error}"))
                })?,
                actor_role: row.try_get("actor_role").map_err(|error| {
                    ApiError::internal(format!("decode affiliate saga actor_role: {error}"))
                })?,
                target_user_id: row.try_get("target_user_id").map_err(|error| {
                    ApiError::internal(format!("decode affiliate saga target_user_id: {error}"))
                })?,
                affiliate_id: row.try_get("affiliate_id").map_err(|error| {
                    ApiError::internal(format!("decode affiliate saga affiliate_id: {error}"))
                })?,
                status: row.try_get("status").map_err(|error| {
                    ApiError::internal(format!("decode affiliate saga status: {error}"))
                })?,
                last_error: row.try_get("last_error").map_err(|error| {
                    ApiError::internal(format!("decode affiliate saga last_error: {error}"))
                })?,
                affiliate: row.try_get("response_payload").map_err(|error| {
                    ApiError::internal(format!("decode affiliate saga response_payload: {error}"))
                })?,
            })
        })
        .transpose()
    }

    async fn put(&self, record: AffiliateAccountSagaRecord) -> Result<(), ApiError> {
        sqlx::query(
            r"
            INSERT INTO api_gateway_affiliate_account_sagas (
                idempotency_key, request_hash, actor_id, actor_role, target_user_id,
                affiliate_id, status, last_error, response_payload, attempt_count
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1)
            ON CONFLICT (idempotency_key) DO UPDATE SET
                status = EXCLUDED.status,
                last_error = EXCLUDED.last_error,
                response_payload = EXCLUDED.response_payload,
                attempt_count = api_gateway_affiliate_account_sagas.attempt_count + 1,
                updated_at = now()
            ",
        )
        .bind(record.idempotency_key)
        .bind(record.request_hash)
        .bind(record.actor_id)
        .bind(record.actor_role)
        .bind(record.target_user_id)
        .bind(record.affiliate_id)
        .bind(record.status)
        .bind(record.last_error)
        .bind(record.affiliate)
        .execute(&self.pool)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
        Ok(())
    }
}

#[async_trait]
impl GatewayDeniedAuditStore for PostgresGatewayDeniedAuditStore {
    async fn record_denied(&self, record: GatewayDeniedAuditRecord) -> Result<(), ApiError> {
        sqlx::query(
            r"
            INSERT INTO api_gateway_denied_operation_audit_logs (
                correlation_id, domain, target_type, target_id, action, actor_id,
                actor_role, reason_or_error_code, attempted_payload, idempotency_key,
                request_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (correlation_id) DO NOTHING
            ",
        )
        .bind(record.correlation_id)
        .bind(record.domain)
        .bind(record.target_type)
        .bind(record.target_id)
        .bind(record.action)
        .bind(record.actor_id)
        .bind(record.actor_role)
        .bind(record.reason_or_error_code)
        .bind(record.attempted_payload)
        .bind(record.idempotency_key)
        .bind(record.request_id)
        .execute(&self.pool)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
        Ok(())
    }
}

impl HttpAffiliateCredentialClient {
    fn from_env() -> Option<Self> {
        let url =
            env::var("USER_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8084".to_owned());
        Url::parse(&url).ok().map(|base_url| Self {
            base_url,
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait]
impl AffiliateCredentialClient for HttpAffiliateCredentialClient {
    async fn create_affiliate_credential(
        &self,
        request: AffiliateCredentialCreateRequest,
        context: AdminProxyContext,
    ) -> Result<(), ApiError> {
        let url = self
            .base_url
            .join("/v1/internal/admin/members")
            .map_err(|error| {
                ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    "user_service_unavailable",
                    error.to_string(),
                )
            })?;
        let body = serde_json::json!({
            "user_id": request.user_id,
            "display_name": request.display_name,
            "email": request.email,
            "phone": request.phone,
            "country": "TW",
            "timezone": "Asia/Taipei",
            "tier": "Affiliate",
            "affiliate_id": request.affiliate_id,
            "role": "affiliate",
            "password": request.password
        });
        self.client
            .post(url)
            .headers(admin_proxy_headers(context)?)
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    "user_service_unavailable",
                    error.to_string(),
                )
            })?
            .error_for_status()
            .map_err(|error| {
                ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    "user_service_unavailable",
                    error.to_string(),
                )
            })?;
        Ok(())
    }

    async fn verify_active_manager(
        &self,
        manager_id: &str,
        context: AdminProxyContext,
    ) -> Result<(), ApiError> {
        let url = self
            .base_url
            .join("/v1/internal/admin/role-accounts?role=manager")
            .map_err(|error| {
                ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    "user_service_unavailable",
                    error.to_string(),
                )
            })?;
        let body = self
            .client
            .get(url)
            .headers(admin_proxy_headers(context)?)
            .send()
            .await
            .map_err(|error| {
                ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    "user_service_unavailable",
                    error.to_string(),
                )
            })?
            .error_for_status()
            .map_err(|error| {
                ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    "user_service_unavailable",
                    error.to_string(),
                )
            })?
            .json::<serde_json::Value>()
            .await
            .map_err(|error| {
                ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    "user_service_unavailable",
                    error.to_string(),
                )
            })?;
        let is_active = body
            .get("accounts")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|accounts| {
                accounts.iter().any(|account| {
                    account
                        .get("userId")
                        .or_else(|| account.get("user_id"))
                        .and_then(serde_json::Value::as_str)
                        == Some(manager_id)
                        && !account
                            .get("disabled")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(true)
                })
            });
        if is_active {
            Ok(())
        } else {
            Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "invalid_managing_manager",
                "managingManagerId must reference an active manager account",
            ))
        }
    }
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
pub struct HttpAffiliateCredentialClient {
    base_url: Url,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct HttpReferralClient {
    base_url: Url,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct HttpRedemptionClient {
    base_url: Url,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct HttpMarketServiceClient {
    base_url: Url,
    client: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct HttpPositionServiceClient {
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

    async fn post_ledger_transaction(
        &self,
        idempotency_key: &str,
        body: &PostLedgerTransactionRequest,
    ) -> Result<LedgerPostResult, ApiError> {
        let url = self
            .base_url
            .join("/internal/ledger/transactions")
            .map_err(|error| ApiError::upstream(error.to_string()))?;
        let response = self
            .client
            .post(url)
            .header(IDEMPOTENCY_KEY_HEADER.as_str(), idempotency_key)
            .json(body)
            .send()
            .await
            .map_err(|error| ApiError::upstream(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::upstream(error.to_string()))?
            .json::<PostLedgerTransactionResponse>()
            .await
            .map_err(|error| ApiError::upstream(error.to_string()))?;

        Ok(LedgerPostResult {
            txn_id: response.txn_id,
            replayed: response.replayed,
        })
    }
}

impl HttpRedemptionClient {
    /// Creates an HTTP client for the internal redemption service.
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

    /// Creates an HTTP client using the configured redemption service base URL.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured base URL is invalid.
    pub fn from_env() -> anyhow::Result<Self> {
        let base_url = redemption_service_base_url_from_env();
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

impl HttpPositionServiceClient {
    /// Creates an HTTP client for the internal position-service facade.
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

    /// Creates a client from `POSITION_SERVICE_BASE_URL`, defaulting to the
    /// local position-service port used by `core-api`.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured URL is invalid.
    pub fn from_env() -> anyhow::Result<Self> {
        let base_url = env::var("POSITION_SERVICE_BASE_URL")
            .unwrap_or_else(|_| DEFAULT_POSITION_SERVICE_BASE_URL.to_owned());
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

    /// Creates a client from `KYC_SERVICE_BASE_URL` / `KYC_SERVICE_URL` plus
    /// `INTERNAL_CALL_HMAC_SECRET`, defaulting to localhost.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured URL is invalid, or when the HMAC
    /// secret is missing in production.
    pub fn from_env() -> anyhow::Result<Self> {
        let base_url = kyc_service_base_url_from_env();
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
impl RedemptionClient for HttpRedemptionClient {
    async fn submit_order(
        &self,
        user_id: &str,
        request: SubmitRedemptionOrderRequest,
    ) -> Result<(StatusCode, OrderView), ApiError> {
        let url = self
            .base_url
            .join("/v1/redemption/orders")
            .map_err(|error| ApiError::redemption_service(error.to_string()))?;
        let request = SubmitOrderRequest {
            user_id: user_id.to_owned(),
            sku_id: request.sku_id,
            idempotency_key: request.idempotency_key,
            correlation_id: request.correlation_id,
        };
        let response = self
            .client
            .post(url)
            .json(&request)
            .send()
            .await
            .map_err(|error| ApiError::redemption_service(error.to_string()))?;

        decode_redemption_response_with_status(response).await
    }

    async fn get_order(&self, order_id: &str) -> Result<Option<OrderView>, ApiError> {
        let url = self
            .base_url
            .join(&format!("/v1/redemption/orders/{order_id}"))
            .map_err(|error| ApiError::redemption_service(error.to_string()))?;
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| ApiError::redemption_service(error.to_string()))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        decode_redemption_response(response).await.map(Some)
    }

    async fn cancel_order(&self, order_id: &str) -> Result<OrderView, ApiError> {
        let url = self
            .base_url
            .join(&format!("/v1/redemption/orders/{order_id}/cancel"))
            .map_err(|error| ApiError::redemption_service(error.to_string()))?;
        let response = self
            .client
            .post(url)
            .send()
            .await
            .map_err(|error| ApiError::redemption_service(error.to_string()))?;

        decode_redemption_response(response).await
    }

    async fn list_entitlements(&self, user_id: &str) -> Result<Vec<Entitlement>, ApiError> {
        let url = self
            .base_url
            .join(&format!("/v1/redemption/entitlements/{user_id}"))
            .map_err(|error| ApiError::redemption_service(error.to_string()))?;
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| ApiError::redemption_service(error.to_string()))?;
        let (_, payload) =
            decode_redemption_response_with_status::<RedemptionEntitlementListResponse>(response)
                .await?;
        Ok(payload.entitlements)
    }

    async fn propose_cs_refund(
        &self,
        order_id: &str,
        user_id: &str,
        reason: &str,
    ) -> Result<(StatusCode, CsRefundProposal), ApiError> {
        let request = ProposeRedemptionRefundRequest {
            reason: reason.to_owned(),
        };
        let url = self
            .base_url
            .join(&format!(
                "/v1/admin/redemption/orders/{order_id}/refund-proposals"
            ))
            .map_err(|error| ApiError::redemption_service(error.to_string()))?;
        let response = self
            .client
            .post(url)
            .header(USER_ID_HEADER.as_str(), user_id)
            .json(&request)
            .send()
            .await
            .map_err(|error| ApiError::redemption_service(error.to_string()))?;

        decode_redemption_response_with_status(response).await
    }

    async fn approve_cs_refund(
        &self,
        proposal_id: &str,
        approver_user_id: &str,
    ) -> Result<(StatusCode, OrderView), ApiError> {
        let url = self
            .base_url
            .join(&format!(
                "/v1/admin/redemption/refund-proposals/{proposal_id}/approve"
            ))
            .map_err(|error| ApiError::redemption_service(error.to_string()))?;
        let response = self
            .client
            .post(url)
            .header(USER_ID_HEADER.as_str(), approver_user_id)
            .send()
            .await
            .map_err(|error| ApiError::redemption_service(error.to_string()))?;

        decode_redemption_response_with_status(response).await
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

        let body = PostLedgerTransactionRequest {
            txn_type: "freeze",
            correlation_id: Some(idempotency_key.to_owned()),
            description: Some("self-exclusion freeze player funds"),
            metadata: serde_json::json!({
                "reason": "self_exclusion",
                "triggered_by": user_id,
                "ref_id": idempotency_key,
            }),
            entries: vec![
                PostLedgerEntry {
                    account_code: format!("Player.Wallet.{user_id}"),
                    direction: "debit",
                    amount_minor_units,
                },
                PostLedgerEntry {
                    account_code: format!("Player.Frozen.{user_id}"),
                    direction: "credit",
                    amount_minor_units,
                },
            ],
        };

        self.post_ledger_transaction(idempotency_key, &body).await?;

        Ok(())
    }

    async fn reserve_order_funds(
        &self,
        user_id: &str,
        market_id: &str,
        amount_minor_units: i64,
        idempotency_key: &str,
    ) -> Result<Option<LedgerPostResult>, ApiError> {
        if amount_minor_units <= 0 {
            return Ok(None);
        }

        let body = PostLedgerTransactionRequest {
            txn_type: "order_lock",
            correlation_id: Some(idempotency_key.to_owned()),
            description: Some("reserve player funds for order"),
            metadata: serde_json::json!({
                "market_id": market_id,
                "user_id": user_id,
                "reason": "order_lock",
            }),
            entries: vec![
                PostLedgerEntry {
                    account_code: format!("Player.Wallet.{user_id}"),
                    direction: "debit",
                    amount_minor_units,
                },
                PostLedgerEntry {
                    account_code: format!("Player.Locked.{user_id}"),
                    direction: "credit",
                    amount_minor_units,
                },
            ],
        };

        self.post_ledger_transaction(idempotency_key, &body)
            .await
            .map(Some)
    }

    async fn release_order_funds(
        &self,
        user_id: &str,
        market_id: &str,
        amount_minor_units: i64,
        idempotency_key: &str,
    ) -> Result<(), ApiError> {
        if amount_minor_units <= 0 {
            return Ok(());
        }

        let body = PostLedgerTransactionRequest {
            txn_type: "order_unlock",
            correlation_id: Some(idempotency_key.to_owned()),
            description: Some("release player order funds after matching failure"),
            metadata: serde_json::json!({
                "market_id": market_id,
                "user_id": user_id,
                "reason": "matching_engine_failure",
            }),
            entries: vec![
                PostLedgerEntry {
                    account_code: format!("Player.Locked.{user_id}"),
                    direction: "debit",
                    amount_minor_units,
                },
                PostLedgerEntry {
                    account_code: format!("Player.Wallet.{user_id}"),
                    direction: "credit",
                    amount_minor_units,
                },
            ],
        };

        self.post_ledger_transaction(idempotency_key, &body).await?;
        Ok(())
    }

    async fn settle_trade_fill(
        &self,
        request: &TradeSettlementRequest,
        idempotency_key: &str,
    ) -> Result<Option<LedgerPostResult>, ApiError> {
        let amount_minor_units =
            trade_fill_notional_minor_units(request.price_pts, request.quantity)?;
        if amount_minor_units <= 0 {
            return Ok(None);
        }

        let body = PostLedgerTransactionRequest {
            txn_type: "trade",
            correlation_id: Some(idempotency_key.to_owned()),
            description: Some("settle matched trade fill"),
            metadata: serde_json::json!({
                "market_id": request.market_id,
                "fill_id": request.fill_id,
                "buyer_user_id": request.buyer_user_id,
                "seller_user_id": request.seller_user_id,
                "taker_user_id": request.taker_user_id,
                "maker_user_id": request.maker_user_id,
                "taker_fee_bps": request.taker_fee_bps,
                "maker_fee_bps": request.maker_fee_bps,
                "taker_fee_minor_units": request.taker_fee_minor_units,
                "maker_fee_minor_units": request.maker_fee_minor_units,
                "reason": "trade_fill",
            }),
            entries: trade_settlement_entries(request, amount_minor_units),
        };

        self.post_ledger_transaction(idempotency_key, &body)
            .await
            .map(Some)
    }
}

#[derive(Debug, Serialize)]
struct PostLedgerTransactionRequest {
    txn_type: &'static str,
    correlation_id: Option<String>,
    description: Option<&'static str>,
    metadata: serde_json::Value,
    entries: Vec<PostLedgerEntry>,
}

#[derive(Debug, Serialize)]
struct PostLedgerEntry {
    account_code: String,
    direction: &'static str,
    amount_minor_units: i64,
}

fn trade_settlement_entries(
    request: &TradeSettlementRequest,
    amount_minor_units: i64,
) -> Vec<PostLedgerEntry> {
    let mut entries = vec![
        PostLedgerEntry {
            account_code: format!("Player.Locked.{}", request.buyer_user_id),
            direction: "debit",
            amount_minor_units,
        },
        PostLedgerEntry {
            account_code: format!("Player.Wallet.{}", request.seller_user_id),
            direction: "credit",
            amount_minor_units,
        },
    ];
    push_trade_fee_entries(
        &mut entries,
        request,
        &request.taker_user_id,
        request.taker_fee_minor_units,
    );
    push_trade_fee_entries(
        &mut entries,
        request,
        &request.maker_user_id,
        request.maker_fee_minor_units,
    );
    entries
}

fn push_trade_fee_entries(
    entries: &mut Vec<PostLedgerEntry>,
    request: &TradeSettlementRequest,
    user_id: &str,
    fee_minor_units: i64,
) {
    if fee_minor_units <= 0 {
        return;
    }
    entries.push(PostLedgerEntry {
        account_code: format!("Player.Wallet.{user_id}"),
        direction: "debit",
        amount_minor_units: fee_minor_units,
    });
    entries.push(PostLedgerEntry {
        account_code: "Revenue.TradingFee".to_owned(),
        direction: "credit",
        amount_minor_units: fee_minor_units,
    });
    debug_assert!(
        user_id == request.buyer_user_id || user_id == request.seller_user_id,
        "trade fee user must be one side of the fill"
    );
}

#[derive(Debug, Deserialize)]
struct PostLedgerTransactionResponse {
    txn_id: i64,
    replayed: bool,
}

fn matching_engine_user_id_header_value(user_id: &str) -> String {
    if let Ok(parsed) = user_id.parse::<u64>() {
        if parsed >= 9 {
            return parsed.to_string();
        }
    }

    let digest = Sha256::digest(user_id.as_bytes());
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    let mut mapped = u64::from_be_bytes(bytes);
    if mapped < 9 {
        mapped = mapped.saturating_add(9);
    }
    mapped.to_string()
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
            .header(
                USER_ID_HEADER.as_str(),
                matching_engine_user_id_header_value(user_id),
            )
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
            .header(
                USER_ID_HEADER.as_str(),
                matching_engine_user_id_header_value(user_id),
            )
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

    async fn list_admin_affiliates(
        &self,
        context: AdminProxyContext,
    ) -> Result<serde_json::Value, ApiError> {
        let url = self
            .base_url
            .join("/internal/admin/affiliates")
            .map_err(|error| ApiError::referral_service(error.to_string()))?;
        self.client
            .get(url)
            .headers(admin_proxy_headers(context)?)
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
        context: AdminProxyContext,
    ) -> Result<serde_json::Value, ApiError> {
        let url = self
            .base_url
            .join(&format!("/internal/admin/affiliates/{affiliate_id}"))
            .map_err(|error| ApiError::referral_service(error.to_string()))?;
        self.client
            .get(url)
            .headers(admin_proxy_headers(context)?)
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
impl PositionServiceClient for HttpPositionServiceClient {
    async fn list_positions(
        &self,
        user_id: &str,
        status: Option<&str>,
    ) -> Result<PositionListResponse, ApiError> {
        let mut url = self
            .base_url
            .join(&format!("/internal/users/{user_id}/positions"))
            .map_err(|error| ApiError::position_service(error.to_string()))?;
        if let Some(status) = status {
            url.query_pairs_mut().append_pair("status", status);
        }
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| ApiError::position_service(error.to_string()))?;

        response
            .error_for_status()
            .map_err(|error| ApiError::position_service(error.to_string()))?
            .json::<PositionListResponse>()
            .await
            .map_err(|error| ApiError::position_service(error.to_string()))
    }
}

#[async_trait]
impl PositionProjectionClient for HttpPositionServiceClient {
    async fn project_fill(
        &self,
        request: PositionFillProjectionRequest,
        idempotency_key: &str,
    ) -> Result<(), ApiError> {
        let url = self
            .base_url
            .join("/internal/positions/fills")
            .map_err(|error| ApiError::position_service(error.to_string()))?;
        self.client
            .post(url)
            .header(IDEMPOTENCY_KEY_HEADER.as_str(), idempotency_key)
            .json(&request)
            .send()
            .await
            .map_err(|error| ApiError::position_service(error.to_string()))?
            .error_for_status()
            .map_err(|error| ApiError::position_service(error.to_string()))?;

        Ok(())
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
            .unwrap_or(DEFAULT_TAKER_FEE_CURVE_BPS)
            .max(DEFAULT_TAKER_FEE_CURVE_BPS))
    }
}

#[async_trait]
impl KycTierClient for HttpKycServiceClient {
    async fn get_kyc_tier(&self, user_id: &str) -> Result<KycTier, ApiError> {
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
    async fn get_kyc_tier(&self, _user_id: &str) -> Result<KycTier, ApiError> {
        Ok(KycTier::Tier1)
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

async fn decode_redemption_response<T>(response: reqwest::Response) -> Result<T, ApiError>
where
    T: serde::de::DeserializeOwned,
{
    let status = response.status();
    if !status.is_success() {
        let message = response.text().await.map_or_else(
            |error| error.to_string(),
            |text| {
                serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|body| {
                        body.get("message")
                            .and_then(serde_json::Value::as_str)
                            .map(ToOwned::to_owned)
                    })
                    .unwrap_or_else(|| text.to_string())
            },
        );
        return Err(ApiError::redemption_service_status(status, message));
    }

    response.json::<T>().await.map_err(|error| {
        ApiError::redemption_service_status(StatusCode::BAD_GATEWAY, error.to_string())
    })
}

async fn decode_redemption_response_with_status<T>(
    response: reqwest::Response,
) -> Result<(StatusCode, T), ApiError>
where
    T: serde::de::DeserializeOwned,
{
    let status = response.status();
    let payload = decode_redemption_response(response).await?;
    Ok((status, payload))
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

impl OrderOwnerStore {
    #[cfg(test)]
    fn record(&self, order_id: &str, user_id: &str) -> Result<(), ApiError> {
        self.owners
            .lock()
            .map_err(|_| ApiError::internal("order owner store lock poisoned"))?
            .insert(order_id.to_owned(), user_id.to_owned());
        Ok(())
    }
}

#[async_trait]
impl OrderOwnerRepository for OrderOwnerStore {
    async fn record_owner(&self, owner: OrderOwnerRecord) -> Result<(), ApiError> {
        self.owners
            .lock()
            .map_err(|_| ApiError::internal("order owner store lock poisoned"))?
            .insert(owner.order, owner.user);
        Ok(())
    }

    async fn find_owner(&self, order_id: &str) -> Result<Option<String>, ApiError> {
        self.owners
            .lock()
            .map_err(|_| ApiError::internal("order owner store lock poisoned"))
            .map(|owners| owners.get(order_id).cloned())
    }
}

#[async_trait]
impl OrderOwnerRepository for PostgresOrderOwnerStore {
    async fn record_owner(&self, owner: OrderOwnerRecord) -> Result<(), ApiError> {
        sqlx::query(
            "
            INSERT INTO api_gateway_order_owners
                (order_id, user_id, market_id, matching_user_id)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (order_id) DO UPDATE SET
                user_id = EXCLUDED.user_id,
                market_id = EXCLUDED.market_id,
                matching_user_id = EXCLUDED.matching_user_id,
                updated_at = now()
            ",
        )
        .bind(owner.order)
        .bind(owner.user)
        .bind(owner.market)
        .bind(owner.matching_user)
        .execute(&self.pool)
        .await
        .map_err(|error| ApiError::internal(format!("order owner persistence failed: {error}")))?;
        Ok(())
    }

    async fn find_owner(&self, order_id: &str) -> Result<Option<String>, ApiError> {
        sqlx::query_scalar("SELECT user_id FROM api_gateway_order_owners WHERE order_id = $1")
            .bind(order_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| ApiError::internal(format!("order owner lookup failed: {error}")))
    }
}

#[async_trait]
impl TradeFillSagaStore for InMemoryTradeFillSagaStore {
    async fn enqueue(&self, saga: TradeFillSaga) -> Result<(), ApiError> {
        let mut sagas = self
            .sagas
            .lock()
            .map_err(|_| ApiError::internal("trade fill saga store lock poisoned"))?;
        sagas.entry(saga.fill_key.clone()).or_insert(saga);
        Ok(())
    }

    async fn claim_due(&self, limit: i64) -> Result<Vec<TradeFillSaga>, ApiError> {
        let mut sagas = self
            .sagas
            .lock()
            .map_err(|_| ApiError::internal("trade fill saga store lock poisoned"))?;
        let limit = usize::try_from(limit.max(0)).unwrap_or(usize::MAX);
        let claimed_keys = sagas
            .iter()
            .filter(|(_, saga)| {
                matches!(
                    saga.status,
                    TradeFillSagaStatus::Pending
                        | TradeFillSagaStatus::Retryable
                        | TradeFillSagaStatus::Running
                )
            })
            .take(limit)
            .map(|(fill_key, _)| fill_key.clone())
            .collect::<Vec<_>>();
        let mut claimed = Vec::with_capacity(claimed_keys.len());
        for fill_key in claimed_keys {
            if let Some(saga) = sagas.get_mut(&fill_key) {
                saga.status = TradeFillSagaStatus::Running;
                saga.attempts = saga.attempts.saturating_add(1);
                claimed.push(saga.clone());
            }
        }
        Ok(claimed)
    }

    async fn mark_settlement_posted(
        &self,
        fill_key: &str,
        ledger_txn_id: Option<i64>,
    ) -> Result<(), ApiError> {
        self.update(fill_key, |saga| {
            saga.ledger_txn_id = ledger_txn_id;
            saga.settlement_posted = true;
        })
    }

    async fn mark_taker_projected(&self, fill_key: &str) -> Result<(), ApiError> {
        self.update(fill_key, |saga| saga.taker_projected = true)
    }

    async fn mark_maker_projected(&self, fill_key: &str) -> Result<(), ApiError> {
        self.update(fill_key, |saga| saga.maker_projected = true)
    }

    async fn mark_completed(&self, fill_key: &str) -> Result<(), ApiError> {
        self.update(fill_key, |saga| {
            saga.status = TradeFillSagaStatus::Completed;
        })
    }

    async fn mark_retryable(&self, fill_key: &str, error: &str) -> Result<(), ApiError> {
        let _ = error;
        self.update(fill_key, |saga| {
            saga.status = TradeFillSagaStatus::Retryable;
        })
    }
}

impl InMemoryTradeFillSagaStore {
    fn update(
        &self,
        fill_key: &str,
        apply: impl FnOnce(&mut TradeFillSaga),
    ) -> Result<(), ApiError> {
        let mut sagas = self
            .sagas
            .lock()
            .map_err(|_| ApiError::internal("trade fill saga store lock poisoned"))?;
        let Some(saga) = sagas.get_mut(fill_key) else {
            return Err(ApiError::internal("trade fill saga record was not found"));
        };
        apply(saga);
        Ok(())
    }
}

#[async_trait]
impl TradeFillSagaStore for PostgresTradeFillSagaStore {
    async fn enqueue(&self, saga: TradeFillSaga) -> Result<(), ApiError> {
        sqlx::query(
            "
            INSERT INTO api_gateway_trade_fill_sagas (
                fill_key, market_id, fill_id, taker_user_id, maker_user_id,
                buyer_user_id, seller_user_id, taker_order_id, maker_order_id,
                taker_side, maker_side, outcome, price_pts, quantity, fee_curve_bps,
                reservation_txn_id, ledger_txn_id, settlement_idempotency_key,
                taker_projection_idempotency_key, maker_projection_idempotency_key,
                status, attempts
            )
            VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, $9,
                $10, $11, $12, $13, $14, $15,
                $16, $17, $18,
                $19, $20,
                $21, $22
            )
            ON CONFLICT (fill_key) DO NOTHING
            ",
        )
        .bind(saga.fill_key)
        .bind(saga.market_id)
        .bind(saga.fill_id)
        .bind(saga.taker_user_id)
        .bind(saga.maker_user_id)
        .bind(saga.buyer_user_id)
        .bind(saga.seller_user_id)
        .bind(saga.taker_order_id)
        .bind(saga.maker_order_id)
        .bind(saga.taker_side)
        .bind(saga.maker_side)
        .bind(saga.outcome)
        .bind(saga.price_pts)
        .bind(saga.quantity)
        .bind(i32::try_from(saga.fee_curve_bps).unwrap_or(i32::MAX))
        .bind(saga.reservation_txn_id)
        .bind(saga.ledger_txn_id)
        .bind(saga.settlement_idempotency_key)
        .bind(saga.taker_projection_idempotency_key)
        .bind(saga.maker_projection_idempotency_key)
        .bind(saga.status.as_str())
        .bind(saga.attempts)
        .execute(&self.pool)
        .await
        .map_err(|error| ApiError::internal(format!("trade fill saga enqueue failed: {error}")))?;
        Ok(())
    }

    async fn claim_due(&self, limit: i64) -> Result<Vec<TradeFillSaga>, ApiError> {
        let rows = sqlx::query(
            "
            WITH due AS (
                SELECT fill_key
                FROM api_gateway_trade_fill_sagas
                WHERE (
                    status IN ('pending', 'retryable')
                    AND next_attempt_at <= now()
                ) OR (
                    status = 'running'
                    AND updated_at < now() - interval '1 minute'
                )
                ORDER BY created_at
                LIMIT $1
                FOR UPDATE SKIP LOCKED
            )
            UPDATE api_gateway_trade_fill_sagas AS saga
            SET status = 'running',
                attempts = saga.attempts + 1,
                updated_at = now()
            FROM due
            WHERE saga.fill_key = due.fill_key
            RETURNING
                saga.*,
                saga.settlement_posted_at IS NOT NULL AS settlement_posted,
                saga.taker_projected_at IS NOT NULL AS taker_projected,
                saga.maker_projected_at IS NOT NULL AS maker_projected
            ",
        )
        .bind(limit.clamp(1, 500))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| ApiError::internal(format!("trade fill saga claim failed: {error}")))?;

        rows.iter().map(trade_fill_saga_from_row).collect()
    }

    async fn mark_settlement_posted(
        &self,
        fill_key: &str,
        ledger_txn_id: Option<i64>,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "
            UPDATE api_gateway_trade_fill_sagas
            SET status = 'running',
                ledger_txn_id = $2,
                settlement_posted_at = COALESCE(settlement_posted_at, now()),
                updated_at = now()
            WHERE fill_key = $1
            ",
        )
        .bind(fill_key)
        .bind(ledger_txn_id)
        .execute(&self.pool)
        .await
        .map_err(|error| ApiError::internal(format!("trade fill saga update failed: {error}")))?;
        Ok(())
    }

    async fn mark_taker_projected(&self, fill_key: &str) -> Result<(), ApiError> {
        self.mark_projection(fill_key, "taker_projected_at").await
    }

    async fn mark_maker_projected(&self, fill_key: &str) -> Result<(), ApiError> {
        self.mark_projection(fill_key, "maker_projected_at").await
    }

    async fn mark_completed(&self, fill_key: &str) -> Result<(), ApiError> {
        sqlx::query(
            "
            UPDATE api_gateway_trade_fill_sagas
            SET status = 'completed',
                completed_at = COALESCE(completed_at, now()),
                updated_at = now()
            WHERE fill_key = $1
            ",
        )
        .bind(fill_key)
        .execute(&self.pool)
        .await
        .map_err(|error| {
            ApiError::internal(format!("trade fill saga completion failed: {error}"))
        })?;
        Ok(())
    }

    async fn mark_retryable(&self, fill_key: &str, error: &str) -> Result<(), ApiError> {
        sqlx::query(
            "
            UPDATE api_gateway_trade_fill_sagas
            SET status = 'retryable',
                last_error = $2,
                next_attempt_at = now() + make_interval(
                    secs => LEAST(300, CAST(POWER(2, LEAST(attempts, 8)) AS INTEGER))
                ),
                updated_at = now()
            WHERE fill_key = $1
            ",
        )
        .bind(fill_key)
        .bind(error.chars().take(2_000).collect::<String>())
        .execute(&self.pool)
        .await
        .map_err(|error| {
            ApiError::internal(format!("trade fill saga retry update failed: {error}"))
        })?;
        Ok(())
    }
}

impl PostgresTradeFillSagaStore {
    async fn mark_projection(&self, fill_key: &str, column: &str) -> Result<(), ApiError> {
        let statement = format!(
            "
            UPDATE api_gateway_trade_fill_sagas
            SET status = 'running',
                {column} = COALESCE({column}, now()),
                updated_at = now()
            WHERE fill_key = $1
            "
        );
        sqlx::query(&statement)
            .bind(fill_key)
            .execute(&self.pool)
            .await
            .map_err(|error| {
                ApiError::internal(format!("trade fill saga projection update failed: {error}"))
            })?;
        Ok(())
    }
}

fn trade_fill_saga_from_row(row: &sqlx::postgres::PgRow) -> Result<TradeFillSaga, ApiError> {
    let status: String = row.try_get("status").map_err(|error| {
        ApiError::internal(format!("trade fill saga status decode failed: {error}"))
    })?;
    let settlement_posted: bool = row.try_get("settlement_posted").map_err(|error| {
        ApiError::internal(format!("trade fill saga settlement decode failed: {error}"))
    })?;
    let taker_projected: bool = row.try_get("taker_projected").map_err(|error| {
        ApiError::internal(format!("trade fill saga taker decode failed: {error}"))
    })?;
    let maker_projected: bool = row.try_get("maker_projected").map_err(|error| {
        ApiError::internal(format!("trade fill saga maker decode failed: {error}"))
    })?;

    Ok(TradeFillSaga {
        fill_key: row.try_get("fill_key").map_err(|error| {
            ApiError::internal(format!("trade fill saga key decode failed: {error}"))
        })?,
        market_id: row.try_get("market_id").map_err(|error| {
            ApiError::internal(format!("trade fill saga market decode failed: {error}"))
        })?,
        fill_id: row.try_get("fill_id").map_err(|error| {
            ApiError::internal(format!("trade fill saga fill decode failed: {error}"))
        })?,
        taker_user_id: row.try_get("taker_user_id").map_err(|error| {
            ApiError::internal(format!("trade fill saga taker user decode failed: {error}"))
        })?,
        maker_user_id: row.try_get("maker_user_id").map_err(|error| {
            ApiError::internal(format!("trade fill saga maker user decode failed: {error}"))
        })?,
        buyer_user_id: row.try_get("buyer_user_id").map_err(|error| {
            ApiError::internal(format!("trade fill saga buyer decode failed: {error}"))
        })?,
        seller_user_id: row.try_get("seller_user_id").map_err(|error| {
            ApiError::internal(format!("trade fill saga seller decode failed: {error}"))
        })?,
        taker_order_id: row.try_get("taker_order_id").map_err(|error| {
            ApiError::internal(format!(
                "trade fill saga taker order decode failed: {error}"
            ))
        })?,
        maker_order_id: row.try_get("maker_order_id").map_err(|error| {
            ApiError::internal(format!(
                "trade fill saga maker order decode failed: {error}"
            ))
        })?,
        taker_side: row.try_get("taker_side").map_err(|error| {
            ApiError::internal(format!("trade fill saga taker side decode failed: {error}"))
        })?,
        maker_side: row.try_get("maker_side").map_err(|error| {
            ApiError::internal(format!("trade fill saga maker side decode failed: {error}"))
        })?,
        outcome: row.try_get("outcome").map_err(|error| {
            ApiError::internal(format!("trade fill saga outcome decode failed: {error}"))
        })?,
        price_pts: row.try_get("price_pts").map_err(|error| {
            ApiError::internal(format!("trade fill saga price decode failed: {error}"))
        })?,
        quantity: row.try_get("quantity").map_err(|error| {
            ApiError::internal(format!("trade fill saga quantity decode failed: {error}"))
        })?,
        fee_curve_bps: row
            .try_get::<i32, _>("fee_curve_bps")
            .map(|value| u32::try_from(value).unwrap_or(DEFAULT_TAKER_FEE_CURVE_BPS))
            .map_err(|error| {
                ApiError::internal(format!("trade fill saga fee curve decode failed: {error}"))
            })?,
        reservation_txn_id: row.try_get("reservation_txn_id").map_err(|error| {
            ApiError::internal(format!(
                "trade fill saga reservation decode failed: {error}"
            ))
        })?,
        ledger_txn_id: row.try_get("ledger_txn_id").map_err(|error| {
            ApiError::internal(format!("trade fill saga ledger decode failed: {error}"))
        })?,
        settlement_idempotency_key: row.try_get("settlement_idempotency_key").map_err(|error| {
            ApiError::internal(format!(
                "trade fill saga settlement key decode failed: {error}"
            ))
        })?,
        taker_projection_idempotency_key: row.try_get("taker_projection_idempotency_key").map_err(
            |error| ApiError::internal(format!("trade fill saga taker key decode failed: {error}")),
        )?,
        maker_projection_idempotency_key: row.try_get("maker_projection_idempotency_key").map_err(
            |error| ApiError::internal(format!("trade fill saga maker key decode failed: {error}")),
        )?,
        status: TradeFillSagaStatus::from_str(&status),
        attempts: row.try_get("attempts").map_err(|error| {
            ApiError::internal(format!("trade fill saga attempts decode failed: {error}"))
        })?,
        settlement_posted,
        taker_projected,
        maker_projected,
    })
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
    router_with_engine_and_kyc_and_market_fees_and_position_projection(
        wallet_ledger,
        engine,
        kyc,
        market_fees,
        NoopPositionProjectionClient,
    )
}

pub fn router_with_engine_and_kyc_and_market_fees_and_position_projection<W, E, K, M, P>(
    wallet_ledger: W,
    engine: E,
    kyc: K,
    market_fees: M,
    position_projection: P,
) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
    K: KycTierClient + Clone + Send + Sync + 'static,
    M: MarketFeeReader + Send + Sync + 'static,
    P: PositionProjectionClient + Send + Sync + 'static,
{
    router_with_engine_and_kyc_market_fees_position_projection_and_trade_stores(
        wallet_ledger,
        engine,
        kyc,
        market_fees,
        position_projection,
        TradeFillStores::in_memory(),
    )
}

fn router_with_engine_and_kyc_market_fees_position_projection_and_trade_stores<W, E, K, M, P>(
    wallet_ledger: W,
    engine: E,
    kyc: K,
    market_fees: M,
    position_projection: P,
    trade_stores: TradeFillStores,
) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
    K: KycTierClient + Clone + Send + Sync + 'static,
    M: MarketFeeReader + Send + Sync + 'static,
    P: PositionProjectionClient + Send + Sync + 'static,
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
        .with_state(Arc::new(
            AppStateWithEngine::new_with_market_fees_position_projection_and_trade_stores(
                wallet_ledger,
                engine,
                kyc,
                market_fees,
                position_projection,
                trade_stores,
            ),
        ))
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
    router_with_services_and_kyc_and_payment_flags_and_market_fees_and_position_projection(
        wallet_ledger,
        engine,
        referral,
        kyc,
        payment_flags,
        market_fees,
        NoopPositionProjectionClient,
    )
}

pub fn router_with_services_and_kyc_and_payment_flags_and_market_fees_and_position_projection<
    W,
    E,
    R,
    K,
    F,
    M,
    P,
>(
    wallet_ledger: W,
    engine: E,
    referral: R,
    kyc: K,
    payment_flags: F,
    market_fees: M,
    position_projection: P,
) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
    R: ReferralClient + Clone + Send + Sync + 'static,
    K: KycTierClient + Clone + Send + Sync + 'static,
    F: PaymentsRealMoneyFlagReader + Clone + Send + Sync + 'static,
    M: MarketFeeReader + Send + Sync + 'static,
    P: PositionProjectionClient + Send + Sync + 'static,
{
    router_with_services_kyc_payment_flags_market_fees_position_projection_and_trade_stores(
        GatewayRouterServices {
            wallet_ledger,
            engine,
            referral,
            kyc,
            payment_flags,
            market_fees,
            position_projection,
        },
        GatewayStateStores::in_memory_noop(),
    )
}

struct GatewayRouterServices<W, E, R, K, F, M, P> {
    wallet_ledger: W,
    engine: E,
    referral: R,
    kyc: K,
    payment_flags: F,
    market_fees: M,
    position_projection: P,
}

fn router_with_services_kyc_payment_flags_market_fees_position_projection_and_trade_stores<
    W,
    E,
    R,
    K,
    F,
    M,
    P,
>(
    services: GatewayRouterServices<W, E, R, K, F, M, P>,
    stores: GatewayStateStores,
) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
    R: ReferralClient + Clone + Send + Sync + 'static,
    K: KycTierClient + Clone + Send + Sync + 'static,
    F: PaymentsRealMoneyFlagReader + Clone + Send + Sync + 'static,
    M: MarketFeeReader + Send + Sync + 'static,
    P: PositionProjectionClient + Send + Sync + 'static,
{
    let GatewayRouterServices {
        wallet_ledger,
        engine,
        referral,
        kyc,
        payment_flags,
        market_fees,
        position_projection,
    } = services;
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
            "/api/internal/admin/affiliate-accounts",
            post(create_affiliate_account_saga::<W, E, R, K>),
        )
        .route(
            "/api/internal/admin/affiliate-account-sagas/:idempotency_key",
            get(get_affiliate_account_saga::<W, E, R, K>),
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
        .with_state(Arc::new(
            AppStateWithServices::new_with_market_fees_position_projection_and_trade_stores(
                wallet_ledger,
                engine,
                referral,
                kyc,
                market_fees,
                position_projection,
                stores,
            ),
        ))
        .merge(rg_router)
        .merge(payments_real_money_router(payment_flags))
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn router_with_services_and_affiliate_account_clients<W, E, R, C, S>(
    wallet_ledger: W,
    engine: E,
    referral: R,
    affiliate_credentials: C,
    affiliate_account_sagas: S,
    gateway_denied_audits: InMemoryGatewayDeniedAuditStore,
) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    E: MatchingEngineClient + Clone + Send + Sync + 'static,
    R: ReferralClient + Clone + Send + Sync + 'static,
    C: AffiliateCredentialClient + 'static,
    S: AffiliateAccountSagaStore + 'static,
{
    let rg_router = responsible_gaming::responsible_gaming_router(
        wallet_ledger.clone(),
        responsible_gaming::InMemorySelfExclusionStore::default(),
        responsible_gaming::ResponsibleGamingClock::system(),
    );

    Router::new()
        .route(
            "/api/internal/admin/affiliate-accounts",
            post(create_affiliate_account_saga::<W, E, R, AllowAllKycTierClient>),
        )
        .route(
            "/api/internal/admin/affiliate-account-sagas/:idempotency_key",
            get(get_affiliate_account_saga::<W, E, R, AllowAllKycTierClient>),
        )
        .route(
            "/api/internal/admin/affiliates/:affiliate_id",
            axum::routing::patch(update_admin_affiliate::<W, E, R, AllowAllKycTierClient>),
        )
        .with_state(Arc::new(
            AppStateWithServices::new_with_market_fees_position_projection_and_trade_stores(
                wallet_ledger,
                engine,
                referral,
                AllowAllKycTierClient,
                DefaultMarketFeeReader,
                NoopPositionProjectionClient,
                GatewayStateStores {
                    trade: TradeFillStores::in_memory(),
                    affiliate_admin: AffiliateAdminStores {
                        affiliate_credentials: Arc::new(affiliate_credentials),
                        affiliate_account_sagas: Arc::new(affiliate_account_sagas),
                        gateway_denied_audits: Arc::new(gateway_denied_audits),
                    },
                },
            ),
        ))
        .merge(rg_router)
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

pub fn positions_router<P>(position_service: P) -> Router
where
    P: PositionServiceClient + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/v1/me/positions", get(list_my_positions::<P>))
        .with_state(Arc::new(position_service))
}

pub(crate) fn redemption_router<R>(redemption_client: R) -> Router
where
    R: RedemptionClient + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/v1/redemption/orders", post(submit_redemption_order::<R>))
        .route(
            "/v1/redemption/orders/:order_id",
            get(get_redemption_order::<R>),
        )
        .route(
            "/v1/redemption/orders/:order_id/cancel",
            post(cancel_redemption_order::<R>),
        )
        .route(
            "/v1/redemption/entitlements/:user_id",
            get(list_my_entitlements::<R>),
        )
        .route(
            "/v1/admin/redemption/orders/:order_id/refund-proposals",
            post(propose_redemption_cs_refund::<R>),
        )
        .route(
            "/v1/admin/redemption/refund-proposals/:proposal_id/approve",
            post(approve_redemption_cs_refund::<R>),
        )
        .with_state(Arc::new(RedemptionState::new(redemption_client)))
}

async fn submit_redemption_order<R>(
    State(state): State<Arc<RedemptionState<R>>>,
    auth: AuthContext,
    Json(request): Json<SubmitRedemptionOrderRequest>,
) -> Result<(StatusCode, Json<OrderView>), ApiError>
where
    R: RedemptionClient + Send + Sync,
{
    let request = SubmitRedemptionOrderRequest {
        sku_id: request.sku_id,
        idempotency_key: request.idempotency_key,
        correlation_id: request.correlation_id,
    };
    let (status, order) = state.client.submit_order(&auth.user_id, request).await?;
    Ok((status, Json(order)))
}

async fn get_redemption_order<R>(
    State(state): State<Arc<RedemptionState<R>>>,
    auth: AuthContext,
    Path(order_id): Path<String>,
) -> Result<Json<OrderView>, ApiError>
where
    R: RedemptionClient + Send + Sync,
{
    let Some(order) = state.client.get_order(&order_id).await? else {
        return Err(ApiError::not_found(
            "order_not_found",
            "redemption order not found",
        ));
    };
    if order.user_id != auth.user_id {
        return Err(ApiError::forbidden(
            "order_access_forbidden",
            "cannot access another user's redemption order",
        ));
    }
    Ok(Json(order))
}

async fn cancel_redemption_order<R>(
    State(state): State<Arc<RedemptionState<R>>>,
    auth: AuthContext,
    Path(order_id): Path<String>,
) -> Result<Json<OrderView>, ApiError>
where
    R: RedemptionClient + Send + Sync,
{
    let Some(order) = state.client.get_order(&order_id).await? else {
        return Err(ApiError::not_found(
            "order_not_found",
            "redemption order not found",
        ));
    };
    if order.user_id != auth.user_id {
        return Err(ApiError::forbidden(
            "order_access_forbidden",
            "cannot cancel another user's redemption order",
        ));
    }

    let order = state.client.cancel_order(&order_id).await?;
    Ok(Json(order))
}

#[derive(Debug, Serialize, Deserialize)]
struct RedemptionEntitlementListResponse {
    entitlements: Vec<Entitlement>,
}

async fn list_my_entitlements<R>(
    State(state): State<Arc<RedemptionState<R>>>,
    auth: AuthContext,
    Path(user_id): Path<String>,
) -> Result<Json<RedemptionEntitlementListResponse>, ApiError>
where
    R: RedemptionClient + Send + Sync,
{
    if user_id != auth.user_id {
        return Err(ApiError::forbidden(
            "forbidden",
            "can only view entitlements for the authenticated user",
        ));
    }

    let entitlements = state.client.list_entitlements(&user_id).await?;
    Ok(Json(RedemptionEntitlementListResponse { entitlements }))
}

async fn propose_redemption_cs_refund<R>(
    State(state): State<Arc<RedemptionState<R>>>,
    auth: AuthContext,
    Path(order_id): Path<String>,
    Json(request): Json<ProposeRedemptionRefundRequest>,
) -> Result<(StatusCode, Json<CsRefundProposal>), ApiError>
where
    R: RedemptionClient + Send + Sync,
{
    if request.reason.trim().is_empty() {
        return Err(ApiError::bad_request(
            "empty_reason",
            "reason must not be empty",
        ));
    }

    let (status, proposal) = state
        .client
        .propose_cs_refund(&order_id, &auth.user_id, &request.reason)
        .await?;
    Ok((status, Json(proposal)))
}

async fn approve_redemption_cs_refund<R>(
    State(state): State<Arc<RedemptionState<R>>>,
    auth: AuthContext,
    Path(proposal_id): Path<String>,
) -> Result<(StatusCode, Json<OrderView>), ApiError>
where
    R: RedemptionClient + Send + Sync,
{
    let (status, order) = state
        .client
        .approve_cs_refund(&proposal_id, &auth.user_id)
        .await?;
    Ok((status, Json(order)))
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

async fn list_my_positions<P>(
    State(position_service): State<Arc<P>>,
    auth: AuthContext,
    Query(query): Query<PositionListQuery>,
) -> Result<Json<PositionListResponse>, ApiError>
where
    P: PositionServiceClient,
{
    position_service
        .list_positions(&auth.user_id, query.status.as_deref())
        .await
        .map(Json)
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
) -> Result<OrderPrecheckOutcome, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    // Parse + bounds-check the request shape BEFORE issuing any downstream
    // RPC. SPEC-023 plan v0.1.3 follow-up I-5: rejecting obvious garbage
    // here prevents an attacker from amplifying load on kyc-service and
    // wallet-ledger-service by spraying invalid orders.
    let order_kind = parse_order_kind(&request.order_type).inspect_err(|_| {
        record_order_precheck_rejected("invalid_order_type");
    })?;
    let side = parse_order_side(&request.side).inspect_err(|_| {
        record_order_precheck_rejected("invalid_order_side");
    })?;
    validate_order_bounds(request, order_kind).inspect_err(|_| {
        // Metric label is set inside validate_order_bounds via an
        // unsuccessful path; surface a stable counter label here.
        record_order_precheck_rejected("invalid_order_shape");
    })?;

    let tier = kyc.get_kyc_tier(user_id).await.inspect_err(|_| {
        record_order_precheck_rejected("kyc_lookup_unavailable");
    })?;
    if !tier.allows_trading() {
        record_order_precheck_rejected("kyc_tier_insufficient");
        return Err(ApiError::kyc_tier_insufficient(tier));
    }

    // fee_bps_cap 是市場屬性，不依 side 變化。`required_minor_units` 在 ASK 時
    // 會自行回 0（賣方不預扣餘額），但同一個值會作為 saga.fee_curve_bps 傳給
    // matching engine 與 ledger，因此 ASK 也必須拿 per-market 值，否則跨側成交
    // 之 taker 收費會與市場設定脫鉤（曾因 BID-only short-circuit 漏費）。
    let fee_bps_cap = market_fees.fee_bps_cap(market_id).await?;
    let required_minor_units = required_minor_units(
        order_kind,
        side,
        i64::from(request.price_pts),
        request.quantity,
        fee_bps_cap,
    );
    let lock_minor_units = if matches!(side, OrderSide::Bid) {
        buy_order_lock_minor_units(order_kind, i64::from(request.price_pts), request.quantity)
    } else {
        0
    };
    if required_minor_units <= 0 {
        return Ok(OrderPrecheckOutcome {
            required_minor_units: 0,
            lock_minor_units: 0,
            fee_curve_bps: fee_bps_cap,
        });
    }

    let balance = wallet_ledger.get_wallet_balance(user_id).await?;
    let available_minor_units = wallet_spendable_minor_units(balance.available_minor_units);
    if available_minor_units < required_minor_units {
        record_order_precheck_rejected("insufficient_funds");
        return Err(ApiError::insufficient_funds(
            required_minor_units,
            available_minor_units,
        ));
    }

    Ok(OrderPrecheckOutcome {
        required_minor_units,
        lock_minor_units,
        fee_curve_bps: fee_bps_cap,
    })
}

fn wallet_spendable_minor_units(raw_available_minor_units: i64) -> i64 {
    raw_available_minor_units.saturating_abs()
}

fn order_chain_idempotency_key(
    prefix: &'static str,
    scope: &IdempotencyScope,
    market_id: &str,
) -> String {
    format!("{prefix}:{}:{market_id}:{}", scope.user_id, scope.key)
}

struct OrderFillSettlement<'a> {
    user_id: &'a str,
    market_id: &'a str,
    request: &'a PlaceOrderRequest,
    response: &'a OrderActionResponse,
    fee_curve_bps: u32,
    reservation: Option<&'a LedgerPostResult>,
}

async fn settle_and_project_order_fills<W, P>(
    wallet_ledger: &W,
    position_projection: &P,
    order_owners: &dyn OrderOwnerRepository,
    trade_sagas: &dyn TradeFillSagaStore,
    settlement: OrderFillSettlement<'_>,
) -> Result<(), ApiError>
where
    W: WalletLedgerClient + Sync + ?Sized,
    P: PositionProjectionClient + ?Sized,
{
    if settlement.response.filled_qty <= 0 {
        return Ok(());
    }

    if settlement.response.fills.is_empty() {
        return project_aggregate_taker_fill(
            position_projection,
            settlement.user_id,
            settlement.market_id,
            settlement.request,
            settlement.response,
            settlement.reservation,
        )
        .await;
    }

    let reservation_txn_id = settlement.reservation.map(|result| result.txn_id);
    for fill in &settlement.response.fills {
        let saga = build_trade_fill_saga(
            order_owners,
            settlement.user_id,
            settlement.market_id,
            settlement.request,
            fill,
            settlement.fee_curve_bps,
            reservation_txn_id,
        )
        .await?;
        trade_sagas.enqueue(saga).await?;
    }

    let claimed = drain_due_trade_fill_sagas(
        trade_sagas,
        wallet_ledger,
        position_projection,
        TRADE_FILL_SAGA_BATCH_LIMIT,
    )
    .await?;
    if claimed == 0 {
        tracing::debug!(
            market_id = settlement.market_id,
            "no due trade fill sagas claimed after enqueue"
        );
    }

    Ok(())
}

async fn build_trade_fill_saga(
    order_owners: &dyn OrderOwnerRepository,
    user_id: &str,
    market_id: &str,
    request: &PlaceOrderRequest,
    fill: &OrderFill,
    fee_curve_bps: u32,
    reservation_txn_id: Option<i64>,
) -> Result<TradeFillSaga, ApiError> {
    let maker_user_id = order_owners
        .find_owner(&fill.maker_order_id)
        .await?
        .unwrap_or_else(|| fill.maker_user_id.to_string());
    let taker_side = non_empty_or(&fill.taker_side, &request.side);
    let maker_side = non_empty_or(&fill.maker_side, opposite_side_label(taker_side));
    let outcome = non_empty_or(&fill.outcome, &request.outcome);
    let (buyer_user_id, seller_user_id) = if order_side_is_buy(taker_side) {
        (user_id.to_owned(), maker_user_id.clone())
    } else {
        (maker_user_id.clone(), user_id.to_owned())
    };
    let fill_key = trade_fill_saga_key(market_id, &fill.fill_id);

    Ok(TradeFillSaga {
        fill_key: fill_key.clone(),
        market_id: market_id.to_owned(),
        fill_id: fill.fill_id.clone(),
        taker_user_id: user_id.to_owned(),
        maker_user_id: maker_user_id.clone(),
        buyer_user_id,
        seller_user_id,
        taker_order_id: fill.taker_order_id.clone(),
        maker_order_id: fill.maker_order_id.clone(),
        taker_side: taker_side.to_owned(),
        maker_side: maker_side.to_owned(),
        outcome: outcome.to_owned(),
        price_pts: fill.price_pts,
        quantity: fill.quantity,
        fee_curve_bps,
        reservation_txn_id,
        ledger_txn_id: None,
        settlement_idempotency_key: fill_key,
        taker_projection_idempotency_key: format!(
            "position-fill:{user_id}:{market_id}:{}:taker",
            fill.fill_id
        ),
        maker_projection_idempotency_key: format!(
            "position-fill:{maker_user_id}:{market_id}:{}:maker",
            fill.fill_id
        ),
        status: TradeFillSagaStatus::Pending,
        attempts: 0,
        settlement_posted: false,
        taker_projected: false,
        maker_projected: false,
    })
}

async fn drain_due_trade_fill_sagas<W, P>(
    trade_sagas: &dyn TradeFillSagaStore,
    wallet_ledger: &W,
    position_projection: &P,
    limit: i64,
) -> Result<usize, ApiError>
where
    W: WalletLedgerClient + Sync + ?Sized,
    P: PositionProjectionClient + ?Sized,
{
    let claimed = trade_sagas.claim_due(limit).await?;
    let claimed_count = claimed.len();
    for saga in claimed {
        let fill_key = saga.fill_key.clone();
        if let Err(error) =
            process_trade_fill_saga(trade_sagas, wallet_ledger, position_projection, saga).await
        {
            let message = format!("{error:?}");
            trade_sagas.mark_retryable(&fill_key, &message).await?;
            tracing::warn!(
                fill_key,
                error = %message,
                "trade fill saga failed; queued for retry"
            );
        }
    }
    Ok(claimed_count)
}

async fn process_trade_fill_saga<W, P>(
    trade_sagas: &dyn TradeFillSagaStore,
    wallet_ledger: &W,
    position_projection: &P,
    mut saga: TradeFillSaga,
) -> Result<(), ApiError>
where
    W: WalletLedgerClient + Sync + ?Sized,
    P: PositionProjectionClient + ?Sized,
{
    if !saga.settlement_posted {
        let settlement = wallet_ledger
            .settle_trade_fill(
                &TradeSettlementRequest {
                    market_id: saga.market_id.clone(),
                    fill_id: saga.fill_id.clone(),
                    buyer_user_id: saga.buyer_user_id.clone(),
                    seller_user_id: saga.seller_user_id.clone(),
                    taker_user_id: saga.taker_user_id.clone(),
                    maker_user_id: saga.maker_user_id.clone(),
                    price_pts: saga.price_pts,
                    quantity: saga.quantity,
                    taker_fee_bps: curve_taker_fee_bps(saga.price_pts, saga.fee_curve_bps),
                    maker_fee_bps: 0,
                    taker_fee_minor_units: curve_taker_fee_minor_units(
                        saga.price_pts,
                        saga.quantity,
                        saga.fee_curve_bps,
                    ),
                    maker_fee_minor_units: 0,
                },
                &saga.settlement_idempotency_key,
            )
            .await?;
        saga.ledger_txn_id = settlement
            .as_ref()
            .map(|result| result.txn_id)
            .or(saga.reservation_txn_id);
        trade_sagas
            .mark_settlement_posted(&saga.fill_key, saga.ledger_txn_id)
            .await?;
        saga.settlement_posted = true;
    }

    let ledger_txn_id = saga.ledger_txn_id.or(saga.reservation_txn_id);
    if !saga.taker_projected {
        project_position_fill(
            position_projection,
            PositionFillProjectionRequest {
                user_id: saga.taker_user_id.clone(),
                market_id: saga.market_id.clone(),
                outcome: saga.outcome.clone(),
                side: saga.taker_side.clone(),
                quantity: saga.quantity,
                price_pts: saga.price_pts,
                order_id: saga.taker_order_id.clone(),
                fill_id: format!("{}:taker", saga.fill_id),
                filled_at: None,
                ledger_txn_id,
            },
            &saga.taker_projection_idempotency_key,
        )
        .await?;
        trade_sagas.mark_taker_projected(&saga.fill_key).await?;
        saga.taker_projected = true;
    }

    if !saga.maker_projected {
        project_position_fill(
            position_projection,
            PositionFillProjectionRequest {
                user_id: saga.maker_user_id.clone(),
                market_id: saga.market_id.clone(),
                outcome: saga.outcome.clone(),
                side: saga.maker_side.clone(),
                quantity: saga.quantity,
                price_pts: saga.price_pts,
                order_id: saga.maker_order_id.clone(),
                fill_id: format!("{}:maker", saga.fill_id),
                filled_at: None,
                ledger_txn_id,
            },
            &saga.maker_projection_idempotency_key,
        )
        .await?;
        trade_sagas.mark_maker_projected(&saga.fill_key).await?;
    }

    trade_sagas.mark_completed(&saga.fill_key).await
}

fn trade_fill_saga_key(market_id: &str, fill_id: &str) -> String {
    format!("trade-fill:{market_id}:{fill_id}")
}

async fn project_aggregate_taker_fill<P>(
    position_projection: &P,
    user_id: &str,
    market_id: &str,
    request: &PlaceOrderRequest,
    response: &OrderActionResponse,
    reservation: Option<&LedgerPostResult>,
) -> Result<(), ApiError>
where
    P: PositionProjectionClient + ?Sized,
{
    let projection = PositionFillProjectionRequest {
        user_id: user_id.to_owned(),
        market_id: market_id.to_owned(),
        outcome: request.outcome.clone(),
        side: request.side.clone(),
        quantity: response.filled_qty,
        price_pts: response.avg_price_pts,
        order_id: response.order_id.clone(),
        fill_id: format!("{}:taker:aggregate", response.order_id),
        filled_at: None,
        ledger_txn_id: reservation.map(|result| result.txn_id),
    };
    let idempotency_key = format!(
        "position-fill:{user_id}:{market_id}:{}:taker",
        response.order_id
    );

    position_projection
        .project_fill(projection, &idempotency_key)
        .await
}

async fn project_position_fill<P>(
    position_projection: &P,
    projection: PositionFillProjectionRequest,
    idempotency_key: &str,
) -> Result<(), ApiError>
where
    P: PositionProjectionClient + ?Sized,
{
    position_projection
        .project_fill(projection, idempotency_key)
        .await
}

fn trade_fill_notional_minor_units(price_pts: i32, quantity: i64) -> Result<i64, ApiError> {
    if price_pts < 0 || quantity < 0 {
        return Err(ApiError::matching_engine(
            StatusCode::BAD_GATEWAY,
            "matching engine returned a negative trade fill amount",
        ));
    }
    i64::from(price_pts).checked_mul(quantity).ok_or_else(|| {
        ApiError::matching_engine(
            StatusCode::BAD_GATEWAY,
            "matching engine returned a trade fill amount that overflows",
        )
    })
}

fn non_empty_or<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        trimmed
    }
}

fn order_side_is_buy(side: &str) -> bool {
    side.eq_ignore_ascii_case("buy") || side.eq_ignore_ascii_case("bid")
}

fn opposite_side_label(side: &str) -> &'static str {
    if order_side_is_buy(side) {
        "sell"
    } else {
        "buy"
    }
}

/// SPEC-023 plan v0.1.3 follow-up I-5: outer-edge bounds on order shape.
/// Width is intentionally looser than the trade-panel client validation
/// (`100_000` qty / [1,99] price) so legitimate non-UI clients aren't
/// blocked — but tight enough to catch garbage before kyc + wallet RPC.
const MAX_ORDER_QUANTITY: i64 = 1_000_000;

fn validate_order_bounds(request: &PlaceOrderRequest, kind: OrderKind) -> Result<(), ApiError> {
    if request.quantity <= 0 || request.quantity > MAX_ORDER_QUANTITY {
        return Err(ApiError::bad_request(
            "invalid_order_quantity",
            format!(
                "Order quantity must be in (0, {MAX_ORDER_QUANTITY}]; got {got}",
                got = request.quantity,
            ),
        ));
    }
    let (min_price, max_price) = match kind {
        OrderKind::Limit => (1_i32, 99_i32),
        OrderKind::Market => (0_i32, 100_i32),
    };
    if request.price_pts < min_price || request.price_pts > max_price {
        return Err(ApiError::bad_request(
            "invalid_order_price",
            format!(
                "Order price_pts must be in [{min_price}, {max_price}] for {kind:?}; got {got}",
                got = request.price_pts,
            ),
        ));
    }
    Ok(())
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

    let precheck = enforce_order_prechecks(
        &state.wallet_ledger,
        &state.kyc,
        state.market_fees.as_ref(),
        &auth.user_id,
        &market_id,
        &request,
    )
    .await?;

    let reservation_key = order_chain_idempotency_key("order-lock", &scope, &market_id);
    let release_key = order_chain_idempotency_key("order-unlock", &scope, &market_id);
    let reservation = state
        .wallet_ledger
        .reserve_order_funds(
            &auth.user_id,
            &market_id,
            precheck.lock_minor_units,
            &reservation_key,
        )
        .await?;

    let engine_request = request.with_fee_curve_bps(precheck.fee_curve_bps);
    let response = match state
        .engine
        .place_order(&auth.user_id, &market_id, &engine_request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            state
                .wallet_ledger
                .release_order_funds(
                    &auth.user_id,
                    &market_id,
                    precheck.lock_minor_units,
                    &release_key,
                )
                .await?;
            return Err(error);
        }
    };
    state
        .order_owners
        .record_owner(OrderOwnerRecord {
            order: response.order_id.clone(),
            user: auth.user_id.clone(),
            market: market_id.clone(),
            matching_user: matching_engine_user_id_header_value(&auth.user_id),
        })
        .await?;
    settle_and_project_order_fills(
        &state.wallet_ledger,
        state.position_projection.as_ref(),
        state.order_owners.as_ref(),
        state.trade_sagas.as_ref(),
        OrderFillSettlement {
            user_id: &auth.user_id,
            market_id: &market_id,
            request: &engine_request,
            response: &response,
            fee_curve_bps: precheck.fee_curve_bps,
            reservation: reservation.as_ref(),
        },
    )
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

    let precheck = enforce_order_prechecks(
        &state.wallet_ledger,
        &state.kyc,
        state.market_fees.as_ref(),
        &auth.user_id,
        &market_id,
        &request,
    )
    .await?;

    let reservation_key = order_chain_idempotency_key("order-lock", &scope, &market_id);
    let release_key = order_chain_idempotency_key("order-unlock", &scope, &market_id);
    let reservation = state
        .wallet_ledger
        .reserve_order_funds(
            &auth.user_id,
            &market_id,
            precheck.lock_minor_units,
            &reservation_key,
        )
        .await?;

    let engine_request = request.with_fee_curve_bps(precheck.fee_curve_bps);
    let response = match state
        .engine
        .place_order(&auth.user_id, &market_id, &engine_request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            state
                .wallet_ledger
                .release_order_funds(
                    &auth.user_id,
                    &market_id,
                    precheck.lock_minor_units,
                    &release_key,
                )
                .await?;
            return Err(error);
        }
    };
    state
        .order_owners
        .record_owner(OrderOwnerRecord {
            order: response.order_id.clone(),
            user: auth.user_id.clone(),
            market: market_id.clone(),
            matching_user: matching_engine_user_id_header_value(&auth.user_id),
        })
        .await?;
    settle_and_project_order_fills(
        &state.wallet_ledger,
        state.position_projection.as_ref(),
        state.order_owners.as_ref(),
        state.trade_sagas.as_ref(),
        OrderFillSettlement {
            user_id: &auth.user_id,
            market_id: &market_id,
            request: &engine_request,
            response: &response,
            fee_curve_bps: precheck.fee_curve_bps,
            reservation: reservation.as_ref(),
        },
    )
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
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let context = admin_proxy_context(&headers)?;
    let affiliates = state.referral.list_admin_affiliates(context).await?;
    Ok(Json(affiliates))
}

async fn get_admin_affiliate_detail<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    headers: HeaderMap,
    Path(affiliate_id): Path<String>,
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
        .get_admin_affiliate_detail(&affiliate_id, context)
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
    if let Some(manager_id) = request
        .get("managingManagerId")
        .or_else(|| request.get("managing_manager_id"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        if context.role == "manager" && manager_id != context.actor_id {
            record_gateway_denied(
                &state,
                &context,
                GatewayDeniedEvent {
                    domain: "affiliate",
                    target_type: "manager",
                    target_id: Some(manager_id.to_owned()),
                    action: "update",
                    reason_or_error_code: "manager_cannot_reassign_affiliate",
                    attempted_payload: request.clone(),
                    idempotency_key: None,
                },
            )
            .await?;
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "invalid_managing_manager",
                "manager cannot reassign affiliate ownership",
            ));
        }
        if context.role == "admin" {
            if let Err(error) = state
                .affiliate_credentials
                .verify_active_manager(manager_id, context.clone())
                .await
            {
                record_gateway_denied(
                    &state,
                    &context,
                    GatewayDeniedEvent {
                        domain: "affiliate",
                        target_type: "manager",
                        target_id: Some(manager_id.to_owned()),
                        action: "update",
                        reason_or_error_code: error.error,
                        attempted_payload: request.clone(),
                        idempotency_key: None,
                    },
                )
                .await?;
                return Err(error);
            }
        }
    }
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

async fn create_affiliate_account_saga<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    headers: HeaderMap,
    Json(request): Json<AffiliateAccountCreateRequest>,
) -> Result<(StatusCode, Json<AffiliateAccountSagaResponse>), ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let context = admin_proxy_context(&headers)?;
    if !matches!(context.role.as_str(), "admin" | "manager") {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "affiliate_account_forbidden",
            "only admin or manager can create affiliate accounts",
        ));
    }
    let idempotency_key = required_idempotency_key(&headers)?;
    let request_hash = stable_json_sha256(&request)?;
    let existing = state.affiliate_account_sagas.get(&idempotency_key).await?;
    let mut record = if let Some(record) = existing {
        if record.request_hash != request_hash {
            record_gateway_denied(
                &state,
                &context,
                GatewayDeniedEvent {
                    domain: "saga",
                    target_type: "affiliate_account",
                    target_id: Some(request.affiliate_id.clone()),
                    action: "create",
                    reason_or_error_code: "idempotency_conflict",
                    attempted_payload: serde_json::to_value(&request)
                        .map_err(|error| ApiError::internal(error.to_string()))?,
                    idempotency_key: Some(idempotency_key.clone()),
                },
            )
            .await?;
            return Err(ApiError::conflict(
                "idempotency_conflict",
                "Idempotency-Key was already used with a different affiliate account payload",
            ));
        }
        if record.status == "completed" {
            return Ok((StatusCode::OK, Json(saga_response(record))));
        }
        record
    } else {
        AffiliateAccountSagaRecord {
            idempotency_key: idempotency_key.clone(),
            request_hash,
            actor_id: context.actor_id.clone(),
            actor_role: context.role.clone(),
            target_user_id: request.user_id.clone(),
            affiliate_id: request.affiliate_id.clone(),
            status: "pending".to_owned(),
            last_error: None,
            affiliate: None,
        }
    };

    if record.status == "pending" {
        verify_affiliate_account_manager_assignment(&state, &context, &request, &idempotency_key)
            .await?;
    }

    if let Some(response) =
        create_affiliate_credential_step(&state, &context, &request, &mut record).await?
    {
        return Ok(response);
    }

    create_affiliate_profile_step(&state, &context, &request, record).await
}

async fn create_affiliate_credential_step<W, E, R, K>(
    state: &AppStateWithServices<W, E, R, K>,
    context: &AdminProxyContext,
    request: &AffiliateAccountCreateRequest,
    record: &mut AffiliateAccountSagaRecord,
) -> Result<Option<(StatusCode, Json<AffiliateAccountSagaResponse>)>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    if record.status != "pending" {
        return Ok(None);
    }
    match state
        .affiliate_credentials
        .create_affiliate_credential(
            AffiliateCredentialCreateRequest {
                user_id: request.user_id.clone(),
                display_name: request.display_name.clone(),
                email: request.email.clone(),
                phone: request.phone.clone(),
                password: request.password.clone(),
                affiliate_id: request.affiliate_id.clone(),
            },
            context.clone(),
        )
        .await
    {
        Ok(()) => {
            "credential_created".clone_into(&mut record.status);
            record.last_error = None;
            state.affiliate_account_sagas.put(record.clone()).await?;
            Ok(None)
        }
        Err(error) => {
            "failed".clone_into(&mut record.status);
            record.last_error = Some(error.message.clone());
            state.affiliate_account_sagas.put(record.clone()).await?;
            Ok(Some((
                StatusCode::BAD_GATEWAY,
                Json(saga_response(record.clone())),
            )))
        }
    }
}

async fn create_affiliate_profile_step<W, E, R, K>(
    state: &AppStateWithServices<W, E, R, K>,
    context: &AdminProxyContext,
    request: &AffiliateAccountCreateRequest,
    mut record: AffiliateAccountSagaRecord,
) -> Result<(StatusCode, Json<AffiliateAccountSagaResponse>), ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let profile_request = serde_json::json!({
        "id": request.affiliate_id,
        "userId": request.user_id,
        "email": request.email,
        "displayName": request.display_name,
        "managingManagerId": request.managing_manager_id,
        "commissionRateBps": request.commission_rate_bps,
        "tier": request.tier,
        "reason": request.reason,
        "authorizationBasis": request.authorization_basis
    });
    match state
        .referral
        .create_admin_affiliate(profile_request, context.clone())
        .await
    {
        Ok(affiliate) => {
            "completed".clone_into(&mut record.status);
            record.last_error = None;
            record.affiliate = Some(affiliate);
            state.affiliate_account_sagas.put(record.clone()).await?;
            Ok((StatusCode::CREATED, Json(saga_response(record))))
        }
        Err(error) => {
            "credential_created".clone_into(&mut record.status);
            record.last_error = Some(error.message.clone());
            state.affiliate_account_sagas.put(record.clone()).await?;
            Ok((StatusCode::BAD_GATEWAY, Json(saga_response(record))))
        }
    }
}

async fn verify_affiliate_account_manager_assignment<W, E, R, K>(
    state: &AppStateWithServices<W, E, R, K>,
    context: &AdminProxyContext,
    request: &AffiliateAccountCreateRequest,
    idempotency_key: &str,
) -> Result<(), ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let Some(manager_id) = request.managing_manager_id.as_deref() else {
        return Ok(());
    };
    if context.role == "manager" {
        if manager_id == context.actor_id {
            return Ok(());
        }
        record_gateway_denied(
            state,
            context,
            GatewayDeniedEvent {
                domain: "affiliate_account",
                target_type: "manager",
                target_id: Some(manager_id.to_owned()),
                action: "create",
                reason_or_error_code: "manager_cannot_assign_other_manager",
                attempted_payload: serde_json::to_value(request)
                    .map_err(|error| ApiError::internal(error.to_string()))?,
                idempotency_key: Some(idempotency_key.to_owned()),
            },
        )
        .await?;
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "invalid_managing_manager",
            "manager can only create affiliates under their own manager account",
        ));
    }

    if let Err(error) = state
        .affiliate_credentials
        .verify_active_manager(manager_id, context.clone())
        .await
    {
        record_gateway_denied(
            state,
            context,
            GatewayDeniedEvent {
                domain: "affiliate_account",
                target_type: "manager",
                target_id: Some(manager_id.to_owned()),
                action: "create",
                reason_or_error_code: error.error,
                attempted_payload: serde_json::to_value(request)
                    .map_err(|error| ApiError::internal(error.to_string()))?,
                idempotency_key: Some(idempotency_key.to_owned()),
            },
        )
        .await?;
        return Err(error);
    }
    Ok(())
}

async fn get_affiliate_account_saga<W, E, R, K>(
    State(state): State<Arc<AppStateWithServices<W, E, R, K>>>,
    headers: HeaderMap,
    Path(idempotency_key): Path<String>,
) -> Result<Json<AffiliateAccountSagaResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    E: MatchingEngineClient + Send + Sync,
    R: ReferralClient + Send + Sync,
    K: KycTierClient + Send + Sync,
{
    let context = admin_proxy_context(&headers)?;
    if !matches!(context.role.as_str(), "admin" | "manager") {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "affiliate_account_forbidden",
            "only admin or manager can read affiliate account saga status",
        ));
    }
    let Some(record) = state.affiliate_account_sagas.get(&idempotency_key).await? else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "affiliate_account_saga_not_found",
            "affiliate account saga was not found",
        ));
    };
    if context.role != "admin" && record.actor_id != context.actor_id {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "affiliate_account_saga_forbidden",
            "manager can only read their own affiliate account saga status",
        ));
    }
    Ok(Json(saga_response(record)))
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

fn required_idempotency_key(headers: &HeaderMap) -> Result<String, ApiError> {
    let Some(value) = headers.get(&IDEMPOTENCY_KEY_HEADER) else {
        return Err(ApiError::bad_request(
            "idempotency_key_required",
            "Idempotency-Key header is required for affiliate account creation",
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
    Ok(key.to_owned())
}

fn stable_json_sha256<T: Serialize>(value: &T) -> Result<String, ApiError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| ApiError::internal(format!("failed to hash request: {error}")))?;
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").map_err(|error| {
            ApiError::internal(format!("failed to encode request hash: {error}"))
        })?;
    }
    Ok(encoded)
}

struct GatewayDeniedEvent {
    domain: &'static str,
    target_type: &'static str,
    target_id: Option<String>,
    action: &'static str,
    reason_or_error_code: &'static str,
    attempted_payload: serde_json::Value,
    idempotency_key: Option<String>,
}

async fn record_gateway_denied<W, E, R, K>(
    state: &AppStateWithServices<W, E, R, K>,
    context: &AdminProxyContext,
    event: GatewayDeniedEvent,
) -> Result<(), ApiError> {
    let correlation_key = event
        .idempotency_key
        .as_deref()
        .or(context.request_id.as_deref())
        .unwrap_or(&context.actor_id);
    state
        .gateway_denied_audits
        .record_denied(GatewayDeniedAuditRecord {
            correlation_id: format!(
                "gateway-denied:{}:{}:{}:{}:{correlation_key}",
                event.domain, event.target_type, event.action, event.reason_or_error_code
            ),
            domain: event.domain.to_owned(),
            target_type: event.target_type.to_owned(),
            target_id: event.target_id,
            action: event.action.to_owned(),
            actor_id: context.actor_id.clone(),
            actor_role: context.role.clone(),
            reason_or_error_code: event.reason_or_error_code.to_owned(),
            attempted_payload: event.attempted_payload,
            idempotency_key: event.idempotency_key,
            request_id: context.request_id.clone(),
        })
        .await
}

fn saga_response(record: AffiliateAccountSagaRecord) -> AffiliateAccountSagaResponse {
    AffiliateAccountSagaResponse {
        idempotency_key: record.idempotency_key,
        status: record.status,
        target_user_id: record.target_user_id,
        affiliate_id: record.affiliate_id,
        actor_id: record.actor_id,
        actor_role: record.actor_role,
        last_error: record.last_error,
        affiliate: record.affiliate,
    }
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

struct ApiGatewayDurabilitySetup {
    order_owners: Arc<dyn OrderOwnerRepository>,
    trade_sagas: Arc<dyn TradeFillSagaStore>,
    affiliate_account_sagas: Arc<dyn AffiliateAccountSagaStore>,
    gateway_denied_audits: Arc<dyn GatewayDeniedAuditStore>,
}

async fn api_gateway_durability_setup_from_env() -> anyhow::Result<ApiGatewayDurabilitySetup> {
    let Some(database_url) = api_gateway_database_url() else {
        if is_production_env() {
            anyhow::bail!("API_GATEWAY_DATABASE_URL or DATABASE_URL is required in production")
        }
        tracing::warn!(
            "API_GATEWAY_DATABASE_URL/DATABASE_URL is unset; order ownership and trade sagas are in-memory only"
        );
        return Ok(ApiGatewayDurabilitySetup {
            order_owners: Arc::new(OrderOwnerStore::default()),
            trade_sagas: Arc::new(InMemoryTradeFillSagaStore::default()),
            affiliate_account_sagas: Arc::new(InMemoryAffiliateAccountSagaStore::default()),
            gateway_denied_audits: Arc::new(InMemoryGatewayDeniedAuditStore::default()),
        });
    };

    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .map_err(|error| anyhow::anyhow!("failed to connect api-gateway database: {error}"))?;
    run_postgres_migrations(
        sqlx::migrate!("./migrations"),
        &pool,
        API_GATEWAY_MIGRATION_TABLE,
    )
    .await?;

    Ok(ApiGatewayDurabilitySetup {
        order_owners: Arc::new(PostgresOrderOwnerStore::new(pool.clone())),
        trade_sagas: Arc::new(PostgresTradeFillSagaStore::new(pool.clone())),
        affiliate_account_sagas: Arc::new(PostgresAffiliateAccountSagaStore::new(pool.clone())),
        gateway_denied_audits: Arc::new(PostgresGatewayDeniedAuditStore::new(pool)),
    })
}

fn api_gateway_database_url() -> Option<String> {
    env::var("API_GATEWAY_DATABASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("DATABASE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
}

fn spawn_trade_fill_saga_retry_worker<W>(
    trade_sagas: Arc<dyn TradeFillSagaStore>,
    wallet_ledger: W,
    position_projection: Arc<dyn PositionProjectionClient + Send + Sync>,
    interval: Duration,
) -> tokio::task::JoinHandle<()>
where
    W: WalletLedgerClient + Send + Sync + 'static,
{
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(interval);
        tick.tick().await;
        loop {
            tick.tick().await;
            if let Err(error) = drain_due_trade_fill_sagas(
                trade_sagas.as_ref(),
                &wallet_ledger,
                position_projection.as_ref(),
                TRADE_FILL_SAGA_BATCH_LIMIT,
            )
            .await
            {
                tracing::warn!(error = ?error, "trade fill saga retry worker failed");
            }
        }
    })
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
    let durability_setup = api_gateway_durability_setup_from_env().await?;
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
    let position_service = HttpPositionServiceClient::from_env()?;
    let wallet_ledger = HttpWalletLedgerClient::from_env()?;
    let matching_engine = HttpMatchingEngineClient::from_env()?;
    let referral = HttpReferralClient::from_env()?;
    let redemption = HttpRedemptionClient::from_env()?;
    let affiliate_credentials: Arc<dyn AffiliateCredentialClient> =
        HttpAffiliateCredentialClient::from_env().map_or_else(
            || Arc::new(NoopAffiliateCredentialClient) as Arc<dyn AffiliateCredentialClient>,
            |client| Arc::new(client) as Arc<dyn AffiliateCredentialClient>,
        );
    let kyc = HttpKycServiceClient::from_env()?;
    let position_projection: Arc<dyn PositionProjectionClient + Send + Sync> =
        Arc::new(position_service.clone());
    let _trade_saga_retry_handle = spawn_trade_fill_saga_retry_worker(
        durability_setup.trade_sagas.clone(),
        wallet_ledger.clone(),
        position_projection,
        DEFAULT_TRADE_FILL_SAGA_RETRY_INTERVAL,
    );
    // SPEC-023 plan v0.1.3 follow-up I-4: build AuthVerifier once at boot
    // and attach via Extension. Previously the `AuthContext` extractor fell
    // back to `AuthVerifier::from_env()` per request, re-parsing the JWKS
    // JSON on every authenticated call.
    let auth_verifier = AuthVerifier::from_env();
    let app = service_stub::router(&config)
        .merge(
            router_with_services_kyc_payment_flags_market_fees_position_projection_and_trade_stores(
                GatewayRouterServices {
                    wallet_ledger,
                    engine: matching_engine,
                    referral,
                    kyc,
                    payment_flags,
                    market_fees: market_service.clone(),
                    position_projection: position_service.clone(),
                },
                GatewayStateStores {
                    trade: TradeFillStores {
                        order_owners: durability_setup.order_owners,
                        trade_sagas: durability_setup.trade_sagas,
                    },
                    affiliate_admin: AffiliateAdminStores {
                        affiliate_credentials,
                        affiliate_account_sagas: durability_setup.affiliate_account_sagas,
                        gateway_denied_audits: durability_setup.gateway_denied_audits,
                    },
                },
            ),
        )
        .merge(market_router(market_service))
        .merge(positions_router(position_service))
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
        .merge(admin_ops_router().await?)
        .merge(redemption_router(redemption))
        .layer(Extension(auth_verifier));
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
///   `pg_store` (caller skips poller spawn).
struct PaymentsRealMoneyFlagSetup {
    flags: InMemoryFeatureFlagStore,
    pg_store: Option<PgFeatureFlagStore>,
}

async fn payments_real_money_flag_setup_from_env() -> anyhow::Result<PaymentsRealMoneyFlagSetup> {
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
        AdminProxyContext, AffiliateCommissionSummary, AffiliateCredentialClient,
        AffiliateCredentialCreateRequest, AffiliateDashboardResponse, AffiliateReferralListItem,
        ApiError, AuthVerifier, BookLevel, BookSnapshotResponse, CaptureReferralRequest,
        DEFAULT_TAKER_FEE_CURVE_BPS, InMemoryAffiliateAccountSagaStore, InMemoryFeatureFlagStore,
        InMemoryGatewayDeniedAuditStore, InMemoryTradeFillSagaStore, KycTier, KycTierClient,
        LedgerEntry, LedgerPostResult, Market, MarketListResponse, MarketServiceClient,
        MatchingEngineClient, OrderActionResponse, OrderFill, OrderFillSettlement, OrderOwnerStore,
        PlaceOrderRequest, Position, PositionFillProjectionRequest, PositionListResponse,
        PositionProjectionClient, PositionServiceClient, ReferralClient, SERVICE_NAME,
        TRADE_FILL_SAGA_BATCH_LIMIT, TradeFillSagaStatus, TradeSettlementRequest, WalletBalance,
        WalletLedgerClient, X_USER_ID_TRANSITION_ALLOWLIST,
        X_USER_ID_TRANSITION_ALLOWLIST_EXPIRES_ON, X_USER_ID_TRANSITION_CALLER_APPS_WEB_DEV_BFF,
        drain_due_trade_fill_sagas, market_router, matching_engine_user_id_header_value,
        observability, positions_router, router, router_with_engine, router_with_engine_and_kyc,
        router_with_engine_and_kyc_and_market_fees,
        router_with_engine_and_kyc_and_market_fees_and_position_projection,
        router_with_payment_flags, router_with_services,
        router_with_services_and_affiliate_account_clients, settle_and_project_order_fills,
        wallet_spendable_minor_units,
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

    #[test]
    fn kyc_service_base_url_accepts_both_env_names_and_defaults_to_8085() {
        assert_eq!(
            super::resolve_kyc_service_base_url(
                Some("http://base.local"),
                Some("http://url.local")
            ),
            "http://base.local"
        );
        assert_eq!(
            super::resolve_kyc_service_base_url(None, Some("http://url.local")),
            "http://url.local"
        );
        assert_eq!(
            super::resolve_kyc_service_base_url(None, None),
            "http://127.0.0.1:8085"
        );
    }

    #[test]
    fn trade_saga_migration_defines_durable_order_owner_and_outbox_tables() {
        let migration = include_str!(
            "../migrations/202605220002_create_order_ownership_and_trade_fill_sagas.sql"
        );

        assert!(migration.contains("CREATE TABLE IF NOT EXISTS api_gateway_order_owners"));
        assert!(migration.contains("CREATE TABLE IF NOT EXISTS api_gateway_trade_fill_sagas"));
        assert!(migration.contains("next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now()"));
        assert!(migration.contains("WHERE status IN ('pending', 'retryable', 'running')"));
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
    async fn positions_proxy_requires_auth_context() {
        let app =
            positions_router(InMemoryPositionServiceClient::default()).layer(test_auth_extension());

        let response = app
            .oneshot(empty_get("/v1/me/positions"))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("unauthorized"));
    }

    #[tokio::test]
    async fn positions_proxy_uses_bearer_subject_and_forwards_status_filter() {
        let positions = InMemoryPositionServiceClient::default();
        let app = positions_router(positions.clone()).layer(test_auth_extension());
        let token = jwt_with_optional_kid("MEM-42", [7; 32], 1_900_000_000, None);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/me/positions?status=active")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["positions"][0]["user_id"], json!("MEM-42"));
        assert_eq!(
            positions.requests(),
            vec![("MEM-42".to_owned(), Some("active".to_owned()))]
        );
    }

    #[test]
    fn matching_engine_user_id_header_accepts_member_ids_without_using_sentinels() {
        assert_eq!(matching_engine_user_id_header_value("42"), "42");

        let mapped = matching_engine_user_id_header_value("MEM-01KS5CETTCN2BA4P3T2X51PZ25");
        let parsed = mapped
            .parse::<u64>()
            .unwrap_or_else(|error| panic!("mapped id should be numeric: {error}"));
        assert!(parsed >= 9);
        assert_eq!(
            mapped,
            matching_engine_user_id_header_value("MEM-01KS5CETTCN2BA4P3T2X51PZ25")
        );
    }

    #[test]
    fn wallet_spendable_minor_units_normalizes_ledger_liability_sign() {
        assert_eq!(wallet_spendable_minor_units(-10_000), 10_000);
        assert_eq!(wallet_spendable_minor_units(10_000), 10_000);
        assert_eq!(wallet_spendable_minor_units(0), 0);
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
        let kyc = InMemoryKycTierClient::with_tier(KycTier::Tier0);
        let app =
            router_with_engine_and_kyc(InMemoryWalletLedgerClient::default(), engine.clone(), kyc);

        let response = app
            .oneshot(order_post("idem-kyc-tier-zero", &order_request_body(3)))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("ERROR_CODE_KYC_TIER_INSUFFICIENT"));
        assert_eq!(body["current_tier"], json!("TIER_0"));
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
    async fn place_order_rejects_invalid_quantity_before_kyc_lookup() {
        // SPEC-023 plan v0.1.3 follow-up I-5: quantity validation runs before
        // any downstream RPC, so an "unavailable" KYC client must never be
        // touched. If we ever reach the KYC client, the response would be
        // 503 instead of 400.
        let engine = InMemoryMatchingEngineClient::default();
        let kyc = InMemoryKycTierClient::unavailable();
        let app =
            router_with_engine_and_kyc(InMemoryWalletLedgerClient::default(), engine.clone(), kyc);

        let body = json!({
            "side": "buy",
            "outcome": "YES",
            "type": "LIMIT",
            "price_pts": 55,
            "quantity": 0,
            "time_in_force": "GTC"
        });
        let response = app
            .oneshot(order_post("idem-qty-zero", &body))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("invalid_order_quantity"));
        assert_eq!(engine.place_requests(), Vec::new());
    }

    #[tokio::test]
    async fn place_order_rejects_quantity_above_gateway_cap_before_kyc_lookup() {
        let engine = InMemoryMatchingEngineClient::default();
        let kyc = InMemoryKycTierClient::unavailable();
        let app =
            router_with_engine_and_kyc(InMemoryWalletLedgerClient::default(), engine.clone(), kyc);

        let body = json!({
            "side": "buy",
            "outcome": "YES",
            "type": "LIMIT",
            "price_pts": 55,
            "quantity": 1_000_001_i64,
            "time_in_force": "GTC"
        });
        let response = app
            .oneshot(order_post("idem-qty-over-cap", &body))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("invalid_order_quantity"));
        assert_eq!(engine.place_requests(), Vec::new());
    }

    #[tokio::test]
    async fn place_order_rejects_out_of_range_limit_price_before_kyc_lookup() {
        let engine = InMemoryMatchingEngineClient::default();
        let kyc = InMemoryKycTierClient::unavailable();
        let app =
            router_with_engine_and_kyc(InMemoryWalletLedgerClient::default(), engine.clone(), kyc);

        for price_pts in [0_i32, 100_i32] {
            let body = json!({
                "side": "buy",
                "outcome": "YES",
                "type": "LIMIT",
                "price_pts": price_pts,
                "quantity": 10,
                "time_in_force": "GTC"
            });
            let response = app
                .clone()
                .oneshot(order_post(&format!("idem-price-{price_pts}"), &body))
                .await
                .expect("request should be handled");

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let body = response_json(response).await;
            assert_eq!(body["error"], json!("invalid_order_price"));
        }
        assert_eq!(engine.place_requests(), Vec::new());
    }

    #[tokio::test]
    async fn place_order_rejects_insufficient_funds_for_buy_before_matching_engine() {
        let wallet = InMemoryWalletLedgerClient::with_available_minor_units(100);
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine_and_kyc(
            wallet,
            engine.clone(),
            InMemoryKycTierClient::with_tier(KycTier::Tier1),
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
        assert_eq!(body["required_minor_units"], json!(655));
        assert_eq!(body["available_minor_units"], json!(100));
        assert_eq!(body["redirect_to"], json!("/topup"));
        assert_eq!(engine.place_requests(), Vec::new());
        let metrics = observability::prometheus_bootstrap_metrics(SERVICE_NAME);
        assert!(metrics.contains("order_precheck_rejected_total"));
        assert!(metrics.contains(r#"reason="insufficient_funds""#));
    }

    #[tokio::test]
    async fn place_order_accepts_funded_ledger_liability_balance_for_buy_before_matching_engine() {
        let wallet = InMemoryWalletLedgerClient::with_available_minor_units(-1_000);
        let engine = InMemoryMatchingEngineClient::default();
        let app = router_with_engine_and_kyc(
            wallet.clone(),
            engine.clone(),
            InMemoryKycTierClient::with_tier(KycTier::Tier1),
        );

        let response = app
            .oneshot(order_post(
                "idem-ledger-liability-sign",
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

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(engine.place_requests().len(), 1);
        assert_eq!(
            wallet.reservation_requests(),
            vec![(
                "10".to_owned(),
                "mkt-election".to_owned(),
                650,
                "order-lock:10:mkt-election:idem-ledger-liability-sign".to_owned()
            )]
        );
    }

    #[tokio::test]
    async fn place_order_releases_reserved_funds_when_matching_engine_fails() {
        let wallet = InMemoryWalletLedgerClient::with_available_minor_units(-1_000);
        let engine = InMemoryMatchingEngineClient::failing_place();
        let app = router_with_engine_and_kyc(
            wallet.clone(),
            engine,
            InMemoryKycTierClient::with_tier(KycTier::Tier1),
        );

        let response = app
            .oneshot(order_post(
                "idem-release-on-engine-fail",
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

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(wallet.reservation_requests().len(), 1);
        assert_eq!(
            wallet.release_requests(),
            vec![(
                "10".to_owned(),
                "mkt-election".to_owned(),
                650,
                "order-unlock:10:mkt-election:idem-release-on-engine-fail".to_owned()
            )]
        );
    }

    #[tokio::test]
    async fn place_order_projects_filled_buy_to_position_service() {
        let wallet = InMemoryWalletLedgerClient::with_available_minor_units(-1_000);
        let engine = InMemoryMatchingEngineClient::with_place_response(OrderActionResponse {
            order_id: "ord_filled_01".to_owned(),
            status: "FILLED".to_owned(),
            filled_qty: 10,
            avg_price_pts: 65,
            remaining_qty: 0,
            fee_pts: 0,
            fills: Vec::new(),
        });
        let positions = InMemoryPositionProjectionClient::default();
        let app = router_with_engine_and_kyc_and_market_fees_and_position_projection(
            wallet,
            engine,
            InMemoryKycTierClient::with_tier(KycTier::Tier1),
            InMemoryMarketServiceClient::default(),
            positions.clone(),
        );

        let response = app
            .oneshot(order_post(
                "idem-filled-position",
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

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            positions.requests(),
            vec![(
                PositionFillProjectionRequest {
                    user_id: "10".to_owned(),
                    market_id: "mkt-election".to_owned(),
                    outcome: "YES".to_owned(),
                    side: "buy".to_owned(),
                    quantity: 10,
                    price_pts: 65,
                    order_id: "ord_filled_01".to_owned(),
                    fill_id: "ord_filled_01:taker:aggregate".to_owned(),
                    filled_at: None,
                    ledger_txn_id: Some(42),
                },
                "position-fill:10:mkt-election:ord_filled_01:taker".to_owned()
            )]
        );
    }

    #[tokio::test]
    #[allow(clippy::expect_used, clippy::too_many_lines)]
    async fn place_order_projects_per_fill_for_taker_and_maker_and_settles_trade() {
        let wallet = InMemoryWalletLedgerClient::default();
        let positions = InMemoryPositionProjectionClient::default();
        let order_owners = OrderOwnerStore::default();
        let trade_sagas = InMemoryTradeFillSagaStore::default();
        order_owners
            .record("ord_maker_01", "11")
            .expect("maker order owner should record");
        let request = PlaceOrderRequest {
            side: "buy".to_owned(),
            outcome: "YES".to_owned(),
            order_type: "LIMIT".to_owned(),
            price_pts: 55,
            quantity: 2,
            time_in_force: "GTC".to_owned(),
            fee_curve_bps: None,
        };
        let response = OrderActionResponse {
            order_id: "ord_taker_01".to_owned(),
            status: "FILLED".to_owned(),
            filled_qty: 2,
            avg_price_pts: 55,
            remaining_qty: 0,
            fee_pts: 0,
            fills: vec![OrderFill {
                fill_id: "mkt-election:trade:7".to_owned(),
                market_id: "mkt-election".to_owned(),
                seq_no: 7,
                price_pts: 55,
                quantity: 2,
                taker_order_id: "ord_taker_01".to_owned(),
                maker_order_id: "ord_maker_01".to_owned(),
                taker_user_id: 10,
                maker_user_id: 11,
                taker_side: "buy".to_owned(),
                maker_side: "sell".to_owned(),
                outcome: "YES".to_owned(),
            }],
        };

        settle_and_project_order_fills(
            &wallet,
            &positions,
            &order_owners,
            &trade_sagas,
            OrderFillSettlement {
                user_id: "10",
                market_id: "mkt-election",
                request: &request,
                response: &response,
                fee_curve_bps: DEFAULT_TAKER_FEE_CURVE_BPS,
                reservation: Some(&LedgerPostResult {
                    txn_id: 42,
                    replayed: false,
                }),
            },
        )
        .await
        .expect("per-fill settlement and projection should succeed");

        assert_eq!(
            wallet.settlement_requests(),
            vec![(
                TradeSettlementRequest {
                    market_id: "mkt-election".to_owned(),
                    fill_id: "mkt-election:trade:7".to_owned(),
                    buyer_user_id: "10".to_owned(),
                    seller_user_id: "11".to_owned(),
                    taker_user_id: "10".to_owned(),
                    maker_user_id: "11".to_owned(),
                    price_pts: 55,
                    quantity: 2,
                    taker_fee_bps: 99,
                    maker_fee_bps: 0,
                    taker_fee_minor_units: 1,
                    maker_fee_minor_units: 0,
                },
                "trade-fill:mkt-election:mkt-election:trade:7".to_owned()
            )]
        );
        assert_eq!(
            positions.requests(),
            vec![
                (
                    PositionFillProjectionRequest {
                        user_id: "10".to_owned(),
                        market_id: "mkt-election".to_owned(),
                        outcome: "YES".to_owned(),
                        side: "buy".to_owned(),
                        quantity: 2,
                        price_pts: 55,
                        order_id: "ord_taker_01".to_owned(),
                        fill_id: "mkt-election:trade:7:taker".to_owned(),
                        filled_at: None,
                        ledger_txn_id: Some(84),
                    },
                    "position-fill:10:mkt-election:mkt-election:trade:7:taker".to_owned()
                ),
                (
                    PositionFillProjectionRequest {
                        user_id: "11".to_owned(),
                        market_id: "mkt-election".to_owned(),
                        outcome: "YES".to_owned(),
                        side: "sell".to_owned(),
                        quantity: 2,
                        price_pts: 55,
                        order_id: "ord_maker_01".to_owned(),
                        fill_id: "mkt-election:trade:7:maker".to_owned(),
                        filled_at: None,
                        ledger_txn_id: Some(84),
                    },
                    "position-fill:11:mkt-election:mkt-election:trade:7:maker".to_owned()
                )
            ]
        );
        let saga = trade_sagas
            .sagas
            .lock()
            .expect("saga store lock should not poison")
            .get("trade-fill:mkt-election:mkt-election:trade:7")
            .cloned()
            .expect("trade fill saga should persist");
        assert_eq!(saga.status, TradeFillSagaStatus::Completed);
    }

    #[tokio::test]
    #[allow(clippy::expect_used)]
    async fn trade_fill_saga_retries_after_settlement_failure() {
        let wallet = InMemoryWalletLedgerClient::with_settlement_failures(1);
        let positions = InMemoryPositionProjectionClient::default();
        let order_owners = OrderOwnerStore::default();
        let trade_sagas = InMemoryTradeFillSagaStore::default();
        order_owners
            .record("ord_maker_retry", "11")
            .expect("maker order owner should record");
        let request = PlaceOrderRequest {
            side: "buy".to_owned(),
            outcome: "YES".to_owned(),
            order_type: "LIMIT".to_owned(),
            price_pts: 55,
            quantity: 2,
            time_in_force: "GTC".to_owned(),
            fee_curve_bps: None,
        };
        let response = OrderActionResponse {
            order_id: "ord_taker_retry".to_owned(),
            status: "FILLED".to_owned(),
            filled_qty: 2,
            avg_price_pts: 55,
            remaining_qty: 0,
            fee_pts: 0,
            fills: vec![OrderFill {
                fill_id: "mkt-election:trade:8".to_owned(),
                market_id: "mkt-election".to_owned(),
                seq_no: 8,
                price_pts: 55,
                quantity: 2,
                taker_order_id: "ord_taker_retry".to_owned(),
                maker_order_id: "ord_maker_retry".to_owned(),
                taker_user_id: 10,
                maker_user_id: 11,
                taker_side: "buy".to_owned(),
                maker_side: "sell".to_owned(),
                outcome: "YES".to_owned(),
            }],
        };

        settle_and_project_order_fills(
            &wallet,
            &positions,
            &order_owners,
            &trade_sagas,
            OrderFillSettlement {
                user_id: "10",
                market_id: "mkt-election",
                request: &request,
                response: &response,
                fee_curve_bps: DEFAULT_TAKER_FEE_CURVE_BPS,
                reservation: Some(&LedgerPostResult {
                    txn_id: 42,
                    replayed: false,
                }),
            },
        )
        .await
        .expect("failed settlement should leave retryable saga, not fail order path");

        assert_eq!(wallet.settlement_requests(), Vec::new());
        assert_eq!(positions.requests(), Vec::new());
        let retryable = trade_sagas
            .sagas
            .lock()
            .expect("saga store lock should not poison")
            .get("trade-fill:mkt-election:mkt-election:trade:8")
            .cloned()
            .expect("trade fill saga should persist");
        assert_eq!(retryable.status, TradeFillSagaStatus::Retryable);

        let claimed = drain_due_trade_fill_sagas(
            &trade_sagas,
            &wallet,
            &positions,
            TRADE_FILL_SAGA_BATCH_LIMIT,
        )
        .await
        .expect("retry drain should process the pending saga");

        assert_eq!(claimed, 1);
        assert_eq!(wallet.settlement_requests().len(), 1);
        assert_eq!(positions.requests().len(), 2);
        let completed = trade_sagas
            .sagas
            .lock()
            .expect("saga store lock should not poison")
            .get("trade-fill:mkt-election:mkt-election:trade:8")
            .cloned()
            .expect("trade fill saga should persist");
        assert_eq!(completed.status, TradeFillSagaStatus::Completed);
    }

    #[tokio::test]
    async fn place_order_passes_market_fee_curve_to_matching_engine() {
        let wallet = InMemoryWalletLedgerClient::with_available_minor_units(1_000);
        let engine = InMemoryMatchingEngineClient::default();
        let market_service = InMemoryMarketServiceClient::with_fee_bps(700);
        let app = router_with_engine_and_kyc_and_market_fees(
            wallet,
            engine.clone(),
            InMemoryKycTierClient::with_tier(KycTier::Tier1),
            market_service.clone(),
        );

        let response = app
            .oneshot(order_post(
                "idem-market-fee-bps-forward",
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

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(market_service.detail_requests(), vec!["mkt-election"]);
        assert_eq!(engine.place_requests().len(), 1);
        assert_eq!(engine.place_requests()[0].2.fee_curve_bps, Some(700));
    }

    /// Regression: ASK orders 也必須帶 per-market fee_curve_bps。先前 gateway 對 ASK
    /// short-circuit 回 DEFAULT_TAKER_FEE_CURVE_BPS，導致跨側成交時若 taker 落在 ASK
    /// 端會以 default 而非 market 設定收費（market.fee_bps > DEFAULT 時漏費）。
    #[tokio::test]
    async fn place_order_passes_market_fee_curve_to_matching_engine_on_ask_side() {
        let wallet = InMemoryWalletLedgerClient::with_available_minor_units(1_000);
        let engine = InMemoryMatchingEngineClient::default();
        let market_service = InMemoryMarketServiceClient::with_fee_bps(700);
        let app = router_with_engine_and_kyc_and_market_fees(
            wallet,
            engine.clone(),
            InMemoryKycTierClient::with_tier(KycTier::Tier1),
            market_service.clone(),
        );

        let response = app
            .oneshot(order_post(
                "idem-market-fee-bps-forward-ask",
                &json!({
                    "side": "sell",
                    "outcome": "YES",
                    "type": "LIMIT",
                    "price_pts": 65,
                    "quantity": 10,
                    "time_in_force": "GTC"
                }),
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(engine.place_requests().len(), 1);
        assert_eq!(engine.place_requests()[0].2.fee_curve_bps, Some(700));
    }

    #[tokio::test]
    async fn place_order_uses_market_fee_bps_cap_for_buy_balance_precheck() {
        let wallet = InMemoryWalletLedgerClient::with_available_minor_units(665);
        let engine = InMemoryMatchingEngineClient::default();
        let market_service = InMemoryMarketServiceClient::with_fee_bps(1300);
        let app = router_with_engine_and_kyc_and_market_fees(
            wallet,
            engine.clone(),
            InMemoryKycTierClient::with_tier(KycTier::Tier1),
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
                    fee_curve_bps: Some(DEFAULT_TAKER_FEE_CURVE_BPS),
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
                    fee_curve_bps: Some(DEFAULT_TAKER_FEE_CURVE_BPS),
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

    #[tokio::test]
    async fn affiliate_account_saga_creates_credential_and_profile_idempotently() {
        let referral = InMemoryReferralClient::default();
        let credentials =
            InMemoryAffiliateCredentialClient::default().with_active_manager("manager-1");
        let sagas = InMemoryAffiliateAccountSagaStore::default();
        let denied_audits = InMemoryGatewayDeniedAuditStore::default();
        let app = router_with_services_and_affiliate_account_clients(
            InMemoryWalletLedgerClient::default(),
            InMemoryMatchingEngineClient::default(),
            referral.clone(),
            credentials.clone(),
            sagas,
            denied_audits,
        );
        let payload = affiliate_account_payload();

        let created = app
            .clone()
            .oneshot(admin_json_request_with_idempotency(
                "POST",
                "/api/internal/admin/affiliate-accounts",
                "affiliate-account-1",
                &payload,
            ))
            .await
            .expect("create should be handled");
        assert_eq!(created.status(), StatusCode::CREATED);
        let body = response_json(created).await;
        assert_eq!(body["status"], "completed");
        assert_eq!(credentials.create_requests().len(), 1);
        assert_eq!(referral.affiliate_create_requests().len(), 1);

        let replayed = app
            .oneshot(admin_json_request_with_idempotency(
                "POST",
                "/api/internal/admin/affiliate-accounts",
                "affiliate-account-1",
                &payload,
            ))
            .await
            .expect("replay should be handled");
        assert_eq!(replayed.status(), StatusCode::OK);
        assert_eq!(response_json(replayed).await["status"], "completed");
        assert_eq!(credentials.create_requests().len(), 1);
        assert_eq!(referral.affiliate_create_requests().len(), 1);
    }

    #[tokio::test]
    async fn affiliate_account_saga_resumes_after_profile_failure() {
        let referral = InMemoryReferralClient::default().fail_next_affiliate_create();
        let credentials =
            InMemoryAffiliateCredentialClient::default().with_active_manager("manager-1");
        let sagas = InMemoryAffiliateAccountSagaStore::default();
        let denied_audits = InMemoryGatewayDeniedAuditStore::default();
        let app = router_with_services_and_affiliate_account_clients(
            InMemoryWalletLedgerClient::default(),
            InMemoryMatchingEngineClient::default(),
            referral.clone(),
            credentials.clone(),
            sagas,
            denied_audits,
        );
        let payload = affiliate_account_payload();

        let failed = app
            .clone()
            .oneshot(admin_json_request_with_idempotency(
                "POST",
                "/api/internal/admin/affiliate-accounts",
                "affiliate-account-retry",
                &payload,
            ))
            .await
            .expect("failed create should be handled");
        assert_eq!(failed.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(response_json(failed).await["status"], "credential_created");
        assert_eq!(credentials.create_requests().len(), 1);
        assert_eq!(referral.affiliate_create_requests().len(), 0);

        let resumed = app
            .oneshot(admin_json_request_with_idempotency(
                "POST",
                "/api/internal/admin/affiliate-accounts",
                "affiliate-account-retry",
                &payload,
            ))
            .await
            .expect("resume should be handled");
        assert_eq!(resumed.status(), StatusCode::CREATED);
        assert_eq!(response_json(resumed).await["status"], "completed");
        assert_eq!(credentials.create_requests().len(), 1);
        assert_eq!(referral.affiliate_create_requests().len(), 1);
    }

    #[tokio::test]
    async fn affiliate_account_saga_rejects_idempotency_conflict() {
        let credentials =
            InMemoryAffiliateCredentialClient::default().with_active_manager("manager-1");
        let denied_audits = InMemoryGatewayDeniedAuditStore::default();
        let app = router_with_services_and_affiliate_account_clients(
            InMemoryWalletLedgerClient::default(),
            InMemoryMatchingEngineClient::default(),
            InMemoryReferralClient::default(),
            credentials,
            InMemoryAffiliateAccountSagaStore::default(),
            denied_audits.clone(),
        );
        let first_payload = affiliate_account_payload();
        let mut second_payload = affiliate_account_payload();
        second_payload["email"] = json!("other-affiliate@example.com");

        let first = app
            .clone()
            .oneshot(admin_json_request_with_idempotency(
                "POST",
                "/api/internal/admin/affiliate-accounts",
                "affiliate-account-conflict",
                &first_payload,
            ))
            .await
            .expect("first create should be handled");
        assert_eq!(first.status(), StatusCode::CREATED);

        let conflict = app
            .oneshot(admin_json_request_with_idempotency(
                "POST",
                "/api/internal/admin/affiliate-accounts",
                "affiliate-account-conflict",
                &second_payload,
            ))
            .await
            .expect("conflict should be handled");
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        assert_eq!(
            response_json(conflict).await["error"],
            json!("idempotency_conflict")
        );
        assert_eq!(denied_audits.records().len(), 1);
    }

    #[tokio::test]
    async fn affiliate_account_saga_rejects_inactive_managing_manager_before_side_effects() {
        let referral = InMemoryReferralClient::default();
        let credentials = InMemoryAffiliateCredentialClient::default();
        let denied_audits = InMemoryGatewayDeniedAuditStore::default();
        let app = router_with_services_and_affiliate_account_clients(
            InMemoryWalletLedgerClient::default(),
            InMemoryMatchingEngineClient::default(),
            referral.clone(),
            credentials.clone(),
            InMemoryAffiliateAccountSagaStore::default(),
            denied_audits.clone(),
        );

        let rejected = app
            .oneshot(admin_json_request_with_idempotency(
                "POST",
                "/api/internal/admin/affiliate-accounts",
                "affiliate-account-inactive-manager",
                &affiliate_account_payload(),
            ))
            .await
            .expect("rejected create should be handled");

        assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response_json(rejected).await["error"],
            json!("invalid_managing_manager")
        );
        assert_eq!(credentials.create_requests().len(), 0);
        assert_eq!(referral.affiliate_create_requests().len(), 0);
        assert_eq!(
            denied_audits.records()[0].reason_or_error_code,
            "invalid_managing_manager"
        );
    }

    #[tokio::test]
    async fn affiliate_reassign_update_prechecks_active_manager_before_referral_call() {
        let referral = InMemoryReferralClient::default();
        let denied_audits = InMemoryGatewayDeniedAuditStore::default();
        let app = router_with_services_and_affiliate_account_clients(
            InMemoryWalletLedgerClient::default(),
            InMemoryMatchingEngineClient::default(),
            referral,
            InMemoryAffiliateCredentialClient::default(),
            InMemoryAffiliateAccountSagaStore::default(),
            denied_audits.clone(),
        );

        let rejected = app
            .oneshot(admin_json_request(
                "PATCH",
                "/api/internal/admin/affiliates/AFF-200",
                &json!({
                    "managingManagerId": "inactive-manager",
                    "reason": "reassign",
                    "authorizationBasis": "ticket-1"
                }),
            ))
            .await
            .expect("reassign should be handled");

        assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response_json(rejected).await["error"],
            json!("invalid_managing_manager")
        );
        assert_eq!(denied_audits.records().len(), 1);
        assert_eq!(denied_audits.records()[0].action, "update");
    }

    #[tokio::test]
    async fn affiliate_account_saga_status_can_be_queried_for_repair_context() {
        let referral = InMemoryReferralClient::default().fail_next_affiliate_create();
        let credentials =
            InMemoryAffiliateCredentialClient::default().with_active_manager("manager-1");
        let app = router_with_services_and_affiliate_account_clients(
            InMemoryWalletLedgerClient::default(),
            InMemoryMatchingEngineClient::default(),
            referral,
            credentials,
            InMemoryAffiliateAccountSagaStore::default(),
            InMemoryGatewayDeniedAuditStore::default(),
        );
        let failed = app
            .clone()
            .oneshot(admin_json_request_with_idempotency(
                "POST",
                "/api/internal/admin/affiliate-accounts",
                "affiliate-account-status",
                &affiliate_account_payload(),
            ))
            .await
            .expect("failed create should be handled");
        assert_eq!(failed.status(), StatusCode::BAD_GATEWAY);

        let status = app
            .oneshot(admin_empty_get(
                "/api/internal/admin/affiliate-account-sagas/affiliate-account-status",
            ))
            .await
            .expect("status should be handled");
        assert_eq!(status.status(), StatusCode::OK);
        let body = response_json(status).await;
        assert_eq!(body["status"], json!("credential_created"));
        assert_eq!(body["actorId"], json!("admin-1"));
        assert_eq!(body["affiliateId"], json!("AFF-ACCT1"));
    }

    fn empty_get(uri: &str) -> Request<Body> {
        Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap_or_else(|error| panic!("request should build: {error}"))
    }

    fn admin_empty_get(uri: &str) -> Request<Body> {
        Request::builder()
            .method("GET")
            .uri(uri)
            .header("x-actor-id", "admin-1")
            .header("x-operator-role", "admin")
            .header("x-request-id", "req-1")
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

    fn admin_json_request_with_idempotency(
        method: &str,
        uri: &str,
        idempotency_key: &str,
        body: &Value,
    ) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("x-actor-id", "admin-1")
            .header("x-operator-role", "admin")
            .header("x-request-id", "req-1")
            .header("idempotency-key", idempotency_key)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap_or_else(|error| panic!("request should build: {error}"))
    }

    fn affiliate_account_payload() -> Value {
        json!({
            "userId": "018f4f75-5c1f-7b61-9d52-0242ac120002",
            "affiliateId": "AFF-ACCT1",
            "email": "affiliate-account@example.com",
            "phone": "0912345009",
            "password": "correct horse affiliate",
            "displayName": "Affiliate Account",
            "managingManagerId": "manager-1",
            "commissionRateBps": 600,
            "tier": 1,
            "reason": "affiliate account onboarding",
            "authorizationBasis": "SPEC-017 §6.3.5"
        })
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

    fn multi_kid_jwks_json() -> String {
        let primary = test_signing_key();
        let secondary = SigningKey::from_bytes(&[9; 32]);
        json!({
            "keys": [
                {
                    "kty": "OKP",
                    "crv": "Ed25519",
                    "kid": "test-key",
                    "alg": "EdDSA",
                    "use": "sig",
                    "x": base64_url_encode(&primary.verifying_key().to_bytes())
                },
                {
                    "kty": "OKP",
                    "crv": "Ed25519",
                    "kid": "test-key-rotated",
                    "alg": "EdDSA",
                    "use": "sig",
                    "x": base64_url_encode(&secondary.verifying_key().to_bytes())
                }
            ]
        })
        .to_string()
    }

    fn jwt_with_optional_kid(subject: &str, seed: [u8; 32], exp: u64, kid: Option<&str>) -> String {
        let key = SigningKey::from_bytes(&seed);
        let mut header = json!({
            "alg": "EdDSA",
            "typ": "JWT"
        });
        if let Some(kid) = kid {
            header["kid"] = json!(kid);
        }
        let header_b64 = base64_url_encode(header.to_string().as_bytes());
        let payload_b64 = base64_url_encode(
            json!({
                "sub": subject,
                "iat": 1_800_000_000_u64,
                "exp": exp
            })
            .to_string()
            .as_bytes(),
        );
        let signing_input = format!("{header_b64}.{payload_b64}");
        let signature = key.sign(signing_input.as_bytes());
        format!(
            "{signing_input}.{}",
            base64_url_encode(&signature.to_bytes())
        )
    }

    #[test]
    fn verify_bearer_accepts_kidless_token_when_jwks_has_single_key() {
        let verifier = AuthVerifier::from_jwks_json(&test_jwks_json(), true)
            .unwrap_or_else(|error| panic!("test JWKS should parse: {error}"));
        let token = jwt_with_optional_kid("user-1", [7; 32], 1_900_000_000, None);
        let auth = verifier
            .verify_bearer(&token, 1_800_000_000)
            .unwrap_or_else(|error| {
                panic!("single-key JWKS should accept kid-less token: {error:?}")
            });
        assert_eq!(auth.user_id, "user-1");
    }

    #[test]
    fn verify_bearer_rejects_kidless_token_when_jwks_has_multiple_keys() {
        // SPEC-023 plan §11 D-1 / Phase 0 I-3: prevent kid-downgrade against
        // multi-key JWKS — a leaked retired key must not be allowed to forge
        // tokens that omit `kid` and slip past the key match.
        let verifier = AuthVerifier::from_jwks_json(&multi_kid_jwks_json(), true)
            .unwrap_or_else(|error| panic!("multi-key JWKS should parse: {error}"));
        let token = jwt_with_optional_kid("user-1", [7; 32], 1_900_000_000, None);
        assert!(verifier.verify_bearer(&token, 1_800_000_000).is_err());
    }

    #[test]
    fn verify_bearer_accepts_kid_specific_token_against_multi_key_jwks() {
        let verifier = AuthVerifier::from_jwks_json(&multi_kid_jwks_json(), true)
            .unwrap_or_else(|error| panic!("multi-key JWKS should parse: {error}"));
        let token = jwt_with_optional_kid("user-1", [7; 32], 1_900_000_000, Some("test-key"));
        let auth = verifier
            .verify_bearer(&token, 1_800_000_000)
            .unwrap_or_else(|error| {
                panic!("kid-tagged token should verify against matching JWKS entry: {error:?}")
            });
        assert_eq!(auth.user_id, "user-1");
    }

    #[test]
    fn verify_bearer_rejects_token_signed_by_a_key_under_a_different_kid() {
        // Token claims kid "test-key" but is signed with the secondary key's
        // material — the kid filter should pin verification to the primary
        // key only, which will reject the signature.
        let verifier = AuthVerifier::from_jwks_json(&multi_kid_jwks_json(), true)
            .unwrap_or_else(|error| panic!("multi-key JWKS should parse: {error}"));
        let token = jwt_with_optional_kid("user-1", [9; 32], 1_900_000_000, Some("test-key"));
        assert!(verifier.verify_bearer(&token, 1_800_000_000).is_err());
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

    type OrderFundsRequests = Arc<Mutex<Vec<(String, String, i64, String)>>>;
    type TradeSettlementRequests = Arc<Mutex<Vec<(TradeSettlementRequest, String)>>>;

    #[derive(Clone, Default)]
    struct InMemoryWalletLedgerClient {
        requested_user_ids: Arc<Mutex<Vec<String>>>,
        entry_requests: Arc<Mutex<Vec<(String, i64)>>>,
        reservations: OrderFundsRequests,
        releases: OrderFundsRequests,
        settlements: TradeSettlementRequests,
        settlement_failures_remaining: Arc<Mutex<usize>>,
        available_minor_units: Option<i64>,
    }

    #[derive(Clone, Default)]
    struct InMemoryMatchingEngineClient {
        placed_orders: Arc<Mutex<Vec<(String, String, PlaceOrderRequest)>>>,
        canceled_orders: Arc<Mutex<Vec<(String, String)>>>,
        snapshot_reads: Arc<Mutex<Vec<(String, usize)>>>,
        place_response: Option<OrderActionResponse>,
        fail_place: bool,
    }

    #[derive(Clone, Default)]
    struct InMemoryReferralClient {
        captures: Arc<Mutex<Vec<CaptureReferralRequest>>>,
        dashboards: Arc<Mutex<Vec<(String, usize)>>>,
        referral_lists: Arc<Mutex<Vec<(String, usize)>>>,
        summaries: Arc<Mutex<Vec<String>>>,
        affiliate_creates: Arc<Mutex<Vec<serde_json::Value>>>,
        fail_affiliate_creates: Arc<Mutex<usize>>,
    }

    #[derive(Clone, Default)]
    struct InMemoryAffiliateCredentialClient {
        creates: Arc<Mutex<Vec<AffiliateCredentialCreateRequest>>>,
        active_managers: Arc<Mutex<Vec<String>>>,
    }

    type PositionListRequests = Arc<Mutex<Vec<(String, Option<String>)>>>;

    #[derive(Clone, Default)]
    struct InMemoryPositionServiceClient {
        requests: PositionListRequests,
    }

    type PositionProjectionRequests = Arc<Mutex<Vec<(PositionFillProjectionRequest, String)>>>;

    #[derive(Clone, Default)]
    struct InMemoryPositionProjectionClient {
        requests: PositionProjectionRequests,
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
        tier: Option<KycTier>,
    }

    impl Default for InMemoryKycTierClient {
        fn default() -> Self {
            Self {
                tier: Some(KycTier::Tier1),
            }
        }
    }

    impl InMemoryKycTierClient {
        fn with_tier(tier: KycTier) -> Self {
            Self { tier: Some(tier) }
        }

        fn unavailable() -> Self {
            Self { tier: None }
        }
    }

    impl InMemoryReferralClient {
        fn fail_next_affiliate_create(self) -> Self {
            *self.fail_affiliate_creates.lock().unwrap_or_else(|error| {
                panic!("referral failure lock should not poison: {error}")
            }) = 1;
            self
        }

        fn affiliate_create_requests(&self) -> Vec<serde_json::Value> {
            self.affiliate_creates
                .lock()
                .unwrap_or_else(|error| panic!("affiliate create lock should not poison: {error}"))
                .clone()
        }
    }

    impl InMemoryAffiliateCredentialClient {
        fn with_active_manager(self, manager_id: &str) -> Self {
            self.active_managers
                .lock()
                .unwrap_or_else(|error| panic!("active manager lock should not poison: {error}"))
                .push(manager_id.to_owned());
            self
        }

        fn create_requests(&self) -> Vec<AffiliateCredentialCreateRequest> {
            self.creates
                .lock()
                .unwrap_or_else(|error| panic!("credential create lock should not poison: {error}"))
                .clone()
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

    impl InMemoryPositionServiceClient {
        fn requests(&self) -> Vec<(String, Option<String>)> {
            self.requests
                .lock()
                .unwrap_or_else(|error| panic!("position requests lock should not poison: {error}"))
                .clone()
        }
    }

    impl InMemoryPositionProjectionClient {
        fn requests(&self) -> Vec<(PositionFillProjectionRequest, String)> {
            self.requests
                .lock()
                .unwrap_or_else(|error| {
                    panic!("position projection requests lock should not poison: {error}")
                })
                .clone()
        }
    }

    impl InMemoryMatchingEngineClient {
        fn with_place_response(response: OrderActionResponse) -> Self {
            Self {
                place_response: Some(response),
                ..Self::default()
            }
        }

        fn failing_place() -> Self {
            Self {
                fail_place: true,
                ..Self::default()
            }
        }

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

        fn with_settlement_failures(failures: usize) -> Self {
            Self {
                settlement_failures_remaining: Arc::new(Mutex::new(failures)),
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

        fn reservation_requests(&self) -> Vec<(String, String, i64, String)> {
            self.reservations
                .lock()
                .unwrap_or_else(|error| {
                    panic!("reservation requests lock should not poison: {error}")
                })
                .clone()
        }

        fn release_requests(&self) -> Vec<(String, String, i64, String)> {
            self.releases
                .lock()
                .unwrap_or_else(|error| panic!("release requests lock should not poison: {error}"))
                .clone()
        }

        fn settlement_requests(&self) -> Vec<(TradeSettlementRequest, String)> {
            self.settlements
                .lock()
                .unwrap_or_else(|error| {
                    panic!("settlement requests lock should not poison: {error}")
                })
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

    #[async_trait::async_trait]
    impl PositionServiceClient for InMemoryPositionServiceClient {
        async fn list_positions(
            &self,
            user_id: &str,
            status: Option<&str>,
        ) -> Result<PositionListResponse, ApiError> {
            self.requests
                .lock()
                .unwrap_or_else(|error| panic!("position requests lock should not poison: {error}"))
                .push((user_id.to_owned(), status.map(str::to_owned)));

            Ok(PositionListResponse {
                positions: vec![Position {
                    id: "pos-1".to_owned(),
                    user_id: user_id.to_owned(),
                    market_id: "mkt-election".to_owned(),
                    outcome: "YES".to_owned(),
                    qty: 100,
                    avg_price_pts: 42,
                    locked_amount: 4_200,
                    opened_at: "2026-05-01T00:00:00+08:00".to_owned(),
                    closed_at: None,
                    settle_txn_id: None,
                    realized_pnl_points: None,
                }],
            })
        }
    }

    #[async_trait::async_trait]
    impl PositionProjectionClient for InMemoryPositionProjectionClient {
        async fn project_fill(
            &self,
            request: PositionFillProjectionRequest,
            idempotency_key: &str,
        ) -> Result<(), ApiError> {
            self.requests
                .lock()
                .unwrap_or_else(|error| {
                    panic!("position projection requests lock should not poison: {error}")
                })
                .push((request, idempotency_key.to_owned()));

            Ok(())
        }
    }

    impl InMemoryMarketServiceClient {
        fn market(&self) -> Market {
            Market {
                fee_bps: Some(self.fee_bps.unwrap_or(DEFAULT_TAKER_FEE_CURVE_BPS)),
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

        async fn reserve_order_funds(
            &self,
            user_id: &str,
            market_id: &str,
            amount_minor_units: i64,
            idempotency_key: &str,
        ) -> Result<Option<LedgerPostResult>, ApiError> {
            if amount_minor_units <= 0 {
                return Ok(None);
            }
            self.reservations
                .lock()
                .unwrap_or_else(|error| {
                    panic!("reservation requests lock should not poison: {error}")
                })
                .push((
                    user_id.to_owned(),
                    market_id.to_owned(),
                    amount_minor_units,
                    idempotency_key.to_owned(),
                ));

            Ok(Some(LedgerPostResult {
                txn_id: 42,
                replayed: false,
            }))
        }

        async fn release_order_funds(
            &self,
            user_id: &str,
            market_id: &str,
            amount_minor_units: i64,
            idempotency_key: &str,
        ) -> Result<(), ApiError> {
            if amount_minor_units <= 0 {
                return Ok(());
            }
            self.releases
                .lock()
                .unwrap_or_else(|error| panic!("release requests lock should not poison: {error}"))
                .push((
                    user_id.to_owned(),
                    market_id.to_owned(),
                    amount_minor_units,
                    idempotency_key.to_owned(),
                ));
            Ok(())
        }

        async fn settle_trade_fill(
            &self,
            request: &TradeSettlementRequest,
            idempotency_key: &str,
        ) -> Result<Option<LedgerPostResult>, ApiError> {
            {
                let mut failures =
                    self.settlement_failures_remaining
                        .lock()
                        .unwrap_or_else(|error| {
                            panic!("settlement failure lock should not poison: {error}")
                        });
                if *failures > 0 {
                    *failures -= 1;
                    return Err(ApiError::upstream("wallet-ledger settlement unavailable"));
                }
            }
            self.settlements
                .lock()
                .unwrap_or_else(|error| {
                    panic!("settlement requests lock should not poison: {error}")
                })
                .push((request.clone(), idempotency_key.to_owned()));

            Ok(Some(LedgerPostResult {
                txn_id: 84,
                replayed: false,
            }))
        }
    }

    #[async_trait::async_trait]
    impl KycTierClient for InMemoryKycTierClient {
        async fn get_kyc_tier(&self, _user_id: &str) -> Result<KycTier, ApiError> {
            self.tier
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
            if self.fail_place {
                return Err(ApiError::matching_engine(
                    StatusCode::BAD_GATEWAY,
                    "matching engine unavailable",
                ));
            }
            self.placed_orders
                .lock()
                .unwrap_or_else(|error| panic!("place requests lock should not poison: {error}"))
                .push((user_id.to_owned(), market_id.to_owned(), request.clone()));

            Ok(self.place_response.clone().unwrap_or(OrderActionResponse {
                order_id: "ord_01HZ0000000000000000000000".to_owned(),
                status: "ACTIVE".to_owned(),
                filled_qty: 0,
                avg_price_pts: 0,
                remaining_qty: 3,
                fee_pts: 0,
                fills: Vec::new(),
            }))
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
                fills: Vec::new(),
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

        async fn list_admin_affiliates(
            &self,
            _context: AdminProxyContext,
        ) -> Result<serde_json::Value, ApiError> {
            Ok(serde_json::json!([{
                "id": "AFF-001",
                "displayName": "Crescent Partners"
            }]))
        }

        async fn get_admin_affiliate_detail(
            &self,
            affiliate_id: &str,
            _context: AdminProxyContext,
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
            let mut failures = self
                .fail_affiliate_creates
                .lock()
                .unwrap_or_else(|error| panic!("referral failure lock should not poison: {error}"));
            if *failures > 0 {
                *failures -= 1;
                return Err(ApiError::referral_service("referral create failed"));
            }
            drop(failures);
            self.affiliate_creates
                .lock()
                .unwrap_or_else(|error| panic!("affiliate create lock should not poison: {error}"))
                .push(request.clone());
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

    #[async_trait::async_trait]
    impl AffiliateCredentialClient for InMemoryAffiliateCredentialClient {
        async fn create_affiliate_credential(
            &self,
            request: AffiliateCredentialCreateRequest,
            _context: AdminProxyContext,
        ) -> Result<(), ApiError> {
            self.creates
                .lock()
                .unwrap_or_else(|error| panic!("credential create lock should not poison: {error}"))
                .push(request);
            Ok(())
        }

        async fn verify_active_manager(
            &self,
            manager_id: &str,
            _context: AdminProxyContext,
        ) -> Result<(), ApiError> {
            let active = self
                .active_managers
                .lock()
                .unwrap_or_else(|error| panic!("active manager lock should not poison: {error}"))
                .iter()
                .any(|id| id == manager_id);
            if active {
                Ok(())
            } else {
                Err(ApiError::new(
                    StatusCode::FORBIDDEN,
                    "invalid_managing_manager",
                    "managingManagerId must reference an active manager account",
                ))
            }
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
