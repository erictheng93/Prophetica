use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, signal};

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

#[derive(Debug, Clone)]
struct AppState {
    positions: Arc<Vec<Position>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ListPositionsQuery {
    status: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListPositionsResponse {
    positions: Vec<Position>,
}

pub fn router(positions: impl IntoIterator<Item = Position>) -> Router {
    Router::new()
        .route("/internal/users/:user_id/positions", get(list_positions))
        .with_state(AppState {
            positions: Arc::new(positions.into_iter().collect()),
        })
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

    let app = sdk::service_stub::router(&config).merge(router(seed_positions()));
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "position service listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME, proph.user_id = %user_id), skip(state))]
async fn list_positions(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Query(query): Query<ListPositionsQuery>,
) -> Json<ListPositionsResponse> {
    let mut positions = state
        .positions
        .iter()
        .filter(|position| position.user_id == user_id)
        .filter(|position| match query.status.as_deref() {
            Some("active") => position.closed_at.is_none(),
            Some("settled") => position.closed_at.is_some(),
            Some(_) | None => true,
        })
        .cloned()
        .collect::<Vec<_>>();
    positions.sort_by(|a, b| b.opened_at.cmp(&a.opened_at));

    Json(ListPositionsResponse { positions })
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

async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        tracing::warn!(%error, "failed to install ctrl-c handler");
    }
}
