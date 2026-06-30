use std::{
    collections::{BTreeMap, HashMap},
    env,
    future::Future,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use axum::{
    Json, Router,
    body::Body,
    extract::{
        MatchedPath, Path, Query, Request as AxumRequest, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use proto::proph::v1::{
    CancelOrderRequest, GetBookSnapshotRequest, MarketId, OrderId as ProtoOrderId, OrderStatus,
    OrderType as ProtoOrderType, Outcome as ProtoOutcome, PlaceOrderRequest, Side as ProtoSide,
    TimeInForce, Trade as ProtoTrade, UserId,
    engine_service_server::{EngineService, EngineServiceServer},
};
use sdk::service_stub::{ReadinessDependency, ReadinessState};
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

#[cfg(feature = "kafka")]
use crate::projection::KafkaProjectionSink;

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
    #[serde(default)]
    fee_curve_bps: Option<i32>,
}

#[derive(Debug, Clone, Copy)]
struct NormalizedHttpOrder {
    side: ProtoSide,
    outcome: ProtoOutcome,
    price_pts: i32,
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
    fills: Vec<OrderFillResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct OrderFillResponse {
    fill_id: String,
    market_id: String,
    seq_no: u64,
    price_pts: i32,
    quantity: i64,
    taker_order_id: String,
    maker_order_id: String,
    taker_user_id: u64,
    maker_user_id: u64,
    taker_side: &'static str,
    maker_side: &'static str,
    outcome: &'static str,
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
        .layer(middleware::from_fn(record_http_metrics))
        .with_state(ServiceState { service })
}

/// Runs all matching-engine transport surfaces.
///
/// # Errors
///
/// Returns an error when either HTTP or gRPC serving fails.
#[tracing::instrument(fields(service.name = crate::SERVICE_NAME), skip_all)]
pub async fn run() -> anyhow::Result<()> {
    let service = engine_service_from_env()?;
    let ((), ()) = tokio::try_join!(
        run_http_with_service(service.clone()),
        run_grpc_with_service(service)
    )?;
    Ok(())
}

/// Runs the matching-engine HTTP server with SPEC-003 read facade routes.
///
/// # Errors
///
/// Returns an error when configuration loading, socket binding, or serving fails.
#[tracing::instrument(fields(service.name = crate::SERVICE_NAME), skip_all)]
pub async fn run_http() -> anyhow::Result<()> {
    run_http_with_service(engine_service_from_env()?).await
}

/// Runs the matching-engine HTTP server with an existing service instance.
///
/// # Errors
///
/// Returns an error when configuration loading, socket binding, or serving fails.
#[tracing::instrument(fields(service.name = crate::SERVICE_NAME), skip_all)]
pub async fn run_http_with_service(service: InMemoryEngineService) -> anyhow::Result<()> {
    let config = sdk::config::ServiceConfig::from_env(crate::SERVICE_NAME, crate::DEFAULT_PORT)?;
    tracing::info!(?config, "matching engine config loaded");

    let app = sdk::service_stub::router_with_readiness(&config, readiness_state_from_env())
        .merge(router(std::iter::empty()))
        .merge(service_router(service));
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
#[tracing::instrument(fields(service.name = crate::SERVICE_NAME), skip_all)]
pub async fn run_grpc() -> anyhow::Result<()> {
    run_grpc_with_service(engine_service_from_env()?).await
}

/// Runs the matching-engine gRPC server with an existing service instance.
///
/// # Errors
///
/// Returns an error when socket binding or serving fails.
#[tracing::instrument(fields(service.name = crate::SERVICE_NAME), skip_all)]
pub async fn run_grpc_with_service(service: InMemoryEngineService) -> anyhow::Result<()> {
    let bind_addr = grpc_bind_addr_from_env();
    tracing::info!(addr = %bind_addr, "matching engine grpc listening");
    run_grpc_on_addr_with_service(bind_addr, service, shutdown_signal()).await
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
    run_grpc_on_addr_with_service(bind_addr, InMemoryEngineService::default(), shutdown).await
}

/// Runs the matching-engine gRPC server on a concrete address with an existing service instance.
///
/// # Errors
///
/// Returns an error when tonic fails to serve the gRPC endpoint.
pub async fn run_grpc_on_addr_with_service(
    bind_addr: SocketAddr,
    service: InMemoryEngineService,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    Server::builder()
        .add_service(EngineServiceServer::new(service))
        .serve_with_shutdown(bind_addr, shutdown)
        .await?;

    Ok(())
}

async fn record_http_metrics(request: AxumRequest<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let route = request.extensions().get::<MatchedPath>().map_or_else(
        || request.uri().path().to_string(),
        |path| path.as_str().to_string(),
    );
    let timer = crate::metrics::Timer::start();
    let response = next.run(request).await;
    crate::metrics::observe_http_request(
        method.as_str(),
        &route,
        response.status().as_u16(),
        timer.elapsed(),
    );
    response
}

fn engine_service_from_env() -> anyhow::Result<InMemoryEngineService> {
    match kafka_brokers_from_env() {
        Some(brokers) => engine_service_with_kafka_projection(&brokers),
        None => Ok(InMemoryEngineService::default()),
    }
}

fn readiness_state_from_env() -> ReadinessState {
    let Some(brokers) = kafka_brokers_from_env() else {
        return ReadinessState::ready();
    };

    ReadinessState::from_dependencies(
        brokers
            .split(',')
            .map(str::trim)
            .filter(|broker| !broker.is_empty())
            .map(|broker| ReadinessDependency::Tcp {
                name: format!("kafka:{broker}"),
                addr: broker.to_string(),
                timeout: Duration::from_millis(500),
            })
            .collect(),
    )
}

fn kafka_brokers_from_env() -> Option<String> {
    env::var("MATCHING_ENGINE_KAFKA_BROKERS")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(feature = "kafka")]
fn engine_service_with_kafka_projection(brokers: &str) -> anyhow::Result<InMemoryEngineService> {
    let sink = KafkaProjectionSink::from_brokers(brokers)?;
    tracing::info!(brokers, "matching engine Kafka projection sink configured");
    Ok(InMemoryEngineService::with_projection_sink(sink))
}

#[cfg(not(feature = "kafka"))]
fn engine_service_with_kafka_projection(_brokers: &str) -> anyhow::Result<InMemoryEngineService> {
    anyhow::bail!("MATCHING_ENGINE_KAFKA_BROKERS requires matching-engine kafka feature")
}

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME, proph.market_id = %market_id), skip(state))]
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

#[tracing::instrument(fields(service.name = crate::SERVICE_NAME, proph.market_id = %market_id), skip(state, headers, payload))]
async fn place_order_http(
    State(state): State<ServiceState>,
    Path(market_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<PlaceOrderHttpRequest>,
) -> Result<Json<OrderActionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let normalized = normalize_http_order(&payload)?;
    let user_id = user_id_from_headers(&headers)?;
    let response = state
        .service
        .place_order(Request::new(PlaceOrderRequest {
            audit: None,
            user_id: Some(UserId { value: user_id }),
            market_id: Some(MarketId {
                value: market_id.clone(),
            }),
            side: normalized.side.into(),
            outcome: normalized.outcome.into(),
            r#type: map_http_order_type(&payload.order_type)?.into(),
            price_pts: normalized.price_pts,
            quantity: payload.quantity,
            tif: map_http_time_in_force(&payload.time_in_force)?.into(),
            post_only: payload.order_type.eq_ignore_ascii_case("POST_ONLY"),
            client_order_id: headers
                .get("x-client-order-id")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_string(),
            fee_curve_bps: payload.fee_curve_bps.unwrap_or_default(),
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
    let fills = response
        .fills
        .iter()
        .map(|fill| http_trade_fill(fill, &payload, &market_id))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(OrderActionResponse {
        order_id,
        status: http_order_status(order.status),
        filled_qty: response.filled_qty,
        avg_price_pts: denormalize_avg_price_pts(
            &payload.outcome,
            response.filled_qty,
            response.avg_price_pts,
        ),
        remaining_qty: order.qty_remaining,
        fee_pts: response.fee.map_or(0, |fee| fee.amount),
        fills,
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
        fills: Vec::new(),
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
            if let Some(message) = book_stream_message(&market_id, &projected)
                && socket.send(Message::Text(message)).await.is_err()
            {
                return;
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
            if let Some(message) = trade_stream_message(&market_id, &projected)
                && socket.send(Message::Text(message)).await.is_err()
            {
                return;
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
            if let Some(message) = order_update_stream_message(user_id, &projected)
                && socket.send(Message::Text(message)).await.is_err()
            {
                return;
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

fn normalize_http_order(
    payload: &PlaceOrderHttpRequest,
) -> Result<NormalizedHttpOrder, (StatusCode, Json<ErrorResponse>)> {
    let side = map_http_side(&payload.side)?;
    match map_http_outcome(&payload.outcome)? {
        ProtoOutcome::Yes => Ok(NormalizedHttpOrder {
            side,
            outcome: ProtoOutcome::Yes,
            price_pts: payload.price_pts,
        }),
        ProtoOutcome::No => Ok(NormalizedHttpOrder {
            side: opposite_side(side),
            outcome: ProtoOutcome::Yes,
            price_pts: 100 - payload.price_pts,
        }),
        ProtoOutcome::Unspecified | ProtoOutcome::Void => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "outcome must be YES or NO".to_string(),
            }),
        )),
    }
}

fn map_http_outcome(outcome: &str) -> Result<ProtoOutcome, (StatusCode, Json<ErrorResponse>)> {
    if outcome.eq_ignore_ascii_case("YES") {
        Ok(ProtoOutcome::Yes)
    } else if outcome.eq_ignore_ascii_case("NO") {
        Ok(ProtoOutcome::No)
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "outcome must be YES or NO".to_string(),
            }),
        ))
    }
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

fn opposite_side(side: ProtoSide) -> ProtoSide {
    match side {
        ProtoSide::Bid => ProtoSide::Ask,
        ProtoSide::Ask => ProtoSide::Bid,
        ProtoSide::Unspecified => ProtoSide::Unspecified,
    }
}

fn denormalize_avg_price_pts(outcome: &str, filled_qty: i64, avg_price_pts: i32) -> i32 {
    if filled_qty <= 0 || avg_price_pts == 0 {
        return avg_price_pts;
    }

    denormalize_price_pts(outcome, avg_price_pts)
}

fn denormalize_price_pts(outcome: &str, price_pts: i32) -> i32 {
    if outcome.eq_ignore_ascii_case("NO") {
        100 - price_pts
    } else {
        price_pts
    }
}

fn http_trade_fill(
    fill: &ProtoTrade,
    request: &PlaceOrderHttpRequest,
    market_id: &str,
) -> Result<OrderFillResponse, (StatusCode, Json<ErrorResponse>)> {
    let taker_order_id = fill
        .taker_order
        .as_ref()
        .map(|order_id| http_order_id(&order_id.value))
        .transpose()?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "trade fill missing taker order id".to_string(),
                }),
            )
        })?;
    let maker_order_id = fill
        .maker_order
        .as_ref()
        .map(|order_id| http_order_id(&order_id.value))
        .transpose()?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "trade fill missing maker order id".to_string(),
                }),
            )
        })?;
    let taker_user_id = fill.taker_user.as_ref().map_or(0, |user| user.value);
    let maker_user_id = fill.maker_user.as_ref().map_or(0, |user| user.value);
    if taker_user_id == 0 || maker_user_id == 0 {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "trade fill missing user id".to_string(),
            }),
        ));
    }

    let taker_side = http_side_label(&request.side);
    Ok(OrderFillResponse {
        fill_id: format!(
            "{}:trade:{}",
            market_id,
            fill.id.as_ref().map_or(fill.seq_no, |id| id.value)
        ),
        market_id: market_id.to_string(),
        seq_no: fill.seq_no,
        price_pts: denormalize_price_pts(&request.outcome, fill.price_pts),
        quantity: fill.qty,
        taker_order_id,
        maker_order_id,
        taker_user_id,
        maker_user_id,
        taker_side,
        maker_side: opposite_http_side_label(taker_side),
        outcome: http_outcome_label(&request.outcome),
    })
}

fn http_side_label(side: &str) -> &'static str {
    if side.eq_ignore_ascii_case("sell") {
        "sell"
    } else {
        "buy"
    }
}

fn opposite_http_side_label(side: &str) -> &'static str {
    if side.eq_ignore_ascii_case("buy") {
        "sell"
    } else {
        "buy"
    }
}

fn http_outcome_label(outcome: &str) -> &'static str {
    if outcome.eq_ignore_ascii_case("NO") {
        "NO"
    } else {
        "YES"
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
