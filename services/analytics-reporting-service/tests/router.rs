#![allow(clippy::expect_used)]

use analytics_reporting_service::{router::router, store::InMemoryStore};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn dashboard_reads_do_not_create_jobs() {
    let store = InMemoryStore::with_seed_templates();
    let app = router(store.clone());

    let first_count = store.job_count();
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/dashboard-metrics")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("handled");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(store.job_count(), first_count);
}

#[tokio::test]
async fn creating_report_job_records_source_snapshot_and_audit() {
    let store = InMemoryStore::with_seed_templates();
    let app = router(store.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/report-jobs")
                .header("content-type", "application/json")
                .header("x-actor-id", "U-ADMIN-001")
                .body(Body::from(
                    json!({
                        "templateCode": "ops-kpi-daily",
                        "params": {
                            "startAt": "2026-05-10T00:00:00+08:00",
                            "endAt": "2026-05-10T23:59:59+08:00",
                            "timezone": "Asia/Taipei",
                            "sortBy": "report_date",
                            "sortDirection": "asc",
                            "columns": ["report_date", "approvals_count"]
                        }
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["job"]["templateCode"], "ops-kpi-daily");
    assert_eq!(
        body["job"]["sourceSnapshot"]["sources"][0]["sourceService"],
        "kyc-service"
    );
    assert_eq!(store.job_count(), 1);
    assert_eq!(store.audit_count(), 1);
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("json")
}
