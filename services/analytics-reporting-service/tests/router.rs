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
                .header("x-operator-role", "admin")
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
                .header("x-operator-role", "admin")
                .body(Body::from(
                    report_job_body("ops-kpi-daily", "Asia/Taipei").to_string(),
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

#[tokio::test]
async fn manager_cannot_spoof_admin_role_to_create_utc_report_job() {
    let store = InMemoryStore::with_seed_templates();
    let app = router(store.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/report-jobs")
                .header("content-type", "application/json")
                .header("x-actor-id", "U-MGR-001")
                .header("x-operator-role", "manager")
                .body(Body::from(
                    json!({
                        "actorId": "U-SPOOFED-ADMIN",
                        "role": "admin",
                        "templateCode": "ops-kpi-daily",
                        "params": {
                            "startAt": "2026-05-10T00:00:00+08:00",
                            "endAt": "2026-05-10T23:59:59+08:00",
                            "timezone": "UTC",
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

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(store.job_count(), 0);
    assert_eq!(store.audit_count(), 0);
}

#[tokio::test]
async fn reporting_routes_reject_missing_operator_role() {
    let store = InMemoryStore::with_seed_templates();
    let app = router(store.clone());
    let job_id = create_seed_job(app.clone()).await;

    let requests = vec![
        Request::builder()
            .method("GET")
            .uri("/v1/report-templates")
            .body(Body::empty())
            .expect("request builds"),
        Request::builder()
            .method("POST")
            .uri("/v1/report-jobs")
            .header("content-type", "application/json")
            .body(Body::from(
                report_job_body("ops-kpi-daily", "Asia/Taipei").to_string(),
            ))
            .expect("request builds"),
        Request::builder()
            .method("GET")
            .uri("/v1/report-jobs")
            .body(Body::empty())
            .expect("request builds"),
        Request::builder()
            .method("GET")
            .uri(format!("/v1/report-jobs/{job_id}"))
            .body(Body::empty())
            .expect("request builds"),
        Request::builder()
            .method("GET")
            .uri(format!("/v1/report-jobs/{job_id}/exports/csv"))
            .body(Body::empty())
            .expect("request builds"),
        Request::builder()
            .method("POST")
            .uri(format!("/v1/report-jobs/{job_id}/export-deletions"))
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "format": "csv",
                    "reason": "retention test"
                })
                .to_string(),
            ))
            .expect("request builds"),
        Request::builder()
            .method("GET")
            .uri("/v1/report-schedules")
            .body(Body::empty())
            .expect("request builds"),
        Request::builder()
            .method("GET")
            .uri("/v1/report-jobs/latest-per-template")
            .body(Body::empty())
            .expect("request builds"),
        Request::builder()
            .method("GET")
            .uri("/v1/dashboard-metrics")
            .body(Body::empty())
            .expect("request builds"),
    ];

    for request in requests {
        let path = request.uri().path().to_owned();
        let response = app.clone().oneshot(request).await.expect("handled");

        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
    }
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

async fn create_seed_job(app: axum::Router) -> String {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/report-jobs")
                .header("content-type", "application/json")
                .header("x-actor-id", "U-ADMIN-001")
                .header("x-operator-role", "admin")
                .body(Body::from(
                    report_job_body("ops-kpi-daily", "Asia/Taipei").to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("handled");
    assert_eq!(response.status(), StatusCode::OK);

    response_json(response).await["job"]["id"]
        .as_str()
        .expect("job id")
        .to_owned()
}

fn report_job_body(template_code: &str, timezone: &str) -> Value {
    json!({
        "templateCode": template_code,
        "params": {
            "startAt": "2026-05-10T00:00:00+08:00",
            "endAt": "2026-05-10T23:59:59+08:00",
            "timezone": timezone,
            "sortBy": "report_date",
            "sortDirection": "asc",
            "columns": ["report_date", "approvals_count"]
        }
    })
}
