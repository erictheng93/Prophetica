use std::{
    collections::{BTreeMap, HashMap},
    future::Future,
    net::SocketAddr,
    sync::Arc,
};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, signal};
use tonic::transport::Server;

use proto::proph::v1::engine_service_server::EngineServiceServer;

use crate::{
    order_book::{OrderBook, Price, Quantity, SnapshotRestingOrder},
    service::InMemoryEngineService,
};

#[derive(Debug, Clone)]
struct AppState {
    books: Arc<HashMap<String, OrderBook>>,
}

#[derive(Debug, Clone, Deserialize)]
struct BookQuery {
    depth: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct BookLevelResponse {
    price_pts: Price,
    qty: Quantity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct BookSnapshotResponse {
    market_id: String,
    bids: Vec<BookLevelResponse>,
    asks: Vec<BookLevelResponse>,
}

pub fn router(books: impl IntoIterator<Item = (String, OrderBook)>) -> Router {
    let state = AppState {
        books: Arc::new(books.into_iter().collect()),
    };

    Router::new()
        .route(
            "/internal/engine/markets/:market_id/book",
            get(get_book_snapshot),
        )
        .with_state(state)
}

/// Runs all matching-engine transport surfaces.
///
/// # Errors
///
/// Returns an error when either HTTP or gRPC serving fails.
pub async fn run() -> anyhow::Result<()> {
    let ((), ()) = tokio::try_join!(run_http(), run_grpc())?;
    Ok(())
}

/// Runs the matching-engine HTTP server with SPEC-003 read facade routes.
///
/// # Errors
///
/// Returns an error when configuration loading, socket binding, or serving fails.
pub async fn run_http() -> anyhow::Result<()> {
    let config = sdk::config::ServiceConfig::from_env(crate::SERVICE_NAME, crate::DEFAULT_PORT)?;
    tracing::info!(?config, "matching engine config loaded");

    let app = sdk::service_stub::router(&config).merge(router(std::iter::empty()));
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "matching engine listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Runs the matching-engine gRPC server with SPEC-003 command routes.
///
/// # Errors
///
/// Returns an error when socket binding or serving fails.
pub async fn run_grpc() -> anyhow::Result<()> {
    let bind_addr = grpc_bind_addr_from_env();
    tracing::info!(addr = %bind_addr, "matching engine grpc listening");
    run_grpc_on_addr(bind_addr, shutdown_signal()).await
}

/// Runs the matching-engine gRPC server on a concrete address.
///
/// # Errors
///
/// Returns an error when tonic fails to serve the gRPC endpoint.
pub async fn run_grpc_on_addr(
    bind_addr: SocketAddr,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    Server::builder()
        .add_service(EngineServiceServer::new(InMemoryEngineService::default()))
        .serve_with_shutdown(bind_addr, shutdown)
        .await?;

    Ok(())
}

async fn get_book_snapshot(
    State(state): State<AppState>,
    Path(market_id): Path<String>,
    Query(query): Query<BookQuery>,
) -> Json<BookSnapshotResponse> {
    let depth = query.depth.unwrap_or(10).clamp(1, 100);
    let Some(book) = state.books.get(&market_id) else {
        return Json(BookSnapshotResponse {
            market_id,
            bids: Vec::new(),
            asks: Vec::new(),
        });
    };

    let snapshot = book.snapshot();
    Json(BookSnapshotResponse {
        market_id,
        bids: aggregate_levels(snapshot.bids, depth, true),
        asks: aggregate_levels(snapshot.asks, depth, false),
    })
}

fn aggregate_levels(
    orders: Vec<SnapshotRestingOrder>,
    depth: usize,
    descending: bool,
) -> Vec<BookLevelResponse> {
    let mut by_price = BTreeMap::<Price, Quantity>::new();
    for order in orders {
        *by_price.entry(order.price).or_default() += order.quantity;
    }

    if descending {
        by_price
            .into_iter()
            .rev()
            .take(depth)
            .map(|(price_pts, qty)| BookLevelResponse { price_pts, qty })
            .collect()
    } else {
        by_price
            .into_iter()
            .take(depth)
            .map(|(price_pts, qty)| BookLevelResponse { price_pts, qty })
            .collect()
    }
}

fn grpc_bind_addr_from_env() -> SocketAddr {
    let port = std::env::var("MATCHING_ENGINE_GRPC_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(crate::DEFAULT_GRPC_PORT);
    SocketAddr::from(([0, 0, 0, 0], port))
}

async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        tracing::warn!(%error, "failed to install ctrl-c handler");
    }
}
