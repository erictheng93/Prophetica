#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use kyc_service::internal_reports::internal_reports_router;
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn kyc_pass_rate_returns_rows_and_source_snapshot() {
    let app = internal_reports_router();
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/internal/reports/kyc-pass-rate?from=2026-05-09T00:00:00Z&to=2026-05-10T23:59:59Z")
                .header("x-request-id", "corr-001")
                .body(Body::empty())
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
    let body: Value = serde_json::from_slice(&bytes).expect("json");
    assert!(body["rows"].is_array());
    assert_eq!(body["source_snapshot"]["source_service"], "kyc-service");
    assert_eq!(body["source_snapshot"]["correlation_id"], "corr-001");
    assert!(body["source_snapshot"]["cutoff_at"].is_string());
}

#[tokio::test]
async fn kyc_pass_rate_rejects_missing_window() {
    let app = internal_reports_router();
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/internal/reports/kyc-pass-rate")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("handled");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
