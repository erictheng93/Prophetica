#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use futures_util::StreamExt;
use http_body_util::BodyExt;
use matching_engine::{
    api::{router, service_router},
    order_book::{NewOrder, OrderBook, OrderId, OrderSide, OrderType, Symbol},
    service::InMemoryEngineService,
};
use proto::proph::v1::{
    MarketId, OrderType as ProtoOrderType, PlaceOrderRequest, Side, TimeInForce, UserId,
    engine_service_server::EngineService,
};
use serde_json::{Value, json};
use tokio::{net::TcpListener, time::Duration};
use tokio_tungstenite::connect_async;
use tower::ServiceExt;

#[tokio::test]
async fn book_snapshot_returns_aggregated_price_levels_for_market() {
    let mut book = OrderBook::new(Symbol::new("mkt-election"));
    book.add_order(order("bid-1", 10, OrderSide::Buy, 54, 3))
        .expect("first bid should rest");
    book.add_order(order("bid-2", 20, OrderSide::Buy, 54, 4))
        .expect("second bid should rest");
    book.add_order(order("ask-1", 30, OrderSide::Sell, 57, 2))
        .expect("ask should rest");

    let app = router([("mkt-election".to_owned(), book)]);
    let response = app
        .oneshot(empty_get(
            "/internal/engine/markets/mkt-election/book?depth=5",
        ))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["market_id"], json!("mkt-election"));
    assert_eq!(body["bids"], json!([{ "price_pts": 54, "qty": 7 }]));
    assert_eq!(body["asks"], json!([{ "price_pts": 57, "qty": 2 }]));
}

#[tokio::test]
async fn book_snapshot_returns_empty_book_for_unknown_market() {
    let app = router([]);
    let response = app
        .oneshot(empty_get("/internal/engine/markets/missing/book"))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["market_id"], json!("missing"));
    assert_eq!(body["bids"], json!([]));
    assert_eq!(body["asks"], json!([]));
}

#[tokio::test]
async fn rest_order_flow_uses_matching_engine_service_state() {
    let app = service_router(InMemoryEngineService::default());

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/v1/markets/mkt-election/orders",
            Some(10),
            &json!({
                "side": "sell",
                "outcome": "YES",
                "type": "LIMIT",
                "price_pts": 55,
                "quantity": 3,
                "time_in_force": "GTC"
            }),
        ))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let placed = response_json(response).await;
    let order_id = placed["order_id"]
        .as_str()
        .expect("response should include order id");
    assert!(order_id.starts_with("ord_"));
    assert_eq!(placed["status"], json!("ACTIVE"));
    assert_eq!(placed["remaining_qty"], json!(3));

    let snapshot = app
        .clone()
        .oneshot(empty_get("/api/v1/markets/mkt-election/book?depth=10"))
        .await
        .expect("request should be handled");
    assert_eq!(snapshot.status(), StatusCode::OK);
    let body = response_json(snapshot).await;
    assert_eq!(body["asks"], json!([{ "price_pts": 55, "qty": 3 }]));

    let rejected = app
        .clone()
        .oneshot(empty_delete(
            &format!("/api/v1/orders/{order_id}"),
            Some(20),
        ))
        .await
        .expect("request should be handled");
    assert_eq!(rejected.status(), StatusCode::FORBIDDEN);

    let cancelled = app
        .oneshot(empty_delete(
            &format!("/api/v1/orders/{order_id}"),
            Some(10),
        ))
        .await
        .expect("request should be handled");
    assert_eq!(cancelled.status(), StatusCode::OK);
    let body = response_json(cancelled).await;
    assert_eq!(body["status"], json!("CANCELLED"));
    assert_eq!(body["remaining_qty"], json!(0));
}

#[tokio::test]
async fn book_websocket_route_is_registered() {
    let app = service_router(InMemoryEngineService::default());

    let response = app
        .oneshot(empty_get("/ws/markets/mkt-election/book"))
        .await
        .expect("request should be handled");

    assert_ne!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn book_websocket_sends_initial_snapshot() {
    let service = InMemoryEngineService::default();
    service
        .place_order(tonic::Request::new(PlaceOrderRequest {
            audit: None,
            user_id: Some(UserId { value: 10 }),
            market_id: Some(MarketId {
                value: "mkt-election".to_string(),
            }),
            side: Side::Ask.into(),
            outcome: 1,
            r#type: ProtoOrderType::Limit.into(),
            price_pts: 55,
            quantity: 3,
            tif: TimeInForce::Gtc.into(),
            post_only: false,
            client_order_id: "maker".to_string(),
        }))
        .await
        .expect("resting order should be accepted");

    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("listener should bind");
    let addr = listener.local_addr().expect("listener should have addr");
    let app = service_router(service);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server should run");
    });

    let (mut socket, _response) =
        connect_async(format!("ws://{addr}/ws/markets/mkt-election/book"))
            .await
            .expect("websocket should connect");
    let message = tokio::time::timeout(Duration::from_secs(1), socket.next())
        .await
        .expect("snapshot should be sent on connect")
        .expect("socket should stay open")
        .expect("message should be valid");
    let body: Value =
        serde_json::from_str(message.to_text().expect("snapshot message should be text"))
            .expect("snapshot should be json");

    assert_eq!(
        body,
        json!({
            "type": "snapshot",
            "market_id": "mkt-election",
            "seq": 0,
            "bids": [],
            "asks": [{ "price_pts": 55, "qty": 3 }]
        })
    );

    server.abort();
}

#[tokio::test]
async fn trade_websocket_streams_market_trades() {
    let service = InMemoryEngineService::default();
    service
        .place_order(tonic::Request::new(place_order_proto(
            10,
            Side::Ask,
            55,
            3,
            "maker",
        )))
        .await
        .expect("resting order should be accepted");

    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("listener should bind");
    let addr = listener.local_addr().expect("listener should have addr");
    let app = service_router(service.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server should run");
    });

    let (mut socket, _response) =
        connect_async(format!("ws://{addr}/ws/markets/mkt-election/trades"))
            .await
            .expect("websocket should connect");

    service
        .place_order(tonic::Request::new(place_order_proto(
            20,
            Side::Bid,
            55,
            2,
            "taker",
        )))
        .await
        .expect("crossing order should trade");

    let message = tokio::time::timeout(Duration::from_secs(1), socket.next())
        .await
        .expect("trade should be streamed")
        .expect("socket should stay open")
        .expect("message should be valid");
    let body: Value =
        serde_json::from_str(message.to_text().expect("trade message should be text"))
            .expect("trade should be json");

    assert_eq!(body["type"], json!("trade"));
    assert_eq!(body["market_id"], json!("mkt-election"));
    assert_eq!(body["price_pts"], json!(55));
    assert_eq!(body["qty"], json!(2));

    server.abort();
}

#[tokio::test]
async fn user_order_websocket_streams_order_updates() {
    let service = InMemoryEngineService::default();
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("listener should bind");
    let addr = listener.local_addr().expect("listener should have addr");
    let app = service_router(service.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server should run");
    });

    let (mut socket, _response) = connect_async(format!("ws://{addr}/ws/users/20/orders"))
        .await
        .expect("websocket should connect");

    service
        .place_order(tonic::Request::new(place_order_proto(
            20,
            Side::Ask,
            55,
            3,
            "maker",
        )))
        .await
        .expect("order should rest");

    let message = tokio::time::timeout(Duration::from_secs(1), socket.next())
        .await
        .expect("order update should be streamed")
        .expect("socket should stay open")
        .expect("message should be valid");
    let body: Value = serde_json::from_str(message.to_text().expect("order update should be text"))
        .expect("order update should be json");

    assert_eq!(body["type"], json!("order_update"));
    assert_eq!(body["user_id"], json!(20));
    assert_eq!(body["market_id"], json!("mkt-election"));
    assert_eq!(body["status"], json!("Active"));
    assert_eq!(body["remaining_qty"], json!(3));

    server.abort();
}

fn order(order_id: &str, user_id: u64, side: OrderSide, price: u64, quantity: u64) -> NewOrder {
    NewOrder {
        order_id: OrderId::new(order_id),
        user_id,
        symbol: Symbol::new("mkt-election"),
        side,
        order_type: OrderType::Limit,
        price,
        quantity,
    }
}

fn place_order_proto(
    user_id: u64,
    side: Side,
    price_pts: i32,
    quantity: i64,
    client_order_id: &str,
) -> PlaceOrderRequest {
    PlaceOrderRequest {
        audit: None,
        user_id: Some(UserId { value: user_id }),
        market_id: Some(MarketId {
            value: "mkt-election".to_string(),
        }),
        side: side.into(),
        outcome: 1,
        r#type: ProtoOrderType::Limit.into(),
        price_pts,
        quantity,
        tif: TimeInForce::Gtc.into(),
        post_only: false,
        client_order_id: client_order_id.to_string(),
    }
}

fn empty_get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .expect("request should build")
}

fn empty_delete(uri: &str, user_id: Option<u64>) -> Request<Body> {
    let mut builder = Request::builder().method("DELETE").uri(uri);
    if let Some(user_id) = user_id {
        builder = builder.header("x-user-id", user_id.to_string());
    }
    builder.body(Body::empty()).expect("request should build")
}

fn json_request(method: &str, uri: &str, user_id: Option<u64>, body: &Value) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(user_id) = user_id {
        builder = builder.header("x-user-id", user_id.to_string());
    }

    builder
        .body(Body::from(body.to_string()))
        .expect("request should build")
}

async fn response_json(response: axum::response::Response) -> Value {
    let collected = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect");
    serde_json::from_slice(&collected.to_bytes()).expect("response should be json")
}
