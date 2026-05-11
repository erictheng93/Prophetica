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
    Router::new().route(
        "/v1/internal/reports/reconciliation-snapshot",
        get(reconciliation_snapshot),
    )
}

#[derive(Debug, Deserialize)]
struct ReconciliationQuery {
    cutoff: String,
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

async fn reconciliation_snapshot(
    headers: HeaderMap,
    query: Option<Query<ReconciliationQuery>>,
) -> Result<Json<ReportResponse>, Response> {
    let Some(Query(window)) = query else {
        return Err((StatusCode::BAD_REQUEST, "missing cutoff").into_response());
    };
    if window.cutoff.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "missing cutoff").into_response());
    }

    let rows = vec![json!({
        "cutoff_at": window.cutoff.clone(),
        "source_version": "ledger.v2026.05.10",
        "settlement_total": 502_500,
        "reconciliation_delta": 0
    })];

    Ok(Json(ReportResponse {
        rows,
        source_snapshot: SourceSnapshot {
            source_service: "wallet-ledger-service",
            source_version: "ledger.v2026.05.10",
            cutoff_at: window.cutoff.clone(),
            source_generated_at: window.cutoff,
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
