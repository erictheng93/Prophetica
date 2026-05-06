use std::{
    collections::{BTreeMap, HashMap},
    future::Future,
    net::SocketAddr,
    sync::Arc,
};

use axum::{
    Json, Router,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
};
use proto::proph::v1::{
    CancelOrderRequest, GetBookSnapshotRequest, MarketId, OrderId as ProtoOrderId, OrderStatus,
    OrderType as ProtoOrderType, PlaceOrderRequest, Side as ProtoSide, TimeInForce, UserId,
    engine_service_server::{EngineService, EngineServiceServer},
};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, signal};
use tonic::{Code, Request, Status, transport::Server};
use ulid::Ulid;

use crate::{
    order_book::{OrderBook, Price, Quantity, SnapshotRestingOrder},
    projection::{
        book_stream_message, order_update_stream_message, project_engine_event,
        trade_stream_message,
    },
    service::InMemoryEngineService,
};

#[derive(Debug, Clone)]
struct AppState {
    books: Arc<HashMap<String, OrderBook>>,
}

#[derive(Debug, Clone)]
struct ServiceState {
    service: InMemoryEngineService,
}

#[derive(Debug, Clone, Deserialize)]
struct BookQuery {
    depth: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlaceOrderHttpRequest {
    side: String,
    outcome: String,
    #[serde(rename = "type")]
    order_type: String,
    price_pts: i32,
    quantity: i64,
    time_in_force: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct BookSnapshotStreamMessage {
    #[serde(rename = "type")]
    message_type: &'static str,
    market_id: String,
    seq: u64,
    bids: Vec<BookLevelResponse>,
    asks: Vec<BookLevelResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct OrderActionResponse {
    order_id: String,
    status: &'static str,
    filled_qty: i64,
    avg_price_pts: i32,
    remaining_qty: i64,
    fee_pts: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ErrorResponse {
    error: String,
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

pub fn service_router(service: InMemoryEngineService) -> Router {
    Router::new()
        .route("/api/v1/markets/:market_id/orders", post(place_order_http))
        .route("/api/v1/orders/:order_id", delete(cancel_order_http))
        .route(
            "/api/v1/markets/:market_id/book",
            get(get_service_book_snapshot),
        )
        .route("/ws/markets/:market_id/book", get(book_websocket))
        .route("/ws/markets/:market_id/trades", get(trade_websocket))
        .route("/ws/users/:user_id/orders", get(order_websocket))
        .with_state(ServiceState { service })
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

    let app = sdk::service_stub::router(&config)
        .merge(router(std::iter::empty()))
        .merge(service_router(InMemoryEngineService::default()));
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

async fn place_order_http(
    State(state): State<ServiceState>,
    Path(market_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<PlaceOrderHttpRequest>,
) -> Result<Json<OrderActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    validate_outcome(&payload.outcome)?;
    let user_id = user_id_from_headers(&headers)?;
    let response = state
        .service
        .place_order(Request::new(PlaceOrderRequest {
            audit: None,
            user_id: Some(UserId { value: user_id }),
            market_id: Some(MarketId { value: market_id }),
            side: map_http_side(&payload.side)?.into(),
            outcome: 1,
            r#type: map_http_order_type(&payload.order_type)?.into(),
            price_pts: payload.price_pts,
            quantity: payload.quantity,
            tif: map_http_time_in_force(&payload.time_in_force)?.into(),
            post_only: payload.order_type.eq_ignore_ascii_case("POST_ONLY"),
            client_order_id: headers
                .get("x-client-order-id")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_string(),
        }))
        .await
        .map_err(|error| http_error(&error))?;

    let response = response.into_inner();
    let order = response.order.ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "place order response missing order".to_string(),
            }),
        )
    })?;
    let order_id = order
        .id
        .as_ref()
        .map(|id| http_order_id(&id.value))
        .transpose()?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "place order response missing order id".to_string(),
                }),
            )
        })?;

    Ok(Json(OrderActionResponse {
        order_id,
        status: http_order_status(order.status),
        filled_qty: response.filled_qty,
        avg_price_pts: response.avg_price_pts,
        remaining_qty: order.qty_remaining,
        fee_pts: response.fee.map_or(0, |fee| fee.amount),
    }))
}

async fn cancel_order_http(
    State(state): State<ServiceState>,
    Path(order_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<OrderActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let user_id = user_id_from_headers(&headers)?;
    let response = state
        .service
        .cancel_order(Request::new(CancelOrderRequest {
            audit: None,
            order_id: Some(ProtoOrderId {
                value: proto_order_id(&order_id)?,
            }),
            user_id: Some(UserId { value: user_id }),
        }))
        .await
        .map_err(|error| http_error(&error))?
        .into_inner();

    let order = response.order.ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "cancel response missing order".to_string(),
            }),
        )
    })?;
    Ok(Json(OrderActionResponse {
        order_id,
        status: http_order_status(order.status),
        filled_qty: 0,
        avg_price_pts: 0,
        remaining_qty: order.qty_remaining,
        fee_pts: 0,
    }))
}

async fn get_service_book_snapshot(
    State(state): State<ServiceState>,
    Path(market_id): Path<String>,
    Query(query): Query<BookQuery>,
) -> Result<Json<BookSnapshotResponse>, (StatusCode, Json<ErrorResponse>)> {
    let depth = i32::try_from(query.depth.unwrap_or(10)).unwrap_or(10);
    let snapshot = state
        .service
        .get_book_snapshot(Request::new(GetBookSnapshotRequest {
            market_id: Some(MarketId {
                value: market_id.clone(),
            }),
            depth,
        }))
        .await
        .map_err(|error| http_error(&error))?
        .into_inner();

    Ok(Json(BookSnapshotResponse {
        market_id,
        bids: proto_levels(snapshot.bids),
        asks: proto_levels(snapshot.asks),
    }))
}

async fn book_websocket(
    State(state): State<ServiceState>,
    Path(market_id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| stream_book_deltas(socket, state.service, market_id))
}

async fn trade_websocket(
    State(state): State<ServiceState>,
    Path(market_id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| stream_trades(socket, state.service, market_id))
}

async fn order_websocket(
    State(state): State<ServiceState>,
    Path(user_id): Path<u64>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| stream_order_updates(socket, state.service, user_id))
}

async fn stream_book_deltas(
    mut socket: WebSocket,
    service: InMemoryEngineService,
    market_id: String,
) {
    let mut events = service.subscribe_events();
    if send_book_snapshot(&mut socket, &service, &market_id)
        .await
        .is_err()
    {
        return;
    }

    loop {
        let Ok(event) = events.recv().await else {
            break;
        };

        for projected in project_engine_event(event) {
            if let Some(message) = book_stream_message(&market_id, &projected) {
                if socket.send(Message::Text(message)).await.is_err() {
                    return;
                }
            }
        }
    }
}

async fn stream_trades(mut socket: WebSocket, service: InMemoryEngineService, market_id: String) {
    let mut events = service.subscribe_events();

    loop {
        let Ok(event) = events.recv().await else {
            break;
        };

        for projected in project_engine_event(event) {
            if let Some(message) = trade_stream_message(&market_id, &projected) {
                if socket.send(Message::Text(message)).await.is_err() {
                    return;
                }
            }
        }
    }
}

async fn stream_order_updates(mut socket: WebSocket, service: InMemoryEngineService, user_id: u64) {
    let mut events = service.subscribe_events();

    loop {
        let Ok(event) = events.recv().await else {
            break;
        };

        for projected in project_engine_event(event) {
            if let Some(message) = order_update_stream_message(user_id, &projected) {
                if socket.send(Message::Text(message)).await.is_err() {
                    return;
                }
            }
        }
    }
}

async fn send_book_snapshot(
    socket: &mut WebSocket,
    service: &InMemoryEngineService,
    market_id: &str,
) -> Result<(), ()> {
    let snapshot = service
        .get_book_snapshot(Request::new(GetBookSnapshotRequest {
            market_id: Some(MarketId {
                value: market_id.to_string(),
            }),
            depth: 100,
        }))
        .await
        .map_err(|_| ())?
        .into_inner();
    let message = serde_json::to_string(&BookSnapshotStreamMessage {
        message_type: "snapshot",
        market_id: market_id.to_string(),
        seq: 0,
        bids: proto_levels(snapshot.bids),
        asks: proto_levels(snapshot.asks),
    })
    .map_err(|_| ())?;

    socket.send(Message::Text(message)).await.map_err(|_| ())
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

fn proto_levels(levels: Vec<proto::proph::v1::BookLevel>) -> Vec<BookLevelResponse> {
    levels
        .into_iter()
        .map(|level| BookLevelResponse {
            price_pts: u64::try_from(level.price_pts).unwrap_or_default(),
            qty: u64::try_from(level.qty).unwrap_or_default(),
        })
        .collect()
}

fn user_id_from_headers(headers: &HeaderMap) -> Result<u64, (StatusCode, Json<ErrorResponse>)> {
    let raw = headers.get("x-user-id").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "x-user-id header is required".to_string(),
            }),
        )
    })?;
    raw.to_str()
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value != 0)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "x-user-id header must be a non-zero integer".to_string(),
                }),
            )
        })
}

fn validate_outcome(outcome: &str) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if outcome.eq_ignore_ascii_case("YES") {
        return Ok(());
    }

    Err((
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "only YES outcome orders are currently supported".to_string(),
        }),
    ))
}

fn map_http_side(side: &str) -> Result<ProtoSide, (StatusCode, Json<ErrorResponse>)> {
    if side.eq_ignore_ascii_case("buy") {
        Ok(ProtoSide::Bid)
    } else if side.eq_ignore_ascii_case("sell") {
        Ok(ProtoSide::Ask)
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "side must be buy or sell".to_string(),
            }),
        ))
    }
}

fn map_http_order_type(
    order_type: &str,
) -> Result<ProtoOrderType, (StatusCode, Json<ErrorResponse>)> {
    if order_type.eq_ignore_ascii_case("LIMIT") {
        Ok(ProtoOrderType::Limit)
    } else if order_type.eq_ignore_ascii_case("IOC") {
        Ok(ProtoOrderType::Ioc)
    } else if order_type.eq_ignore_ascii_case("FOK") {
        Ok(ProtoOrderType::Fok)
    } else if order_type.eq_ignore_ascii_case("POST_ONLY") {
        Ok(ProtoOrderType::PostOnly)
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "type must be LIMIT, IOC, FOK, or POST_ONLY".to_string(),
            }),
        ))
    }
}

fn map_http_time_in_force(
    time_in_force: &str,
) -> Result<TimeInForce, (StatusCode, Json<ErrorResponse>)> {
    if time_in_force.eq_ignore_ascii_case("GTC") {
        Ok(TimeInForce::Gtc)
    } else if time_in_force.eq_ignore_ascii_case("IOC") {
        Ok(TimeInForce::Ioc)
    } else if time_in_force.eq_ignore_ascii_case("FOK") {
        Ok(TimeInForce::Fok)
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "time_in_force must be GTC, IOC, or FOK".to_string(),
            }),
        ))
    }
}

fn http_order_id(order_id: &[u8]) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let bytes: [u8; 16] = order_id.try_into().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "engine returned invalid order id".to_string(),
            }),
        )
    })?;
    Ok(format!("ord_{}", Ulid::from_bytes(bytes)))
}

fn proto_order_id(order_id: &str) -> Result<Vec<u8>, (StatusCode, Json<ErrorResponse>)> {
    let raw = order_id.strip_prefix("ord_").ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "order_id must start with ord_".to_string(),
            }),
        )
    })?;
    raw.parse::<Ulid>()
        .map(|ulid| ulid.to_bytes().to_vec())
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "order_id is not a valid ULID".to_string(),
                }),
            )
        })
}

fn http_order_status(status: i32) -> &'static str {
    match OrderStatus::try_from(status).unwrap_or(OrderStatus::Unspecified) {
        OrderStatus::Active => "ACTIVE",
        OrderStatus::Filled => "FILLED",
        OrderStatus::Partial => "PARTIAL",
        OrderStatus::Cancelled => "CANCELLED",
        OrderStatus::Pending => "PENDING",
        OrderStatus::Unspecified => "UNSPECIFIED",
    }
}

fn http_error(error: &Status) -> (StatusCode, Json<ErrorResponse>) {
    let status = match error.code() {
        Code::InvalidArgument => StatusCode::BAD_REQUEST,
        Code::NotFound => StatusCode::NOT_FOUND,
        Code::AlreadyExists => StatusCode::CONFLICT,
        Code::PermissionDenied => StatusCode::FORBIDDEN,
        Code::FailedPrecondition => StatusCode::PRECONDITION_FAILED,
        Code::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        Json(ErrorResponse {
            error: error.message().to_string(),
        }),
    )
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
