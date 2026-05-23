#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::{
    PgPool, Row,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use tower::ServiceExt;
use user_service::members::{
    InMemoryMemberStore, MemberRecord, MemberStore, MemberStoreError, PostgresMemberStore,
    internal_members_router,
};

#[tokio::test]
async fn member_crud_defaults_inactive_until_kyc_verified() {
    let app = internal_members_router(InMemoryMemberStore::default());

    let created = request_json(
        app.clone(),
        "POST",
        "/v1/internal/admin/members",
        Some(json!({
            "user_id": "MEM-900",
            "display_name": "Test Member",
            "email": "member900@example.com",
            "phone": "0912345900",
            "country": "TW",
            "timezone": "Asia/Taipei",
            "tier": "Standard",
            "affiliate_id": "AFF-001",
            "registration_audit": {
                "referral_capture_required": true,
                "referral_code": "AFF-001",
                "referred_user_id": "MEM-900"
            }
        })),
        admin_headers(),
    )
    .await;

    assert_eq!(created.status, StatusCode::CREATED);
    assert_eq!(created.body["member"]["user_id"], "MEM-900");
    assert_eq!(created.body["member"]["status"], "inactive");
    assert_eq!(created.body["kyc_risk"]["kyc_status"], "pending");
    assert_eq!(created.body["source"]["affiliate_id"], "AFF-001");
    assert_eq!(
        created.body["audit"]["registration_audit"]["referral_code"],
        "AFF-001"
    );

    let updated = request_json(
        app.clone(),
        "PATCH",
        "/v1/internal/admin/members/MEM-900",
        Some(json!({ "kyc_status": "verified", "risk_level": "low", "risk_score": 10 })),
        admin_headers(),
    )
    .await;

    assert_eq!(updated.status, StatusCode::OK);
    assert_eq!(updated.body["member"]["status"], "active");
    assert_eq!(updated.body["kyc_risk"]["kyc_status"], "verified");
    assert_eq!(updated.body["kyc_risk"]["restrictions"], json!([]));
}

#[tokio::test]
async fn manager_member_mutation_is_denied_and_audited() {
    let store = InMemoryMemberStore::new(vec![fixture_record("MEM-001", None)]);
    let app = internal_members_router(store.clone());

    let response = request_json(
        app,
        "PATCH",
        "/v1/internal/admin/members/MEM-001",
        Some(json!({ "tier": "VIP" })),
        manager_headers(),
    )
    .await;

    assert_eq!(response.status, StatusCode::FORBIDDEN);
    assert_eq!(
        response.body["error"],
        json!("ERROR_CODE_MEMBER_ACCOUNT_FORBIDDEN")
    );
    let denied_audits = store.denied_audit_logs();
    assert_eq!(denied_audits.len(), 1);
    assert_eq!(denied_audits[0].target_user_id.as_deref(), Some("MEM-001"));
    assert_eq!(denied_audits[0].action, "update");
    assert_eq!(denied_audits[0].actor_id, "manager-1");
    assert_eq!(denied_audits[0].actor_role, "manager");
    assert_eq!(
        denied_audits[0].reason_or_error_code,
        "ERROR_CODE_MEMBER_ACCOUNT_FORBIDDEN"
    );
}

#[tokio::test]
async fn role_account_create_enforces_assignment_rules() {
    let store = InMemoryMemberStore::default();
    let app = internal_members_router(store.clone());

    let admin_manager = request_json(
        app.clone(),
        "POST",
        "/v1/internal/admin/members",
        Some(json!({
            "user_id": "MAN-001",
            "display_name": "Manager One",
            "email": "manager-one@example.com",
            "phone": "0912345001",
            "country": "TW",
            "timezone": "Asia/Taipei",
            "tier": "Internal",
            "role": "manager",
            "password": "correct horse manager"
        })),
        admin_headers(),
    )
    .await;

    assert_eq!(admin_manager.status, StatusCode::CREATED);
    assert_eq!(
        store.credential_role_for_user_id("MAN-001").as_deref(),
        Some("manager")
    );
    let account_audits = store.account_audit_logs();
    assert_eq!(account_audits.len(), 1);
    assert_eq!(account_audits[0].target_user_id, "MAN-001");
    assert_eq!(account_audits[0].action, "create");
    assert_eq!(account_audits[0].actor_role, "admin");

    let manager_affiliate = request_json(
        app.clone(),
        "POST",
        "/v1/internal/admin/members",
        Some(json!({
            "user_id": "AFF-USER-001",
            "display_name": "Affiliate One",
            "email": "affiliate-one@example.com",
            "phone": "0912345002",
            "country": "TW",
            "timezone": "Asia/Taipei",
            "tier": "Affiliate",
            "role": "affiliate",
            "password": "correct horse affiliate",
            "affiliate_id": "AFF-001"
        })),
        manager_headers(),
    )
    .await;

    assert_eq!(manager_affiliate.status, StatusCode::CREATED);
    assert_eq!(
        store.credential_role_for_user_id("AFF-USER-001").as_deref(),
        Some("affiliate")
    );

    let denied_manager = request_json(
        app,
        "POST",
        "/v1/internal/admin/members",
        Some(json!({
            "user_id": "MAN-002",
            "display_name": "Manager Two",
            "email": "manager-two@example.com",
            "phone": "0912345003",
            "country": "TW",
            "timezone": "Asia/Taipei",
            "tier": "Internal",
            "role": "manager",
            "password": "correct horse denied"
        })),
        manager_headers(),
    )
    .await;

    assert_eq!(denied_manager.status, StatusCode::FORBIDDEN);
    assert_eq!(
        denied_manager.body["error"],
        json!("ERROR_CODE_MEMBER_ACCOUNT_FORBIDDEN")
    );
    let denied_audits = store.denied_audit_logs();
    assert_eq!(denied_audits.len(), 1);
    assert_eq!(denied_audits[0].target_user_id.as_deref(), Some("MAN-002"));
    assert_eq!(denied_audits[0].actor_role, "manager");
}

#[tokio::test]
async fn admin_update_and_delete_manager_account_are_audited_and_soft_disabled() {
    let store = InMemoryMemberStore::new(vec![fixture_record("MAN-001", None)]);
    store
        .seed_login_credential(user_service::members::MemberLoginCredentialUpsert {
            user_id: "MAN-001".to_owned(),
            email: "manager-one@example.com".to_owned(),
            phone: "0912345001".to_owned(),
            password_hash: "hash".to_owned(),
            role: "manager".to_owned(),
            affiliate_id: None,
        })
        .expect("seed credential");
    let app = internal_members_router(store.clone());

    let updated = request_json(
        app.clone(),
        "PATCH",
        "/v1/internal/admin/members/MAN-001",
        Some(json!({
            "display_name": "Manager Updated",
            "reason": "profile correction",
            "authorization_basis": "ticket-123"
        })),
        admin_headers(),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK);
    assert_eq!(updated.body["member"]["display_name"], "Manager Updated");

    let deleted = request_json(
        app.clone(),
        "DELETE",
        "/v1/internal/admin/members/MAN-001",
        None,
        admin_headers(),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK);

    let listed = request_json(
        app,
        "GET",
        "/v1/internal/admin/role-accounts?role=manager",
        None,
        admin_headers(),
    )
    .await;
    assert_eq!(listed.status, StatusCode::OK);
    assert_eq!(listed.body["accounts"][0]["disabled"], true);

    let account_audits = store.account_audit_logs();
    assert_eq!(account_audits.len(), 2);
    assert_eq!(account_audits[0].action, "update");
    assert_eq!(account_audits[0].reason, "profile correction");
    assert_eq!(account_audits[1].action, "disable");
}

#[tokio::test]
async fn actor_cannot_update_own_role_and_denied_audit_is_recorded() {
    let store = InMemoryMemberStore::new(vec![fixture_record("admin-1", None)]);
    let app = internal_members_router(store.clone());

    let response = request_json(
        app,
        "PATCH",
        "/v1/internal/admin/members/admin-1",
        Some(json!({ "role": "manager" })),
        admin_headers(),
    )
    .await;

    assert_eq!(response.status, StatusCode::FORBIDDEN);
    let denied_audits = store.denied_audit_logs();
    assert_eq!(denied_audits.len(), 1);
    assert_eq!(denied_audits[0].target_user_id.as_deref(), Some("admin-1"));
    assert_eq!(
        denied_audits[0].reason_or_error_code,
        "ERROR_CODE_MEMBER_ACCOUNT_SELF_ROLE_UPDATE_FORBIDDEN"
    );
}

#[tokio::test]
async fn admin_can_list_manager_role_accounts() {
    let store = InMemoryMemberStore::default();
    store
        .seed_login_credential(user_service::members::MemberLoginCredentialUpsert {
            user_id: "MAN-001".to_owned(),
            email: "manager-one@example.com".to_owned(),
            phone: "0912345001".to_owned(),
            password_hash: "hash".to_owned(),
            role: "manager".to_owned(),
            affiliate_id: None,
        })
        .expect("seed credential");
    store
        .seed_login_credential(user_service::members::MemberLoginCredentialUpsert {
            user_id: "AFF-001".to_owned(),
            email: "affiliate-one@example.com".to_owned(),
            phone: "0912345002".to_owned(),
            password_hash: "hash".to_owned(),
            role: "affiliate".to_owned(),
            affiliate_id: Some("AFF-001".to_owned()),
        })
        .expect("seed credential");
    let app = internal_members_router(store);

    let response = request_json(
        app,
        "GET",
        "/v1/internal/admin/role-accounts?role=manager",
        None,
        admin_headers(),
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response.body["accounts"]
            .as_array()
            .expect("accounts")
            .len(),
        1
    );
    assert_eq!(response.body["accounts"][0]["user_id"], "MAN-001");
    assert_eq!(response.body["accounts"][0]["role"], "manager");
}

#[tokio::test]
async fn list_members_can_scope_to_affiliate() {
    let app = internal_members_router(InMemoryMemberStore::new(vec![
        fixture_record("MEM-001", Some("AFF-001")),
        fixture_record("MEM-002", Some("AFF-002")),
        fixture_record("MEM-003", None),
    ]));

    let response = request_json(
        app,
        "GET",
        "/v1/internal/admin/members?affiliate_id=AFF-001",
        None,
        Vec::new(),
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response.body["members"].as_array().expect("members").len(),
        1
    );
    assert_eq!(response.body["members"][0]["member"]["user_id"], "MEM-001");
    assert_eq!(response.body["source"], "user-service:member-store");
}

#[tokio::test]
async fn contact_lookup_reads_masked_contact_from_member_store() {
    let app = internal_members_router(InMemoryMemberStore::new(vec![fixture_record(
        "MEM-001", None,
    )]));

    let response = request_json(
        app,
        "POST",
        "/v1/internal/users/contacts:lookup-masked",
        Some(json!({ "ids": ["MEM-001"] })),
        Vec::new(),
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["contacts"][0]["id"], "MEM-001");
    assert_eq!(
        response.body["contacts"][0]["email_masked"],
        "m***@example.com"
    );
    assert_eq!(response.body["contacts"][0]["phone_masked"], "0912***001");
}

#[tokio::test]
async fn public_registration_creates_inactive_member_with_referral_audit() {
    let app = internal_members_router(InMemoryMemberStore::default());

    let response = request_json(
        app,
        "POST",
        "/v1/public/registrations",
        Some(json!({
            "email": " New.Member@Example.COM ",
            "phone": "0912-345-698",
            "password": "correct horse battery staple",
            "display_name": " New Member ",
            "referral_code": "AFF_2026"
        })),
        Vec::new(),
    )
    .await;

    assert_eq!(response.status, StatusCode::CREATED);
    assert!(
        response.body["user_id"]
            .as_str()
            .expect("user_id should be returned")
            .starts_with("MEM-")
    );
    assert_eq!(response.body["member"]["member"]["status"], "inactive");
    assert_eq!(
        response.body["member"]["member"]["display_name"],
        "New Member"
    );
    assert_eq!(
        response.body["member"]["member"]["contact"]["email_masked"],
        "n***@example.com"
    );
    assert_eq!(
        response.body["member"]["audit"]["registration_audit"]["source"],
        "public-web-signup"
    );
    assert_eq!(
        response.body["member"]["audit"]["registration_audit"]["referral_code"],
        "AFF_2026"
    );
    assert_eq!(
        response.body["member"]["audit"]["registration_audit"]["referred_user_id"],
        response.body["user_id"]
    );
}

#[tokio::test]
async fn public_registration_rejects_invalid_input_before_member_create() {
    let app = internal_members_router(InMemoryMemberStore::default());

    let response = request_json(
        app,
        "POST",
        "/v1/public/registrations",
        Some(json!({
            "email": "not-an-email",
            "phone": "0812345698",
            "password": "short",
            "display_name": "",
            "referral_code": "AFF_2026"
        })),
        Vec::new(),
    )
    .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        response.body["error"],
        json!("ERROR_CODE_INVALID_REGISTRATION")
    );
}

#[tokio::test]
async fn postgres_member_store_persists_crud_soft_delete_and_constraints() {
    let Some(pool) = postgres_test_pool().await else {
        return;
    };
    let store = PostgresMemberStore::new(pool.clone());
    let user_id = unique_test_id("MEM-PG");
    cleanup_member_rows(&pool, &user_id).await;

    let created = store
        .create_member(user_service::members::CreateMemberRequest {
            user_id: user_id.clone(),
            display_name: "Postgres Member".to_owned(),
            email: format!("{}@example.com", user_id.to_lowercase()),
            phone: "0912345900".to_owned(),
            line_id: Some("pg-line".to_owned()),
            country: "tw".to_owned(),
            timezone: "Asia/Taipei".to_owned(),
            tier: "Standard".to_owned(),
            affiliate_id: Some("AFF-PG".to_owned()),
            registration_audit: None,
            role: None,
            password: None,
            reason: None,
            authorization_basis: None,
        })
        .await
        .unwrap_or_else(|error| panic!("member insert should persist to postgres: {error}"));

    assert_eq!(created.member.user_id, user_id);
    assert_eq!(created.member.status, "inactive");
    assert_eq!(created.kyc_risk.restrictions, vec!["kyc_pending"]);
    assert_eq!(
        persisted_status(&pool, &user_id).await,
        Some("inactive".to_owned())
    );

    let duplicate = store
        .create_member(user_service::members::CreateMemberRequest {
            user_id: user_id.clone(),
            display_name: "Duplicate".to_owned(),
            email: format!("dupe-{}@example.com", user_id.to_lowercase()),
            phone: "0912345999".to_owned(),
            line_id: None,
            country: "TW".to_owned(),
            timezone: "Asia/Taipei".to_owned(),
            tier: "Standard".to_owned(),
            affiliate_id: None,
            registration_audit: None,
            role: None,
            password: None,
            reason: None,
            authorization_basis: None,
        })
        .await;
    assert_eq!(duplicate, Err(MemberStoreError::Duplicate));

    let updated = store
        .update_member(
            &user_id,
            user_service::members::UpdateMemberRequest {
                status: None,
                display_name: Some("Postgres Member Verified".to_owned()),
                email: None,
                phone: None,
                line_id: Some(None),
                country: None,
                timezone: None,
                tier: None,
                affiliate_id: None,
                kyc_status: Some("verified".to_owned()),
                risk_level: Some("low".to_owned()),
                risk_score: Some(9),
                restrictions: None,
                role: None,
                password: None,
                reason: None,
                authorization_basis: None,
            },
        )
        .await
        .unwrap_or_else(|error| panic!("member update should persist to postgres: {error}"));

    assert_eq!(updated.member.display_name, "Postgres Member Verified");
    assert_eq!(updated.member.status, "active");
    assert_eq!(updated.kyc_risk.kyc_status, "verified");
    assert!(updated.kyc_risk.restrictions.is_empty());
    assert_eq!(
        persisted_status(&pool, &user_id).await,
        Some("active".to_owned())
    );

    let deleted = store
        .delete_member(&user_id)
        .await
        .unwrap_or_else(|error| panic!("member delete should soft-delete in postgres: {error}"));

    assert_eq!(deleted.user_id, user_id);
    assert_eq!(
        store.member_summary(&user_id).await,
        Err(MemberStoreError::NotFound)
    );
    assert!(persisted_deleted_at(&pool, &user_id).await.is_some());
}

#[derive(Debug)]
struct TestResponse {
    status: StatusCode,
    body: Value,
}

async fn request_json(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    headers: Vec<(&'static str, &'static str)>,
) -> TestResponse {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    for (name, value) in headers {
        builder = builder.header(name, value);
    }
    let response = app
        .oneshot(
            builder
                .body(Body::from(
                    body.map_or_else(String::new, |body| body.to_string()),
                ))
                .expect("request builds"),
        )
        .await
        .expect("request handled");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let body = serde_json::from_slice(&bytes).expect("json body");
    TestResponse { status, body }
}

fn admin_headers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("x-actor-id", "admin-1"),
        ("x-operator-role", "admin"),
        ("x-request-id", "req-admin"),
    ]
}

fn manager_headers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("x-actor-id", "manager-1"),
        ("x-operator-role", "manager"),
        ("x-request-id", "req-manager"),
    ]
}

fn fixture_record(user_id: &str, affiliate_id: Option<&str>) -> MemberRecord {
    MemberRecord {
        user_id: user_id.to_owned(),
        display_name: format!("Member {user_id}"),
        status: "inactive".to_owned(),
        tier: "Standard".to_owned(),
        email: format!("member-{user_id}@example.com").to_lowercase(),
        phone: format!("091234{}", &user_id[user_id.len() - 3..]),
        line_id: None,
        country: "TW".to_owned(),
        timezone: "Asia/Taipei".to_owned(),
        affiliate_id: affiliate_id.map(str::to_owned),
        kyc_status: "pending".to_owned(),
        risk_level: "medium".to_owned(),
        risk_score: 50,
        restrictions: vec!["kyc_pending".to_owned()],
        last_reviewed_at: None,
        last_login_at: None,
        device_count: 0,
        two_factor_enabled: false,
        created_at: "2026-05-15T00:00:00Z".to_owned(),
        registration_audit: None,
    }
}

async fn postgres_test_pool() -> Option<PgPool> {
    let Some(database_url) = std::env::var("USER_SERVICE_TEST_DATABASE_URL").ok() else {
        eprintln!("skipping postgres member store test: USER_SERVICE_TEST_DATABASE_URL is not set");
        return None;
    };
    let bootstrap_pool = PgPool::connect(&database_url)
        .await
        .unwrap_or_else(|error| panic!("test database should be reachable: {error}"));
    let schema = unique_test_schema("user_service_members");
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
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap_or_else(|error| panic!("user-service migrations should run: {error}"));
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
