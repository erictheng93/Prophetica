#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use user_service::members::{InMemoryMemberStore, MemberRecord, internal_members_router};

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
            "affiliate_id": "AFF-001"
        })),
    )
    .await;

    assert_eq!(created.status, StatusCode::CREATED);
    assert_eq!(created.body["member"]["user_id"], "MEM-900");
    assert_eq!(created.body["member"]["status"], "inactive");
    assert_eq!(created.body["kyc_risk"]["kyc_status"], "pending");
    assert_eq!(created.body["source"]["affiliate_id"], "AFF-001");

    let updated = request_json(
        app.clone(),
        "PATCH",
        "/v1/internal/admin/members/MEM-900",
        Some(json!({ "kyc_status": "verified", "risk_level": "low", "risk_score": 10 })),
    )
    .await;

    assert_eq!(updated.status, StatusCode::OK);
    assert_eq!(updated.body["member"]["status"], "active");
    assert_eq!(updated.body["kyc_risk"]["kyc_status"], "verified");
    assert_eq!(updated.body["kyc_risk"]["restrictions"], json!([]));
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
) -> TestResponse {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
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
    }
}
