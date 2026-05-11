use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{OriginalUri, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use hmac::{Hmac, Mac};
use reqwest::Url;
use serde::Serialize;
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone)]
pub struct AnalyticsProxyConfig {
    upstream: Url,
    secret: String,
    now_override: Option<i64>,
}

impl AnalyticsProxyConfig {
    /// Creates analytics reporting proxy configuration.
    ///
    /// # Errors
    ///
    /// Returns an error when `upstream` is not a valid URL.
    pub fn new(upstream: &str, secret: String, now_override: Option<i64>) -> anyhow::Result<Self> {
        Ok(Self {
            upstream: Url::parse(upstream)?,
            secret,
            now_override,
        })
    }

    /// Creates analytics reporting proxy configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns an error when required environment variables are missing or when
    /// `ANALYTICS_REPORTING_SERVICE_URL` is not a valid URL.
    pub fn from_env() -> anyhow::Result<Self> {
        let upstream = std::env::var("ANALYTICS_REPORTING_SERVICE_URL")?;
        Self::new(&upstream, std::env::var("INTERNAL_CALL_HMAC_SECRET")?, None)
    }

    fn now(&self) -> i64 {
        self.now_override.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| i64::try_from(duration.as_secs()).unwrap_or(0))
        })
    }
}

pub fn proxy_router(config: AnalyticsProxyConfig) -> Router {
    analytics_routes().with_state(Some(config))
}

pub fn unconfigured_router() -> Router {
    analytics_routes().with_state(None::<AnalyticsProxyConfig>)
}

fn analytics_routes() -> Router<Option<AnalyticsProxyConfig>> {
    Router::new()
        .route("/api/internal/admin/report-templates", get(proxy))
        .route("/api/internal/admin/report-jobs", get(proxy).post(proxy))
        .route(
            "/api/internal/admin/report-jobs/latest-per-template",
            get(proxy),
        )
        .route("/api/internal/admin/report-jobs/:id", get(proxy))
        .route(
            "/api/internal/admin/report-jobs/:id/exports/:format",
            get(proxy),
        )
        .route(
            "/api/internal/admin/report-jobs/:id/export-deletions",
            post(proxy),
        )
        .route("/api/internal/admin/report-schedules", get(proxy))
        .route("/api/internal/admin/dashboard-metrics", get(proxy))
}

async fn proxy(
    State(config): State<Option<AnalyticsProxyConfig>>,
    method: Method,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    body: Bytes,
) -> Response {
    let Some(config) = config else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "analytics_reporting_unconfigured",
        );
    };

    let path = uri.path();
    let actor_id = header_value(&headers, "x-actor-id").unwrap_or_default();
    let role = header_value(&headers, "x-operator-role").unwrap_or_default();
    let timestamp = header_value(&headers, "x-internal-call-timestamp")
        .and_then(|value| value.parse::<i64>().ok());
    let Some(timestamp) = timestamp else {
        return error(StatusCode::UNAUTHORIZED, "invalid_internal_call_signature");
    };
    if (config.now() - timestamp).abs() > 300 {
        return error(StatusCode::UNAUTHORIZED, "invalid_internal_call_signature");
    }

    let Some(signature) = header_value(&headers, "x-internal-call-signature") else {
        return error(StatusCode::UNAUTHORIZED, "invalid_internal_call_signature");
    };
    let expected = sign_internal_call(
        method.as_str(),
        path,
        &body,
        timestamp,
        &actor_id,
        &role,
        &config.secret,
    );
    if !constant_time_eq(signature.as_bytes(), expected.as_bytes()) {
        return error(StatusCode::UNAUTHORIZED, "invalid_internal_call_signature");
    }

    if role != "admin" && role != "manager" {
        return error(StatusCode::FORBIDDEN, "analytics_reporting_forbidden");
    }

    let Some(upstream_path) = upstream_path(&uri) else {
        return error(StatusCode::BAD_REQUEST, "invalid_analytics_reporting_path");
    };
    let Ok(upstream_url) = config.upstream.join(&upstream_path) else {
        return error(
            StatusCode::BAD_GATEWAY,
            "analytics_reporting_upstream_invalid",
        );
    };

    let client = reqwest::Client::new();
    let mut request = client
        .request(method.clone(), upstream_url)
        .body(body.to_vec())
        .header("x-actor-id", actor_id)
        .header("x-operator-role", role);
    if let Some(request_id) = header_value(&headers, "x-request-id") {
        request = request.header("x-request-id", request_id);
    }
    if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
        request = request.header(header::CONTENT_TYPE, content_type.clone());
    }

    let upstream_response = match request.send().await {
        Ok(response) => response,
        Err(request_error) => {
            tracing::warn!(%request_error, "analytics reporting upstream request failed");
            return error(
                StatusCode::BAD_GATEWAY,
                "analytics_reporting_upstream_failed",
            );
        }
    };
    let status = upstream_response.status();
    let content_type = upstream_response
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned();
    let content_disposition = upstream_response
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .cloned();
    let bytes = match upstream_response.bytes().await {
        Ok(bytes) => bytes,
        Err(body_error) => {
            tracing::warn!(%body_error, "analytics reporting upstream body failed");
            return error(
                StatusCode::BAD_GATEWAY,
                "analytics_reporting_upstream_failed",
            );
        }
    };

    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = status;
    if let Some(value) = content_type {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    if let Some(value) = content_disposition {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

fn upstream_path(uri: &axum::http::Uri) -> Option<String> {
    let suffix = uri.path().strip_prefix("/api/internal/admin")?;
    let query = uri
        .query()
        .map_or(String::new(), |query| format!("?{query}"));
    Some(format!("/v1{suffix}{query}"))
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn sign_internal_call(
    method: &str,
    path: &str,
    body: &[u8],
    timestamp: i64,
    actor_id: &str,
    role: &str,
    secret: &str,
) -> String {
    let body_hash = sha256_hex(body);
    let canonical = format!("{method}\n{path}\n{body_hash}\n{timestamp}\n{actor_id}\n{role}");
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return String::new();
    };
    mac.update(canonical.as_bytes());
    hex_lower(&mac.finalize().into_bytes())
}

#[must_use]
pub fn sign_internal_call_for_test(
    method: &str,
    path: &str,
    body: &str,
    timestamp: i64,
    actor_id: &str,
    role: &str,
    secret: &str,
) -> String {
    sign_internal_call(
        method,
        path,
        body.as_bytes(),
        timestamp,
        actor_id,
        role,
        secret,
    )
}

fn sha256_hex(body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body);
    hex_lower(&hasher.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

fn error(status: StatusCode, code: &'static str) -> Response {
    #[derive(Serialize)]
    struct Body {
        error: &'static str,
    }

    let mut response = axum::Json(Body { error: code }).into_response();
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
}
