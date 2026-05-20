#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use market_service::api::{Market, router};
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn get_market_returns_market_by_id() {
    let app = router([sample_market("mkt-election", "Election market", "open")]);
    let response = app
        .oneshot(empty_get("/internal/markets/mkt-election"))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["id"], json!("mkt-election"));
    assert_eq!(body["title"], json!("Election market"));
}

#[tokio::test]
async fn list_markets_filters_by_status() {
    let app = router([
        sample_market("open-1", "Open market", "open"),
        sample_market("settled-1", "Settled market", "settled"),
    ]);
    let response = app
        .oneshot(empty_get("/internal/markets?status=open"))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["markets"].as_array().map(Vec::len), Some(1));
    assert_eq!(body["markets"][0]["id"], json!("open-1"));
}

#[tokio::test]
async fn get_market_returns_not_found_for_unknown_market() {
    let app = router([]);
    let response = app
        .oneshot(empty_get("/internal/markets/missing"))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn two_matching_oracle_submissions_move_closed_market_to_pending() {
    let app = router([sample_market("mkt-election", "Election market", "closed")]);

    let first = app
        .clone()
        .oneshot(json_post(
            "/internal/markets/mkt-election/oracle-submissions",
            &json!({
                "submitted_by": "operator_01",
                "outcome_result": "YES",
                "evidence_urls": ["https://example.test/result"],
                "evidence_hash": "sha256:first",
                "notes": "official result",
                "signature": "sig-1"
            }),
        ))
        .await
        .expect("request should be handled");
    assert_eq!(first.status(), StatusCode::CREATED);
    assert_eq!(
        response_json(first).await["ready_for_pending"],
        json!(false)
    );

    let second = app
        .oneshot(json_post(
            "/internal/markets/mkt-election/oracle-submissions",
            &json!({
                "submitted_by": "operator_02",
                "outcome_result": "YES",
                "evidence_urls": ["https://example.test/result"],
                "evidence_hash": "sha256:second",
                "notes": "second operator agrees",
                "signature": "sig-2"
            }),
        ))
        .await
        .expect("request should be handled");

    assert_eq!(second.status(), StatusCode::CREATED);
    let body = response_json(second).await;
    assert_eq!(body["ready_for_pending"], json!(true));
    assert_eq!(body["market_status"], json!("pending"));
    assert_eq!(body["dispute_until"], json!("2026-06-02T00:00:00+08:00"));
}

#[tokio::test]
async fn mismatched_oracle_submissions_move_closed_market_to_disputed() {
    let app = router([sample_market("mkt-election", "Election market", "closed")]);

    let first = app
        .clone()
        .oneshot(json_post(
            "/internal/markets/mkt-election/oracle-submissions",
            &json!({
                "submitted_by": "operator_01",
                "outcome_result": "YES",
                "evidence_urls": ["https://example.test/yes"],
                "evidence_hash": "sha256:first",
                "notes": "source says yes",
                "signature": "sig-1"
            }),
        ))
        .await
        .expect("request should be handled");
    assert_eq!(first.status(), StatusCode::CREATED);

    let second = app
        .oneshot(json_post(
            "/internal/markets/mkt-election/oracle-submissions",
            &json!({
                "submitted_by": "operator_02",
                "outcome_result": "NO",
                "evidence_urls": ["https://example.test/no"],
                "evidence_hash": "sha256:second",
                "notes": "source says no",
                "signature": "sig-2"
            }),
        ))
        .await
        .expect("request should be handled");

    assert_eq!(second.status(), StatusCode::CREATED);
    let body = response_json(second).await;
    assert_eq!(body["ready_for_pending"], json!(false));
    assert_eq!(body["market_status"], json!("disputed"));
}

#[tokio::test]
async fn oracle_submission_requires_evidence_hash_url_and_signature() {
    let app = router([sample_market("mkt-election", "Election market", "closed")]);

    let response = app
        .oneshot(json_post(
            "/internal/markets/mkt-election/oracle-submissions",
            &json!({
                "submitted_by": "operator_01",
                "outcome_result": "YES",
                "evidence_urls": [],
                "evidence_hash": "",
                "notes": "missing evidence",
                "signature": ""
            }),
        ))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await["error"],
        json!("oracle_evidence_required")
    );
}

#[tokio::test]
async fn internal_settlement_writes_require_step_up_mfa_and_allowlisted_ip() {
    let app = router([sample_market("mkt-election", "Election market", "closed")]);

    let response = app
        .oneshot(insecure_json_post(
            "/internal/markets/mkt-election/oracle-submissions",
            &json!({
                "submitted_by": "operator_01",
                "outcome_result": "YES",
                "evidence_urls": ["https://example.test/result"],
                "evidence_hash": "sha256:first",
                "notes": "official result",
                "signature": "sig-1"
            }),
        ))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response_json(response).await["error"],
        json!("settlement_step_up_required")
    );
}

#[tokio::test]
async fn dispute_submission_moves_pending_market_to_disputed() {
    let app = router([pending_market("mkt-election", "Election market")]);

    let response = app
        .oneshot(json_post(
            "/api/v1/markets/mkt-election/disputes",
            &json!({
                "user_id": "user_01",
                "reason": "official source reports NO",
                "evidence_urls": ["https://example.test/no"],
                "claimed_outcome": "NO"
            }),
        ))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response_json(response).await;
    assert_eq!(body["status"], json!("PENDING_REVIEW"));
    assert_eq!(body["market_status"], json!("disputed"));
    assert_eq!(body["deadline"], json!("2026-06-02T00:00:00+08:00"));
}

#[tokio::test]
async fn public_settlements_list_returns_resolved_markets_since_timestamp() {
    let app = router([
        settled_market("mkt-settled", "Settled market", "2026-06-02T00:00:00+08:00"),
        settled_market("mkt-old", "Old market", "2026-05-01T00:00:00+08:00"),
        sample_market("mkt-open", "Open market", "open"),
    ]);

    let response = app
        .oneshot(empty_get(
            "/api/v1/settlements?since=2026-06-01T00:00:00+08:00",
        ))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["settlements"].as_array().map(Vec::len), Some(1));
    assert_eq!(body["settlements"][0]["market_id"], json!("mkt-settled"));
    assert_eq!(body["settlements"][0]["status"], json!("settled"));
}

#[tokio::test]
async fn uphold_dispute_resolution_moves_disputed_market_to_settling_then_settled() {
    let app = router([disputed_market("mkt-election", "Election market")]);

    let resolution = app
        .clone()
        .oneshot(json_post(
            "/internal/markets/mkt-election/dispute-resolutions",
            &json!({
                "decision": "UPHOLD_INITIAL",
                "votes": [
                    {"adjudicator": "adj_01", "vote": "UPHOLD_INITIAL", "reason": "source verified"},
                    {"adjudicator": "adj_02", "vote": "UPHOLD_INITIAL", "reason": "evidence matches"},
                    {"adjudicator": "adj_03", "vote": "OVERTURN", "reason": "minor concern"}
                ],
                "final_signoff_by": "compliance_01",
                "audit_hash": "sha256:votes"
            }),
        ))
        .await
        .expect("request should be handled");

    assert_eq!(resolution.status(), StatusCode::CREATED);
    assert_eq!(
        response_json(resolution).await["market_status"],
        json!("settling")
    );

    let settled = app
        .oneshot(json_post(
            "/internal/markets/mkt-election/settle",
            &json!({"reviewer": "settlement_reviewer_01"}),
        ))
        .await
        .expect("request should be handled");

    assert_eq!(settled.status(), StatusCode::OK);
    let body = response_json(settled).await;
    assert_eq!(body["market_status"], json!("settled"));
    assert_eq!(body["ledger_txn_type"], json!("TXN_TYPE_MARKET_SETTLE"));
}

#[tokio::test]
async fn void_market_resolution_cancels_market_with_void_refund_type() {
    let app = router([disputed_market("mkt-election", "Election market")]);

    let response = app
        .oneshot(json_post(
            "/internal/markets/mkt-election/dispute-resolutions",
            &json!({
                "decision": "VOID_MARKET",
                "votes": [
                    {"adjudicator": "adj_01", "vote": "VOID_MARKET", "reason": "source failed"},
                    {"adjudicator": "adj_02", "vote": "VOID_MARKET", "reason": "event unknowable"},
                    {"adjudicator": "adj_03", "vote": "VOID_MARKET", "reason": "refund users"}
                ],
                "final_signoff_by": "compliance_01",
                "audit_hash": "sha256:void"
            }),
        ))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response_json(response).await;
    assert_eq!(body["market_status"], json!("cancelled"));
    assert_eq!(
        body["ledger_txn_type"],
        json!("TXN_TYPE_MARKET_VOID_REFUND")
    );
}

#[tokio::test]
async fn status_events_record_settlement_state_transitions() {
    let app = router([disputed_market("mkt-election", "Election market")]);

    let _resolution = app
        .clone()
        .oneshot(json_post(
            "/internal/markets/mkt-election/dispute-resolutions",
            &json!({
                "decision": "UPHOLD_INITIAL",
                "votes": [
                    {"adjudicator": "adj_01", "vote": "UPHOLD_INITIAL", "reason": "source verified"},
                    {"adjudicator": "adj_02", "vote": "UPHOLD_INITIAL", "reason": "evidence matches"},
                    {"adjudicator": "adj_03", "vote": "UPHOLD_INITIAL", "reason": "unanimous"}
                ],
                "final_signoff_by": "compliance_01",
                "audit_hash": "sha256:votes"
            }),
        ))
        .await
        .expect("request should be handled");
    let _settled = app
        .clone()
        .oneshot(json_post(
            "/internal/markets/mkt-election/settle",
            &json!({"reviewer": "settlement_reviewer_01"}),
        ))
        .await
        .expect("request should be handled");

    let response = app
        .oneshot(empty_get("/internal/markets/mkt-election/status-events"))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["events"].as_array().map(Vec::len), Some(2));
    assert_eq!(body["events"][0]["old_status"], json!("disputed"));
    assert_eq!(body["events"][0]["new_status"], json!("settling"));
    assert_eq!(body["events"][1]["old_status"], json!("settling"));
    assert_eq!(body["events"][1]["new_status"], json!("settled"));
}

fn sample_market(id: &str, title: &str, status: &str) -> Market {
    Market {
        id: id.to_owned(),
        title: title.to_owned(),
        brief: "Brief".to_owned(),
        category: "finance".to_owned(),
        status: status.to_owned(),
        yes_price_cents: 55,
        yes_bid_cents: 54,
        yes_ask_cents: 56,
        fee_bps: 200,
        volume_24h_points: 1_000,
        volume_total_points: 10_000,
        opened_at: "2026-05-01T00:00:00+08:00".to_owned(),
        close_at: "2026-06-01T00:00:00+08:00".to_owned(),
        oracle_source: "Official source".to_owned(),
        resolved_outcome: None,
        resolved_at: None,
    }
}

fn pending_market(id: &str, title: &str) -> Market {
    Market {
        status: "pending".to_owned(),
        resolved_outcome: Some("YES".to_owned()),
        resolved_at: Some("2026-06-01T00:00:00+08:00".to_owned()),
        ..sample_market(id, title, "pending")
    }
}

fn disputed_market(id: &str, title: &str) -> Market {
    Market {
        status: "disputed".to_owned(),
        resolved_outcome: Some("YES".to_owned()),
        resolved_at: Some("2026-06-01T00:00:00+08:00".to_owned()),
        ..sample_market(id, title, "disputed")
    }
}

fn settled_market(id: &str, title: &str, resolved_at: &str) -> Market {
    Market {
        status: "settled".to_owned(),
        resolved_outcome: Some("YES".to_owned()),
        resolved_at: Some(resolved_at.to_owned()),
        ..sample_market(id, title, "settled")
    }
}

fn empty_get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .expect("request should build")
}

fn json_post(uri: &str, payload: &Value) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-operator-role", "oracle_operator")
        .header("x-step-up-mfa", "fresh")
        .header("x-settlement-ip-allowlisted", "true")
        .body(Body::from(payload.to_string()))
        .expect("request should build")
}

fn insecure_json_post(uri: &str, payload: &Value) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .expect("request should build")
}

async fn response_json(response: axum::response::Response) -> Value {
    let collected = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect");
    serde_json::from_slice(&collected.to_bytes()).expect("response should be json")
}
