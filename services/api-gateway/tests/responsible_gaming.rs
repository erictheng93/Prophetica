#![allow(clippy::expect_used)]

use api_gateway::{
    AuthVerifier,
    responsible_gaming::{InMemoryCooldownStore, ResponsibleGamingClock, cooldown_router},
};
use axum::{
    Extension,
    body::Body,
    http::{Request, StatusCode},
};
use ed25519_dalek::{Signer, SigningKey};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const NOW: i64 = 1_800_000_000;

#[tokio::test]
async fn cooldown_status_starts_available_for_authenticated_user() {
    let app = cooldown_router(
        InMemoryCooldownStore::default(),
        ResponsibleGamingClock::fixed(NOW),
    )
    .layer(test_auth_extension());

    let response = app
        .oneshot(empty_get("/v1/me/responsible-gaming/cooldown"))
        .await
        .expect("cooldown status request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["user_id"], json!("user_42"));
    assert_eq!(body["active"], json!(false));
    assert_eq!(body["duration"], json!(null));
    assert_eq!(body["remaining_seconds"], json!(0));
    assert_eq!(body["available_durations"], json!(["24h", "7d", "30d"]));
}

#[tokio::test]
async fn cooldown_start_records_fixed_duration_and_remaining_seconds() {
    let store = InMemoryCooldownStore::default();
    let app =
        cooldown_router(store, ResponsibleGamingClock::fixed(NOW)).layer(test_auth_extension());

    let response = app
        .oneshot(json_post(
            "/v1/me/responsible-gaming/cooldown",
            &json!({
                "duration": "7d",
                "consent": true
            }),
        ))
        .await
        .expect("cooldown start request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["user_id"], json!("user_42"));
    assert_eq!(body["active"], json!(true));
    assert_eq!(body["duration"], json!("7d"));
    assert_eq!(body["started_at_epoch_seconds"], json!(NOW));
    assert_eq!(body["ends_at_epoch_seconds"], json!(NOW + 7 * 24 * 60 * 60));
    assert_eq!(body["remaining_seconds"], json!(7 * 24 * 60 * 60));
}

#[tokio::test]
async fn cooldown_start_requires_explicit_consent() {
    let app = cooldown_router(
        InMemoryCooldownStore::default(),
        ResponsibleGamingClock::fixed(NOW),
    )
    .layer(test_auth_extension());

    let response = app
        .oneshot(json_post(
            "/v1/me/responsible-gaming/cooldown",
            &json!({
                "duration": "24h",
                "consent": false
            }),
        ))
        .await
        .expect("cooldown start request should be handled");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["error"], json!("cooldown_consent_required"));
}

fn empty_get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("authorization", format!("Bearer {}", test_jwt("user_42")))
        .body(Body::empty())
        .unwrap_or_else(|error| panic!("request should build: {error}"))
}

fn json_post(uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {}", test_jwt("user_42")))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|error| panic!("request should build: {error}"))
}

fn test_auth_extension() -> Extension<AuthVerifier> {
    Extension(
        AuthVerifier::from_jwks_json(&test_jwks_json(), true)
            .unwrap_or_else(|error| panic!("test JWKS should parse: {error}")),
    )
}

fn test_jwks_json() -> String {
    let key = test_signing_key();
    json!({
        "keys": [{
            "kty": "OKP",
            "crv": "Ed25519",
            "kid": "test-key",
            "alg": "EdDSA",
            "use": "sig",
            "x": base64_url_encode(&key.verifying_key().to_bytes())
        }]
    })
    .to_string()
}

fn test_jwt(subject: &str) -> String {
    let key = test_signing_key();
    let header = base64_url_encode(
        json!({
            "alg": "EdDSA",
            "kid": "test-key",
            "typ": "JWT"
        })
        .to_string()
        .as_bytes(),
    );
    let payload = base64_url_encode(
        json!({
            "sub": subject,
            "role": "user",
            "iat": NOW,
            "exp": 1_900_000_000_u64
        })
        .to_string()
        .as_bytes(),
    );
    let signing_input = format!("{header}.{payload}");
    let signature = key.sign(signing_input.as_bytes());
    format!(
        "{signing_input}.{}",
        base64_url_encode(&signature.to_bytes())
    )
}

fn test_signing_key() -> SigningKey {
    SigningKey::from_bytes(&[7; 32])
}

fn base64_url_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    let mut index = 0;
    while index < bytes.len() {
        let b0 = bytes[index];
        let b1 = bytes.get(index + 1).copied();
        let b2 = bytes.get(index + 2).copied();
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(
            TABLE[(((b0 & 0b0000_0011) << 4) | (b1.unwrap_or_default() >> 4)) as usize] as char,
        );
        if let Some(b1) = b1 {
            out.push(
                TABLE[(((b1 & 0b0000_1111) << 2) | (b2.unwrap_or_default() >> 6)) as usize] as char,
            );
        }
        if let Some(b2) = b2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        }
        index += 3;
    }
    out
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
