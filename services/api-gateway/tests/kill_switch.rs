#![allow(clippy::expect_used)]

use api_gateway::kill_switch::{
    AuditLogMetadata, HaltFlagStore, InMemoryAuditLogStore, InMemoryHaltFlagStore,
    MultiSigDecision, MultiSigReleaseRequest, MultiSigRole, SignerApproval, internal_admin_router,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use sdk::feature_flags::EngineHaltFlagReader;
use serde_json::{Value, json};
use std::time::{Duration, SystemTime};
use tower::ServiceExt;

#[test]
fn halt_endpoint_sets_engine_halted_feature_flag() {
    let store = InMemoryHaltFlagStore::default();

    store.halt("ops incident").expect("halt should write flag");

    assert!(store.is_engine_halted());
    assert_eq!(store.reason().as_deref(), Some("ops incident"));
}

#[test]
fn single_signer_release_is_rejected_with_multisig_error() {
    let request = MultiSigReleaseRequest::new(
        "MS-9",
        vec![SignerApproval::hardware_mfa(
            "cto-1",
            MultiSigRole::Cto,
            SystemTime::UNIX_EPOCH + Duration::from_secs(60),
        )],
        SystemTime::UNIX_EPOCH + Duration::from_secs(120),
        "incident resolved",
    );

    let error = request
        .evaluate()
        .expect_err("single signer must not release halt");

    assert_eq!(error.code(), "ERROR_CODE_MULTISIG_REJECTED");
}

#[test]
fn same_user_double_signature_is_rejected() {
    let request = MultiSigReleaseRequest::new(
        "MS-9",
        vec![
            SignerApproval::hardware_mfa(
                "senior-1",
                MultiSigRole::SeniorEngineer,
                SystemTime::UNIX_EPOCH + Duration::from_secs(60),
            ),
            SignerApproval::hardware_mfa(
                "senior-1",
                MultiSigRole::Compliance,
                SystemTime::UNIX_EPOCH + Duration::from_secs(70),
            ),
        ],
        SystemTime::UNIX_EPOCH + Duration::from_secs(120),
        "incident resolved",
    );

    let error = request
        .evaluate()
        .expect_err("same user must not self-approve twice");

    assert_eq!(error.code(), "ERROR_CODE_MULTISIG_REJECTED");
}

#[test]
fn stale_or_non_hardware_mfa_signature_is_rejected() {
    let request = MultiSigReleaseRequest::new(
        "MS-9",
        vec![
            SignerApproval::hardware_mfa("cto-1", MultiSigRole::Cto, SystemTime::UNIX_EPOCH),
            SignerApproval::software_mfa(
                "compliance-1",
                MultiSigRole::Compliance,
                SystemTime::UNIX_EPOCH + Duration::from_secs(119),
            ),
        ],
        SystemTime::UNIX_EPOCH + Duration::from_secs(31 * 60),
        "incident resolved",
    );

    let error = request
        .evaluate()
        .expect_err("release must require hardware MFA within 30 minutes");

    assert_eq!(error.code(), "ERROR_CODE_MULTISIG_REJECTED");
}

#[test]
fn two_of_three_valid_ms9_release_writes_audit_metadata() {
    let request = MultiSigReleaseRequest::new(
        "MS-9",
        vec![
            SignerApproval::hardware_mfa(
                "cto-1",
                MultiSigRole::Cto,
                SystemTime::UNIX_EPOCH + Duration::from_secs(60),
            ),
            SignerApproval::hardware_mfa(
                "compliance-1",
                MultiSigRole::Compliance,
                SystemTime::UNIX_EPOCH + Duration::from_secs(90),
            ),
        ],
        SystemTime::UNIX_EPOCH + Duration::from_secs(120),
        "incident resolved",
    );

    let metadata = request
        .evaluate()
        .expect("2-of-3 valid hardware MFA release should pass");

    assert_eq!(
        metadata,
        AuditLogMetadata {
            ms_id: "MS-9".to_owned(),
            signers: vec!["cto-1".to_owned(), "compliance-1".to_owned()],
            decision: MultiSigDecision::Approved,
            reason: "incident resolved".to_owned(),
        }
    );
}

#[tokio::test]
async fn halt_route_sets_feature_flag() {
    let store = InMemoryHaltFlagStore::default();
    let app = internal_admin_router(store.clone(), InMemoryAuditLogStore::default());

    let response = app
        .oneshot(json_post(
            "/api/internal/admin/halt",
            &json!({ "reason": "ops incident" }),
        ))
        .await
        .expect("halt request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    assert!(store.is_engine_halted());
    assert_eq!(response_json(response).await["engine_halted"], json!(true));
}

#[tokio::test]
async fn release_route_rejects_single_signer() {
    let store = InMemoryHaltFlagStore::default();
    store.halt("ops incident").expect("halt should write flag");
    let audit_store = InMemoryAuditLogStore::default();
    let app = internal_admin_router(store.clone(), audit_store.clone());

    let response = app
        .oneshot(json_post(
            "/api/internal/admin/halt/release",
            &json!({
                "reason": "incident resolved",
                "evaluated_at_epoch_seconds": 120,
                "signers": [
                    {
                        "user_id": "cto-1",
                        "role": "CTO",
                        "hardware_mfa": true,
                        "mfa_verified_at_epoch_seconds": 60
                    }
                ]
            }),
        ))
        .await
        .expect("release request should be handled");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(store.is_engine_halted());
    assert!(audit_store.records().is_empty());
    assert_eq!(
        response_json(response).await["error"],
        json!("ERROR_CODE_MULTISIG_REJECTED")
    );
}

#[tokio::test]
async fn valid_release_route_clears_halt_flag_and_writes_audit_log() {
    let store = InMemoryHaltFlagStore::default();
    store.halt("incident-123").expect("halt should write flag");
    let audit_store = InMemoryAuditLogStore::default();
    let app = internal_admin_router(store.clone(), audit_store.clone());

    let response = app
        .oneshot(json_post(
            "/api/internal/admin/halt/release",
            &json!({
                "reason": "incident resolved",
                "evaluated_at_epoch_seconds": 120,
                "signers": [
                    {
                        "user_id": "cto-1",
                        "role": "CTO",
                        "hardware_mfa": true,
                        "mfa_verified_at_epoch_seconds": 60
                    },
                    {
                        "user_id": "compliance-1",
                        "role": "Compliance",
                        "hardware_mfa": true,
                        "mfa_verified_at_epoch_seconds": 90
                    }
                ]
            }),
        ))
        .await
        .expect("release request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    assert!(!store.is_engine_halted());

    let body = response_json(response).await;
    assert_eq!(body["engine_halted"], json!(false));
    assert_eq!(body["audit_log"]["action"], json!("kill_switch_release"));
    assert_eq!(body["audit_log"]["subject_id"], json!("incident-123"));
    assert_eq!(body["audit_log"]["metadata"]["ms_id"], json!("MS-9"));
    assert_eq!(body["audit_log"]["metadata"]["decision"], json!("Approved"));

    let records = audit_store.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].action, "kill_switch_release");
    assert_eq!(records[0].subject_id, "incident-123");
    assert_eq!(records[0].metadata.ms_id, "MS-9");
}

#[tokio::test]
async fn halt_and_release_routes_update_shared_feature_flag_store() {
    let store = InMemoryHaltFlagStore::default();
    let flags = store.feature_flags();
    let app = internal_admin_router(store.clone(), InMemoryAuditLogStore::default());

    let halt_response = app
        .clone()
        .oneshot(json_post(
            "/api/internal/admin/halt",
            &json!({ "reason": "incident-123" }),
        ))
        .await
        .expect("halt request should be handled");
    assert_eq!(halt_response.status(), StatusCode::OK);
    assert!(flags.engine_halt_snapshot().enabled);

    let release_response = app
        .oneshot(json_post(
            "/api/internal/admin/halt/release",
            &json!({
                "reason": "incident resolved",
                "evaluated_at_epoch_seconds": 120,
                "signers": [
                    {
                        "user_id": "cto-1",
                        "role": "CTO",
                        "hardware_mfa": true,
                        "mfa_verified_at_epoch_seconds": 60
                    },
                    {
                        "user_id": "compliance-1",
                        "role": "Compliance",
                        "hardware_mfa": true,
                        "mfa_verified_at_epoch_seconds": 90
                    }
                ]
            }),
        ))
        .await
        .expect("release request should be handled");

    assert_eq!(release_response.status(), StatusCode::OK);
    assert!(!flags.engine_halt_snapshot().enabled);
}

fn json_post(uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
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
