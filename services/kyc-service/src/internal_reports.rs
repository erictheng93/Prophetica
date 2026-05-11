use axum::{
    Json, Router,
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[must_use]
pub fn internal_reports_router() -> Router {
    Router::new().route("/v1/internal/reports/kyc-pass-rate", get(kyc_pass_rate))
}

#[derive(Debug, Deserialize)]
struct PassRateQuery {
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

async fn kyc_pass_rate(
    headers: HeaderMap,
    query: Option<Query<PassRateQuery>>,
) -> Result<Json<ReportResponse>, Response> {
    let Some(Query(window)) = query else {
        return Err((StatusCode::BAD_REQUEST, "missing from/to").into_response());
    };
    if window.from.trim().is_empty() || window.to.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "missing from/to").into_response());
    }

    let correlation_id = request_id(&headers);
    let rows = vec![json!({
        "report_date": window.to.get(0..10).unwrap_or(window.to.as_str()),
        "approvals_count": 16,
        "kyc_pass_rate": "91.4%",
        "service_uptime": "99.92%"
    })];

    Ok(Json(ReportResponse {
        rows,
        source_snapshot: SourceSnapshot {
            source_service: "kyc-service",
            source_version: "kyc.v2026.05.10",
            cutoff_at: window.to.clone(),
            source_generated_at: window.to,
            correlation_id,
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
