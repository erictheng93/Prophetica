#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use user_service::internal_contacts::internal_contacts_router;

#[tokio::test]
async fn lookup_masked_returns_first_char_plus_domain() {
    let app = internal_contacts_router();
    let body = json!({ "ids": ["MEM-001"] }).to_string();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/internal/users/contacts:lookup-masked")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("handled");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let parsed: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(parsed["contacts"][0]["id"], "MEM-001");
    assert_eq!(parsed["contacts"][0]["email_masked"], "m***@example.com");
}

#[tokio::test]
async fn lookup_masked_rejects_empty_ids() {
    let app = internal_contacts_router();
    let body = json!({ "ids": [""] }).to_string();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/internal/users/contacts:lookup-masked")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("handled");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn reveal_returns_501_with_error_code() {
    let app = internal_contacts_router();
    let body = json!({
        "ids": ["MEM-001"],
        "actor_id": "U-ADMIN-001",
        "second_authorization_id": "MFA-1",
        "job_id": "rpt-job-000001"
    })
    .to_string();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/internal/users/contacts:reveal")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .expect("request builds"),
        )
        .await
        .expect("handled");

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let parsed: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(parsed["error"], "ERROR_CODE_REVEAL_NOT_AVAILABLE");
}
