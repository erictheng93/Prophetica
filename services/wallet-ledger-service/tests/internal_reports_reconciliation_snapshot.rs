#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use wallet_ledger_service::internal_reports::internal_reports_router;

#[tokio::test]
async fn reconciliation_snapshot_returns_rows_and_source_snapshot() {
    let app = internal_reports_router();
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/internal/reports/reconciliation-snapshot?cutoff=2026-05-10T07:59:59%2B08:00")
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
    assert_eq!(body["rows"][0]["source_version"], "ledger.v2026.05.10");
    assert_eq!(body["rows"][0]["reconciliation_delta"], 0);
    assert_eq!(
        body["source_snapshot"]["source_service"],
        "wallet-ledger-service"
    );
    assert_eq!(body["source_snapshot"]["correlation_id"], "corr-001");
}

#[tokio::test]
async fn reconciliation_snapshot_rejects_missing_cutoff() {
    let app = internal_reports_router();
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/internal/reports/reconciliation-snapshot")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("handled");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
