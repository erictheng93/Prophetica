use std::{
    collections::HashMap,
    env,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderName, StatusCode},
    routing::{get, post},
};
use sdk::postgres_migrations::{POSITION_SERVICE_MIGRATION_TABLE, run_postgres_migrations};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use thiserror::Error;
use tokio::{net::TcpListener, signal};

const IDEMPOTENCY_KEY_HEADER: HeaderName = HeaderName::from_static("idempotency-key");

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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PositionFillProjectionResponse {
    pub position: Position,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
struct AppState<S> {
    store: S,
}

#[derive(Debug, Clone, Deserialize)]
struct ListPositionsQuery {
    status: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListPositionsResponse {
    positions: Vec<Position>,
}

#[derive(Debug, Serialize)]
struct ApiErrorResponse {
    error: &'static str,
    message: String,
}

#[derive(Debug, Error)]
pub enum PositionStoreError {
    #[error("position store is unavailable")]
    Unavailable,
    #[error("position projection is invalid: {message}")]
    InvalidProjection { message: String },
}

#[async_trait]
pub trait PositionStore: Clone + Send + Sync + 'static {
    async fn list_positions(
        &self,
        user_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<Position>, PositionStoreError>;

    async fn project_fill(
        &self,
        idempotency_key: &str,
        request: PositionFillProjectionRequest,
    ) -> Result<PositionFillProjectionResponse, PositionStoreError>;
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryPositionStore {
    positions: Arc<Mutex<Vec<Position>>>,
    projected_fills: Arc<Mutex<HashMap<String, Position>>>,
}

impl InMemoryPositionStore {
    #[must_use]
    pub fn new(positions: impl IntoIterator<Item = Position>) -> Self {
        Self {
            positions: Arc::new(Mutex::new(positions.into_iter().collect())),
            projected_fills: Arc::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PostgresPositionStore {
    pool: PgPool,
}

impl PostgresPositionStore {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

pub fn router<S>(store: S) -> Router
where
    S: PositionStore,
{
    Router::new()
        .route(
            "/internal/users/:user_id/positions",
            get(list_positions::<S>),
        )
        .route("/internal/positions/fills", post(project_fill::<S>))
        .with_state(AppState { store })
}

pub fn in_memory_router(positions: impl IntoIterator<Item = Position>) -> Router {
    router(InMemoryPositionStore::new(positions))
}

/// Runs the position-service HTTP server with the SPEC-015 positions read facade.
///
/// # Errors
///
/// Returns an error when configuration loading, socket binding, or serving fails.
#[tracing::instrument(fields(service.name = crate::SERVICE_NAME), skip_all)]
pub async fn run_http() -> anyhow::Result<()> {
    let config = sdk::config::ServiceConfig::from_env(crate::SERVICE_NAME, crate::DEFAULT_PORT)?;
    tracing::info!(?config, "position service config loaded");

    let app = if store_mode(env::var("POSITION_SERVICE_STORE").ok().as_deref()) == "postgres" {
        validate_position_service_store_mode(
            &env::var("PROPHETICA_ENV").unwrap_or_else(|_| "dev".to_owned()),
            "postgres",
        )?;
        let database_url = env::var("POSITION_SERVICE_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .map_err(|_| {
                anyhow::anyhow!(
                    "POSITION_SERVICE_DATABASE_URL or DATABASE_URL is required when POSITION_SERVICE_STORE=postgres"
                )
            })?;
        let pool = PgPool::connect(&database_url).await?;
        run_postgres_migrations(
            sqlx::migrate!("./migrations"),
            &pool,
            POSITION_SERVICE_MIGRATION_TABLE,
        )
        .await?;
        sdk::service_stub::router(&config).merge(router(PostgresPositionStore::new(pool)))
    } else {
        let prophetica_env = env::var("PROPHETICA_ENV").unwrap_or_else(|_| "dev".to_owned());
        validate_position_service_store_mode(&prophetica_env, "memory")?;
        sdk::service_stub::router(&config)
            .merge(router(InMemoryPositionStore::new(seed_positions())))
    };
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "position service listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME, proph.user_id = %user_id), skip(state))]
async fn list_positions<S>(
    State(state): State<AppState<S>>,
    Path(user_id): Path<String>,
    Query(query): Query<ListPositionsQuery>,
) -> Result<Json<ListPositionsResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: PositionStore,
{
    let positions = state
        .store
        .list_positions(&user_id, query.status.as_deref())
        .await
        .map_err(|error| store_error(&error))?;

    Ok(Json(ListPositionsResponse { positions }))
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME), skip(state, headers, request))]
async fn project_fill<S>(
    State(state): State<AppState<S>>,
    headers: HeaderMap,
    Json(request): Json<PositionFillProjectionRequest>,
) -> Result<Json<PositionFillProjectionResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: PositionStore,
{
    let idempotency_key = idempotency_key_from_headers(&headers)?;
    validate_fill_request(&request)?;
    let response = state
        .store
        .project_fill(&idempotency_key, request)
        .await
        .map_err(|error| store_error(&error))?;

    Ok(Json(response))
}

#[async_trait]
impl PositionStore for InMemoryPositionStore {
    async fn list_positions(
        &self,
        user_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<Position>, PositionStoreError> {
        let mut positions = self
            .positions
            .lock()
            .map_err(|_| PositionStoreError::Unavailable)?
            .iter()
            .filter(|position| position.user_id == user_id)
            .filter(|position| status_matches(position, status))
            .cloned()
            .collect::<Vec<_>>();
        positions.sort_by(|a, b| b.opened_at.cmp(&a.opened_at));

        Ok(positions)
    }

    async fn project_fill(
        &self,
        idempotency_key: &str,
        request: PositionFillProjectionRequest,
    ) -> Result<PositionFillProjectionResponse, PositionStoreError> {
        if let Some(position) = self
            .projected_fills
            .lock()
            .map_err(|_| PositionStoreError::Unavailable)?
            .get(idempotency_key)
            .cloned()
        {
            return Ok(PositionFillProjectionResponse {
                position,
                replayed: true,
            });
        }

        let projection = FillProjection::try_from_request(request)?;
        let mut positions = self
            .positions
            .lock()
            .map_err(|_| PositionStoreError::Unavailable)?;
        let position = apply_fill_to_positions(&mut positions, &projection)?;
        self.projected_fills
            .lock()
            .map_err(|_| PositionStoreError::Unavailable)?
            .insert(idempotency_key.to_owned(), position.clone());

        Ok(PositionFillProjectionResponse {
            position,
            replayed: false,
        })
    }
}

#[async_trait]
impl PositionStore for PostgresPositionStore {
    async fn list_positions(
        &self,
        user_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<Position>, PositionStoreError> {
        let rows = match status {
            Some("active") => {
                sqlx::query(POSITION_SELECT_ACTIVE_SQL)
                    .bind(user_id)
                    .fetch_all(&self.pool)
                    .await
            }
            Some("settled") => {
                sqlx::query(POSITION_SELECT_SETTLED_SQL)
                    .bind(user_id)
                    .fetch_all(&self.pool)
                    .await
            }
            Some(_) | None => {
                sqlx::query(POSITION_SELECT_ALL_SQL)
                    .bind(user_id)
                    .fetch_all(&self.pool)
                    .await
            }
        }
        .map_err(|error| sqlx_store_error(&error))?;

        rows.iter().map(row_to_position).collect()
    }

    async fn project_fill(
        &self,
        idempotency_key: &str,
        request: PositionFillProjectionRequest,
    ) -> Result<PositionFillProjectionResponse, PositionStoreError> {
        let projection = FillProjection::try_from_request(request)?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| sqlx_store_error(&error))?;

        if let Some(row) = sqlx::query(POSITION_SELECT_BY_FILL_SQL)
            .bind(idempotency_key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| sqlx_store_error(&error))?
        {
            let position = row_to_position(&row)?;
            tx.commit()
                .await
                .map_err(|error| sqlx_store_error(&error))?;
            return Ok(PositionFillProjectionResponse {
                position,
                replayed: true,
            });
        }

        let position = match projection.side {
            FillSide::Buy => postgres_project_buy(&mut tx, &projection).await?,
            FillSide::Sell => postgres_project_sell(&mut tx, &projection).await?,
        };

        sqlx::query(
            r"
            INSERT INTO position_fills (
                idempotency_key, position_id, user_id, market_id, outcome, side, quantity,
                price_pts, order_id, fill_id, ledger_txn_id, projected_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ",
        )
        .bind(idempotency_key)
        .bind(&position.id)
        .bind(&projection.user_id)
        .bind(&projection.market_id)
        .bind(&projection.outcome)
        .bind(projection.side.as_str())
        .bind(projection.quantity)
        .bind(projection.price_pts)
        .bind(&projection.order_id)
        .bind(&projection.fill_id)
        .bind(projection.ledger_txn_id)
        .bind(&projection.filled_at)
        .execute(&mut *tx)
        .await
        .map_err(|error| sqlx_store_error(&error))?;

        tx.commit()
            .await
            .map_err(|error| sqlx_store_error(&error))?;

        Ok(PositionFillProjectionResponse {
            position,
            replayed: false,
        })
    }
}

async fn postgres_project_buy(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    projection: &FillProjection,
) -> Result<Position, PositionStoreError> {
    let locked_delta = notional_amount(projection.quantity, projection.price_pts)?;
    let position_id = position_id(
        &projection.user_id,
        &projection.market_id,
        &projection.outcome,
    );
    let row = sqlx::query(
        r"
        INSERT INTO positions (
            id, user_id, market_id, outcome, qty, avg_price_pts, locked_amount, opened_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (id) DO UPDATE SET
            qty = positions.qty + EXCLUDED.qty,
            avg_price_pts = (
                (
                    positions.qty * positions.avg_price_pts
                    + EXCLUDED.qty * EXCLUDED.avg_price_pts
                ) / NULLIF(positions.qty + EXCLUDED.qty, 0)
            )::INTEGER,
            locked_amount = positions.locked_amount + EXCLUDED.locked_amount,
            closed_at = NULL,
            settle_txn_id = NULL
        RETURNING id, user_id, market_id, outcome, qty, avg_price_pts, locked_amount,
                  opened_at, closed_at, settle_txn_id, realized_pnl_points
        ",
    )
    .bind(&position_id)
    .bind(&projection.user_id)
    .bind(&projection.market_id)
    .bind(&projection.outcome)
    .bind(projection.quantity)
    .bind(projection.price_pts)
    .bind(locked_delta)
    .bind(&projection.filled_at)
    .fetch_one(&mut **tx)
    .await
    .map_err(|error| sqlx_store_error(&error))?;

    row_to_position(&row)
}

async fn postgres_project_sell(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    projection: &FillProjection,
) -> Result<Position, PositionStoreError> {
    let position_id = position_id(
        &projection.user_id,
        &projection.market_id,
        &projection.outcome,
    );
    let Some(existing_row) = sqlx::query(
        r"
        SELECT id, user_id, market_id, outcome, qty, avg_price_pts, locked_amount,
               opened_at, closed_at, settle_txn_id, realized_pnl_points
        FROM positions
        WHERE id = $1
        FOR UPDATE
        ",
    )
    .bind(&position_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|error| sqlx_store_error(&error))?
    else {
        return Err(PositionStoreError::InvalidProjection {
            message: "sell fill requires an existing active position".to_owned(),
        });
    };
    let existing = row_to_position(&existing_row)?;
    let updated = apply_sell_to_position(existing, projection)?;
    let row = sqlx::query(
        r"
        UPDATE positions
        SET qty = $2,
            locked_amount = $3,
            closed_at = $4,
            realized_pnl_points = $5
        WHERE id = $1
        RETURNING id, user_id, market_id, outcome, qty, avg_price_pts, locked_amount,
                  opened_at, closed_at, settle_txn_id, realized_pnl_points
        ",
    )
    .bind(&updated.id)
    .bind(updated.qty)
    .bind(updated.locked_amount)
    .bind(&updated.closed_at)
    .bind(updated.realized_pnl_points)
    .fetch_one(&mut **tx)
    .await
    .map_err(|error| sqlx_store_error(&error))?;

    row_to_position(&row)
}

fn apply_fill_to_positions(
    positions: &mut Vec<Position>,
    projection: &FillProjection,
) -> Result<Position, PositionStoreError> {
    let target_id = position_id(
        &projection.user_id,
        &projection.market_id,
        &projection.outcome,
    );
    if let Some(existing) = positions
        .iter_mut()
        .find(|position| position.id == target_id && position.closed_at.is_none())
    {
        match projection.side {
            FillSide::Buy => apply_buy_to_position(existing, projection)?,
            FillSide::Sell => {
                let updated = apply_sell_to_position(existing.clone(), projection)?;
                *existing = updated;
            }
        }
        return Ok(existing.clone());
    }

    if projection.side == FillSide::Sell {
        return Err(PositionStoreError::InvalidProjection {
            message: "sell fill requires an existing active position".to_owned(),
        });
    }

    let locked_amount = notional_amount(projection.quantity, projection.price_pts)?;
    let position = Position {
        id: target_id,
        user_id: projection.user_id.clone(),
        market_id: projection.market_id.clone(),
        outcome: projection.outcome.clone(),
        qty: projection.quantity,
        avg_price_pts: projection.price_pts,
        locked_amount,
        opened_at: projection.filled_at.clone(),
        closed_at: None,
        settle_txn_id: None,
        realized_pnl_points: None,
    };
    positions.push(position.clone());
    Ok(position)
}

fn apply_buy_to_position(
    position: &mut Position,
    projection: &FillProjection,
) -> Result<(), PositionStoreError> {
    let original_qty = position.qty;
    let filled_qty = projection.quantity;
    let total_qty = original_qty.checked_add(filled_qty).ok_or_else(|| {
        PositionStoreError::InvalidProjection {
            message: "position quantity overflow".to_owned(),
        }
    })?;
    let locked_delta = notional_amount(filled_qty, projection.price_pts)?;
    position.avg_price_pts = weighted_average_price(
        original_qty,
        position.avg_price_pts,
        filled_qty,
        projection.price_pts,
    )?;
    position.qty = total_qty;
    position.locked_amount = position
        .locked_amount
        .checked_add(locked_delta)
        .ok_or_else(|| PositionStoreError::InvalidProjection {
            message: "position locked amount overflow".to_owned(),
        })?;
    position.closed_at = None;
    position.settle_txn_id = None;
    Ok(())
}

fn apply_sell_to_position(
    mut position: Position,
    projection: &FillProjection,
) -> Result<Position, PositionStoreError> {
    if position.closed_at.is_some() {
        return Err(PositionStoreError::InvalidProjection {
            message: "sell fill requires an active position".to_owned(),
        });
    }
    if projection.quantity > position.qty {
        return Err(PositionStoreError::InvalidProjection {
            message: "sell fill quantity exceeds active position quantity".to_owned(),
        });
    }

    let locked_reduction =
        proportional_amount(position.locked_amount, projection.quantity, position.qty)?;
    let realized_delta = i64::from(projection.price_pts - position.avg_price_pts)
        .checked_mul(projection.quantity)
        .ok_or_else(|| PositionStoreError::InvalidProjection {
            message: "realized pnl overflow".to_owned(),
        })?;

    position.qty -= projection.quantity;
    position.locked_amount -= locked_reduction;
    position.realized_pnl_points = Some(
        position
            .realized_pnl_points
            .unwrap_or_default()
            .checked_add(realized_delta)
            .ok_or_else(|| PositionStoreError::InvalidProjection {
                message: "realized pnl overflow".to_owned(),
            })?,
    );
    if position.qty == 0 {
        position.closed_at = Some(projection.filled_at.clone());
    }

    Ok(position)
}

fn seed_positions() -> Vec<Position> {
    vec![
        Position {
            id: "pos-wbsc-yes".to_owned(),
            user_id: "user_01".to_owned(),
            market_id: "wbsc-2026-tw-semifinal".to_owned(),
            outcome: "YES".to_owned(),
            qty: 100,
            avg_price_pts: 32,
            locked_amount: 3_200,
            opened_at: "2026-05-01T00:00:00+08:00".to_owned(),
            closed_at: None,
            settle_txn_id: None,
            realized_pnl_points: None,
        },
        Position {
            id: "pos-nobel-yes".to_owned(),
            user_id: "user_01".to_owned(),
            market_id: "nobel-2025-literature-korean".to_owned(),
            outcome: "YES".to_owned(),
            qty: 60,
            avg_price_pts: 45,
            locked_amount: 2_700,
            opened_at: "2025-08-08T00:00:00+08:00".to_owned(),
            closed_at: Some("2025-10-09T19:00:00+08:00".to_owned()),
            settle_txn_id: Some(9001),
            realized_pnl_points: Some(33),
        },
    ]
}

fn store_mode(value: Option<&str>) -> &'static str {
    match value {
        Some("postgres") => "postgres",
        _ => "memory",
    }
}

fn validate_position_service_store_mode(prophetica_env: &str, mode: &str) -> anyhow::Result<()> {
    if prophetica_env.eq_ignore_ascii_case("production") && mode != "postgres" {
        anyhow::bail!("POSITION_SERVICE_STORE=postgres is required when PROPHETICA_ENV=production");
    }
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        tracing::warn!(%error, "failed to install ctrl-c handler");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FillSide {
    Buy,
    Sell,
}

impl FillSide {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FillProjection {
    user_id: String,
    market_id: String,
    outcome: String,
    side: FillSide,
    quantity: i64,
    price_pts: i32,
    order_id: String,
    fill_id: String,
    filled_at: String,
    ledger_txn_id: Option<i64>,
}

impl FillProjection {
    fn try_from_request(
        request: PositionFillProjectionRequest,
    ) -> Result<Self, PositionStoreError> {
        Ok(Self {
            user_id: request.user_id.trim().to_owned(),
            market_id: request.market_id.trim().to_owned(),
            outcome: normalize_outcome(&request.outcome).map_err(invalid_projection)?,
            side: normalize_side(&request.side).map_err(invalid_projection)?,
            quantity: request.quantity,
            price_pts: request.price_pts,
            order_id: request.order_id.trim().to_owned(),
            fill_id: request.fill_id.trim().to_owned(),
            filled_at: request.filled_at.unwrap_or_else(current_timestamp_string),
            ledger_txn_id: request.ledger_txn_id,
        })
    }
}

fn validate_fill_request(
    request: &PositionFillProjectionRequest,
) -> Result<(), (StatusCode, Json<ApiErrorResponse>)> {
    if request.user_id.trim().is_empty() {
        return Err(validation_error("user_id is required"));
    }
    if request.market_id.trim().is_empty() {
        return Err(validation_error("market_id is required"));
    }
    if request.order_id.trim().is_empty() {
        return Err(validation_error("order_id is required"));
    }
    if request.fill_id.trim().is_empty() {
        return Err(validation_error("fill_id is required"));
    }
    normalize_outcome(&request.outcome).map_err(validation_error)?;
    normalize_side(&request.side).map_err(validation_error)?;
    if request.quantity <= 0 {
        return Err(validation_error("quantity must be positive"));
    }
    if !(0..=100).contains(&request.price_pts) {
        return Err(validation_error("price_pts must be in [0, 100]"));
    }
    Ok(())
}

fn idempotency_key_from_headers(
    headers: &HeaderMap,
) -> Result<String, (StatusCode, Json<ApiErrorResponse>)> {
    headers
        .get(&IDEMPOTENCY_KEY_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| validation_error("idempotency-key header is required"))
}

fn normalize_outcome(value: &str) -> Result<String, &'static str> {
    if value.eq_ignore_ascii_case("YES") {
        return Ok("YES".to_owned());
    }
    if value.eq_ignore_ascii_case("NO") {
        return Ok("NO".to_owned());
    }
    Err("outcome must be YES or NO")
}

fn normalize_side(value: &str) -> Result<FillSide, &'static str> {
    if value.eq_ignore_ascii_case("BUY") || value.eq_ignore_ascii_case("BID") {
        return Ok(FillSide::Buy);
    }
    if value.eq_ignore_ascii_case("SELL") || value.eq_ignore_ascii_case("ASK") {
        return Ok(FillSide::Sell);
    }
    Err("side must be BUY or SELL")
}

fn invalid_projection(message: &'static str) -> PositionStoreError {
    PositionStoreError::InvalidProjection {
        message: message.to_owned(),
    }
}

fn store_error(error: &PositionStoreError) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        PositionStoreError::Unavailable => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "position_store_unavailable",
            error.to_string(),
        ),
        PositionStoreError::InvalidProjection { .. } => api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_position_projection",
            error.to_string(),
        ),
    }
}

fn validation_error(message: impl Into<String>) -> (StatusCode, Json<ApiErrorResponse>) {
    api_error(
        StatusCode::UNPROCESSABLE_ENTITY,
        "validation_failed",
        message,
    )
}

fn api_error(
    status: StatusCode,
    error: &'static str,
    message: impl Into<String>,
) -> (StatusCode, Json<ApiErrorResponse>) {
    (
        status,
        Json(ApiErrorResponse {
            error,
            message: message.into(),
        }),
    )
}

fn status_matches(position: &Position, status: Option<&str>) -> bool {
    match status {
        Some("active") => position.closed_at.is_none(),
        Some("settled") => position.closed_at.is_some(),
        Some(_) | None => true,
    }
}

fn position_id(user_id: &str, market_id: &str, outcome: &str) -> String {
    format!("pos:{user_id}:{market_id}:{outcome}")
}

fn notional_amount(quantity: i64, price_pts: i32) -> Result<i64, PositionStoreError> {
    quantity.checked_mul(i64::from(price_pts)).ok_or_else(|| {
        PositionStoreError::InvalidProjection {
            message: "notional amount overflow".to_owned(),
        }
    })
}

fn weighted_average_price(
    original_qty: i64,
    original_price: i32,
    filled_qty: i64,
    fill_price: i32,
) -> Result<i32, PositionStoreError> {
    let total_qty = original_qty.checked_add(filled_qty).ok_or_else(|| {
        PositionStoreError::InvalidProjection {
            message: "position quantity overflow".to_owned(),
        }
    })?;
    if total_qty <= 0 {
        return Err(PositionStoreError::InvalidProjection {
            message: "position quantity must stay positive".to_owned(),
        });
    }

    let weighted = original_qty
        .checked_mul(i64::from(original_price))
        .and_then(|value| {
            filled_qty
                .checked_mul(i64::from(fill_price))
                .and_then(|fill_value| value.checked_add(fill_value))
        })
        .ok_or_else(|| PositionStoreError::InvalidProjection {
            message: "weighted average overflow".to_owned(),
        })?;
    i32::try_from(weighted / total_qty).map_err(|_| PositionStoreError::InvalidProjection {
        message: "weighted average overflow".to_owned(),
    })
}

fn proportional_amount(
    amount: i64,
    quantity: i64,
    total_quantity: i64,
) -> Result<i64, PositionStoreError> {
    if total_quantity <= 0 {
        return Err(PositionStoreError::InvalidProjection {
            message: "position quantity must be positive".to_owned(),
        });
    }
    amount
        .checked_mul(quantity)
        .map(|value| value / total_quantity)
        .ok_or_else(|| PositionStoreError::InvalidProjection {
            message: "proportional amount overflow".to_owned(),
        })
}

fn current_timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
        .to_string()
}

fn sqlx_store_error(error: &sqlx::Error) -> PositionStoreError {
    tracing::error!(%error, "position postgres store error");
    PositionStoreError::Unavailable
}

fn row_to_position(row: &sqlx::postgres::PgRow) -> Result<Position, PositionStoreError> {
    Ok(Position {
        id: row
            .try_get("id")
            .map_err(|error| sqlx_store_error(&error))?,
        user_id: row
            .try_get("user_id")
            .map_err(|error| sqlx_store_error(&error))?,
        market_id: row
            .try_get("market_id")
            .map_err(|error| sqlx_store_error(&error))?,
        outcome: row
            .try_get("outcome")
            .map_err(|error| sqlx_store_error(&error))?,
        qty: row
            .try_get("qty")
            .map_err(|error| sqlx_store_error(&error))?,
        avg_price_pts: row
            .try_get("avg_price_pts")
            .map_err(|error| sqlx_store_error(&error))?,
        locked_amount: row
            .try_get("locked_amount")
            .map_err(|error| sqlx_store_error(&error))?,
        opened_at: row
            .try_get("opened_at")
            .map_err(|error| sqlx_store_error(&error))?,
        closed_at: row
            .try_get("closed_at")
            .map_err(|error| sqlx_store_error(&error))?,
        settle_txn_id: row
            .try_get("settle_txn_id")
            .map_err(|error| sqlx_store_error(&error))?,
        realized_pnl_points: row
            .try_get("realized_pnl_points")
            .map_err(|error| sqlx_store_error(&error))?,
    })
}

const POSITION_SELECT_ALL_SQL: &str = r"
    SELECT id, user_id, market_id, outcome, qty, avg_price_pts, locked_amount,
           opened_at, closed_at, settle_txn_id, realized_pnl_points
    FROM positions
    WHERE user_id = $1
    ORDER BY opened_at DESC, id DESC
";

const POSITION_SELECT_ACTIVE_SQL: &str = r"
    SELECT id, user_id, market_id, outcome, qty, avg_price_pts, locked_amount,
           opened_at, closed_at, settle_txn_id, realized_pnl_points
    FROM positions
    WHERE user_id = $1 AND closed_at IS NULL
    ORDER BY opened_at DESC, id DESC
";

const POSITION_SELECT_SETTLED_SQL: &str = r"
    SELECT id, user_id, market_id, outcome, qty, avg_price_pts, locked_amount,
           opened_at, closed_at, settle_txn_id, realized_pnl_points
    FROM positions
    WHERE user_id = $1 AND closed_at IS NOT NULL
    ORDER BY opened_at DESC, id DESC
";

const POSITION_SELECT_BY_FILL_SQL: &str = r"
    SELECT p.id, p.user_id, p.market_id, p.outcome, p.qty, p.avg_price_pts, p.locked_amount,
           p.opened_at, p.closed_at, p.settle_txn_id, p.realized_pnl_points
    FROM position_fills f
    INNER JOIN positions p ON p.id = f.position_id
    WHERE f.idempotency_key = $1
";

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        InMemoryPositionStore, Position, PositionFillProjectionRequest, PositionStore, store_mode,
        validate_position_service_store_mode,
    };

    #[test]
    fn store_mode_defaults_to_memory() {
        assert_eq!(store_mode(None), "memory");
    }

    #[test]
    fn store_mode_accepts_postgres() {
        assert_eq!(store_mode(Some("postgres")), "postgres");
    }

    #[test]
    fn production_requires_postgres_store() {
        assert!(validate_position_service_store_mode("production", "postgres").is_ok());
        assert!(validate_position_service_store_mode("dev", "memory").is_ok());

        let result = validate_position_service_store_mode("production", "memory");
        assert!(result.is_err());
        let error = result.err().map(|error| error.to_string());
        assert_eq!(
            error.as_deref(),
            Some("POSITION_SERVICE_STORE=postgres is required when PROPHETICA_ENV=production")
        );
    }

    #[tokio::test]
    async fn in_memory_store_projects_buy_fill_idempotently() {
        let store = InMemoryPositionStore::default();
        let request = fill_request("BUY", 10, 40);

        let first = store
            .project_fill("fill-key-1", request.clone())
            .await
            .expect("first fill should project");
        let replay = store
            .project_fill("fill-key-1", request)
            .await
            .expect("same fill should replay");

        assert!(!first.replayed);
        assert!(replay.replayed);
        assert_eq!(first.position.qty, 10);
        assert_eq!(first.position.avg_price_pts, 40);
        assert_eq!(first.position.locked_amount, 400);
        assert_eq!(replay.position, first.position);
    }

    #[tokio::test]
    async fn in_memory_store_reduces_position_on_sell_fill() {
        let store = InMemoryPositionStore::new([Position {
            id: "pos:user_01:mkt-election:YES".to_owned(),
            user_id: "user_01".to_owned(),
            market_id: "mkt-election".to_owned(),
            outcome: "YES".to_owned(),
            qty: 10,
            avg_price_pts: 40,
            locked_amount: 400,
            opened_at: "2026-05-21T00:00:00Z".to_owned(),
            closed_at: None,
            settle_txn_id: None,
            realized_pnl_points: None,
        }]);

        let response = store
            .project_fill("fill-key-sell", fill_request("SELL", 4, 55))
            .await
            .expect("sell fill should project");

        assert_eq!(response.position.qty, 6);
        assert_eq!(response.position.locked_amount, 240);
        assert_eq!(response.position.realized_pnl_points, Some(60));
        assert_eq!(response.position.closed_at, None);
    }

    fn fill_request(side: &str, quantity: i64, price_pts: i32) -> PositionFillProjectionRequest {
        PositionFillProjectionRequest {
            user_id: "user_01".to_owned(),
            market_id: "mkt-election".to_owned(),
            outcome: "YES".to_owned(),
            side: side.to_owned(),
            quantity,
            price_pts,
            order_id: "ord_01".to_owned(),
            fill_id: "fill_01".to_owned(),
            filled_at: Some("2026-05-22T00:00:00Z".to_owned()),
            ledger_txn_id: Some(42),
        }
    }
}
