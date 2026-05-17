#![allow(clippy::expect_used)]

use api_gateway::admin_ops::{
    HttpAdminOpsStore, InMemoryAdminOpsStore, PostgresCompensationProposalStore,
    ProductionAdminOpsStore, internal_admin_router,
};
use axum::{
    Json, Router,
    body::Body,
    http::{Request, StatusCode},
    routing::{delete, get, patch, post},
};
use http_body_util::BodyExt;
use kyc_service::{SecretConfig, api::router as kyc_router, kyc::PostgresKycStore};
use notification_service::{InMemoryNotificationStore, router as notification_router};
use risk_antifraud_service::member_actions::{
    InMemoryRiskEventStore, router as risk_member_actions_router,
};
use serde_json::{Value, json};
use sqlx::{
    PgPool, Row,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpListener;
use tower::ServiceExt;
use user_service::members::{PostgresMemberStore, internal_members_router as user_members_router};
use wallet_ledger_service::{api::router as wallet_router, ledger::PostgresLedgerStore};

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
async fn pending_member_cases_proxy_enqueues_email_and_in_app_reminders() {
    let upstream = Router::new()
        .route(
            "/v1/internal/admin/members/pending-cases",
            get(|| async {
                Json(json!({
                    "items": [{
                        "member_id": "member_pending_notification",
                        "display_name": "Pending Notification Member",
                        "restriction_reason": "withdrawals_locked",
                        "contact": {
                            "email_masked": "not_available",
                            "phone_masked": "not_available"
                        },
                        "status": "restricted",
                        "kyc_status": "review_required",
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
                        "id": "member_pending_notification",
                        "email_masked": "p***@example.com",
                        "phone_masked": "0912***998"
                    }]
                }))
            }),
        );
    let upstream_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream");
    let upstream_url = format!(
        "http://{}",
        upstream_listener.local_addr().expect("local addr")
    );
    tokio::spawn(async move {
        axum::serve(upstream_listener, upstream)
            .await
            .expect("serve upstream");
    });

    let notification_store = InMemoryNotificationStore::default();
    let notification_service = notification_router(notification_store);
    let notification_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind notification service");
    let notification_url = format!(
        "http://{}",
        notification_listener.local_addr().expect("local addr")
    );
    tokio::spawn(async move {
        axum::serve(notification_listener, notification_service)
            .await
            .expect("serve notification service");
    });

    let store = HttpAdminOpsStore::new(upstream_url.clone(), upstream_url)
        .expect("store config")
        .with_notification_service(notification_url.clone())
        .expect("notification config");
    let app = internal_admin_router(store);
    let response = app
        .oneshot(empty_get(
            "/api/internal/admin/members/pending-cases?status=restricted",
        ))
        .await
        .expect("pending cases request handled");
    assert_eq!(response.status(), StatusCode::OK);

    let messages = reqwest::Client::new()
        .get(format!(
            "{notification_url}/internal/notifications/messages?recipient_role=risk_ops&status=queued"
        ))
        .send()
        .await
        .expect("notification list request sends")
        .error_for_status()
        .expect("notification list request succeeds")
        .json::<Value>()
        .await
        .expect("notification list decodes");
    assert_eq!(messages["messages"].as_array().map(Vec::len), Some(2));
    assert!(messages["messages"].as_array().is_some_and(|items| {
        items.iter().any(|item| item["channel"] == "email")
            && items.iter().any(|item| item["channel"] == "in_app")
    }));
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
async fn member_management_proxy_persists_to_postgres_user_service() {
    let Some(pool) = postgres_test_pool().await else {
        return;
    };
    let user_id = unique_test_id("MEM-E2E");
    cleanup_member_rows(&pool, &user_id).await;

    let user_service = user_members_router(PostgresMemberStore::new(pool.clone()));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind user service");
    let user_service_url = format!("http://{}", listener.local_addr().expect("local addr"));
    tokio::spawn(async move {
        axum::serve(listener, user_service)
            .await
            .expect("serve user service");
    });

    let store =
        HttpAdminOpsStore::new("http://127.0.0.1:1", user_service_url).expect("store config");
    let app = internal_admin_router(store);

    let created = app
        .clone()
        .oneshot(json_post(
            "/api/internal/admin/members",
            &json!({
                "user_id": user_id.clone(),
                "display_name": "E2E Postgres Member",
                "email": format!("{}@example.com", user_id.to_lowercase()),
                "phone": "0912345001",
                "country": "TW",
                "timezone": "Asia/Taipei",
                "tier": "Standard",
                "affiliate_id": "AFF-E2E"
            }),
        ))
        .await
        .expect("create request should be handled");
    assert_eq!(created.status(), StatusCode::CREATED);
    assert_eq!(response_json(created).await["member"]["status"], "inactive");
    assert_eq!(
        persisted_status(&pool, &user_id).await,
        Some("inactive".to_owned())
    );

    let updated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/internal/admin/members/{user_id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "kyc_status": "verified",
                        "risk_level": "low",
                        "risk_score": 7
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("update request should be handled");
    assert_eq!(updated.status(), StatusCode::OK);
    assert_eq!(response_json(updated).await["member"]["status"], "active");
    assert_eq!(
        persisted_status(&pool, &user_id).await,
        Some("active".to_owned())
    );

    let listed = app
        .clone()
        .oneshot(empty_get(
            "/api/internal/admin/members?affiliate_id=AFF-E2E",
        ))
        .await
        .expect("list request should be handled");
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body = response_json(listed).await;
    assert_eq!(listed_body["source"], "user-service:member-store");
    assert_eq!(listed_body["members"][0]["member"]["user_id"], user_id);

    let deleted = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/internal/admin/members/{user_id}"))
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("delete request should be handled");
    assert_eq!(deleted.status(), StatusCode::OK);
    assert_eq!(response_json(deleted).await["user_id"], user_id);
    assert!(persisted_deleted_at(&pool, &user_id).await.is_some());
}

#[tokio::test]
async fn approved_member_action_applies_restriction_to_postgres_user_service() {
    let Some(pool) = postgres_test_pool().await else {
        return;
    };
    ensure_admin_member_actions_table(&pool).await;
    let user_id = unique_test_id("MEM-ACTION");
    cleanup_member_rows(&pool, &user_id).await;
    cleanup_member_actions(&pool, &user_id).await;

    let user_service = user_members_router(PostgresMemberStore::new(pool.clone()));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind user service");
    let user_service_url = format!("http://{}", listener.local_addr().expect("local addr"));
    tokio::spawn(async move {
        axum::serve(listener, user_service)
            .await
            .expect("serve user service");
    });

    let store = ProductionAdminOpsStore::new(
        HttpAdminOpsStore::new("http://127.0.0.1:1", user_service_url).expect("store config"),
        PostgresCompensationProposalStore::new(pool.clone()),
    );
    let app = internal_admin_router(store);

    let created = app
        .clone()
        .oneshot(json_post(
            "/api/internal/admin/members",
            &json!({
                "user_id": user_id.clone(),
                "display_name": "Action Target",
                "email": format!("{}@example.com", user_id.to_lowercase()),
                "phone": "0912345002",
                "country": "TW",
                "timezone": "Asia/Taipei",
                "tier": "Standard"
            }),
        ))
        .await
        .expect("create member request should be handled");
    assert_eq!(created.status(), StatusCode::CREATED);

    let action = app
        .clone()
        .oneshot(json_post(
            "/api/internal/admin/members/actions",
            &json!({
                "target_user_id": user_id.clone(),
                "action_type": "restrict_withdrawals",
                "ticket_id": "TICKET-PG-ACTION",
                "reason": "risk review hold",
                "correlation_id": format!("case-{}", user_id)
            }),
        ))
        .await
        .expect("create action request should be handled");
    assert_eq!(action.status(), StatusCode::ACCEPTED);
    let action_body = response_json(action).await;
    let action_id = action_body["action_id"]
        .as_str()
        .expect("action id")
        .to_owned();

    let approved = app
        .oneshot(json_post_as(
            &format!("/api/internal/admin/members/actions/{action_id}/reviews"),
            "manager-2",
            "manager",
            &json!({
                "decision": "approve",
                "reason": "second reviewer confirmed withdrawals lock"
            }),
        ))
        .await
        .expect("approval request should be handled");
    assert_eq!(approved.status(), StatusCode::OK);
    let approved_body = response_json(approved).await;
    assert_eq!(approved_body["status"], "approved");
    assert_eq!(
        persisted_status(&pool, &user_id).await,
        Some("restricted".to_owned())
    );
    assert!(
        persisted_restrictions(&pool, &user_id)
            .await
            .iter()
            .any(|restriction| restriction == "withdrawals_locked")
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn approved_withdrawal_restriction_freezes_wallet_and_records_risk_event() {
    let Some(member_pool) = postgres_test_pool().await else {
        return;
    };
    let Some(wallet_pool) = postgres_wallet_test_pool().await else {
        return;
    };
    ensure_admin_member_actions_table(&member_pool).await;
    let user_id = unique_test_id("MEM-WALLET-ACTION");
    cleanup_member_rows(&member_pool, &user_id).await;
    cleanup_member_actions(&member_pool, &user_id).await;

    let user_service = user_members_router(PostgresMemberStore::new(member_pool.clone()));
    let user_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind user service");
    let user_service_url = format!("http://{}", user_listener.local_addr().expect("local addr"));
    tokio::spawn(async move {
        axum::serve(user_listener, user_service)
            .await
            .expect("serve user service");
    });

    let wallet_service = wallet_router(PostgresLedgerStore::new(wallet_pool.clone()));
    let wallet_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind wallet service");
    let wallet_service_url = format!(
        "http://{}",
        wallet_listener.local_addr().expect("local addr")
    );
    tokio::spawn(async move {
        axum::serve(wallet_listener, wallet_service)
            .await
            .expect("serve wallet service");
    });

    let risk_store = InMemoryRiskEventStore::default();
    let risk_service = risk_member_actions_router(risk_store.clone());
    let risk_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind risk service");
    let risk_service_url = format!("http://{}", risk_listener.local_addr().expect("local addr"));
    tokio::spawn(async move {
        axum::serve(risk_listener, risk_service)
            .await
            .expect("serve risk service");
    });

    seed_wallet_balance(&wallet_service_url, &user_id, 12_500).await;

    let store = ProductionAdminOpsStore::new(
        HttpAdminOpsStore::with_side_effect_services(
            "http://127.0.0.1:1",
            user_service_url,
            Some(wallet_service_url.clone()),
            Some(risk_service_url),
        )
        .expect("store config"),
        PostgresCompensationProposalStore::new(member_pool.clone()),
    );
    let app = internal_admin_router(store);

    let created = app
        .clone()
        .oneshot(json_post(
            "/api/internal/admin/members",
            &json!({
                "user_id": user_id.clone(),
                "display_name": "Wallet Action Target",
                "email": format!("{}@example.com", user_id.to_lowercase()),
                "phone": "0912345004",
                "country": "TW",
                "timezone": "Asia/Taipei",
                "tier": "Standard"
            }),
        ))
        .await
        .expect("create member request should be handled");
    assert_eq!(created.status(), StatusCode::CREATED);

    let action = app
        .clone()
        .oneshot(json_post(
            "/api/internal/admin/members/actions",
            &json!({
                "target_user_id": user_id.clone(),
                "action_type": "restrict_withdrawals",
                "ticket_id": "TICKET-WALLET-ACTION",
                "reason": "withdrawal velocity hold",
                "correlation_id": format!("case-wallet-{}", user_id)
            }),
        ))
        .await
        .expect("create action request should be handled");
    assert_eq!(action.status(), StatusCode::ACCEPTED);
    let action_body = response_json(action).await;
    let action_id = action_body["action_id"]
        .as_str()
        .expect("action id")
        .to_owned();

    let approved = app
        .oneshot(json_post_as(
            &format!("/api/internal/admin/members/actions/{action_id}/reviews"),
            "manager-2",
            "manager",
            &json!({
                "decision": "approve",
                "reason": "second reviewer confirmed wallet freeze"
            }),
        ))
        .await
        .expect("approval request should be handled");
    assert_eq!(approved.status(), StatusCode::OK);
    let approved_body = response_json(approved).await;
    assert_eq!(
        approved_body["side_effects"],
        json!([
            "member_restriction:withdrawals_locked",
            "wallet_freeze:available_balance",
            "risk_event:member_action_recorded"
        ])
    );
    let wallet_balance = wallet_balance(&wallet_service_url, &user_id).await;
    assert_eq!(wallet_balance["available_minor_units"], 0);
    assert_eq!(wallet_balance["frozen_minor_units"], -12_500);
    assert!(
        risk_store
            .events_for_user(&user_id)
            .iter()
            .any(|event| event.action_type == "restrict_withdrawals")
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn approved_kyc_review_action_creates_kyc_task_and_restricts_member() {
    let Some(member_pool) = postgres_test_pool().await else {
        return;
    };
    let Some(kyc_pool) = postgres_kyc_test_pool().await else {
        return;
    };
    ensure_admin_member_actions_table(&member_pool).await;
    let user_id = unique_test_id("MEM-KYC-ACTION");
    cleanup_member_rows(&member_pool, &user_id).await;
    cleanup_member_actions(&member_pool, &user_id).await;

    let user_service = user_members_router(PostgresMemberStore::new(member_pool.clone()));
    let user_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind user service");
    let user_service_url = format!("http://{}", user_listener.local_addr().expect("local addr"));
    tokio::spawn(async move {
        axum::serve(user_listener, user_service)
            .await
            .expect("serve user service");
    });

    let kyc_service = kyc_router(
        PostgresKycStore::new(kyc_pool.clone(), "test-kyc-salt".to_owned()),
        SecretConfig::dev_default(),
    );
    let kyc_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind kyc service");
    let kyc_service_url = format!("http://{}", kyc_listener.local_addr().expect("local addr"));
    tokio::spawn(async move {
        axum::serve(kyc_listener, kyc_service)
            .await
            .expect("serve kyc service");
    });

    let store = ProductionAdminOpsStore::new(
        HttpAdminOpsStore::new(kyc_service_url, user_service_url).expect("store config"),
        PostgresCompensationProposalStore::new(member_pool.clone()),
    );
    let app = internal_admin_router(store);

    let created = app
        .clone()
        .oneshot(json_post(
            "/api/internal/admin/members",
            &json!({
                "user_id": user_id.clone(),
                "display_name": "KYC Action Target",
                "email": format!("{}@example.com", user_id.to_lowercase()),
                "phone": "0912345003",
                "country": "TW",
                "timezone": "Asia/Taipei",
                "tier": "Standard"
            }),
        ))
        .await
        .expect("create member request should be handled");
    assert_eq!(created.status(), StatusCode::CREATED);

    let action = app
        .clone()
        .oneshot(json_post(
            "/api/internal/admin/members/actions",
            &json!({
                "target_user_id": user_id.clone(),
                "action_type": "request_kyc_review",
                "ticket_id": "TICKET-KYC-ACTION",
                "reason": "document mismatch requires KYC review",
                "correlation_id": format!("case-kyc-{}", user_id)
            }),
        ))
        .await
        .expect("create action request should be handled");
    assert_eq!(action.status(), StatusCode::ACCEPTED);
    let action_body = response_json(action).await;
    let action_id = action_body["action_id"]
        .as_str()
        .expect("action id")
        .to_owned();

    let approved = app
        .oneshot(json_post_as(
            &format!("/api/internal/admin/members/actions/{action_id}/reviews"),
            "manager-2",
            "manager",
            &json!({
                "decision": "approve",
                "reason": "second reviewer confirmed KYC review task"
            }),
        ))
        .await
        .expect("approval request should be handled");
    assert_eq!(approved.status(), StatusCode::OK);
    let approved_body = response_json(approved).await;
    assert_eq!(approved_body["status"], "approved");
    assert_eq!(
        approved_body["side_effects"],
        json!(["kyc_review_task:requested"])
    );
    assert_eq!(
        persisted_status(&member_pool, &user_id).await,
        Some("restricted".to_owned())
    );
    assert!(
        persisted_restrictions(&member_pool, &user_id)
            .await
            .iter()
            .any(|restriction| restriction == "manual_review_required")
    );
    assert_eq!(persisted_kyc_status(&kyc_pool, &user_id).await, "pending");
    assert_eq!(persisted_kyc_tier(&kyc_pool, &user_id).await, 2);
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
async fn production_member_action_enqueues_second_approval_reminder() {
    let Some(pool) = postgres_test_pool().await else {
        return;
    };
    ensure_admin_member_actions_table(&pool).await;
    let user_id = unique_test_id("MEM-NOTIFY-ACTION");
    cleanup_member_actions(&pool, &user_id).await;

    let notification_store = InMemoryNotificationStore::default();
    let notification_service = notification_router(notification_store);
    let notification_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind notification service");
    let notification_url = format!(
        "http://{}",
        notification_listener.local_addr().expect("local addr")
    );
    tokio::spawn(async move {
        axum::serve(notification_listener, notification_service)
            .await
            .expect("serve notification service");
    });

    let upstream = HttpAdminOpsStore::new("http://127.0.0.1:1", "http://127.0.0.1:1")
        .expect("store config")
        .with_notification_service(notification_url.clone())
        .expect("notification config");
    let store =
        ProductionAdminOpsStore::new(upstream, PostgresCompensationProposalStore::new(pool));
    let app = internal_admin_router(store);

    let created = app
        .oneshot(json_post(
            "/api/internal/admin/members/actions",
            &json!({
                "target_user_id": user_id,
                "action_type": "restrict_withdrawals",
                "ticket_id": "TICKET-NOTIFY-ACTION",
                "reason": "risk review hold",
                "correlation_id": "case-notification-action"
            }),
        ))
        .await
        .expect("valid member action request should be handled");
    assert_eq!(created.status(), StatusCode::ACCEPTED);

    let messages = reqwest::Client::new()
        .get(format!(
            "{notification_url}/internal/notifications/messages?recipient_role=ops_control&status=queued"
        ))
        .send()
        .await
        .expect("notification list request sends")
        .error_for_status()
        .expect("notification list request succeeds")
        .json::<Value>()
        .await
        .expect("notification list decodes");
    assert_eq!(messages["messages"].as_array().map(Vec::len), Some(2));
    assert!(messages["messages"].as_array().is_some_and(|items| {
        items.iter().any(|item| item["channel"] == "email")
            && items.iter().any(|item| item["channel"] == "in_app")
    }));
}

#[tokio::test]
async fn member_action_lists_pending_actions_and_second_approver_can_approve() {
    let app = internal_admin_router(InMemoryAdminOpsStore::default());

    let created = app
        .clone()
        .oneshot(json_post(
            "/api/internal/admin/members/actions",
            &json!({
                "target_user_id": "user_123",
                "action_type": "restrict_withdrawals",
                "ticket_id": "TICKET-6",
                "reason": "risk review hold",
                "correlation_id": "case-risk-456"
            }),
        ))
        .await
        .expect("valid member action request should be handled");
    assert_eq!(created.status(), StatusCode::ACCEPTED);

    let list = app
        .clone()
        .oneshot(empty_get(
            "/api/internal/admin/members/actions?status=pending_second_approval",
        ))
        .await
        .expect("list request should be handled");
    assert_eq!(list.status(), StatusCode::OK);
    let body = response_json(list).await;
    assert_eq!(body["source"], "api-gateway:admin-ops");
    assert_eq!(body["actions"].as_array().map(Vec::len), Some(1));
    assert_eq!(body["actions"][0]["action_id"], json!("mem-act-000001"));
    assert_eq!(
        body["actions"][0]["status"],
        json!("pending_second_approval")
    );

    let approved = app
        .oneshot(json_post_as(
            "/api/internal/admin/members/actions/mem-act-000001/reviews",
            "manager-2",
            "manager",
            &json!({
                "decision": "approve",
                "reason": "second reviewer confirmed the risk hold"
            }),
        ))
        .await
        .expect("approval request should be handled");
    assert_eq!(approved.status(), StatusCode::OK);
    let body = response_json(approved).await;
    assert_eq!(body["status"], json!("approved"));
    assert_eq!(body["review"]["reviewer_user_id"], json!("manager-2"));
    assert_eq!(
        body["side_effects"],
        json!([
            "member_restriction:withdrawals_locked",
            "wallet_freeze:available_balance",
            "risk_event:member_action_recorded"
        ])
    );
}

#[tokio::test]
async fn member_action_rejects_self_approval_and_supports_rejection() {
    let app = internal_admin_router(InMemoryAdminOpsStore::default());

    let created = app
        .clone()
        .oneshot(json_post(
            "/api/internal/admin/members/actions",
            &json!({
                "target_user_id": "user_123",
                "action_type": "request_kyc_review",
                "ticket_id": "TICKET-7",
                "reason": "document mismatch"
            }),
        ))
        .await
        .expect("valid member action request should be handled");
    assert_eq!(created.status(), StatusCode::ACCEPTED);

    let self_approval = app
        .clone()
        .oneshot(json_post(
            "/api/internal/admin/members/actions/mem-act-000001/reviews",
            &json!({
                "decision": "approve",
                "reason": "same person tries to approve"
            }),
        ))
        .await
        .expect("self approval request should be handled");
    assert_eq!(self_approval.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(self_approval).await["error"],
        json!("ERROR_CODE_MEMBER_ACTION_SELF_APPROVAL")
    );

    let rejected = app
        .oneshot(json_post_as(
            "/api/internal/admin/members/actions/mem-act-000001/reviews",
            "manager-2",
            "manager",
            &json!({
                "decision": "reject",
                "reason": "insufficient evidence"
            }),
        ))
        .await
        .expect("rejection request should be handled");
    assert_eq!(rejected.status(), StatusCode::OK);
    let body = response_json(rejected).await;
    assert_eq!(body["status"], json!("rejected"));
    assert_eq!(body["side_effects"], json!([]));
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
    json_post_as(uri, "admin-1", "admin", body)
}

fn json_post_as(uri: &str, actor_id: &str, role: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("x-actor-id", actor_id)
        .header("x-operator-role", role)
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

async fn postgres_test_pool() -> Option<PgPool> {
    let Some(database_url) = std::env::var("USER_SERVICE_TEST_DATABASE_URL").ok() else {
        eprintln!(
            "skipping api-gateway postgres member proxy test: USER_SERVICE_TEST_DATABASE_URL is not set"
        );
        return None;
    };
    let bootstrap_pool = PgPool::connect(&database_url)
        .await
        .unwrap_or_else(|error| panic!("test database should be reachable: {error}"));
    let schema = unique_test_schema("api_gateway_members");
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"))
        .execute(&bootstrap_pool)
        .await
        .unwrap_or_else(|error| panic!("test schema should be created: {error}"));
    bootstrap_pool.close().await;
    let options = PgConnectOptions::from_str(&database_url)
        .unwrap_or_else(|error| panic!("test database URL should parse: {error}"))
        .options([("search_path", schema.as_str())]);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap_or_else(|error| panic!("test schema database should be reachable: {error}"));
    sqlx::migrate!("../user-service/migrations")
        .run(&pool)
        .await
        .unwrap_or_else(|error| panic!("user-service migrations should run: {error}"));
    Some(pool)
}

async fn postgres_kyc_test_pool() -> Option<PgPool> {
    let Some(database_url) = std::env::var("USER_SERVICE_TEST_DATABASE_URL").ok() else {
        eprintln!(
            "skipping api-gateway postgres kyc action test: USER_SERVICE_TEST_DATABASE_URL is not set"
        );
        return None;
    };
    let bootstrap_pool = PgPool::connect(&database_url)
        .await
        .unwrap_or_else(|error| panic!("test database should be reachable: {error}"));
    let schema = unique_test_schema("api_gateway_kyc");
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"))
        .execute(&bootstrap_pool)
        .await
        .unwrap_or_else(|error| panic!("test schema should be created: {error}"));
    bootstrap_pool.close().await;
    let options = PgConnectOptions::from_str(&database_url)
        .unwrap_or_else(|error| panic!("test database URL should parse: {error}"))
        .options([("search_path", schema.as_str())]);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap_or_else(|error| panic!("test schema database should be reachable: {error}"));
    sqlx::migrate!("../kyc-service/migrations")
        .run(&pool)
        .await
        .unwrap_or_else(|error| panic!("kyc-service migrations should run: {error}"));
    Some(pool)
}

async fn postgres_wallet_test_pool() -> Option<PgPool> {
    let Some(database_url) = std::env::var("USER_SERVICE_TEST_DATABASE_URL").ok() else {
        eprintln!(
            "skipping api-gateway postgres wallet action test: USER_SERVICE_TEST_DATABASE_URL is not set"
        );
        return None;
    };
    let bootstrap_pool = PgPool::connect(&database_url)
        .await
        .unwrap_or_else(|error| panic!("test database should be reachable: {error}"));
    let schema = unique_test_schema("api_gateway_wallet");
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"))
        .execute(&bootstrap_pool)
        .await
        .unwrap_or_else(|error| panic!("test schema should be created: {error}"));
    bootstrap_pool.close().await;
    let options = PgConnectOptions::from_str(&database_url)
        .unwrap_or_else(|error| panic!("test database URL should parse: {error}"))
        .options([("search_path", schema.as_str())]);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap_or_else(|error| panic!("test schema database should be reachable: {error}"));
    sqlx::migrate!("../wallet-ledger-service/migrations")
        .run(&pool)
        .await
        .unwrap_or_else(|error| panic!("wallet-ledger-service migrations should run: {error}"));
    Some(pool)
}

fn unique_test_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|error| panic!("system time should be after unix epoch: {error}"))
        .as_nanos();
    format!("{prefix}-{nanos}")
}

fn unique_test_schema(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|error| panic!("system time should be after unix epoch: {error}"))
        .as_nanos();
    format!("{prefix}_{nanos}")
}

async fn cleanup_member_rows(pool: &PgPool, user_id: &str) {
    sqlx::query("DELETE FROM user_members WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap_or_else(|error| panic!("test cleanup should delete user_members row: {error}"));
}

async fn cleanup_member_actions(pool: &PgPool, user_id: &str) {
    sqlx::query("DELETE FROM admin_member_actions WHERE target_user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap_or_else(|error| {
            panic!("test cleanup should delete admin_member_actions rows: {error}")
        });
}

async fn ensure_admin_member_actions_table(pool: &PgPool) {
    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS admin_member_actions (
            id BIGSERIAL PRIMARY KEY,
            action_id TEXT GENERATED ALWAYS AS ('mem-act-' || lpad(id::text, 6, '0')) STORED UNIQUE,
            target_user_id TEXT NOT NULL,
            action_type TEXT NOT NULL CHECK (action_type IN (
                'add_note',
                'request_kyc_review',
                'restrict_withdrawals',
                'escalate_case'
            )),
            ticket_id TEXT NOT NULL,
            reason TEXT NOT NULL,
            status TEXT NOT NULL CHECK (
                status IN ('submitted', 'pending_second_approval', 'approved', 'rejected')
            ),
            requires_second_approval BOOLEAN NOT NULL,
            actor_user_id TEXT NOT NULL,
            actor_role TEXT NOT NULL,
            correlation_id TEXT NOT NULL UNIQUE,
            idempotency_key TEXT UNIQUE,
            request_hash CHAR(64),
            ledger_transaction_posted BOOLEAN NOT NULL DEFAULT FALSE,
            reviewer_user_id TEXT,
            reviewer_role TEXT,
            review_decision TEXT CHECK (review_decision IN ('approve', 'reject')),
            review_reason TEXT,
            reviewed_at TIMESTAMPTZ,
            side_effects JSONB NOT NULL DEFAULT '[]',
            created_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )
        ",
    )
    .execute(pool)
    .await
    .unwrap_or_else(|error| panic!("admin member action table should be available: {error}"));
}

async fn persisted_status(pool: &PgPool, user_id: &str) -> Option<String> {
    sqlx::query("SELECT status FROM user_members WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .unwrap_or_else(|error| panic!("status query should run: {error}"))
        .map(|row| {
            row.try_get("status")
                .unwrap_or_else(|error| panic!("status column should decode: {error}"))
        })
}

async fn persisted_restrictions(pool: &PgPool, user_id: &str) -> Vec<String> {
    sqlx::query("SELECT restrictions FROM user_members WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("restrictions query should run: {error}"))
        .try_get::<Value, _>("restrictions")
        .unwrap_or_else(|error| panic!("restrictions column should decode: {error}"))
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

async fn persisted_deleted_at(pool: &PgPool, user_id: &str) -> Option<String> {
    sqlx::query("SELECT deleted_at::text AS deleted_at FROM user_members WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .unwrap_or_else(|error| panic!("deleted_at query should run: {error}"))
        .and_then(|row| {
            row.try_get("deleted_at")
                .unwrap_or_else(|error| panic!("deleted_at column should decode: {error}"))
        })
}

async fn persisted_kyc_status(pool: &PgPool, user_id: &str) -> String {
    sqlx::query("SELECT status FROM user_kyc_records WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("kyc status query should run: {error}"))
        .try_get("status")
        .unwrap_or_else(|error| panic!("kyc status column should decode: {error}"))
}

async fn persisted_kyc_tier(pool: &PgPool, user_id: &str) -> i16 {
    sqlx::query("SELECT tier FROM user_kyc_records WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| panic!("kyc tier query should run: {error}"))
        .try_get("tier")
        .unwrap_or_else(|error| panic!("kyc tier column should decode: {error}"))
}

async fn seed_wallet_balance(wallet_service_url: &str, user_id: &str, amount_minor_units: i64) {
    let response = reqwest::Client::new()
        .post(format!("{wallet_service_url}/internal/ledger/transactions"))
        .header("idempotency-key", format!("seed-wallet:{user_id}"))
        .json(&json!({
            "txn_type": "deposit",
            "correlation_id": format!("seed-wallet:{user_id}"),
            "description": "seed wallet balance for admin ops test",
            "entries": [
                {
                    "account_code": "Bank.TrustAccount.TWD",
                    "direction": "debit",
                    "amount_minor_units": amount_minor_units
                },
                {
                    "account_code": format!("Player.Wallet.{user_id}"),
                    "direction": "credit",
                    "amount_minor_units": amount_minor_units
                }
            ]
        }))
        .send()
        .await
        .unwrap_or_else(|error| panic!("seed wallet request should send: {error}"));
    assert!(
        response.status().is_success(),
        "seed wallet request should succeed: {}",
        response.text().await.unwrap_or_default()
    );
}

async fn wallet_balance(wallet_service_url: &str, user_id: &str) -> Value {
    reqwest::Client::new()
        .get(format!(
            "{wallet_service_url}/internal/ledger/wallets/{user_id}/balance"
        ))
        .header("x-actor-id", "test-admin")
        .header("x-operator-role", "admin")
        .send()
        .await
        .unwrap_or_else(|error| panic!("wallet balance request should send: {error}"))
        .error_for_status()
        .unwrap_or_else(|error| panic!("wallet balance request should succeed: {error}"))
        .json::<Value>()
        .await
        .unwrap_or_else(|error| panic!("wallet balance response should decode: {error}"))
}
