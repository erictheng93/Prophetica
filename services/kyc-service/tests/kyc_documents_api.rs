#![allow(clippy::unwrap_used, clippy::expect_used)]

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use kyc_service::{
    SecretConfig,
    api::router_with_sumsub_client,
    kyc::{InMemoryKycStore, KycStore, StartUpgradeDraft, KycTier},
    sumsub_documents::{SumsubDocument, SumsubDocumentClient},
};
use serde_json::Value;
use tower::ServiceExt;
use wiremock::{MockServer, Mock, ResponseTemplate, matchers::{method, path}};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn test_store() -> InMemoryKycStore {
    InMemoryKycStore::new("test-salt")
}

fn test_router_with_server<S: KycStore>(store: S, sumsub_base_url: &str) -> Router {
    let client =
        SumsubDocumentClient::new(sumsub_base_url.to_owned(), "app-token".to_owned(), "secret".to_owned());
    router_with_sumsub_client(store, SecretConfig::dev_default(), client)
}

async fn seed_tier2_user(store: &InMemoryKycStore, user_id: &str) -> String {
    let outcome = store
        .start_upgrade(StartUpgradeDraft {
            user_id: user_id.to_owned(),
            target_tier: KycTier::Tier2,
            idempotency_key: format!("kyc:{user_id}:tier2:001"),
            declared_monthly_income: None,
        })
        .await
        .expect("start_upgrade should succeed");
    outcome
        .record
        .provider_applicant_id
        .expect("Tier-2 record should have applicant_id")
}

fn request_as_role(uri: &str, role: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("x-actor-id", "reviewer_1")
        .header("x-operator-role", role)
        .header("x-request-id", "req-test-001")
        .body(Body::empty())
        .unwrap()
}

fn admin_get(uri: &str) -> Request<Body> {
    request_as_role(uri, "admin")
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).expect("response should be JSON")
}

async fn collect_bytes(response: axum::response::Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

fn mount_doc_list(applicant_id: &str, documents: &[SumsubDocument]) -> ResponseTemplate {
    let body = serde_json::json!({
        "applicant_id": applicant_id,
        "documents": documents,
    });
    ResponseTemplate::new(200).set_body_json(body)
}

fn sample_doc(doc_id: &str) -> SumsubDocument {
    SumsubDocument {
        doc_id: doc_id.to_owned(),
        kind: "id_doc".to_owned(),
        country: Some("TWN".to_owned()),
        side: Some("front".to_owned()),
        content_type: "image/png".to_owned(),
        captured_at: "2026-05-19T00:00:00Z".to_owned(),
    }
}

// ---------------------------------------------------------------------------
// Scenario 1: user with Tier-2 record whose `provider_applicant_id` is Some
//             → list returns 200 + document array from SumSub
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_documents_returns_doc_array_for_tier2_user() {
    let server = MockServer::start().await;
    let store = test_store();
    let applicant_id = seed_tier2_user(&store, "user_docs_1").await;

    let doc = sample_doc("doc_abc");
    Mock::given(method("GET"))
        .and(path(format!(
            "/mock/sumsub/applicants/{applicant_id}/documents"
        )))
        .respond_with(mount_doc_list(&applicant_id, &[doc.clone()]))
        .mount(&server)
        .await;

    let app = test_router_with_server(store, &server.uri());
    let response = app
        .oneshot(admin_get(
            "/v1/internal/admin/kyc/members/user_docs_1/documents",
        ))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    let items = body.as_array().expect("body should be array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["doc_id"], "doc_abc");
}

// ---------------------------------------------------------------------------
// Scenario 2: image endpoint with valid (user_id, doc_id) → 200 + image/png bytes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_document_image_returns_png_bytes_for_valid_doc() {
    let server = MockServer::start().await;
    let store = test_store();
    let applicant_id = seed_tier2_user(&store, "user_docs_2").await;
    let png_bytes: &[u8] = b"\x89PNG\r\n\x1a\nFAKE";

    // Mount list endpoint (needed for doc-binding check)
    let doc = sample_doc("doc_img_1");
    Mock::given(method("GET"))
        .and(path(format!(
            "/mock/sumsub/applicants/{applicant_id}/documents"
        )))
        .respond_with(mount_doc_list(&applicant_id, &[doc]))
        .mount(&server)
        .await;

    // Mount image endpoint
    Mock::given(method("GET"))
        .and(path(format!(
            "/mock/sumsub/applicants/{applicant_id}/documents/doc_img_1/image"
        )))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(png_bytes)
                .append_header("content-type", "image/png"),
        )
        .mount(&server)
        .await;

    let app = test_router_with_server(store, &server.uri());
    let response = app
        .oneshot(admin_get(
            "/v1/internal/admin/kyc/members/user_docs_2/documents/doc_img_1",
        ))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "image/png"
    );
    assert_eq!(
        response.headers().get("cache-control").unwrap(),
        "private, no-store"
    );
    let body = collect_bytes(response).await;
    assert_eq!(body.as_slice(), png_bytes);
}

// ---------------------------------------------------------------------------
// Scenario 3: doc_id NOT belonging to the user's applicant → 404 identical
//             to scenario 4's "no documents" 404 (must not leak whether doc exists)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_document_unbound_doc_id_returns_404_identical_to_no_applicant() {
    let server = MockServer::start().await;
    let store = test_store();
    let applicant_id = seed_tier2_user(&store, "user_docs_3").await;

    // The user has doc_real but we request doc_fake
    let doc = sample_doc("doc_real");
    Mock::given(method("GET"))
        .and(path(format!(
            "/mock/sumsub/applicants/{applicant_id}/documents"
        )))
        .respond_with(mount_doc_list(&applicant_id, &[doc]))
        .mount(&server)
        .await;

    let app = test_router_with_server(store, &server.uri());
    let bound_response = app
        .oneshot(admin_get(
            "/v1/internal/admin/kyc/members/user_docs_3/documents/doc_fake",
        ))
        .await
        .expect("request should be handled");

    assert_eq!(bound_response.status(), StatusCode::NOT_FOUND);
    let bound_bytes = collect_bytes(bound_response).await;

    // Compare with a user that has no KYC record at all
    let store_empty = test_store();
    let app2 = test_router_with_server(store_empty, &server.uri());
    let no_applicant_response = app2
        .oneshot(admin_get(
            "/v1/internal/admin/kyc/members/unknown_user/documents/doc_fake",
        ))
        .await
        .expect("request should be handled");

    assert_eq!(no_applicant_response.status(), StatusCode::NOT_FOUND);
    let no_applicant_bytes = collect_bytes(no_applicant_response).await;

    assert_eq!(
        bound_bytes, no_applicant_bytes,
        "404 body must be byte-identical regardless of whether doc exists"
    );
}

// ---------------------------------------------------------------------------
// Scenario 4: user with no KYC record / no provider_applicant_id
//             → list returns empty []; image endpoint returns 404
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_kyc_record_list_returns_empty_and_image_returns_404() {
    let server = MockServer::start().await;
    let store = test_store();

    let app = test_router_with_server(store, &server.uri());

    // List
    let list_response = app
        .clone()
        .oneshot(admin_get(
            "/v1/internal/admin/kyc/members/ghost_user/documents",
        ))
        .await
        .expect("list request should be handled");
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = response_json(list_response).await;
    assert_eq!(list_body.as_array().expect("should be array").len(), 0);

    // Image
    let image_response = app
        .oneshot(admin_get(
            "/v1/internal/admin/kyc/members/ghost_user/documents/any_doc",
        ))
        .await
        .expect("image request should be handled");
    assert_eq!(image_response.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Scenario 5: after a successful list + view, kyc_document_access_log contains
//             rows with result='success'
// ---------------------------------------------------------------------------

#[tokio::test]
async fn successful_list_and_view_write_success_audit_rows() {
    let server = MockServer::start().await;
    let store = test_store();
    let applicant_id = seed_tier2_user(&store, "user_audit_5").await;
    let png_bytes: &[u8] = b"\x89PNG\r\n";

    // Mount list endpoint (used twice: once for list call, once for binding check in view)
    Mock::given(method("GET"))
        .and(path(format!(
            "/mock/sumsub/applicants/{applicant_id}/documents"
        )))
        .respond_with(mount_doc_list(&applicant_id, &[sample_doc("doc_audit")]))
        .expect(2)
        .mount(&server)
        .await;

    // Mount image endpoint
    Mock::given(method("GET"))
        .and(path(format!(
            "/mock/sumsub/applicants/{applicant_id}/documents/doc_audit/image"
        )))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(png_bytes)
                .append_header("content-type", "image/png"),
        )
        .mount(&server)
        .await;

    // List documents
    let list_resp = test_router_with_server(store.clone(), &server.uri())
        .oneshot(admin_get(
            "/v1/internal/admin/kyc/members/user_audit_5/documents",
        ))
        .await
        .expect("list request should be handled");
    assert_eq!(list_resp.status(), StatusCode::OK);
    let _ = collect_bytes(list_resp).await;

    // View document image
    let view_resp = test_router_with_server(store.clone(), &server.uri())
        .oneshot(admin_get(
            "/v1/internal/admin/kyc/members/user_audit_5/documents/doc_audit",
        ))
        .await
        .expect("view request should be handled");
    assert_eq!(view_resp.status(), StatusCode::OK);
    let _ = collect_bytes(view_resp).await;

    // Inspect audit log
    let log = store
        .document_access_log
        .lock()
        .expect("audit log lock should succeed");
    let success_rows: Vec<_> = log.iter().filter(|e| e.result == "success").collect();
    assert!(
        success_rows.len() >= 2,
        "expected at least 2 success rows, got {}: {:?}",
        success_rows.len(),
        success_rows.iter().map(|e| &e.action).collect::<Vec<_>>()
    );
    let list_row = success_rows
        .iter()
        .find(|e| e.action == "list")
        .expect("should have a list success row");
    assert_eq!(list_row.result, "success");
    let view_row = success_rows
        .iter()
        .find(|e| e.action == "view")
        .expect("should have a view success row");
    assert_eq!(view_row.result, "success");
}

// ---------------------------------------------------------------------------
// Scenario 6: a doc-binding failure writes a row with result='denied'
//             and a denial_reason
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unbound_doc_writes_denied_audit_row_with_denial_reason() {
    let server = MockServer::start().await;
    let store = test_store();
    let applicant_id = seed_tier2_user(&store, "user_audit_6").await;

    // Applicant has doc_real, but we request doc_unrelated
    Mock::given(method("GET"))
        .and(path(format!(
            "/mock/sumsub/applicants/{applicant_id}/documents"
        )))
        .respond_with(mount_doc_list(&applicant_id, &[sample_doc("doc_real")]))
        .mount(&server)
        .await;

    let resp = test_router_with_server(store.clone(), &server.uri())
        .oneshot(admin_get(
            "/v1/internal/admin/kyc/members/user_audit_6/documents/doc_unrelated",
        ))
        .await
        .expect("request should be handled");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let _ = collect_bytes(resp).await;

    let log = store
        .document_access_log
        .lock()
        .expect("audit log lock should succeed");
    let denied = log
        .iter()
        .find(|e| e.result == "denied")
        .expect("should have a denied row");
    assert_eq!(
        denied.denial_reason.as_deref(),
        Some("doc_not_bound"),
        "denial_reason should be doc_not_bound"
    );
}

// ---------------------------------------------------------------------------
// Scenario 7: a non-reviewer role is rejected with 403 and writes a denied
//             audit row (SPEC-005 §5.3.2 / §8 — backend role enforcement).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn non_reviewer_role_is_forbidden_and_writes_denied_audit_row() {
    let server = MockServer::start().await;
    let store = test_store();

    let resp = test_router_with_server(store.clone(), &server.uri())
        .oneshot(request_as_role(
            "/v1/internal/admin/kyc/members/user_role_7/documents",
            "support",
        ))
        .await
        .expect("request should be handled");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let _ = collect_bytes(resp).await;

    let log = store
        .document_access_log
        .lock()
        .expect("audit log lock should succeed");
    let denied = log
        .iter()
        .find(|e| e.result == "denied")
        .expect("should have a denied row");
    assert_eq!(denied.denial_reason.as_deref(), Some("role_not_permitted"));
    assert_eq!(denied.action, "list");
}
