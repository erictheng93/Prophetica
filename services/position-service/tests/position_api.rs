#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use position_service::api::{Position, router};
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn list_positions_returns_positions_for_user() {
    let app = router([
        sample_position("pos-1", "user_01", "mkt-election", None),
        sample_position("pos-2", "user_02", "mkt-other", None),
    ]);
    let response = app
        .oneshot(empty_get("/internal/users/user_01/positions"))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["positions"].as_array().map(Vec::len), Some(1));
    assert_eq!(body["positions"][0]["id"], json!("pos-1"));
    assert_eq!(body["positions"][0]["market_id"], json!("mkt-election"));
}

#[tokio::test]
async fn list_positions_filters_active_and_settled() {
    let app = router([
        sample_position("active-1", "user_01", "mkt-active", None),
        sample_position(
            "settled-1",
            "user_01",
            "mkt-settled",
            Some("2026-05-03T00:00:00+08:00"),
        ),
    ]);

    let active_response = app
        .clone()
        .oneshot(empty_get("/internal/users/user_01/positions?status=active"))
        .await
        .expect("active request should be handled");
    let settled_response = app
        .oneshot(empty_get(
            "/internal/users/user_01/positions?status=settled",
        ))
        .await
        .expect("settled request should be handled");

    assert_eq!(active_response.status(), StatusCode::OK);
    assert_eq!(settled_response.status(), StatusCode::OK);
    assert_eq!(
        response_json(active_response).await["positions"][0]["id"],
        json!("active-1")
    );
    assert_eq!(
        response_json(settled_response).await["positions"][0]["id"],
        json!("settled-1")
    );
}

fn sample_position(id: &str, user_id: &str, market_id: &str, closed_at: Option<&str>) -> Position {
    Position {
        id: id.to_owned(),
        user_id: user_id.to_owned(),
        market_id: market_id.to_owned(),
        outcome: "YES".to_owned(),
        qty: 100,
        avg_price_pts: 42,
        locked_amount: 4_200,
        opened_at: "2026-05-01T00:00:00+08:00".to_owned(),
        closed_at: closed_at.map(str::to_owned),
        settle_txn_id: None,
        realized_pnl_points: None,
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
