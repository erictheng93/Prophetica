#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use matching_engine::{
    api::{router, service_router},
    order_book::{NewOrder, OrderBook, OrderId, OrderSide, OrderType, Symbol},
    service::InMemoryEngineService,
};
use serde_json::{Value, json};
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
