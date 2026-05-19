#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{SystemTime, UNIX_EPOCH};

use api_gateway::{
    admin_ops::{InMemoryAdminOpsStore, internal_admin_router},
    internal_call_auth::sign_internal_call,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const SECRET: &str = "test-kyc-proxy-secret-32-bytes!!";
const NOW: i64 = 1_780_000_000;

fn signed_headers(
    http_method: &str,
    request_path: &str,
    actor: &str,
    role: &str,
    secret: &str,
    ts: i64,
) -> Vec<(&'static str, String)> {
    let sig = sign_internal_call(http_method, request_path, b"", ts, actor, role, secret);
    vec![
        ("x-actor-id", actor.to_owned()),
        ("x-operator-role", role.to_owned()),
        ("x-request-id", "req-001".to_owned()),
        ("x-internal-call-timestamp", ts.to_string()),
        ("x-internal-call-signature", sig),
    ]
}

fn build_request(
    http_method: &str,
    uri: &str,
    headers: Vec<(&'static str, String)>,
) -> Request<Body> {
    let mut builder = Request::builder().method(http_method).uri(uri);
    for (name, value) in headers {
        builder = builder.header(name, value);
    }
    builder.body(Body::empty()).expect("request should build")
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(NOW, |d| i64::try_from(d.as_secs()).unwrap_or(NOW))
}

fn store_with_kyc_config(kyc_url: &str) -> InMemoryAdminOpsStore {
    InMemoryAdminOpsStore::default().with_kyc_proxy_config(SECRET, kyc_url)
}

// ─── Test 1: missing / invalid signed-call headers → 401 ──────────────────────

#[tokio::test]
async fn missing_signed_headers_returns_401_for_document_list() {
    let server = MockServer::start().await;
    let app = internal_admin_router(store_with_kyc_config(&server.uri()));
    let request = Request::builder()
        .method("GET")
        .uri("/api/internal/admin/members/user-1/kyc-documents")
        .body(Body::empty())
        .expect("builds");

    let response = app.oneshot(request).await.expect("handled");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn missing_signed_headers_returns_401_for_document_image() {
    let server = MockServer::start().await;
    let app = internal_admin_router(store_with_kyc_config(&server.uri()));
    let request = Request::builder()
        .method("GET")
        .uri("/api/internal/admin/members/user-1/kyc-documents/doc-1")
        .body(Body::empty())
        .expect("builds");

    let response = app.oneshot(request).await.expect("handled");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn invalid_signature_returns_401_for_document_list() {
    let server = MockServer::start().await;
    let ts = now_ts();
    let app = internal_admin_router(store_with_kyc_config(&server.uri()));
    let request = Request::builder()
        .method("GET")
        .uri("/api/internal/admin/members/user-1/kyc-documents")
        .header("x-actor-id", "U-ADMIN-1")
        .header("x-operator-role", "admin")
        .header("x-internal-call-timestamp", ts.to_string())
        .header("x-internal-call-signature", "deadbeef")
        .body(Body::empty())
        .expect("builds");

    let response = app.oneshot(request).await.expect("handled");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

// ─── Test 2: valid signed call + 200 image bytes passed through ────────────────

#[tokio::test]
async fn valid_signed_call_streams_image_bytes_and_content_type() {
    let server = MockServer::start().await;
    let ts = now_ts();

    let png_bytes: &[u8] = b"\x89PNG\r\n\x1a\n";
    Mock::given(method("GET"))
        .and(path(
            "/v1/internal/admin/kyc/members/user-1/documents/doc-1",
        ))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(png_bytes)
                .append_header("content-type", "image/png"),
        )
        .mount(&server)
        .await;

    let request_path = "/api/internal/admin/members/user-1/kyc-documents/doc-1";
    let headers = signed_headers("GET", request_path, "U-ADMIN-1", "admin", SECRET, ts);
    let request = build_request("GET", request_path, headers);

    let app = internal_admin_router(store_with_kyc_config(&server.uri()));
    let response = app.oneshot(request).await.expect("handled");

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("image/png"),
        "content-type should be image/png, got: {content_type}"
    );
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    assert_eq!(body_bytes.as_ref(), png_bytes);
}

#[tokio::test]
async fn valid_signed_call_streams_document_list_json() {
    let server = MockServer::start().await;
    let ts = now_ts();

    let doc_list = serde_json::json!({
        "documents": [{ "doc_id": "doc-1", "kind": "id_doc", "content_type": "image/png" }]
    });
    Mock::given(method("GET"))
        .and(path("/v1/internal/admin/kyc/members/user-1/documents"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(doc_list.clone())
                .append_header("content-type", "application/json"),
        )
        .mount(&server)
        .await;

    let request_path = "/api/internal/admin/members/user-1/kyc-documents";
    let headers = signed_headers("GET", request_path, "U-ADMIN-1", "admin", SECRET, ts);
    let request = build_request("GET", request_path, headers);

    let app = internal_admin_router(store_with_kyc_config(&server.uri()));
    let response = app.oneshot(request).await.expect("handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let body_json: serde_json::Value =
        serde_json::from_slice(&body_bytes).expect("valid JSON response");
    assert_eq!(body_json["documents"][0]["doc_id"], "doc-1");
}

// ─── Test 3: valid signed call + kyc-service 404 → gateway passes through 404 ─

#[tokio::test]
async fn kyc_service_404_is_passed_through_by_image_handler() {
    let server = MockServer::start().await;
    let ts = now_ts();

    Mock::given(method("GET"))
        .and(path(
            "/v1/internal/admin/kyc/members/user-1/documents/doc-missing",
        ))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let request_path = "/api/internal/admin/members/user-1/kyc-documents/doc-missing";
    let headers = signed_headers("GET", request_path, "U-ADMIN-1", "admin", SECRET, ts);
    let request = build_request("GET", request_path, headers);

    let app = internal_admin_router(store_with_kyc_config(&server.uri()));
    let response = app.oneshot(request).await.expect("handled");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn kyc_service_404_is_passed_through_by_list_handler() {
    let server = MockServer::start().await;
    let ts = now_ts();

    Mock::given(method("GET"))
        .and(path("/v1/internal/admin/kyc/members/no-such-user/documents"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let request_path = "/api/internal/admin/members/no-such-user/kyc-documents";
    let headers = signed_headers("GET", request_path, "U-ADMIN-1", "admin", SECRET, ts);
    let request = build_request("GET", request_path, headers);

    let app = internal_admin_router(store_with_kyc_config(&server.uri()));
    let response = app.oneshot(request).await.expect("handled");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
