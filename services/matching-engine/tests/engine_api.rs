#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use matching_engine::{
    api::router,
    order_book::{NewOrder, OrderBook, OrderId, OrderSide, OrderType, Symbol},
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

async fn response_json(response: axum::response::Response) -> Value {
    let collected = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect");
    serde_json::from_slice(&collected.to_bytes()).expect("response should be json")
}
