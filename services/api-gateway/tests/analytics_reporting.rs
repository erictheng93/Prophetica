#![allow(clippy::expect_used)]

use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use api_gateway::analytics_reporting::{
    AnalyticsProxyConfig, proxy_router, sign_internal_call_for_test,
};
use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    response::IntoResponse,
    routing::get,
};
use serde_json::json;
use tokio::net::TcpListener;
use tower::ServiceExt;

const SECRET: &str = "shared-secret-32-bytes-1234567890";

#[tokio::test]
async fn valid_signature_forwards_to_reporting_service() {
    let upstream = TestUpstream::start().await;
    let app = proxy_router(config(upstream.url()));

    let response = app
        .oneshot(signed_request(
            "GET",
            "/api/internal/admin/report-templates",
            "",
            "admin",
            1_780_000_000,
        ))
        .await
        .expect("handled");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(upstream.hits(), 1);
    upstream.abort();
}

#[tokio::test]
async fn affiliate_role_returns_403_without_hitting_upstream() {
    let upstream = TestUpstream::start().await;
    let app = proxy_router(config(upstream.url()));

    let response = app
        .oneshot(signed_request(
            "GET",
            "/api/internal/admin/report-templates",
            "",
            "affiliate",
            1_780_000_000,
        ))
        .await
        .expect("handled");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(upstream.hits(), 0);
    upstream.abort();
}

#[tokio::test]
async fn missing_signature_returns_401() {
    let upstream = TestUpstream::start().await;
    let app = proxy_router(config(upstream.url()));
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/internal/admin/report-templates")
                .header("x-actor-id", "U-ADMIN-001")
                .header("x-operator-role", "admin")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await
        .expect("handled");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    upstream.abort();
}

#[tokio::test]
async fn tampered_signature_returns_401() {
    let upstream = TestUpstream::start().await;
    let app = proxy_router(config(upstream.url()));
    let mut request = signed_request(
        "GET",
        "/api/internal/admin/report-templates",
        "",
        "admin",
        1_780_000_000,
    );
    request
        .headers_mut()
        .insert("x-internal-call-signature", "bad".parse().expect("header"));

    let response = app.oneshot(request).await.expect("handled");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    upstream.abort();
}

#[tokio::test]
async fn timestamp_drift_returns_401() {
    let upstream = TestUpstream::start().await;
    let app = proxy_router(config(upstream.url()));

    let response = app
        .oneshot(signed_request(
            "GET",
            "/api/internal/admin/report-templates",
            "",
            "admin",
            1,
        ))
        .await
        .expect("handled");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    upstream.abort();
}

struct TestUpstream {
    addr: SocketAddr,
    hits: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl TestUpstream {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let hits = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/v1/report-templates", get(upstream_templates))
            .with_state(hits.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });

        Self { addr, hits, task }
    }

    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }

    fn abort(self) {
        self.task.abort();
    }
}

async fn upstream_templates(
    State(hits): State<Arc<AtomicUsize>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    assert_eq!(
        headers
            .get("x-actor-id")
            .and_then(|value| value.to_str().ok()),
        Some("U-ADMIN-001")
    );
    hits.fetch_add(1, Ordering::SeqCst);
    axum::Json(json!([{ "code": "ops-kpi-daily" }]))
}

fn config(upstream: String) -> AnalyticsProxyConfig {
    AnalyticsProxyConfig::new(upstream, SECRET.to_owned(), Some(1_780_000_000))
        .expect("config builds")
}

fn signed_request(
    method: &str,
    path: &str,
    body: &str,
    role: &str,
    timestamp: i64,
) -> Request<Body> {
    let signature =
        sign_internal_call_for_test(method, path, body, timestamp, "U-ADMIN-001", role, SECRET);
    Request::builder()
        .method(method)
        .uri(path)
        .header("x-actor-id", "U-ADMIN-001")
        .header("x-operator-role", role)
        .header("x-request-id", "corr-001")
        .header("x-internal-call-timestamp", timestamp.to_string())
        .header("x-internal-call-signature", signature)
        .body(Body::from(body.to_owned()))
        .expect("request builds")
}
