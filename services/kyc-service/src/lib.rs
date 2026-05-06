pub const SERVICE_NAME: &str = "kyc-service";
pub const DEFAULT_PORT: u16 = 8085;

pub mod api;
pub mod kyc;

#[cfg(test)]
mod api_contract_tests {
    #![allow(clippy::expect_used)]

    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[tokio::test]
    async fn status_returns_tier_zero_for_unknown_user() {
        let app = crate::api::router(crate::kyc::InMemoryKycStore::default());

        let response = app
            .oneshot(empty_get("/api/v1/kyc/users/user_01/status"))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["user_id"], json!("user_01"));
        assert_eq!(body["current_tier"], json!("TIER_0"));
        assert_eq!(body["status"], json!("pending"));
        assert_eq!(body["declared_monthly_income"], Value::Null);
    }

    #[tokio::test]
    async fn start_upgrade_creates_pending_record_and_returns_sdk_token() {
        let store = crate::kyc::InMemoryKycStore::default();
        let app = crate::api::router(store.clone());

        let response = app
            .oneshot(json_post(
                "/api/v1/kyc/users/user_01/upgrades",
                &json!({
                    "target_tier": "TIER_2",
                    "idempotency_key": "kyc:user_01:tier2:001",
                    "declared_monthly_income": 6_000_000
                }),
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response_json(response).await;
        assert_eq!(body["user_id"], json!("user_01"));
        assert_eq!(body["target_tier"], json!("TIER_2"));
        assert_eq!(body["provider"], json!("sumsub"));
        assert_eq!(body["replayed"], json!(false));
        assert!(
            body["sdk_token"]
                .as_str()
                .is_some_and(|token| { token.starts_with("sumsub_sandbox_user_01_TIER_2_") })
        );

        let status = crate::api::router(store)
            .oneshot(empty_get("/api/v1/kyc/users/user_01/status"))
            .await
            .expect("status request should be handled");
        let status_body = response_json(status).await;
        assert_eq!(status_body["current_tier"], json!("TIER_2"));
        assert_eq!(status_body["status"], json!("pending"));
        assert_eq!(status_body["declared_monthly_income"], json!(6_000_000));
    }

    #[tokio::test]
    async fn tier_one_verification_hashes_document_id_and_marks_user_passed() {
        let store = crate::kyc::InMemoryKycStore::default();
        let app = crate::api::router(store.clone());

        let response = app
            .oneshot(json_post(
                "/api/v1/kyc/users/user_02/tier-1",
                &json!({
                    "idempotency_key": "kyc:user_02:tier1:001",
                    "legal_name": "Test User",
                    "birthday": "1990-01-02",
                    "country": "TW",
                    "document_id": "A123456789",
                    "consent": true
                }),
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response_json(response).await;
        assert_eq!(body["user_id"], json!("user_02"));
        assert_eq!(body["target_tier"], json!("TIER_1"));
        assert_eq!(body["status"], json!("passed"));
        assert_eq!(body["provider"], json!("manual"));
        assert_ne!(body["document_id_hash"], json!("A123456789"));
        assert_eq!(body["document_id_hash"].as_str().map(str::len), Some(64));

        let status = crate::api::router(store)
            .oneshot(empty_get("/api/v1/kyc/users/user_02/status"))
            .await
            .expect("status request should be handled");
        let status_body = response_json(status).await;
        assert_eq!(status_body["current_tier"], json!("TIER_1"));
        assert_eq!(status_body["status"], json!("passed"));
    }

    #[tokio::test]
    async fn sumsub_webhook_requires_hmac_and_updates_pending_upgrade() {
        let store = crate::kyc::InMemoryKycStore::default();
        let app = crate::api::router(store.clone());
        let upgrade = app
            .oneshot(json_post(
                "/api/v1/kyc/users/user_03/upgrades",
                &json!({
                    "target_tier": "TIER_2",
                    "idempotency_key": "kyc:user_03:tier2:001"
                }),
            ))
            .await
            .expect("upgrade request should be handled");
        let upgrade_body = response_json(upgrade).await;
        let applicant_id = match upgrade_body["provider_applicant_id"].as_str() {
            Some(applicant_id) => applicant_id.to_owned(),
            None => panic!("applicant id should be present"),
        };
        let webhook_body = json!({
            "applicantId": applicant_id,
            "externalUserId": "user_03",
            "type": "applicantReviewed",
            "reviewResult": { "reviewAnswer": "GREEN" },
            "kycReason": "",
            "sanctionMatches": []
        });

        let rejected = crate::api::router(store.clone())
            .oneshot(json_post("/api/internal/webhooks/sumsub", &webhook_body))
            .await
            .expect("webhook request should be handled");
        assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);

        let accepted = crate::api::router(store.clone())
            .oneshot(json_post_with_header(
                "/api/internal/webhooks/sumsub",
                &webhook_body,
                "x-payload-digest",
                &sumsub_digest(&webhook_body),
            ))
            .await
            .expect("signed webhook request should be handled");
        assert_eq!(accepted.status(), StatusCode::OK);
        let accepted_body = response_json(accepted).await;
        assert_eq!(accepted_body["status"], json!("passed"));
        assert_eq!(accepted_body["replayed"], json!(false));

        let status = crate::api::router(store)
            .oneshot(empty_get("/api/v1/kyc/users/user_03/status"))
            .await
            .expect("status request should be handled");
        let status_body = response_json(status).await;
        assert_eq!(status_body["current_tier"], json!("TIER_2"));
        assert_eq!(status_body["status"], json!("passed"));
    }

    #[tokio::test]
    async fn aml_event_creates_sar_candidate_for_high_risk_activity() {
        let response = crate::api::router(crate::kyc::InMemoryKycStore::default())
            .oneshot(json_post(
                "/internal/aml/events",
                &json!({
                    "user_id": "user_04",
                    "event_type": "rapid_deposit_then_withdraw",
                    "amount_minor_units": 1_250_000,
                    "ml_score": 72,
                    "rule_flags": ["velocity_check", "geo_anomaly"]
                }),
            ))
            .await
            .expect("aml request should be handled");

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response_json(response).await;
        assert_eq!(body["user_id"], json!("user_04"));
        assert_eq!(body["requires_compliance_review"], json!(true));
        assert_eq!(body["sar_candidate_id"], json!("sar_user_04_1"));
        assert_eq!(body["reason"], json!("rule_flags"));
    }

    #[tokio::test]
    async fn monthly_report_returns_kyc_volume_and_open_sar_counts() {
        let store = crate::kyc::InMemoryKycStore::default();
        let app = crate::api::router(store.clone());
        let _ = app
            .oneshot(json_post(
                "/api/v1/kyc/users/user_05/tier-1",
                &json!({
                    "idempotency_key": "kyc:user_05:tier1:001",
                    "legal_name": "Test User",
                    "birthday": "1990-01-02",
                    "country": "TW",
                    "document_id": "A123456789",
                    "consent": true
                }),
            ))
            .await
            .expect("tier one request should be handled");
        let _ = crate::api::router(store.clone())
            .oneshot(json_post(
                "/internal/aml/events",
                &json!({
                    "user_id": "user_05",
                    "event_type": "structured_deposits",
                    "amount_minor_units": 2_000_000,
                    "ml_score": 91,
                    "rule_flags": []
                }),
            ))
            .await
            .expect("aml request should be handled");

        let report = crate::api::router(store)
            .oneshot(empty_get("/internal/reports/kyc/monthly?month=2026-05"))
            .await
            .expect("report request should be handled");

        assert_eq!(report.status(), StatusCode::OK);
        let body = response_json(report).await;
        assert_eq!(body["month"], json!("2026-05"));
        assert_eq!(body["total_records"], json!(1));
        assert_eq!(body["by_tier"]["TIER_1"], json!(1));
        assert_eq!(body["by_status"]["passed"], json!(1));
        assert_eq!(body["open_sar_candidates"], json!(1));
    }

    fn json_post(uri: &str, body: &Value) -> Request<Body> {
        match Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
        {
            Ok(request) => request,
            Err(error) => panic!("request should build: {error}"),
        }
    }

    fn json_post_with_header(
        uri: &str,
        body: &Value,
        header_name: &str,
        header_value: &str,
    ) -> Request<Body> {
        match Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .header(header_name, header_value)
            .body(Body::from(body.to_string()))
        {
            Ok(request) => request,
            Err(error) => panic!("request should build: {error}"),
        }
    }

    fn empty_get(uri: &str) -> Request<Body> {
        match Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
        {
            Ok(request) => request,
            Err(error) => panic!("request should build: {error}"),
        }
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let collected = match response.into_body().collect().await {
            Ok(collected) => collected,
            Err(error) => panic!("response body should collect: {error}"),
        };
        match serde_json::from_slice(&collected.to_bytes()) {
            Ok(body) => body,
            Err(error) => panic!("response should be json: {error}"),
        }
    }

    fn sumsub_digest(body: &Value) -> String {
        use hmac::{Hmac, Mac};
        use sha1::Sha1;

        let mut mac = match Hmac::<Sha1>::new_from_slice(b"dev-sumsub-webhook-secret") {
            Ok(mac) => mac,
            Err(error) => panic!("test hmac key should initialize: {error}"),
        };
        mac.update(body.to_string().as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}
