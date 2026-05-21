pub const SERVICE_NAME: &str = "kyc-service";
pub const DEFAULT_PORT: u16 = 8085;

pub mod api;
pub mod internal_auth;
pub mod internal_reports;
pub mod kyc;
pub mod sumsub_documents;

#[derive(Debug, Clone)]
pub struct SecretConfig {
    pub webhook_secret: String,
    pub document_hash_salt: String,
    pub internal_call_secret: String,
}

impl SecretConfig {
    /// Loads required KYC secrets from process environment.
    ///
    /// # Errors
    ///
    /// Returns an error when a required secret is missing outside dev/test.
    pub fn from_env(env: &str) -> anyhow::Result<Self> {
        let webhook_secret =
            read_secret_var(env, "SUMSUB_WEBHOOK_SECRET", "dev-sumsub-webhook-secret")?;
        let document_hash_salt =
            read_secret_var(env, "KYC_DOCUMENT_HASH_SALT", "dev-only-kyc-document-salt")?;
        let internal_call_secret = read_secret_var(
            env,
            "INTERNAL_CALL_HMAC_SECRET",
            "dev-analytics-internal-call-secret",
        )?;
        Ok(Self {
            webhook_secret,
            document_hash_salt,
            internal_call_secret,
        })
    }

    #[must_use]
    pub fn dev_default() -> Self {
        Self {
            webhook_secret: "dev-sumsub-webhook-secret".to_owned(),
            document_hash_salt: "dev-only-kyc-document-salt".to_owned(),
            internal_call_secret: "dev-analytics-internal-call-secret".to_owned(),
        }
    }
}

fn read_secret_var(env: &str, name: &str, dev_fallback: &str) -> anyhow::Result<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        _ if env == "dev" || env == "test" => {
            tracing::warn!(secret = %name, "using dev fallback secret; DO NOT use in production");
            Ok(dev_fallback.to_owned())
        }
        _ => anyhow::bail!(
            "{name} is required when PROPHETICA_ENV={env}; refusing to start with dev fallback"
        ),
    }
}

#[cfg(test)]
mod api_contract_tests {
    #![allow(clippy::expect_used)]

    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[tokio::test]
    async fn status_returns_tier_zero_for_unknown_user() {
        let app = test_router(test_store());

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
    async fn internal_tier_returns_tier_zero_for_unknown_user() {
        let app = test_router(test_store());

        let response = app
            .oneshot(signed_internal_tier_get("user_01"))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["tier"], json!("TIER_0"));
    }

    #[tokio::test]
    async fn internal_tier_rejects_unsigned_call() {
        let response = test_router(test_store())
            .oneshot(empty_get("/internal/kyc/users/user_01/tier"))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("unauthorized"));
    }

    #[tokio::test]
    async fn internal_tier_rejects_wrong_secret() {
        let response = test_router(test_store())
            .oneshot(signed_internal_tier_get_with_secret(
                "user_01",
                "wrong-secret",
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn internal_tier_returns_current_record_tier() {
        let store = test_store();
        let app = test_router(store.clone());

        let upgrade = app
            .oneshot(json_post(
                "/api/v1/kyc/users/user_01/upgrades",
                &json!({
                    "target_tier": "TIER_2",
                    "idempotency_key": "kyc:user_01:tier2:001"
                }),
            ))
            .await
            .expect("upgrade request should be handled");
        assert_eq!(upgrade.status(), StatusCode::CREATED);

        let response = test_router(store)
            .oneshot(signed_internal_tier_get("user_01"))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["tier"], json!("TIER_2"));
    }

    #[tokio::test]
    async fn start_upgrade_creates_pending_record_and_returns_sdk_token() {
        let store = test_store();
        let app = test_router(store.clone());

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

        let status = test_router(store)
            .oneshot(empty_get("/api/v1/kyc/users/user_01/status"))
            .await
            .expect("status request should be handled");
        let status_body = response_json(status).await;
        assert_eq!(status_body["current_tier"], json!("TIER_2"));
        assert_eq!(status_body["status"], json!("pending"));
        assert_eq!(status_body["declared_monthly_income"], json!(6_000_000));
    }

    #[tokio::test]
    async fn start_upgrade_rejects_mismatched_auth_subject() {
        let response = test_router(test_store())
            .oneshot(json_post_with_header(
                "/api/v1/kyc/users/victim/upgrades",
                &json!({
                    "target_tier": "TIER_2",
                    "idempotency_key": "kyc:victim:tier2:001"
                }),
                "x-user-id",
                "attacker",
            ))
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = response_json(response).await;
        assert_eq!(body["error"], json!("subject_mismatch"));
    }

    #[tokio::test]
    async fn start_upgrade_replays_same_idempotency_key_with_same_provider_session() {
        let store = test_store();
        let first = test_router(store.clone())
            .oneshot(json_post(
                "/api/v1/kyc/users/user_idem/upgrades",
                &json!({
                    "target_tier": "TIER_2",
                    "idempotency_key": "kyc:user_idem:TIER_2:tier1_record_1",
                    "declared_monthly_income": 6_000_000
                }),
            ))
            .await
            .expect("first upgrade request should be handled");
        assert_eq!(first.status(), StatusCode::CREATED);
        let first_body = response_json(first).await;

        let second = test_router(store.clone())
            .oneshot(json_post(
                "/api/v1/kyc/users/user_idem/upgrades",
                &json!({
                    "target_tier": "TIER_2",
                    "idempotency_key": "kyc:user_idem:TIER_2:tier1_record_1",
                    "declared_monthly_income": 6_000_000
                }),
            ))
            .await
            .expect("second upgrade request should be handled");
        assert_eq!(second.status(), StatusCode::OK);
        let second_body = response_json(second).await;

        assert_eq!(second_body["replayed"], json!(true));
        assert_eq!(second_body["sdk_token"], first_body["sdk_token"]);
        assert_eq!(
            second_body["provider_applicant_id"],
            first_body["provider_applicant_id"]
        );

        let status = test_router(store)
            .oneshot(empty_get("/api/v1/kyc/users/user_idem/status"))
            .await
            .expect("status request should be handled");
        let status_body = response_json(status).await;
        assert_eq!(
            status_body["provider_applicant_id"],
            first_body["provider_applicant_id"]
        );
    }

    #[tokio::test]
    async fn tier_one_verification_hashes_document_id_and_marks_user_passed() {
        let store = test_store();
        let app = test_router(store.clone());

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

        let status = test_router(store)
            .oneshot(empty_get("/api/v1/kyc/users/user_02/status"))
            .await
            .expect("status request should be handled");
        let status_body = response_json(status).await;
        assert_eq!(status_body["current_tier"], json!("TIER_1"));
        assert_eq!(status_body["status"], json!("passed"));
        assert_eq!(status_body["country"], json!("TW"));
        assert_eq!(status_body["tier1_record_id"], json!("1"));
    }

    #[tokio::test]
    async fn status_returns_tier_one_country_and_record_id_after_upgrade() {
        let store = crate::kyc::InMemoryKycStore::default();
        let app = test_router(store.clone());

        let tier_one = app
            .clone()
            .oneshot(json_post(
                "/api/v1/kyc/users/user_ctx/tier-1",
                &json!({
                    "idempotency_key": "kyc:user_ctx:tier1:001",
                    "legal_name": "Context User",
                    "birthday": "1990-01-02",
                    "country": "TW",
                    "document_id": "A123456789",
                    "consent": true
                }),
            ))
            .await
            .expect("tier one request should be handled");
        assert_eq!(tier_one.status(), StatusCode::CREATED);

        let upgrade = app
            .oneshot(json_post(
                "/api/v1/kyc/users/user_ctx/upgrades",
                &json!({
                    "target_tier": "TIER_2",
                    "idempotency_key": "kyc:user_ctx:TIER_2:1",
                    "declared_monthly_income": 6_000_000
                }),
            ))
            .await
            .expect("upgrade request should be handled");
        assert_eq!(upgrade.status(), StatusCode::CREATED);

        let status = test_router(store)
            .oneshot(empty_get("/api/v1/kyc/users/user_ctx/status"))
            .await
            .expect("status request should be handled");
        let status_body = response_json(status).await;
        assert_eq!(status_body["current_tier"], json!("TIER_2"));
        assert_eq!(status_body["country"], json!("TW"));
        assert_eq!(status_body["tier1_record_id"], json!("1"));
    }

    #[tokio::test]
    async fn tier_one_rejects_duplicate_document_hash_for_different_user() {
        let store = test_store();
        let first = test_router(store.clone())
            .oneshot(json_post(
                "/api/v1/kyc/users/user_dup_01/tier-1",
                &json!({
                    "idempotency_key": "kyc:user_dup_01:tier1:001",
                    "legal_name": "First User",
                    "birthday": "1990-01-02",
                    "country": "TW",
                    "document_id": "A123456789",
                    "consent": true
                }),
            ))
            .await
            .expect("first request should be handled");
        assert_eq!(first.status(), StatusCode::CREATED);

        let duplicate = test_router(store)
            .oneshot(json_post(
                "/api/v1/kyc/users/user_dup_02/tier-1",
                &json!({
                    "idempotency_key": "kyc:user_dup_02:tier1:001",
                    "legal_name": "Second User",
                    "birthday": "1990-01-02",
                    "country": "TW",
                    "document_id": "A123456789",
                    "consent": true
                }),
            ))
            .await
            .expect("duplicate request should be handled");

        assert_eq!(duplicate.status(), StatusCode::CONFLICT);
        let body = response_json(duplicate).await;
        assert_eq!(body["error"], json!("duplicate_document"));
    }

    #[tokio::test]
    async fn sumsub_webhook_requires_hmac_and_updates_pending_upgrade() {
        let store = test_store();
        let app = test_router(store.clone());
        let applicant_id = create_tier_two_upgrade(app.clone(), "user_03").await;
        let webhook_body = json!({
            "applicantId": applicant_id,
            "externalUserId": "user_03",
            "type": "applicantReviewed",
            "reviewResult": { "reviewAnswer": "GREEN" },
            "kycReason": "",
            "sanctionMatches": []
        });

        let rejected = test_router(store.clone())
            .oneshot(json_post("/api/internal/webhooks/sumsub", &webhook_body))
            .await
            .expect("webhook request should be handled");
        assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);

        let accepted = test_router(store.clone())
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

        let status = test_router(store)
            .oneshot(empty_get("/api/v1/kyc/users/user_03/status"))
            .await
            .expect("status request should be handled");
        let status_body = response_json(status).await;
        assert_eq!(status_body["current_tier"], json!("TIER_2"));
        assert_eq!(status_body["status"], json!("passed"));
    }

    #[tokio::test]
    async fn sumsub_webhook_red_without_sanction_freezes_user() {
        let store = test_store();
        let app = test_router(store.clone());
        let applicant_id = create_tier_two_upgrade(app, "user_red").await;
        let webhook_body = json!({
            "applicantId": applicant_id,
            "externalUserId": "user_red",
            "type": "applicantReviewed",
            "reviewResult": { "reviewAnswer": "RED" },
            "kycReason": "",
            "sanctionMatches": []
        });

        let accepted = test_router(store.clone())
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
        assert_eq!(accepted_body["status"], json!("frozen"));

        let status = test_router(store)
            .oneshot(empty_get("/api/v1/kyc/users/user_red/status"))
            .await
            .expect("status request should be handled");
        let status_body = response_json(status).await;
        assert_eq!(status_body["current_tier"], json!("TIER_2"));
        assert_eq!(status_body["status"], json!("frozen"));
    }

    #[tokio::test]
    async fn sumsub_webhook_yellow_keeps_user_pending() {
        let store = test_store();
        let app = test_router(store.clone());
        let applicant_id = create_tier_two_upgrade(app, "user_yellow").await;
        let webhook_body = json!({
            "applicantId": applicant_id,
            "externalUserId": "user_yellow",
            "type": "applicantReviewed",
            "reviewResult": { "reviewAnswer": "YELLOW" },
            "kycReason": "",
            "sanctionMatches": []
        });

        let accepted = test_router(store.clone())
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
        assert_eq!(accepted_body["status"], json!("pending"));

        let status = test_router(store)
            .oneshot(empty_get("/api/v1/kyc/users/user_yellow/status"))
            .await
            .expect("status request should be handled");
        let status_body = response_json(status).await;
        assert_eq!(status_body["current_tier"], json!("TIER_2"));
        assert_eq!(status_body["status"], json!("pending"));
    }

    #[tokio::test]
    async fn sumsub_webhook_replays_exact_raw_payload_hash_not_applicant_id() {
        let store = test_store();
        let app = test_router(store.clone());
        let applicant_id = create_tier_two_upgrade(app, "user_replay").await;
        let webhook_body = json!({
            "applicantId": applicant_id,
            "externalUserId": "user_replay",
            "type": "applicantReviewed",
            "reviewResult": { "reviewAnswer": "YELLOW" },
            "kycReason": "",
            "sanctionMatches": []
        });

        let first = test_router(store.clone())
            .oneshot(json_post_with_header(
                "/api/internal/webhooks/sumsub",
                &webhook_body,
                "x-payload-digest",
                &sumsub_digest(&webhook_body),
            ))
            .await
            .expect("first signed webhook request should be handled");
        assert_eq!(first.status(), StatusCode::OK);
        let first_body = response_json(first).await;
        assert_eq!(first_body["status"], json!("pending"));
        assert_eq!(first_body["replayed"], json!(false));

        let replay = test_router(store.clone())
            .oneshot(json_post_with_header(
                "/api/internal/webhooks/sumsub",
                &webhook_body,
                "x-payload-digest",
                &sumsub_digest(&webhook_body),
            ))
            .await
            .expect("replayed signed webhook request should be handled");
        assert_eq!(replay.status(), StatusCode::OK);
        let replay_body = response_json(replay).await;
        assert_eq!(replay_body["status"], json!("pending"));
        assert_eq!(replay_body["replayed"], json!(true));

        let changed_body_same_applicant = json!({
            "applicantId": first_body["provider_applicant_id"],
            "externalUserId": "user_replay",
            "type": "applicantReviewed",
            "reviewResult": { "reviewAnswer": "GREEN" },
            "kycReason": "",
            "sanctionMatches": []
        });
        let changed = test_router(store)
            .oneshot(json_post_with_header(
                "/api/internal/webhooks/sumsub",
                &changed_body_same_applicant,
                "x-payload-digest",
                &sumsub_digest(&changed_body_same_applicant),
            ))
            .await
            .expect("changed signed webhook request should be handled");
        assert_eq!(changed.status(), StatusCode::OK);
        let changed_body = response_json(changed).await;
        assert_eq!(changed_body["status"], json!("passed"));
        assert_eq!(changed_body["replayed"], json!(false));
    }

    #[tokio::test]
    async fn aml_event_creates_sar_candidate_for_high_risk_activity() {
        let response = test_router(test_store())
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
        let store = test_store();
        let app = test_router(store.clone());
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
        let _ = test_router(store.clone())
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

        let report = test_router(store)
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

    #[tokio::test]
    async fn sanctions_rescan_freezes_sanction_hits_and_routes_pep_to_tier_three() {
        let store = test_store();
        let _ = test_router(store.clone())
            .oneshot(json_post(
                "/api/v1/kyc/users/user_sdn/upgrades",
                &json!({
                    "target_tier": "TIER_2",
                    "idempotency_key": "kyc:user_sdn:tier2:001"
                }),
            ))
            .await
            .expect("sdn upgrade should be handled");
        let _ = test_router(store.clone())
            .oneshot(json_post(
                "/api/v1/kyc/users/user_pep/upgrades",
                &json!({
                    "target_tier": "TIER_2",
                    "idempotency_key": "kyc:user_pep:tier2:001"
                }),
            ))
            .await
            .expect("pep upgrade should be handled");

        let rescan = test_router(store.clone())
            .oneshot(json_post(
                "/internal/screening/rescans",
                &json!({
                    "matches": [
                        { "user_id": "user_sdn", "match_type": "sanction", "list_name": "OFAC SDN" },
                        { "user_id": "user_pep", "match_type": "pep", "list_name": "PEP" }
                    ]
                }),
            ))
            .await
            .expect("rescan request should be handled");

        assert_eq!(rescan.status(), StatusCode::OK);
        let body = response_json(rescan).await;
        assert_eq!(body["scanned_matches"], json!(2));
        assert_eq!(body["frozen_users"], json!(["user_sdn"]));
        assert_eq!(body["edd_users"], json!(["user_pep"]));

        let frozen_status = test_router(store.clone())
            .oneshot(empty_get("/api/v1/kyc/users/user_sdn/status"))
            .await
            .expect("frozen status request should be handled");
        assert_eq!(
            response_json(frozen_status).await["status"],
            json!("frozen")
        );

        let pep_status = test_router(store)
            .oneshot(empty_get("/api/v1/kyc/users/user_pep/status"))
            .await
            .expect("pep status request should be handled");
        let pep_body = response_json(pep_status).await;
        assert_eq!(pep_body["current_tier"], json!("TIER_3"));
        assert_eq!(pep_body["status"], json!("pending"));
    }

    fn json_post(uri: &str, body: &Value) -> Request<Body> {
        let mut builder = Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json");
        if let Some(user_id) = path_user_id(uri) {
            builder = builder.header("x-user-id", user_id);
        }
        match builder.body(Body::from(body.to_string())) {
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
        let mut builder = Request::builder().method("GET").uri(uri);
        if let Some(user_id) = path_user_id(uri) {
            builder = builder.header("x-user-id", user_id);
        }
        match builder.body(Body::empty()) {
            Ok(request) => request,
            Err(error) => panic!("request should build: {error}"),
        }
    }

    fn path_user_id(uri: &str) -> Option<&str> {
        uri.split_once("/users/")
            .and_then(|(_, rest)| rest.split('/').next())
            .filter(|user_id| !user_id.is_empty())
    }

    fn test_router<S>(store: S) -> Router
    where
        S: crate::kyc::KycStore,
    {
        crate::api::router(store, crate::SecretConfig::dev_default())
    }

    fn test_store() -> crate::kyc::InMemoryKycStore {
        let secrets = crate::SecretConfig::dev_default();
        crate::kyc::InMemoryKycStore::new(secrets.document_hash_salt)
    }

    async fn create_tier_two_upgrade(app: Router, user_id: &str) -> String {
        let upgrade = app
            .oneshot(json_post(
                &format!("/api/v1/kyc/users/{user_id}/upgrades"),
                &json!({
                    "target_tier": "TIER_2",
                    "idempotency_key": format!("kyc:{user_id}:tier2:001")
                }),
            ))
            .await
            .expect("upgrade request should be handled");
        let upgrade_body = response_json(upgrade).await;
        match upgrade_body["provider_applicant_id"].as_str() {
            Some(applicant_id) => applicant_id.to_owned(),
            None => panic!("applicant id should be present"),
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

    fn signed_internal_tier_get(user_id: &str) -> Request<Body> {
        signed_internal_tier_get_with_secret(
            user_id,
            &crate::SecretConfig::dev_default().internal_call_secret,
        )
    }

    fn signed_internal_tier_get_with_secret(user_id: &str, secret: &str) -> Request<Body> {
        let path = format!("/internal/kyc/users/{user_id}/tier");
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0_i64, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        let signature =
            crate::internal_auth::sign_internal_call("GET", &path, b"", timestamp, secret);
        match Request::builder()
            .method("GET")
            .uri(&path)
            .header(
                crate::internal_auth::INTERNAL_CALL_TIMESTAMP_HEADER,
                timestamp.to_string(),
            )
            .header(
                crate::internal_auth::INTERNAL_CALL_SIGNATURE_HEADER,
                signature,
            )
            .body(Body::empty())
        {
            Ok(request) => request,
            Err(error) => panic!("signed internal tier request should build: {error}"),
        }
    }
}
