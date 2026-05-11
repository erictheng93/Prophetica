#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use risk_antifraud_service::internal_reports::internal_reports_router;
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn aml_alerts_returns_rows_and_source_snapshot() {
    let app = internal_reports_router();
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/internal/reports/aml-alerts?from=2026-05-09T00:00:00Z&to=2026-05-10T23:59:59Z")
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
    assert_eq!(body["rows"][0]["member_id"], "MEM-001");
    assert_eq!(body["rows"][0]["risk_score"], 92);
    assert_eq!(
        body["source_snapshot"]["source_service"],
        "risk-antifraud-service"
    );
    assert_eq!(body["source_snapshot"]["correlation_id"], "corr-001");
}

#[tokio::test]
async fn aml_alerts_rejects_missing_window() {
    let app = internal_reports_router();
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/internal/reports/aml-alerts")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("handled");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
