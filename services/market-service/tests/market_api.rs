#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use market_service::api::{Market, router};
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn get_market_returns_market_by_id() {
    let app = router([sample_market("mkt-election", "Election market", "open")]);
    let response = app
        .oneshot(empty_get("/internal/markets/mkt-election"))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["id"], json!("mkt-election"));
    assert_eq!(body["title"], json!("Election market"));
}

#[tokio::test]
async fn list_markets_filters_by_status() {
    let app = router([
        sample_market("open-1", "Open market", "open"),
        sample_market("settled-1", "Settled market", "settled"),
    ]);
    let response = app
        .oneshot(empty_get("/internal/markets?status=open"))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["markets"].as_array().map(Vec::len), Some(1));
    assert_eq!(body["markets"][0]["id"], json!("open-1"));
}

#[tokio::test]
async fn get_market_returns_not_found_for_unknown_market() {
    let app = router([]);
    let response = app
        .oneshot(empty_get("/internal/markets/missing"))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

fn sample_market(id: &str, title: &str, status: &str) -> Market {
    Market {
        id: id.to_owned(),
        title: title.to_owned(),
        brief: "Brief".to_owned(),
        category: "finance".to_owned(),
        status: status.to_owned(),
        yes_price_cents: 55,
        yes_bid_cents: 54,
        yes_ask_cents: 56,
        volume_24h_points: 1_000,
        volume_total_points: 10_000,
        opened_at: "2026-05-01T00:00:00+08:00".to_owned(),
        close_at: "2026-06-01T00:00:00+08:00".to_owned(),
        oracle_source: "Official source".to_owned(),
        resolved_outcome: None,
        resolved_at: None,
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
