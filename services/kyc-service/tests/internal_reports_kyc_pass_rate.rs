#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use kyc_service::{
    SecretConfig, api,
    kyc::{InMemoryKycStore, KycStore, KycTier, StartUpgradeDraft},
};
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn kyc_pass_rate_returns_rows_and_source_snapshot() {
    let app = kyc_service::internal_reports::internal_reports_router();
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
    let app = kyc_service::internal_reports::internal_reports_router();
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

#[tokio::test]
async fn pending_member_cases_are_loaded_from_kyc_store() {
    let store = InMemoryKycStore::default();
    store
        .start_upgrade(StartUpgradeDraft {
            user_id: "member_pending_001".to_owned(),
            target_tier: KycTier::Tier2,
            idempotency_key: "pending-member-001".to_owned(),
            declared_monthly_income: Some(120_000),
        })
        .await
        .expect("seed pending kyc record");

    let app = api::router(store, SecretConfig::dev_default());
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/internal/admin/members/pending-cases?q=member_pending&page=1&page_size=5")
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
    assert_eq!(body["source"], "kyc-service:kyc-store");
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["member_id"], "member_pending_001");
    assert_eq!(body["items"][0]["restriction_reason"], "kyc_pending");
    assert_eq!(body["items"][0]["status"], "inactive");
    assert_eq!(body["items"][0]["kyc_status"], "pending");
}
