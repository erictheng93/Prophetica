#![allow(clippy::expect_used)]

use api_gateway::admin_ops::{HttpAdminOpsStore, InMemoryAdminOpsStore, internal_admin_router};
use axum::{
    Json, Router,
    body::Body,
    http::{Request, StatusCode},
    routing::{delete, get, patch, post},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tower::ServiceExt;

#[tokio::test]
async fn summary_returns_aggregate_shape() {
    let app = internal_admin_router(InMemoryAdminOpsStore::default());

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/internal/admin/members/user_123/summary")
                .body(Body::empty())
                .unwrap_or_else(|error| panic!("request should build: {error}")),
        )
        .await
        .expect("summary request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["member"]["user_id"], json!("user_123"));
    assert_eq!(body["member"]["display_name"], json!("Member user_123"));
    assert_eq!(body["wallet"]["balance"]["available_minor_units"], json!(0));
    assert_eq!(
        body["wallet"]["entries_summary"]["recent_entries_count"],
        json!(0)
    );
    assert_eq!(body["kyc_risk"]["kyc_status"], json!("not_connected"));
    assert_eq!(body["kyc_risk"]["risk_level"], json!("unknown"));
    assert_eq!(body["affiliate"]["referral_code"], json!(null));
    assert_eq!(body["open_cases"], json!([]));
    assert_eq!(
        body["audit"]["correlation_id"],
        json!("admin-summary-user_123")
    );
    assert_eq!(body["audit"]["source"], json!("api-gateway:admin-ops"));
}

#[tokio::test]
async fn pending_member_cases_filter_and_paginate() {
    let app = internal_admin_router(InMemoryAdminOpsStore::default());

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/internal/admin/members/pending-cases?status=restricted&page=1&page_size=1")
                .body(Body::empty())
                .unwrap_or_else(|error| panic!("request should build: {error}")),
        )
        .await
        .expect("pending cases request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["page"], json!(1));
    assert_eq!(body["page_size"], json!(1));
    assert_eq!(body["total"], json!(1));
    assert_eq!(body["items"][0]["member_id"], json!("MEM-1042"));
    assert_eq!(
        body["items"][0]["restriction_reason"],
        json!("withdrawals_locked")
    );
    assert_eq!(
        body["items"][0]["contact"]["email_masked"],
        json!("c***@example.net")
    );
    assert_eq!(body["items"][0]["status"], json!("restricted"));
}

#[tokio::test]
async fn pending_member_cases_proxy_to_kyc_service() {
    let upstream = Router::new()
        .route(
            "/v1/internal/admin/members/pending-cases",
            get(|| async {
                Json(json!({
                    "items": [{
                        "member_id": "member_from_kyc",
                        "display_name": "Member member_from_kyc",
                        "restriction_reason": "kyc_pending",
                        "contact": {
                            "email_masked": "not_available",
                            "phone_masked": "not_available"
                        },
                        "status": "inactive",
                        "kyc_status": "pending",
                        "affiliate_id": null
                    }],
                    "page": 1,
                    "page_size": 10,
                    "total": 1,
                    "source": "kyc-service:kyc-store"
                }))
            }),
        )
        .route(
            "/v1/internal/users/contacts:lookup-masked",
            post(|| async {
                Json(json!({
                    "contacts": [{
                        "id": "member_from_kyc",
                        "email_masked": "r***@example.com",
                        "phone_masked": "0912***999"
                    }]
                }))
            }),
        );
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream");
    let upstream_url = format!("http://{}", listener.local_addr().expect("local addr"));
    tokio::spawn(async move {
        axum::serve(listener, upstream)
            .await
            .expect("serve upstream");
    });

    let store = HttpAdminOpsStore::new(upstream_url.clone(), upstream_url).expect("store config");
    let app = internal_admin_router(store);
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/internal/admin/members/pending-cases?q=member_from_kyc")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["source"], "kyc-service:kyc-store");
    assert_eq!(body["items"][0]["member_id"], "member_from_kyc");
    assert_eq!(
        body["items"][0]["contact"]["email_masked"],
        "r***@example.com"
    );
}

#[tokio::test]
async fn member_management_proxy_to_user_service() {
    let upstream = Router::new()
        .route(
            "/v1/internal/admin/members",
            get(|| async {
                Json(json!({
                    "members": [{
                        "member": { "user_id": "MEM-900", "display_name": "Real Member" },
                        "source": { "affiliate_id": "AFF-001" }
                    }],
                    "source": "user-service:member-store"
                }))
            }),
        )
        .route(
            "/v1/internal/admin/members",
            post(|Json(body): Json<Value>| async move { Json(body) }),
        )
        .route(
            "/v1/internal/admin/members/MEM-900/summary",
            get(|| async {
                Json(json!({
                    "member": { "user_id": "MEM-900", "display_name": "Real Member" },
                    "source": { "affiliate_id": "AFF-001" }
                }))
            }),
        )
        .route(
            "/v1/internal/admin/members/MEM-900",
            patch(|Json(body): Json<Value>| async move { Json(body) }),
        )
        .route(
            "/v1/internal/admin/members/MEM-900",
            delete(|| async { Json(json!({ "user_id": "MEM-900" })) }),
        );
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream");
    let upstream_url = format!("http://{}", listener.local_addr().expect("local addr"));
    tokio::spawn(async move {
        axum::serve(listener, upstream)
            .await
            .expect("serve upstream");
    });

    let store = HttpAdminOpsStore::new(upstream_url.clone(), upstream_url).expect("store config");
    let app = internal_admin_router(store);

    let list = app
        .clone()
        .oneshot(empty_get(
            "/api/internal/admin/members?affiliate_id=AFF-001",
        ))
        .await
        .expect("handled");
    let summary = app
        .clone()
        .oneshot(empty_get("/api/internal/admin/members/MEM-900/summary"))
        .await
        .expect("handled");
    let created = app
        .clone()
        .oneshot(json_post(
            "/api/internal/admin/members",
            &json!({ "user_id": "MEM-901" }),
        ))
        .await
        .expect("handled");
    let updated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/internal/admin/members/MEM-900")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "display_name": "Updated" }).to_string()))
                .expect("request builds"),
        )
        .await
        .expect("handled");
    let deleted = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/internal/admin/members/MEM-900")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("handled");

    assert_eq!(
        response_json(list).await["source"],
        "user-service:member-store"
    );
    assert_eq!(response_json(summary).await["member"]["user_id"], "MEM-900");
    assert_eq!(response_json(created).await["user_id"], "MEM-901");
    assert_eq!(response_json(updated).await["display_name"], "Updated");
    assert_eq!(response_json(deleted).await["user_id"], "MEM-900");
}

#[tokio::test]
async fn compensation_rejects_missing_or_invalid_amount() {
    let app = internal_admin_router(InMemoryAdminOpsStore::default());

    let missing = app
        .clone()
        .oneshot(json_post(
            "/api/internal/admin/members/user_123/compensation-proposals",
            &json!({
                "ticket_id": "TICKET-1",
                "reason": "manual adjustment review",
                "approvals": [
                    { "approver_user_id": "ops-1", "role": "ops_manager" },
                    { "approver_user_id": "compliance-1", "role": "compliance" }
                ]
            }),
        ))
        .await
        .expect("missing amount request should be handled");

    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(missing).await["error"],
        json!("ERROR_CODE_INVALID_COMPENSATION_PROPOSAL")
    );

    let invalid = app
        .oneshot(json_post(
            "/api/internal/admin/members/user_123/compensation-proposals",
            &json!({
                "ticket_id": "TICKET-1",
                "reason": "manual adjustment review",
                "amount_minor_units": 0,
                "approvals": [
                    { "approver_user_id": "ops-1", "role": "ops_manager" },
                    { "approver_user_id": "compliance-1", "role": "compliance" }
                ]
            }),
        ))
        .await
        .expect("invalid amount request should be handled");

    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(invalid).await["error"],
        json!("ERROR_CODE_INVALID_COMPENSATION_PROPOSAL")
    );
}

#[tokio::test]
async fn compensation_rejects_same_approver_twice() {
    let app = internal_admin_router(InMemoryAdminOpsStore::default());

    let response = app
        .oneshot(json_post(
            "/api/internal/admin/members/user_123/compensation-proposals",
            &json!({
                "ticket_id": "TICKET-2",
                "reason": "service incident credit",
                "amount_minor_units": 5000,
                "approvals": [
                    { "approver_user_id": "ops-1", "role": "ops_manager" },
                    { "approver_user_id": "ops-1", "role": "compliance" }
                ]
            }),
        ))
        .await
        .expect("duplicate approver request should be handled");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await["error"],
        json!("ERROR_CODE_INVALID_COMPENSATION_PROPOSAL")
    );
}

#[tokio::test]
async fn compensation_accepts_two_distinct_approvals_and_records_audit_metadata() {
    let store = InMemoryAdminOpsStore::default();
    let app = internal_admin_router(store.clone());

    let response = app
        .oneshot(json_post(
            "/api/internal/admin/members/user_123/compensation-proposals",
            &json!({
                "ticket_id": "TICKET-3",
                "reason": "service incident credit",
                "amount_minor_units": 5000,
                "approvals": [
                    { "approver_user_id": "ops-1", "role": "ops_manager" },
                    { "approver_user_id": "compliance-1", "role": "compliance" }
                ]
            }),
        ))
        .await
        .expect("valid compensation request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["proposal"]["proposal_id"], json!("cmp-000001"));
    assert_eq!(
        body["proposal"]["status"],
        json!("approved_ready_for_ledger")
    );
    assert_eq!(body["proposal"]["amount_minor_units"], json!(5000));
    assert_eq!(body["proposal"]["ledger_transaction_posted"], json!(false));
    assert_eq!(
        body["audit"]["action"],
        json!("compensation_proposal_created")
    );
    assert_eq!(body["audit"]["subject_user_id"], json!("user_123"));
    assert_eq!(
        body["audit"]["correlation_id"],
        json!("admin-compensation-TICKET-3-1")
    );

    let records = store.audit_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].action, "compensation_proposal_created");
    assert_eq!(records[0].subject_user_id, "user_123");
    assert_eq!(records[0].correlation_id, "admin-compensation-TICKET-3-1");
}

#[tokio::test]
async fn member_action_accepts_admin_context_and_records_audit_metadata() {
    let app = internal_admin_router(InMemoryAdminOpsStore::default());

    let response = app
        .oneshot(json_post(
            "/api/internal/admin/members/actions",
            &json!({
                "target_user_id": "user_123",
                "action_type": "restrict_withdrawals",
                "ticket_id": "TICKET-4",
                "reason": "risk review hold",
                "correlation_id": "case-kyc-123"
            }),
        ))
        .await
        .expect("valid member action request should be handled");

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = response_json(response).await;
    assert_eq!(body["action_id"], json!("mem-act-000001"));
    assert_eq!(body["target_user_id"], json!("user_123"));
    assert_eq!(body["status"], json!("pending_second_approval"));
    assert_eq!(body["requires_second_approval"], json!(true));
    assert_eq!(body["audit"]["actor_user_id"], json!("admin-1"));
    assert_eq!(body["audit"]["actor_role"], json!("admin"));
    assert_eq!(body["audit"]["correlation_id"], json!("case-kyc-123"));
    assert_eq!(body["audit"]["ledger_transaction_posted"], json!(false));
}

#[tokio::test]
async fn member_action_requires_actor_context() {
    let app = internal_admin_router(InMemoryAdminOpsStore::default());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/internal/admin/members/actions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "target_user_id": "user_123",
                        "action_type": "add_note",
                        "ticket_id": "TICKET-5",
                        "reason": "review note"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("missing context request should be handled");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response_json(response).await["error"],
        json!("ERROR_CODE_MISSING_ADMIN_CONTEXT")
    );
}

fn json_post(uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("x-actor-id", "admin-1")
        .header("x-operator-role", "admin")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|error| panic!("request should build: {error}"))
}

fn empty_get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap_or_else(|error| panic!("request should build: {error}"))
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
