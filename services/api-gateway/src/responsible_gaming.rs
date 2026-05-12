use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{Json, Router, extract::State, http::HeaderMap, routing::get};
use serde::{Deserialize, Serialize};

use crate::{ApiError, AuthContext, WalletLedgerClient};

const SIX_MONTHS_SECONDS: i64 = 183 * 24 * 60 * 60;
const ONE_YEAR_SECONDS: i64 = 365 * 24 * 60 * 60;
const DAY_SECONDS: i64 = 24 * 60 * 60;
const AVAILABLE_COOLDOWN_DURATIONS: [CooldownDuration; 3] = [
    CooldownDuration::TwentyFourHours,
    CooldownDuration::SevenDays,
    CooldownDuration::ThirtyDays,
];

#[derive(Debug, Clone, Copy)]
pub struct ResponsibleGamingClock {
    fixed_epoch_seconds: Option<i64>,
}

impl ResponsibleGamingClock {
    #[must_use]
    pub const fn system() -> Self {
        Self {
            fixed_epoch_seconds: None,
        }
    }

    #[must_use]
    pub const fn fixed(epoch_seconds: i64) -> Self {
        Self {
            fixed_epoch_seconds: Some(epoch_seconds),
        }
    }

    fn now_epoch_seconds(self) -> i64 {
        self.fixed_epoch_seconds.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| i64::try_from(duration.as_secs()).unwrap_or(0))
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FixedClock(ResponsibleGamingClock);

impl FixedClock {
    #[must_use]
    pub fn new(epoch_seconds: u64) -> Self {
        let epoch_seconds = i64::try_from(epoch_seconds).unwrap_or(i64::MAX);
        Self(ResponsibleGamingClock::fixed(epoch_seconds))
    }
}

impl From<FixedClock> for ResponsibleGamingClock {
    fn from(value: FixedClock) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SystemClock;

impl From<SystemClock> for ResponsibleGamingClock {
    fn from(_: SystemClock) -> Self {
        ResponsibleGamingClock::system()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelfExclusionMode {
    Timeout,
    Permanent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelfExclusionPeriod {
    SixMonths,
    OneYear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointsDisposition {
    Hold,
    Donate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SelfExclusionRecord {
    pub record_id: String,
    pub user_id: String,
    pub mode: SelfExclusionMode,
    pub period: Option<SelfExclusionPeriod>,
    pub points_disposition: Option<PointsDisposition>,
    pub reason: Option<String>,
    pub starts_at_epoch_seconds: i64,
    pub ends_at_epoch_seconds: Option<i64>,
    pub frozen_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SelfExclusionStatusResponse {
    pub user_id: String,
    pub active: bool,
    pub record: Option<SelfExclusionRecord>,
    pub source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubmitSelfExclusionResponse {
    pub record: SelfExclusionRecord,
    pub ledger_idempotency_key: String,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct SubmitSelfExclusionRequest {
    pub mode: SelfExclusionMode,
    pub period: Option<SelfExclusionPeriod>,
    pub points_disposition: Option<PointsDisposition>,
    #[serde(default)]
    pub consent: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CooldownDuration {
    #[serde(rename = "24h")]
    TwentyFourHours,
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
}

impl CooldownDuration {
    #[must_use]
    pub const fn seconds(self) -> i64 {
        match self {
            Self::TwentyFourHours => DAY_SECONDS,
            Self::SevenDays => 7 * DAY_SECONDS,
            Self::ThirtyDays => 30 * DAY_SECONDS,
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "24h" => Some(Self::TwentyFourHours),
            "7d" => Some(Self::SevenDays),
            "30d" => Some(Self::ThirtyDays),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CooldownRecord {
    duration: CooldownDuration,
    started_at_epoch_seconds: i64,
    ends_at_epoch_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CooldownStatusResponse {
    pub user_id: String,
    pub active: bool,
    pub duration: Option<CooldownDuration>,
    pub started_at_epoch_seconds: Option<i64>,
    pub ends_at_epoch_seconds: Option<i64>,
    pub remaining_seconds: i64,
    pub available_durations: Vec<CooldownDuration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StartCooldownRequest {
    pub duration: String,
    #[serde(default)]
    pub consent: bool,
}

pub trait SelfExclusionStore {
    /// Returns the idempotent response for this user/key, when present.
    ///
    /// # Errors
    ///
    /// Returns an error when the backing store cannot be read.
    fn idempotency_record(
        &self,
        user_id: &str,
        key: &str,
    ) -> Result<Option<SelfExclusionIdempotencyRecord>, ApiError>;

    /// Returns the current active record, if the user is excluded at `now_epoch_seconds`.
    ///
    /// # Errors
    ///
    /// Returns an error when the backing store cannot be read.
    fn active_record(
        &self,
        user_id: &str,
        now_epoch_seconds: i64,
    ) -> Result<Option<SelfExclusionRecord>, ApiError>;

    /// Stores a self-exclusion record and its idempotent response.
    ///
    /// # Errors
    ///
    /// Returns an error when the backing store cannot be written.
    fn insert(
        &self,
        user_id: &str,
        key: &str,
        request_hash: u64,
        response: SubmitSelfExclusionResponse,
    ) -> Result<(), ApiError>;
}

pub trait CooldownStore {
    /// Returns the user's active cooldown status at `now_epoch_seconds`.
    ///
    /// # Errors
    ///
    /// Returns an error when the backing store cannot be read.
    fn cooldown_status(
        &self,
        user_id: &str,
        now_epoch_seconds: i64,
    ) -> Result<CooldownStatusResponse, ApiError>;

    /// Starts a cooldown, or returns the existing active cooldown for the user.
    ///
    /// # Errors
    ///
    /// Returns an error when the backing store cannot be written.
    fn start_cooldown(
        &self,
        user_id: &str,
        duration: CooldownDuration,
        now_epoch_seconds: i64,
    ) -> Result<CooldownStatusResponse, ApiError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfExclusionIdempotencyRecord {
    request_hash: u64,
    response: SubmitSelfExclusionResponse,
}

#[derive(Debug, Clone)]
struct ResponsibleGamingState<W, S> {
    wallet_ledger: W,
    store: S,
    clock: ResponsibleGamingClock,
}

#[derive(Debug, Clone)]
struct CooldownState<S> {
    store: S,
    clock: ResponsibleGamingClock,
}

#[derive(Debug, Clone, Default)]
pub struct InMemorySelfExclusionStore {
    state: Arc<Mutex<InMemorySelfExclusionState>>,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryCooldownStore {
    cooldowns: Arc<Mutex<HashMap<String, CooldownRecord>>>,
}

pub type InMemoryResponsibleGamingStore = InMemoryCooldownStore;

#[derive(Debug, Default)]
struct InMemorySelfExclusionState {
    records: Vec<SelfExclusionRecord>,
    idempotency: HashMap<(String, String), SelfExclusionIdempotencyRecord>,
}

pub fn responsible_gaming_router<W, S>(
    wallet_ledger: W,
    store: S,
    clock: ResponsibleGamingClock,
) -> Router
where
    W: WalletLedgerClient + Clone + Send + Sync + 'static,
    S: SelfExclusionStore + Clone + Send + Sync + 'static,
{
    Router::new()
        .route(
            "/v1/responsible-gaming/self-exclusion",
            get(get_self_exclusion_status::<W, S>).post(submit_self_exclusion::<W, S>),
        )
        .with_state(ResponsibleGamingState {
            wallet_ledger,
            store,
            clock,
        })
}

pub fn cooldown_router<S>(store: S, clock: ResponsibleGamingClock) -> Router
where
    S: CooldownStore + Clone + Send + Sync + 'static,
{
    Router::new()
        .route(
            "/v1/me/responsible-gaming/cooldown",
            get(get_cooldown::<S>).post(start_cooldown::<S>),
        )
        .with_state(CooldownState { store, clock })
}

pub fn router<S, C>(store: S, clock: C) -> Router
where
    S: CooldownStore + Clone + Send + Sync + 'static,
    C: Into<ResponsibleGamingClock>,
{
    cooldown_router(store, clock.into())
}

async fn get_self_exclusion_status<W, S>(
    State(state): State<ResponsibleGamingState<W, S>>,
    auth: AuthContext,
) -> Result<Json<SelfExclusionStatusResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    S: SelfExclusionStore,
{
    let now = state.clock.now_epoch_seconds();
    let record = state.store.active_record(&auth.user_id, now)?;
    Ok(Json(SelfExclusionStatusResponse {
        user_id: auth.user_id,
        active: record.is_some(),
        record,
        source: "api-gateway",
    }))
}

async fn submit_self_exclusion<W, S>(
    State(state): State<ResponsibleGamingState<W, S>>,
    auth: AuthContext,
    headers: HeaderMap,
    Json(request): Json<SubmitSelfExclusionRequest>,
) -> Result<Json<SubmitSelfExclusionResponse>, ApiError>
where
    W: WalletLedgerClient + Send + Sync,
    S: SelfExclusionStore,
{
    validate_request(&request)?;
    let idempotency_key = idempotency_key_from_headers(&headers)?;
    let request_hash = self_exclusion_request_hash(&auth.user_id, &request);

    if let Some(record) = state
        .store
        .idempotency_record(&auth.user_id, &idempotency_key)?
    {
        if record.request_hash != request_hash {
            return Err(ApiError::conflict(
                "idempotency_conflict",
                "Idempotency-Key was already used with a different self-exclusion request",
            ));
        }

        return Ok(Json(SubmitSelfExclusionResponse {
            replayed: true,
            ..record.response
        }));
    }

    let now = state.clock.now_epoch_seconds();
    if state.store.active_record(&auth.user_id, now)?.is_some() {
        return Err(ApiError::conflict(
            "self_exclusion_already_active",
            "An active self-exclusion already exists for this user",
        ));
    }

    let balance = state
        .wallet_ledger
        .get_wallet_balance(&auth.user_id)
        .await?;
    let amount_to_freeze = balance.available_minor_units.saturating_neg().max(0);
    let ledger_idempotency_key = format!("self-exclusion:{}:{idempotency_key}", auth.user_id);
    if amount_to_freeze > 0 {
        state
            .wallet_ledger
            .freeze_player_funds(&auth.user_id, amount_to_freeze, &ledger_idempotency_key)
            .await?;
    }

    let response = SubmitSelfExclusionResponse {
        record: SelfExclusionRecord {
            record_id: format!("se_{}_{}", auth.user_id, now),
            user_id: auth.user_id.clone(),
            mode: request.mode,
            period: request.period,
            points_disposition: request.points_disposition,
            reason: normalized_reason(request.reason),
            starts_at_epoch_seconds: now,
            ends_at_epoch_seconds: ends_at_epoch_seconds(now, request.period),
            frozen_minor_units: amount_to_freeze,
        },
        ledger_idempotency_key,
        replayed: false,
    };
    state.store.insert(
        &auth.user_id,
        &idempotency_key,
        request_hash,
        response.clone(),
    )?;

    Ok(Json(response))
}

async fn get_cooldown<S>(
    State(state): State<CooldownState<S>>,
    auth: AuthContext,
) -> Result<Json<CooldownStatusResponse>, ApiError>
where
    S: CooldownStore,
{
    Ok(Json(state.store.cooldown_status(
        &auth.user_id,
        state.clock.now_epoch_seconds(),
    )?))
}

async fn start_cooldown<S>(
    State(state): State<CooldownState<S>>,
    auth: AuthContext,
    Json(request): Json<StartCooldownRequest>,
) -> Result<Json<CooldownStatusResponse>, ApiError>
where
    S: CooldownStore,
{
    if !request.consent {
        return Err(ApiError::bad_request(
            "cooldown_consent_required",
            "Explicit responsible gaming cooldown consent is required",
        ));
    }

    let Some(duration) = CooldownDuration::parse(&request.duration) else {
        return Err(ApiError::bad_request(
            "invalid_cooldown_duration",
            "Cooldown duration must be one of 24h, 7d, or 30d",
        ));
    };

    Ok(Json(state.store.start_cooldown(
        &auth.user_id,
        duration,
        state.clock.now_epoch_seconds(),
    )?))
}

impl SelfExclusionStore for InMemorySelfExclusionStore {
    fn idempotency_record(
        &self,
        user_id: &str,
        key: &str,
    ) -> Result<Option<SelfExclusionIdempotencyRecord>, ApiError> {
        self.state
            .lock()
            .map_err(|_| ApiError::internal("self-exclusion store lock poisoned"))
            .map(|state| {
                state
                    .idempotency
                    .get(&(user_id.to_owned(), key.to_owned()))
                    .cloned()
            })
    }

    fn active_record(
        &self,
        user_id: &str,
        now_epoch_seconds: i64,
    ) -> Result<Option<SelfExclusionRecord>, ApiError> {
        self.state
            .lock()
            .map_err(|_| ApiError::internal("self-exclusion store lock poisoned"))
            .map(|state| {
                state
                    .records
                    .iter()
                    .rev()
                    .find(|record| {
                        record.user_id == user_id && is_active(record, now_epoch_seconds)
                    })
                    .cloned()
            })
    }

    fn insert(
        &self,
        user_id: &str,
        key: &str,
        request_hash: u64,
        response: SubmitSelfExclusionResponse,
    ) -> Result<(), ApiError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ApiError::internal("self-exclusion store lock poisoned"))?;
        state.records.push(response.record.clone());
        state.idempotency.insert(
            (user_id.to_owned(), key.to_owned()),
            SelfExclusionIdempotencyRecord {
                request_hash,
                response,
            },
        );
        Ok(())
    }
}

impl CooldownStore for InMemoryCooldownStore {
    fn cooldown_status(
        &self,
        user_id: &str,
        now_epoch_seconds: i64,
    ) -> Result<CooldownStatusResponse, ApiError> {
        let mut cooldowns = self
            .cooldowns
            .lock()
            .map_err(|_| ApiError::internal("cooldown store lock poisoned"))?;
        let record = cooldowns.get(user_id).cloned();
        if let Some(record) = record {
            if record.ends_at_epoch_seconds > now_epoch_seconds {
                return Ok(active_cooldown_status(user_id, &record, now_epoch_seconds));
            }
            cooldowns.remove(user_id);
        }

        Ok(inactive_cooldown_status(user_id))
    }

    fn start_cooldown(
        &self,
        user_id: &str,
        duration: CooldownDuration,
        now_epoch_seconds: i64,
    ) -> Result<CooldownStatusResponse, ApiError> {
        let mut cooldowns = self
            .cooldowns
            .lock()
            .map_err(|_| ApiError::internal("cooldown store lock poisoned"))?;

        if let Some(record) = cooldowns.get(user_id).cloned() {
            if record.ends_at_epoch_seconds > now_epoch_seconds {
                return Ok(active_cooldown_status(user_id, &record, now_epoch_seconds));
            }
        }

        let record = CooldownRecord {
            duration,
            started_at_epoch_seconds: now_epoch_seconds,
            ends_at_epoch_seconds: now_epoch_seconds.saturating_add(duration.seconds()),
        };
        cooldowns.insert(user_id.to_owned(), record.clone());

        Ok(active_cooldown_status(user_id, &record, now_epoch_seconds))
    }
}

fn validate_request(request: &SubmitSelfExclusionRequest) -> Result<(), ApiError> {
    if !request.consent {
        return Err(ApiError::bad_request(
            "self_exclusion_consent_required",
            "Self-exclusion consent is required",
        ));
    }

    match request.mode {
        SelfExclusionMode::Timeout => {
            if request.period.is_none() {
                return Err(ApiError::bad_request(
                    "self_exclusion_period_required",
                    "A timeout self-exclusion requires a period",
                ));
            }
            if request.points_disposition.is_some() {
                return Err(ApiError::bad_request(
                    "self_exclusion_points_disposition_not_allowed",
                    "A timeout self-exclusion must not include a points disposition",
                ));
            }
        }
        SelfExclusionMode::Permanent => {
            if request.points_disposition.is_none() {
                return Err(ApiError::bad_request(
                    "self_exclusion_points_disposition_required",
                    "A permanent self-exclusion requires a points disposition",
                ));
            }
            if request.period.is_some() {
                return Err(ApiError::bad_request(
                    "self_exclusion_period_not_allowed",
                    "A permanent self-exclusion must not include a period",
                ));
            }
        }
    }

    Ok(())
}

fn idempotency_key_from_headers(headers: &HeaderMap) -> Result<String, ApiError> {
    let Some(value) = headers.get(&crate::IDEMPOTENCY_KEY_HEADER) else {
        return Err(ApiError::bad_request(
            "idempotency_key_required",
            "Idempotency-Key header is required for self-exclusion requests",
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

fn self_exclusion_request_hash(user_id: &str, request: &SubmitSelfExclusionRequest) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    user_id.hash(&mut hasher);
    request.hash(&mut hasher);
    hasher.finish()
}

fn ends_at_epoch_seconds(
    starts_at_epoch_seconds: i64,
    period: Option<SelfExclusionPeriod>,
) -> Option<i64> {
    period.map(|period| {
        starts_at_epoch_seconds
            + match period {
                SelfExclusionPeriod::SixMonths => SIX_MONTHS_SECONDS,
                SelfExclusionPeriod::OneYear => ONE_YEAR_SECONDS,
            }
    })
}

fn normalized_reason(reason: Option<String>) -> Option<String> {
    reason
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn is_active(record: &SelfExclusionRecord, now_epoch_seconds: i64) -> bool {
    match record.mode {
        SelfExclusionMode::Permanent => true,
        SelfExclusionMode::Timeout => record
            .ends_at_epoch_seconds
            .is_some_and(|ends_at| ends_at > now_epoch_seconds),
    }
}

fn active_cooldown_status(
    user_id: &str,
    record: &CooldownRecord,
    now_epoch_seconds: i64,
) -> CooldownStatusResponse {
    CooldownStatusResponse {
        user_id: user_id.to_owned(),
        active: true,
        duration: Some(record.duration),
        started_at_epoch_seconds: Some(record.started_at_epoch_seconds),
        ends_at_epoch_seconds: Some(record.ends_at_epoch_seconds),
        remaining_seconds: record
            .ends_at_epoch_seconds
            .saturating_sub(now_epoch_seconds),
        available_durations: AVAILABLE_COOLDOWN_DURATIONS.to_vec(),
    }
}

fn inactive_cooldown_status(user_id: &str) -> CooldownStatusResponse {
    CooldownStatusResponse {
        user_id: user_id.to_owned(),
        active: false,
        duration: None,
        started_at_epoch_seconds: None,
        ends_at_epoch_seconds: None,
        remaining_seconds: 0,
        available_durations: AVAILABLE_COOLDOWN_DURATIONS.to_vec(),
    }
}
