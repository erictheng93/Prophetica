#![allow(clippy::expect_used)]

use api_gateway::drill_logs::{
    DrillLog, DrillLogDocument, DrillLogQuery, DrillLogStore, DrillLogType, InMemoryDrillLogStore,
    internal_audit_router,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

#[test]
fn drill_log_document_requires_plan_execution_and_improvement_sections() {
    let document = DrillLogDocument {
        pre_plan: "事前計畫: delete one engine pod".to_owned(),
        execution_record: "實際執行: pod recovered in 20s".to_owned(),
        improvement_actions_24h: "事後 24h 改進建議: tune readiness probe".to_owned(),
    };

    assert!(document.is_complete_for_regulator());
}

#[test]
fn incomplete_drill_log_document_is_rejected() {
    let document = DrillLogDocument {
        pre_plan: "事前計畫: delete one engine pod".to_owned(),
        execution_record: String::new(),
        improvement_actions_24h: "事後 24h 改進建議: tune readiness probe".to_owned(),
    };

    assert!(!document.is_complete_for_regulator());
}

#[tokio::test]
async fn audit_endpoint_returns_drill_logs_filtered_by_time_range() {
    let store = InMemoryDrillLogStore::default();
    store
        .append(DrillLog {
            drill_id: "drill-weekly-1".to_owned(),
            drill_type: DrillLogType::EnginePodKill,
            scheduled_at_epoch_seconds: 100,
            executed_at_epoch_seconds: 120,
            rto_ms: 20_000,
            rpo_lost_events: 0,
            deviation_notes: String::new(),
            improvement_actions: "事後 24h 改進建議: none".to_owned(),
        })
        .expect("drill log should append");
    let app = internal_audit_router(store);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/internal/audit/drill-logs?from=90&to=130")
                .body(Body::empty())
                .unwrap_or_else(|error| panic!("request should build: {error}")),
        )
        .await
        .expect("audit request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(
        body["logs"],
        json!([{
            "drill_id": "drill-weekly-1",
            "drill_type": "engine_pod_kill",
            "scheduled_at_epoch_seconds": 100,
            "executed_at_epoch_seconds": 120,
            "rto_ms": 20000,
            "rpo_lost_events": 0,
            "deviation_notes": "",
            "improvement_actions": "事後 24h 改進建議: none"
        }])
    );
}

#[test]
fn store_filters_drill_logs_by_query_window() {
    let store = InMemoryDrillLogStore::default();
    store
        .append(sample_log("before", 80))
        .expect("before log should append");
    store
        .append(sample_log("inside", 120))
        .expect("inside log should append");

    let logs = store.query(DrillLogQuery { from: 100, to: 130 });

    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].drill_id, "inside");
}

fn sample_log(drill_id: &str, executed_at_epoch_seconds: i64) -> DrillLog {
    DrillLog {
        drill_id: drill_id.to_owned(),
        drill_type: DrillLogType::EnginePodKill,
        scheduled_at_epoch_seconds: executed_at_epoch_seconds - 10,
        executed_at_epoch_seconds,
        rto_ms: 20_000,
        rpo_lost_events: 0,
        deviation_notes: String::new(),
        improvement_actions: "事後 24h 改進建議: none".to_owned(),
    }
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
