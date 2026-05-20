#![allow(clippy::expect_used)]

use api_gateway::responsible_gaming::{
    InMemoryCooldownStore, ResponsibleGamingClock, cooldown_router,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const NOW: i64 = 1_800_000_000;

#[tokio::test]
async fn cooldown_status_starts_available_for_authenticated_user() {
    let app = cooldown_router(
        InMemoryCooldownStore::default(),
        ResponsibleGamingClock::fixed(NOW),
    );

    let response = app
        .oneshot(empty_get("/v1/me/responsible-gaming/cooldown"))
        .await
        .expect("cooldown status request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["user_id"], json!("user_42"));
    assert_eq!(body["active"], json!(false));
    assert_eq!(body["duration"], json!(null));
    assert_eq!(body["remaining_seconds"], json!(0));
    assert_eq!(body["available_durations"], json!(["24h", "7d", "30d"]));
}

#[tokio::test]
async fn cooldown_start_records_fixed_duration_and_remaining_seconds() {
    let store = InMemoryCooldownStore::default();
    let app = cooldown_router(store, ResponsibleGamingClock::fixed(NOW));

    let response = app
        .oneshot(json_post(
            "/v1/me/responsible-gaming/cooldown",
            &json!({
                "duration": "7d",
                "consent": true
            }),
        ))
        .await
        .expect("cooldown start request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["user_id"], json!("user_42"));
    assert_eq!(body["active"], json!(true));
    assert_eq!(body["duration"], json!("7d"));
    assert_eq!(body["started_at_epoch_seconds"], json!(NOW));
    assert_eq!(body["ends_at_epoch_seconds"], json!(NOW + 7 * 24 * 60 * 60));
    assert_eq!(body["remaining_seconds"], json!(7 * 24 * 60 * 60));
}

#[tokio::test]
async fn cooldown_start_requires_explicit_consent() {
    let app = cooldown_router(
        InMemoryCooldownStore::default(),
        ResponsibleGamingClock::fixed(NOW),
    );

    let response = app
        .oneshot(json_post(
            "/v1/me/responsible-gaming/cooldown",
            &json!({
                "duration": "24h",
                "consent": false
            }),
        ))
        .await
        .expect("cooldown start request should be handled");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["error"], json!("cooldown_consent_required"));
}

fn empty_get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("x-user-id", "user_42")
        .header("x-transition-caller", "apps-web-dev-bff")
        .body(Body::empty())
        .unwrap_or_else(|error| panic!("request should build: {error}"))
}

fn json_post(uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("x-user-id", "user_42")
        .header("x-transition-caller", "apps-web-dev-bff")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|error| panic!("request should build: {error}"))
}

async fn response_json(response: axum::response::Response) -> Value {
    let collected = response
        .into_body()
        .collect()
        .await
        .unwrap_or_else(|error| panic!("response body should collect: {error}"));
    serde_json::from_slice(&collected.to_bytes())
        .unwrap_or_else(|error| panic!("response should be json: {error}"))
}
