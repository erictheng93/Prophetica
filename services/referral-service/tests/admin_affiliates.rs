#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use referral_service::{
    AffiliateCreditProfile, AffiliateDetail, AffiliateSummary, InMemoryAffiliateDashboardStore,
    router,
};
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn admin_affiliate_crud_mutates_with_audit_context() {
    let store = InMemoryAffiliateDashboardStore::default().with_admin_affiliates(vec![sample()]);
    let app = router(store);

    let created = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/internal/admin/affiliates",
            &json!({
                "id": "AFF-200",
                "userId": "user_aff_200",
                "displayName": "Orion Partners",
                "emailMasked": "o***@orion.partners",
                "status": "onboarding",
                "commissionRateBps": 600,
                "tier": 1,
                "creditRating": "restricted",
                "creditStatus": "pending_review",
                "reason": "signed B2B onboarding packet",
                "authorizationBasis": "SPEC-011 §4.7"
            }),
        ))
        .await
        .expect("create should be handled");
    assert_eq!(created.status(), StatusCode::CREATED);
    let body = response_json(created).await;
    assert_eq!(body["affiliate"]["id"], "AFF-200");
    assert_eq!(body["affiliate"]["displayName"], "Orion Partners");
    assert_eq!(
        body["auditCorrelationId"],
        "audit:affiliate-create:AFF-200:req-1"
    );

    let updated = app
        .clone()
        .oneshot(json_request(
            "PATCH",
            "/internal/admin/affiliates/AFF-200",
            &json!({
                "displayName": "Orion Growth Partners",
                "commissionRateBps": 700,
                "creditRating": "B",
                "creditStatus": "active",
                "maxActiveCreditAccounts": 4,
                "totalCreditLimitMinorUnits": 500_000,
                "perAccountLimitMinorUnits": 125_000,
                "reason": "KYB approved and contract amended",
                "authorizationBasis": "admin + compliance approval",
                "effectiveAt": "2026-05-15"
            }),
        ))
        .await
        .expect("update should be handled");
    assert_eq!(updated.status(), StatusCode::OK);
    let body = response_json(updated).await;
    assert_eq!(body["affiliate"]["displayName"], "Orion Growth Partners");
    assert_eq!(body["affiliate"]["commissionRateBps"], 700);
    assert_eq!(body["affiliate"]["creditProfile"]["rating"], "B");
    assert_eq!(body["affiliate"]["rateHistory"][0]["previousBps"], 600);

    let archived = app
        .clone()
        .oneshot(json_request(
            "DELETE",
            "/internal/admin/affiliates/AFF-200",
            &json!({
                "reason": "offboarding requested by legal",
                "authorizationBasis": "B2B termination notice"
            }),
        ))
        .await
        .expect("archive should be handled");
    assert_eq!(archived.status(), StatusCode::OK);
    let body = response_json(archived).await;
    assert_eq!(body["affiliate"]["status"], "archived");
    assert_eq!(body["affiliate"]["creditProfile"]["status"], "closed");

    let list = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/internal/admin/affiliates")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("list should be handled");
    let body = response_json(list).await;
    assert_eq!(body.as_array().expect("list is array").len(), 1);
    assert_eq!(body[0]["id"], "AFF-100");
}

#[tokio::test]
async fn manager_cannot_mutate_credit_controls() {
    let store = InMemoryAffiliateDashboardStore::default().with_admin_affiliates(vec![sample()]);
    let app = router(store);

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/internal/admin/affiliates/AFF-100")
                .header("x-actor-id", "mgr-1")
                .header("x-operator-role", "manager")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "creditRating": "A",
                        "reason": "manager proposal",
                        "authorizationBasis": "manager review"
                    })
                    .to_string(),
                ))
                .expect("request builds"),
        )
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(response).await["error"],
        "affiliate_forbidden"
    );
}

fn json_request(method: &str, uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("x-actor-id", "admin-1")
        .header("x-operator-role", "admin")
        .header("x-request-id", "req-1")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request builds")
}

async fn response_json(response: axum::response::Response) -> Value {
    let collected = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect");
    serde_json::from_slice(&collected.to_bytes()).expect("response should be json")
}

fn sample() -> AffiliateDetail {
    AffiliateDetail {
        summary: AffiliateSummary {
            id: "AFF-100".to_owned(),
            user_id: Some("user_aff_100".to_owned()),
            display_name: "Existing Partners".to_owned(),
            email_masked: "e***@partners.test".to_owned(),
            status: "active".to_owned(),
            owner_sponsor_id: None,
            commission_rate_bps: 500,
            tier: 1,
            sub_tree_size: 0,
            active_sub_count: 0,
            pending_payout_minor: 0,
            pending_payout_currency: "USDT".to_owned(),
            last_activity_at: "2026-05-15T00:00:00+08:00".to_owned(),
            risk_flags: Vec::new(),
        },
        sub_tree_breakdown: Vec::new(),
        recent_payouts: Vec::new(),
        rate_history: Vec::new(),
        audit_correlation_ids: Vec::new(),
        credit_profile: AffiliateCreditProfile {
            rating: "C".to_owned(),
            status: "active".to_owned(),
            max_active_credit_accounts: 1,
            active_credit_accounts: 0,
            total_credit_limit_minor_units: 100_000,
            per_account_limit_minor_units: 100_000,
            review_expires_at: None,
            last_changed_by: "admin-0".to_owned(),
            last_changed_at: "2026-05-15T00:00:00+08:00".to_owned(),
            audit_correlation_ids: Vec::new(),
            accounts: Vec::new(),
        },
    }
}
