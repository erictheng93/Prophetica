use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, signal};

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
    pub volume_24h_points: i64,
    pub volume_total_points: i64,
    pub opened_at: String,
    pub close_at: String,
    pub oracle_source: String,
    pub resolved_outcome: Option<String>,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone)]
struct AppState {
    markets: Arc<HashMap<String, Market>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ListMarketsQuery {
    status: Option<String>,
    category: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListMarketsResponse {
    markets: Vec<Market>,
}

#[derive(Debug, Serialize)]
struct ApiErrorResponse {
    error: &'static str,
    message: String,
}

pub fn router(markets: impl IntoIterator<Item = Market>) -> Router {
    let state = AppState {
        markets: Arc::new(
            markets
                .into_iter()
                .map(|market| (market.id.clone(), market))
                .collect(),
        ),
    };

    Router::new()
        .route("/internal/markets", get(list_markets))
        .route("/internal/markets/:market_id", get(get_market))
        .with_state(state)
}

/// Runs the market-service HTTP server with SPEC-015 read facade routes.
///
/// # Errors
///
/// Returns an error when configuration loading, socket binding, or serving fails.
pub async fn run_http() -> anyhow::Result<()> {
    let config = sdk::config::ServiceConfig::from_env(crate::SERVICE_NAME, crate::DEFAULT_PORT)?;
    tracing::info!(?config, "market service config loaded");

    let app = sdk::service_stub::router(&config).merge(router(seed_markets()));
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "market service listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn list_markets(
    State(state): State<AppState>,
    Query(query): Query<ListMarketsQuery>,
) -> Json<ListMarketsResponse> {
    let mut markets = state
        .markets
        .values()
        .filter(|market| {
            query
                .status
                .as_ref()
                .is_none_or(|status| market.status == *status)
        })
        .filter(|market| {
            query
                .category
                .as_ref()
                .is_none_or(|category| market.category == *category)
        })
        .cloned()
        .collect::<Vec<_>>();
    markets.sort_by(|a, b| b.opened_at.cmp(&a.opened_at));

    Json(ListMarketsResponse { markets })
}

async fn get_market(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
) -> Result<Json<Market>, (StatusCode, Json<ApiErrorResponse>)> {
    state.markets.get(&market_id).cloned().map(Json).ok_or((
        StatusCode::NOT_FOUND,
        Json(ApiErrorResponse {
            error: "market_not_found",
            message: format!("market {market_id} was not found"),
        }),
    ))
}

fn seed_markets() -> Vec<Market> {
    vec![Market {
        id: "wbsc-2026-tw-semifinal".to_owned(),
        title: "Taiwan reaches the 2026 WBSC semifinal".to_owned(),
        brief: "Seed market used until persistent SPEC-015 storage lands.".to_owned(),
        category: "sports".to_owned(),
        status: "open".to_owned(),
        yes_price_cents: 38,
        yes_bid_cents: 37,
        yes_ask_cents: 39,
        volume_24h_points: 184_500,
        volume_total_points: 2_840_300,
        opened_at: "2026-04-02T00:00:00+08:00".to_owned(),
        close_at: "2026-09-15T20:00:00+08:00".to_owned(),
        oracle_source: "WBSC official result".to_owned(),
        resolved_outcome: None,
        resolved_at: None,
    }]
}

async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        tracing::warn!(%error, "failed to install ctrl-c handler");
    }
}
