use axum::{
    Json, Router,
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub fn internal_reports_router() -> Router {
    Router::new().route("/v1/internal/reports/aml-alerts", get(aml_alerts))
}

#[derive(Debug, Deserialize)]
struct AlertWindowQuery {
    from: String,
    to: String,
}

#[derive(Debug, Serialize)]
struct ReportResponse {
    rows: Vec<Value>,
    source_snapshot: SourceSnapshot,
}

#[derive(Debug, Serialize)]
struct SourceSnapshot {
    source_service: &'static str,
    source_version: &'static str,
    cutoff_at: String,
    source_generated_at: String,
    correlation_id: String,
}

async fn aml_alerts(
    headers: HeaderMap,
    query: Option<Query<AlertWindowQuery>>,
) -> Result<Json<ReportResponse>, Response> {
    let Some(Query(window)) = query else {
        return Err((StatusCode::BAD_REQUEST, "missing from/to").into_response());
    };
    if window.from.trim().is_empty() || window.to.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "missing from/to").into_response());
    }

    let rows = vec![json!({
        "member_id": "MEM-001",
        "risk_score": 92,
        "risk_reason": "device_velocity"
    })];

    Ok(Json(ReportResponse {
        rows,
        source_snapshot: SourceSnapshot {
            source_service: "risk-antifraud-service",
            source_version: "risk.v2026.05.10",
            cutoff_at: window.to.clone(),
            source_generated_at: window.to,
            correlation_id: request_id(&headers),
        },
    }))
}

fn request_id(headers: &HeaderMap) -> String {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned()
}
