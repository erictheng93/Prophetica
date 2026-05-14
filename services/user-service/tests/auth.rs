#![allow(clippy::expect_used)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use user_service::auth::{AuthConfig, LocalLoginAccount, auth_router_with_config};

#[tokio::test]
async fn login_accepts_email_or_canonical_taiwan_mobile() {
    let app = auth_router_with_config(config_with_fixture());

    let email_response = post_login(
        app.clone(),
        json!({
            "identifier": " MEMBER@example.COM ",
            "identifier_kind": "email",
            "password": "member",
            "remember": false
        }),
    )
    .await;
    let phone_response = post_login(
        app,
        json!({
            "identifier": "+886 912-345-698",
            "identifier_kind": "phone",
            "password": "member",
            "remember": true
        }),
    )
    .await;

    assert_eq!(email_response.status, StatusCode::OK);
    assert_eq!(email_response.body["user_id"], json!("user_42"));
    assert_eq!(email_response.body["role"], json!("manager"));
    assert_eq!(phone_response.status, StatusCode::OK);
    assert_eq!(phone_response.body["user_id"], json!("user_42"));
    assert!(phone_response.body["session_token"].as_str().is_some());
}

#[tokio::test]
async fn login_accepts_mobile_with_or_without_leading_zero() {
    let app = auth_router_with_config(config_with_fixture());

    let with_zero = post_login(
        app.clone(),
        json!({
            "identifier": "0912345698",
            "identifier_kind": "phone",
            "password": "member"
        }),
    )
    .await;
    let without_zero = post_login(
        app,
        json!({
            "identifier": "912345698",
            "identifier_kind": "phone",
            "password": "member"
        }),
    )
    .await;

    assert_eq!(with_zero.status, StatusCode::OK);
    assert_eq!(without_zero.status, StatusCode::OK);
}

#[tokio::test]
async fn login_rejects_wrong_password_or_identifier_kind() {
    let app = auth_router_with_config(config_with_fixture());

    let wrong_password = post_login(
        app.clone(),
        json!({
            "identifier": "member@example.com",
            "identifier_kind": "email",
            "password": "wrong"
        }),
    )
    .await;
    let wrong_kind = post_login(
        app,
        json!({
            "identifier": "0912345698",
            "identifier_kind": "email",
            "password": "member"
        }),
    )
    .await;

    assert_eq!(wrong_password.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        wrong_password.body["error"],
        json!("ERROR_CODE_INVALID_CREDENTIALS")
    );
    assert_eq!(wrong_kind.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_fails_closed_when_no_auth_provider_is_configured() {
    let app = auth_router_with_config(AuthConfig::new(Vec::new()));

    let response = post_login(
        app,
        json!({
            "identifier": "member@example.com",
            "identifier_kind": "email",
            "password": "member"
        }),
    )
    .await;

    assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response.body["error"],
        json!("ERROR_CODE_AUTH_PROVIDER_UNAVAILABLE")
    );
}

#[derive(Debug)]
struct LoginTestResponse {
    status: StatusCode,
    body: Value,
}

fn config_with_fixture() -> AuthConfig {
    let account = LocalLoginAccount::new("user_42", "member@example.com", "0912345698", "member")
        .expect("fixture account is valid")
        .with_profile("manager", None);

    AuthConfig::new(vec![account])
}

async fn post_login(app: axum::Router, body: Value) -> LoginTestResponse {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
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
    let body = serde_json::from_slice(&bytes).expect("json response body");

    LoginTestResponse { status, body }
}
