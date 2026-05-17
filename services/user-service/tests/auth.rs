#![allow(clippy::expect_used)]

use std::{net::IpAddr, sync::Arc, time::Duration};

use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version, password_hash::SaltString};
use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use hmac::{Hmac, Mac};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sha1::Sha1;
use tower::ServiceExt;
use user_service::auth::{
    AuthConfig, InMemoryPasswordCredentialStore, LocalLoginAccount, StoredLoginCredential,
    auth_router_with_config,
};
use user_service::login_events::{
    AdminLoginEventFilter, GeoLookupError, GeoLookupResult, InMemoryLoginEventOutbox,
    InMemoryLoginEventStore, LoginEventInput, LoginEventRecord, LoginEventRetentionResult,
    LoginEventStore, LoginEventStoreError, LoginGeoProvider,
};
use user_service::webauthn_runtime::WebauthnRuntimeConfig;
use user_service::webauthn_store::{
    InMemoryWebauthnStore, WebauthnCeremony, WebauthnChallengeRecord, WebauthnCredentialRecord,
    WebauthnStore,
};

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
async fn login_events_capture_success_failure_and_surface_identity() {
    let events = InMemoryLoginEventStore::default();
    let account = LocalLoginAccount::new(
        "U-AFF-LOGIN-EVENT",
        "affiliate-events@prophetica.local",
        "0912345615",
        "affiliate",
    )
    .expect("fixture account is valid")
    .with_profile("affiliate", Some("AFF-EVENTS".to_owned()));
    let app = auth_router_with_config(
        AuthConfig::new(vec![account]).with_login_event_store(Arc::new(events.clone())),
    );

    let success = post_login_with_headers(
        app.clone(),
        json!({
            "identifier": "affiliate-events@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate",
            "login_surface": "affiliate_portal"
        }),
        &[
            (
                "user-agent",
                "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) Safari/605.1",
            ),
            ("cf-connecting-ip", "203.0.113.45"),
            ("cf-ipcountry", "TW"),
            ("x-request-id", "req-login-event-success"),
        ],
    )
    .await;
    let failure = post_login_with_headers(
        app,
        json!({
            "identifier": "missing-admin@prophetica.local",
            "identifier_kind": "email",
            "password": "wrong",
            "login_surface": "admin_console"
        }),
        &[
            (
                "user-agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/125.0",
            ),
            ("x-forwarded-for", "198.51.100.22"),
            ("x-request-id", "req-login-event-failure"),
        ],
    )
    .await;

    assert_eq!(success.status, StatusCode::OK);
    assert_eq!(failure.status, StatusCode::UNAUTHORIZED);
    let stored = events.events();
    assert_eq!(stored.len(), 2);
    let success_event = stored
        .iter()
        .find(|event| event.event_type == "login_success")
        .expect("success event should be written");
    assert_eq!(success_event.login_surface, "affiliate_portal");
    assert_eq!(success_event.principal_type, "affiliate");
    assert_eq!(
        success_event.principal_id.as_deref(),
        Some("U-AFF-LOGIN-EVENT")
    );
    assert_eq!(
        success_event.principal_external_id.as_deref(),
        Some("AFF-EVENTS")
    );
    assert_eq!(success_event.risk_flags, json!([]));
    assert_eq!(success_event.geo_country.as_deref(), Some("TW"));
    assert_eq!(success_event.geo_accuracy, "approximate");

    let failure_event = stored
        .iter()
        .find(|event| event.event_type == "login_failure")
        .expect("failure event should be written");
    assert_eq!(failure_event.login_surface, "admin_console");
    assert_eq!(failure_event.principal_type, "admin");
    assert!(failure_event.principal_id.is_none());
    assert!(failure_event.account_identifier_hash.is_some());
    assert_eq!(
        failure_event.account_identifier_hash_key_version.as_deref(),
        Some("dev-v1")
    );
    assert_eq!(
        failure_event.failure_reason.as_deref(),
        Some("invalid_credentials")
    );
}

#[tokio::test]
async fn self_login_events_query_masks_ip_and_uses_unknown_fallbacks() {
    let events = InMemoryLoginEventStore::default();
    let app = auth_router_with_config(
        config_with_fixture().with_login_event_store(Arc::new(events.clone())),
    );
    let login = post_login_with_headers(
        app.clone(),
        json!({
            "identifier": "member@example.com",
            "identifier_kind": "email",
            "password": "member",
            "login_surface": "user_web"
        }),
        &[
            ("x-real-ip", "203.0.113.99"),
            ("user-agent", "not-a-real-browser"),
        ],
    )
    .await;
    let token = login.body["session_token"]
        .as_str()
        .expect("session token should be returned");

    let response =
        request_with_optional_bearer(app, "GET", "/v1/users/me/login-events", Some(token)).await;

    assert_eq!(response.status, StatusCode::OK);
    let first = &response.body["events"][0];
    assert_eq!(first["ip_masked"], json!("203.0.113.xxx"));
    assert_eq!(first["location_label"], json!("未知"));
    assert_eq!(first["device_label"], json!("未知"));
    assert!(first.get("ip_address").is_none());
    assert!(first.get("user_agent_raw").is_none());
}

#[tokio::test]
async fn admin_login_event_retention_supports_dry_run_and_delete() {
    let events = InMemoryLoginEventStore::default();
    let app = auth_router_with_config(
        config_with_fixture().with_login_event_store(Arc::new(events.clone())),
    );
    let login = post_login(
        app.clone(),
        json!({
            "identifier": "member@example.com",
            "identifier_kind": "email",
            "password": "member"
        }),
    )
    .await;
    let token = login.body["session_token"]
        .as_str()
        .expect("session token should be returned");
    let cutoff = 4_102_444_800_u64;

    let dry_run = request_with_optional_bearer_json(
        app.clone(),
        "POST",
        "/v1/internal/admin/login-events/retention",
        Some(token),
        json!({
            "older_than_epoch_seconds": cutoff,
            "dry_run": true
        }),
    )
    .await;
    let delete = request_with_optional_bearer_json(
        app,
        "POST",
        "/v1/internal/admin/login-events/retention",
        Some(token),
        json!({
            "older_than_epoch_seconds": cutoff,
            "dry_run": false
        }),
    )
    .await;

    assert_eq!(dry_run.status, StatusCode::OK);
    assert_eq!(dry_run.body["matched_events"], json!(1));
    assert_eq!(dry_run.body["deleted_events"], json!(0));
    assert_eq!(delete.status, StatusCode::OK);
    assert_eq!(delete.body["matched_events"], json!(1));
    assert_eq!(delete.body["deleted_events"], json!(1));
    assert!(events.events().is_empty());
}

#[tokio::test]
async fn admin_login_events_query_filters_by_principal_ip_event_and_time() {
    let events = InMemoryLoginEventStore::default();
    let accounts = AuthConfig::from_local_login_accounts_json(
        r#"[
          {
            "user_id": "U-MANAGER-FILTER",
            "email": "manager-filter@prophetica.local",
            "phone": "0912345616",
            "password": "manager",
            "role": "manager"
          },
          {
            "user_id": "U-AFF-FILTER",
            "email": "affiliate-filter@prophetica.local",
            "phone": "0912345617",
            "password": "affiliate",
            "role": "affiliate",
            "affiliate_id": "AFF-FILTER"
          }
        ]"#,
    )
    .expect("accounts parse");
    let app = auth_router_with_config(
        AuthConfig::new(accounts).with_login_event_store(Arc::new(events.clone())),
    );
    let manager_login = post_login_with_headers(
        app.clone(),
        json!({
            "identifier": "manager-filter@prophetica.local",
            "identifier_kind": "email",
            "password": "manager",
            "login_surface": "admin_console"
        }),
        &[("cf-connecting-ip", "203.0.113.11")],
    )
    .await;
    let token = manager_login.body["session_token"]
        .as_str()
        .expect("session token should be returned");
    let affiliate_login = post_login_with_headers(
        app.clone(),
        json!({
            "identifier": "affiliate-filter@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate",
            "login_surface": "affiliate_portal"
        }),
        &[("cf-connecting-ip", "203.0.113.12")],
    )
    .await;

    let filtered = request_with_optional_bearer(
        app,
        "GET",
        "/v1/internal/admin/login-events?principal_type=affiliate&event_type=login_success&ip_address=203.0.113.12&from_epoch_seconds=1&to_epoch_seconds=4102444800",
        Some(token),
    )
    .await;

    assert_eq!(manager_login.status, StatusCode::OK);
    assert_eq!(affiliate_login.status, StatusCode::OK);
    assert_eq!(filtered.status, StatusCode::OK);
    let events = filtered.body["events"].as_array().expect("events array");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["principal_type"], json!("affiliate"));
    assert_eq!(events[0]["principal_id"], json!("U-AFF-FILTER"));
    assert_eq!(events[0]["principal_external_id"], json!("AFF-FILTER"));
    assert_eq!(events[0]["ip_address"], json!("203.0.113.12"));
}

#[tokio::test]
async fn admin_login_event_detail_is_rbac_protected_and_audited() {
    let events = InMemoryLoginEventStore::default();
    let accounts = AuthConfig::from_local_login_accounts_json(
        r#"[
          {
            "user_id": "U-ADMIN-DETAIL",
            "email": "admin-detail@prophetica.local",
            "phone": "0912345618",
            "password": "admin",
            "role": "admin"
          },
          {
            "user_id": "U-PLAYER-DETAIL",
            "email": "player-detail@prophetica.local",
            "phone": "0912345619",
            "password": "player",
            "role": "player"
          }
        ]"#,
    )
    .expect("accounts parse");
    let app = auth_router_with_config(
        AuthConfig::new(accounts).with_login_event_store(Arc::new(events.clone())),
    );
    let admin_login = post_login(
        app.clone(),
        json!({
            "identifier": "admin-detail@prophetica.local",
            "identifier_kind": "email",
            "password": "admin",
            "login_surface": "admin_console"
        }),
    )
    .await;
    let player_login = post_login(
        app.clone(),
        json!({
            "identifier": "player-detail@prophetica.local",
            "identifier_kind": "email",
            "password": "player",
            "login_surface": "user_web"
        }),
    )
    .await;
    let admin_token = admin_login.body["session_token"]
        .as_str()
        .expect("admin session token should be returned");
    let player_token = player_login.body["session_token"]
        .as_str()
        .expect("player session token should be returned");
    let event_id = events
        .events()
        .iter()
        .find(|event| event.principal_id.as_deref() == Some("U-PLAYER-DETAIL"))
        .expect("player login event should be stored")
        .id
        .clone();

    let forbidden = request_with_optional_bearer(
        app.clone(),
        "GET",
        &format!("/v1/internal/admin/login-events/{event_id}"),
        Some(player_token),
    )
    .await;
    let detail = request_with_optional_bearer(
        app.clone(),
        "GET",
        &format!("/v1/internal/admin/login-events/{event_id}"),
        Some(admin_token),
    )
    .await;
    let metrics = request_with_optional_bearer(
        app,
        "GET",
        "/v1/internal/admin/login-events/metrics",
        Some(admin_token),
    )
    .await;

    assert_eq!(forbidden.status, StatusCode::FORBIDDEN);
    assert_eq!(detail.status, StatusCode::OK);
    assert_eq!(detail.body["event"]["id"], json!(event_id));
    assert_eq!(
        metrics.body["login_event_admin_detail_queries_total"],
        json!(1)
    );
}

#[tokio::test]
async fn login_event_metrics_count_writes_and_spoofed_headers() {
    let app = auth_router_with_config(
        config_with_fixture()
            .with_trusted_proxy_cidrs("10.0.0.0/8")
            .with_login_event_store(Arc::new(InMemoryLoginEventStore::default())),
    );
    let login = post_login_with_headers(
        app.clone(),
        json!({
            "identifier": "member@example.com",
            "identifier_kind": "email",
            "password": "member"
        }),
        &[("x-forwarded-for", "198.51.100.88")],
    )
    .await;
    let token = login.body["session_token"]
        .as_str()
        .expect("session token should be returned");

    let metrics = request_with_optional_bearer(
        app,
        "GET",
        "/v1/internal/admin/login-events/metrics",
        Some(token),
    )
    .await;

    assert_eq!(metrics.status, StatusCode::OK);
    assert_eq!(metrics.body["login_events_written_total"], json!(1));
    assert_eq!(
        metrics.body["login_ip_spoof_header_ignored_total"],
        json!(1)
    );
}

#[tokio::test]
async fn geolocation_provider_timeout_does_not_block_login_or_event_query() {
    let events = InMemoryLoginEventStore::default();
    let app = auth_router_with_config(
        config_with_fixture()
            .with_login_event_store(Arc::new(events.clone()))
            .with_login_geo_provider(Arc::new(SlowGeoProvider))
            .with_login_geo_timeout(Duration::from_millis(5)),
    );

    let login = post_login_with_headers(
        app.clone(),
        json!({
            "identifier": "member@example.com",
            "identifier_kind": "email",
            "password": "member",
            "login_surface": "user_web"
        }),
        &[("x-real-ip", "203.0.113.101")],
    )
    .await;
    let token = login.body["session_token"]
        .as_str()
        .expect("session token should be returned");
    let metrics = request_with_optional_bearer(
        app,
        "GET",
        "/v1/internal/admin/login-events/metrics",
        Some(token),
    )
    .await;

    assert_eq!(login.status, StatusCode::OK);
    let event = events
        .events()
        .into_iter()
        .find(|event| event.event_type == "login_success")
        .expect("login event should be stored");
    assert_eq!(event.geo_accuracy, "unknown");
    assert!(event.geo_source.is_none());
    assert_eq!(metrics.body["login_geo_lookup_failed_total"], json!(1));
}

#[tokio::test]
async fn failed_login_event_write_enqueues_outbox_and_keeps_login_fail_open() {
    let outbox = InMemoryLoginEventOutbox::default();
    let app = auth_router_with_config(
        config_with_fixture()
            .with_login_event_store(Arc::new(UnavailableLoginEventStore))
            .with_login_event_outbox(Arc::new(outbox.clone())),
    );

    let login = post_login(
        app.clone(),
        json!({
            "identifier": "member@example.com",
            "identifier_kind": "email",
            "password": "member",
            "login_surface": "user_web"
        }),
    )
    .await;
    let token = login.body["session_token"]
        .as_str()
        .expect("session token should be returned");
    let metrics = request_with_optional_bearer(
        app,
        "GET",
        "/v1/internal/admin/login-events/metrics",
        Some(token),
    )
    .await;

    assert_eq!(login.status, StatusCode::OK);
    assert_eq!(outbox.records().len(), 1);
    assert_eq!(metrics.body["login_events_write_failed_total"], json!(1));
    assert_eq!(metrics.body["login_events_outbox_enqueued_total"], json!(1));
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

#[tokio::test]
async fn login_accepts_multiple_local_accounts_from_json_config() {
    let accounts = AuthConfig::from_local_login_accounts_json(
        r#"[
          {
            "user_id": "U-ADMIN-001",
            "email": "admin@prophetica.local",
            "phone": "0912345601",
            "password": "admin",
            "role": "admin"
          },
          {
            "user_id": "U-AFF-001",
            "email": "affiliate@prophetica.local",
            "phone": "0912345603",
            "password": "affiliate",
            "role": "affiliate",
            "affiliate_id": "AFF-001"
          }
        ]"#,
    )
    .expect("json accounts parse");
    let app = auth_router_with_config(AuthConfig::new(accounts));

    let admin_response = post_login(
        app.clone(),
        json!({
            "identifier": "admin@prophetica.local",
            "identifier_kind": "email",
            "password": "admin"
        }),
    )
    .await;
    let affiliate_response = post_login(
        app,
        json!({
            "identifier": "0912345603",
            "identifier_kind": "phone",
            "password": "affiliate"
        }),
    )
    .await;

    assert_eq!(admin_response.status, StatusCode::OK);
    assert_eq!(admin_response.body["user_id"], json!("U-ADMIN-001"));
    assert_eq!(admin_response.body["role"], json!("admin"));
    assert_eq!(affiliate_response.status, StatusCode::OK);
    assert_eq!(affiliate_response.body["user_id"], json!("U-AFF-001"));
    assert_eq!(affiliate_response.body["role"], json!("affiliate"));
    assert_eq!(affiliate_response.body["affiliate_id"], json!("AFF-001"));
}

#[tokio::test]
async fn affiliate_login_accepts_argon2id_hash_and_rejects_weak_parameters() {
    let strong_hash = argon2id_hash("affiliate", 19_456, 2, 1);
    let weak_hash = argon2id_hash("affiliate", 4_096, 1, 1);
    let accounts = AuthConfig::from_local_login_accounts_json(&format!(
        r#"[
          {{
            "user_id": "U-AFF-001",
            "email": "affiliate@prophetica.local",
            "phone": "0912345603",
            "password": "{strong_hash}",
            "role": "affiliate",
            "affiliate_id": "AFF-001"
          }},
          {{
            "user_id": "U-AFF-WEAK",
            "email": "weak-affiliate@prophetica.local",
            "phone": "0912345604",
            "password": "{weak_hash}",
            "role": "affiliate",
            "affiliate_id": "AFF-WEAK"
          }}
        ]"#
    ))
    .expect("json accounts parse");
    let app = auth_router_with_config(AuthConfig::new(accounts));

    let strong_response = post_login(
        app.clone(),
        json!({
            "identifier": "affiliate@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate"
        }),
    )
    .await;
    let weak_response = post_login(
        app,
        json!({
            "identifier": "weak-affiliate@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate"
        }),
    )
    .await;

    assert_eq!(strong_response.status, StatusCode::OK);
    assert_eq!(strong_response.body["affiliate_id"], json!("AFF-001"));
    assert_eq!(weak_response.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn login_accepts_argon2id_credentials_from_registered_account_store() {
    let password_hash = argon2id_hash("registered-password", 19_456, 2, 1);
    let app = auth_router_with_config(AuthConfig::new(Vec::new()).with_credential_store(Arc::new(
        InMemoryPasswordCredentialStore::new(vec![StoredLoginCredential {
            user_id: "MEM-REGISTERED-001".to_owned(),
            email: "registered@example.com".to_owned(),
            phone: "+886912345699".to_owned(),
            password_hash,
            role: "player".to_owned(),
            affiliate_id: None,
        }]),
    )));

    let response = post_login(
        app.clone(),
        json!({
            "identifier": "registered@example.com",
            "identifier_kind": "email",
            "password": "registered-password"
        }),
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["user_id"], json!("MEM-REGISTERED-001"));
    assert_eq!(response.body["role"], json!("player"));
    let token = response.body["session_token"]
        .as_str()
        .expect("session token should be present");
    let me_response = request_with_optional_bearer(app, "GET", "/v1/auth/me", Some(token)).await;
    assert_eq!(me_response.status, StatusCode::OK);
    assert_eq!(me_response.body["user_id"], json!("MEM-REGISTERED-001"));
}

#[tokio::test]
async fn strict_credential_policy_rejects_plaintext_affiliate_accounts() {
    let strong_hash = argon2id_hash("affiliate", 19_456, 2, 1);
    let accounts = AuthConfig::from_local_login_accounts_json_with_policy(
        &format!(
            r#"[
              {{
                "user_id": "U-AFF-PLAIN",
                "email": "plain-affiliate@prophetica.local",
                "phone": "0912345605",
                "password": "affiliate",
                "role": "affiliate",
                "affiliate_id": "AFF-PLAIN"
              }},
              {{
                "user_id": "U-AFF-HASHED",
                "email": "hashed-affiliate@prophetica.local",
                "phone": "0912345606",
                "password": "{strong_hash}",
                "role": "affiliate",
                "affiliate_id": "AFF-HASHED"
              }}
            ]"#
        ),
        true,
    )
    .expect("hashed account should remain");

    assert_eq!(accounts.len(), 1);
    let app = auth_router_with_config(AuthConfig::new(accounts));
    let plain_response = post_login(
        app.clone(),
        json!({
            "identifier": "plain-affiliate@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate"
        }),
    )
    .await;
    let hashed_response = post_login(
        app,
        json!({
            "identifier": "hashed-affiliate@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate"
        }),
    )
    .await;

    assert_eq!(plain_response.status, StatusCode::UNAUTHORIZED);
    assert_eq!(hashed_response.status, StatusCode::OK);
}

#[tokio::test]
async fn strict_hardware_mfa_policy_rejects_privileged_accounts_without_webauthn() {
    let accounts = AuthConfig::from_local_login_accounts_json_with_security_policy(
        r#"[
          {
            "user_id": "U-AFF-TOTP",
            "email": "totp-affiliate@prophetica.local",
            "phone": "0912345607",
            "password": "affiliate",
            "role": "affiliate",
            "affiliate_id": "AFF-TOTP",
            "mfa_methods": ["totp"],
            "mfa_at_epoch_seconds": 1778880000
          },
          {
            "user_id": "U-AFF-WEBAUTHN",
            "email": "webauthn-affiliate@prophetica.local",
            "phone": "0912345608",
            "password": "affiliate",
            "role": "affiliate",
            "affiliate_id": "AFF-WEBAUTHN",
            "mfa_methods": ["totp", "webauthn"],
            "mfa_at_epoch_seconds": 1778880000
          }
        ]"#,
        false,
        true,
    )
    .expect("hardware-mfa account should remain");

    assert_eq!(accounts.len(), 1);
    let private_jwk = r#"{
      "kty": "OKP",
      "crv": "Ed25519",
      "kid": "session-key-1",
      "d": "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc"
    }"#;
    let app = auth_router_with_config(AuthConfig::new(accounts).with_session_signer(private_jwk));
    let totp_response = post_login(
        app.clone(),
        json!({
            "identifier": "totp-affiliate@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate"
        }),
    )
    .await;
    let webauthn_response = post_login(
        app,
        json!({
            "identifier": "webauthn-affiliate@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate"
        }),
    )
    .await;

    assert_eq!(totp_response.status, StatusCode::UNAUTHORIZED);
    assert_eq!(webauthn_response.status, StatusCode::OK);
    assert_eq!(
        webauthn_response.body["mfa_at_epoch_seconds"],
        json!(1_778_880_000_u64)
    );
    let payload = jwt_payload(
        webauthn_response.body["session_token"]
            .as_str()
            .expect("session token should be present"),
    );
    assert_eq!(payload["mfa_at"], json!(1_778_880_000_u64));
    assert_eq!(payload["amr"], json!(["pwd", "hwk"]));
}

#[tokio::test]
async fn totp_step_up_is_required_when_account_has_totp_secret() {
    let accounts = AuthConfig::from_local_login_accounts_json(
        r#"[
          {
            "user_id": "U-AFF-TOTP",
            "email": "totp-affiliate@prophetica.local",
            "phone": "0912345609",
            "password": "affiliate",
            "role": "affiliate",
            "affiliate_id": "AFF-TOTP",
            "totp_secret_base32": "JBSWY3DPEHPK3PXP",
            "recovery_codes": ["recover-once-1"]
          }
        ]"#,
    )
    .expect("totp account should parse");
    let private_jwk = r#"{
      "kty": "OKP",
      "crv": "Ed25519",
      "kid": "session-key-1",
      "d": "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc"
    }"#;
    let app = auth_router_with_config(AuthConfig::new(accounts).with_session_signer(private_jwk));
    let missing_mfa_response = post_login(
        app.clone(),
        json!({
            "identifier": "totp-affiliate@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate"
        }),
    )
    .await;
    let valid_totp_response = post_login(
        app.clone(),
        json!({
            "identifier": "totp-affiliate@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate",
            "mfa_code": current_totp_code("JBSWY3DPEHPK3PXP")
        }),
    )
    .await;
    let recovery_response = post_login(
        app.clone(),
        json!({
            "identifier": "totp-affiliate@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate",
            "mfa_recovery_code": "recover-once-1"
        }),
    )
    .await;
    let reused_recovery_response = post_login(
        app,
        json!({
            "identifier": "totp-affiliate@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate",
            "mfa_recovery_code": "recover-once-1"
        }),
    )
    .await;

    assert_eq!(missing_mfa_response.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        missing_mfa_response.body["error"],
        json!("ERROR_CODE_MFA_REQUIRED")
    );
    assert_eq!(valid_totp_response.status, StatusCode::OK);
    assert!(
        valid_totp_response.body["mfa_at_epoch_seconds"]
            .as_u64()
            .is_some()
    );
    let payload = jwt_payload(
        valid_totp_response.body["session_token"]
            .as_str()
            .expect("session token should be present"),
    );
    assert!(payload["mfa_at"].as_u64().is_some());
    assert_eq!(payload["amr"], json!(["pwd", "otp"]));
    assert_eq!(recovery_response.status, StatusCode::OK);
    assert_eq!(reused_recovery_response.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn consumed_recovery_codes_persist_across_auth_config_instances() {
    let store_path = std::env::temp_dir().join(format!(
        "prophetica-user-service-recovery-{}.txt",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&store_path);
    let accounts_json = r#"[
      {
        "user_id": "U-AFF-RECOVERY",
        "email": "recovery-affiliate@prophetica.local",
        "phone": "0912345610",
        "password": "affiliate",
        "role": "affiliate",
        "affiliate_id": "AFF-RECOVERY",
        "totp_secret_base32": "JBSWY3DPEHPK3PXP",
        "recovery_codes": ["recover-durable-1"]
      }
    ]"#;
    let first_accounts =
        AuthConfig::from_local_login_accounts_json(accounts_json).expect("accounts parse");
    let first_app = auth_router_with_config(
        AuthConfig::new(first_accounts).with_recovery_code_store_path(store_path.clone()),
    );
    let first_response = post_login(
        first_app,
        json!({
            "identifier": "recovery-affiliate@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate",
            "mfa_recovery_code": "recover-durable-1"
        }),
    )
    .await;

    let second_accounts =
        AuthConfig::from_local_login_accounts_json(accounts_json).expect("accounts parse");
    let second_app = auth_router_with_config(
        AuthConfig::new(second_accounts).with_recovery_code_store_path(store_path.clone()),
    );
    let second_response = post_login(
        second_app,
        json!({
            "identifier": "recovery-affiliate@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate",
            "mfa_recovery_code": "recover-durable-1"
        }),
    )
    .await;

    assert_eq!(first_response.status, StatusCode::OK);
    assert_eq!(second_response.status, StatusCode::UNAUTHORIZED);
    let _ = std::fs::remove_file(store_path);
}

#[tokio::test]
async fn me_and_logout_are_available_on_v1_auth_surface() {
    let app = auth_router_with_config(config_with_fixture());
    let login_response = post_login(
        app.clone(),
        json!({
            "identifier": "member@example.com",
            "identifier_kind": "email",
            "password": "member"
        }),
    )
    .await;
    let token = login_response.body["session_token"]
        .as_str()
        .expect("session token should be present");

    let me_response =
        request_with_optional_bearer(app.clone(), "GET", "/v1/auth/me", Some(token)).await;
    let logout_response =
        request_with_optional_bearer(app, "POST", "/v1/auth/logout", Some(token)).await;

    assert_eq!(me_response.status, StatusCode::OK);
    assert_eq!(me_response.body["user_id"], json!("user_42"));
    assert_eq!(me_response.body["role"], json!("manager"));
    assert_eq!(logout_response.status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn login_issues_ed25519_jws_and_publishes_jwks_when_signer_configured() {
    let account = LocalLoginAccount::new(
        "U-AFF-001",
        "affiliate@prophetica.local",
        "0912345603",
        "affiliate",
    )
    .expect("fixture account is valid")
    .with_profile("affiliate", Some("AFF-001".to_owned()));
    let private_jwk = r#"{
      "kty": "OKP",
      "crv": "Ed25519",
      "kid": "session-key-1",
      "d": "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc"
    }"#;
    let app =
        auth_router_with_config(AuthConfig::new(vec![account]).with_session_signer(private_jwk));

    let login_response = post_login(
        app.clone(),
        json!({
            "identifier": "affiliate@prophetica.local",
            "identifier_kind": "email",
            "password": "affiliate"
        }),
    )
    .await;
    let token = login_response.body["session_token"]
        .as_str()
        .expect("session token should be present");
    assert_eq!(token.split('.').count(), 3);

    let jwks_response =
        request_with_optional_bearer(app.clone(), "GET", "/v1/auth/jwks", None).await;
    assert_eq!(jwks_response.status, StatusCode::OK);
    assert_eq!(jwks_response.body["keys"][0]["kid"], json!("session-key-1"));
    assert_eq!(jwks_response.body["keys"][0]["crv"], json!("Ed25519"));

    let me_response = request_with_optional_bearer(app, "GET", "/v1/auth/me", Some(token)).await;
    assert_eq!(me_response.status, StatusCode::OK);
    assert_eq!(me_response.body["user_id"], json!("U-AFF-001"));
    assert_eq!(me_response.body["role"], json!("affiliate"));
    assert_eq!(me_response.body["affiliate_id"], json!("AFF-001"));
}

#[tokio::test]
async fn webauthn_registration_start_requires_session_and_persists_challenge() {
    let account = LocalLoginAccount::new(
        "U-WEBAUTHN-001",
        "webauthn@prophetica.local",
        "0912345611",
        "member",
    )
    .expect("fixture account is valid")
    .with_profile("manager", None)
    .with_webauthn_user_uuid(Some("018f6f70-1c40-7000-8000-000000000001".to_owned()));
    let app = auth_router_with_config(AuthConfig::new(vec![account]).with_webauthn_config(
        WebauthnRuntimeConfig {
            rp_id: "prophetica.local".to_owned(),
            rp_origin: "https://prophetica.local".to_owned(),
        },
    ));
    let login_response = post_login(
        app.clone(),
        json!({
            "identifier": "webauthn@prophetica.local",
            "identifier_kind": "email",
            "password": "member"
        }),
    )
    .await;
    let token = login_response.body["session_token"]
        .as_str()
        .expect("session token should be present");

    let missing_session = request_with_optional_bearer(
        app.clone(),
        "POST",
        "/v1/auth/webauthn/register/start",
        None,
    )
    .await;
    let start_response = request_with_optional_bearer(
        app.clone(),
        "POST",
        "/v1/auth/webauthn/register/start",
        Some(token),
    )
    .await;

    assert_eq!(missing_session.status, StatusCode::UNAUTHORIZED);
    assert_eq!(start_response.status, StatusCode::OK);
    assert!(start_response.body["challenge_id"].as_str().is_some());
    assert!(start_response.body["creation_options"]["publicKey"].is_object());

    let finish_response = request_with_optional_bearer_json(
        app,
        "POST",
        "/v1/auth/webauthn/register/finish",
        Some(token),
        json!({
            "challenge_id": start_response.body["challenge_id"]
                .as_str()
                .expect("challenge id should be returned"),
            "registration_data": { "not": "a browser RegisterPublicKeyCredential" }
        }),
    )
    .await;

    assert_eq!(finish_response.status, StatusCode::BAD_GATEWAY);
    assert_eq!(
        finish_response.body["error"],
        json!("ERROR_CODE_WEBAUTHN_REGISTRATION_FINISH_FAILED")
    );
}

#[tokio::test]
async fn webauthn_assertion_endpoints_require_credentials_and_verify_payloads() {
    let store = InMemoryWebauthnStore::default();
    let account = LocalLoginAccount::new(
        "U-WEBAUTHN-ASSERT-001",
        "webauthn-assert@prophetica.local",
        "0912345612",
        "member",
    )
    .expect("fixture account is valid")
    .with_profile("manager", None)
    .with_webauthn_user_uuid(Some("018f6f70-1c40-7000-8000-000000000002".to_owned()));
    let app = auth_router_with_config(
        AuthConfig::new(vec![account])
            .with_webauthn_config(WebauthnRuntimeConfig {
                rp_id: "prophetica.local".to_owned(),
                rp_origin: "https://prophetica.local".to_owned(),
            })
            .with_webauthn_store(Arc::new(store.clone())),
    );
    let login_response = post_login(
        app.clone(),
        json!({
            "identifier": "webauthn-assert@prophetica.local",
            "identifier_kind": "email",
            "password": "member"
        }),
    )
    .await;
    let token = login_response.body["session_token"]
        .as_str()
        .expect("session token should be present");

    let missing_credential_response = request_with_optional_bearer(
        app.clone(),
        "POST",
        "/v1/auth/webauthn/assert/start",
        Some(token),
    )
    .await;
    assert_eq!(missing_credential_response.status, StatusCode::CONFLICT);
    assert_eq!(
        missing_credential_response.body["error"],
        json!("ERROR_CODE_WEBAUTHN_CREDENTIAL_REQUIRED")
    );

    store
        .put_credential(WebauthnCredentialRecord {
            credential_id: "credential-assert-1".to_owned(),
            user_id: "U-WEBAUTHN-ASSERT-001".to_owned(),
            user_webauthn_uuid: "018f6f70-1c40-7000-8000-000000000002".to_owned(),
            display_name: "webauthn-assert@prophetica.local".to_owned(),
            public_key_cose: json!({ "not": "a serialized passkey" }),
            signature_counter: 0,
            transports: json!([]),
            user_verified: true,
            backup_eligible: false,
            backup_state: false,
            registration_policy: "required".to_owned(),
            attestation_format: None,
            attestation: json!({}),
            created_at_epoch_seconds: 1000,
            last_used_at_epoch_seconds: None,
            revoked_at_epoch_seconds: None,
            metadata: json!({}),
        })
        .await
        .expect("fixture credential should be stored");
    store
        .put_challenge(WebauthnChallengeRecord {
            challenge_id: "assertion-challenge-1".to_owned(),
            user_id: "U-WEBAUTHN-ASSERT-001".to_owned(),
            ceremony: WebauthnCeremony::Assertion,
            state_json: json!({ "not": "authentication state" }),
            expires_at_epoch_seconds: 4_102_444_800,
            consumed_at_epoch_seconds: None,
            created_at_epoch_seconds: 1_000,
        })
        .await
        .expect("fixture challenge should be stored");

    let finish_response = request_with_optional_bearer_json(
        app,
        "POST",
        "/v1/auth/webauthn/assert/finish",
        Some(token),
        json!({
            "challenge_id": "assertion-challenge-1",
            "assertion_data": { "not": "a browser PublicKeyCredential" }
        }),
    )
    .await;

    assert_eq!(finish_response.status, StatusCode::BAD_GATEWAY);
    assert_eq!(
        finish_response.body["error"],
        json!("ERROR_CODE_WEBAUTHN_ASSERTION_FINISH_FAILED")
    );
}

#[tokio::test]
async fn webauthn_credential_revoke_requires_session_and_removes_active_credential() {
    let store = InMemoryWebauthnStore::default();
    let account = LocalLoginAccount::new(
        "U-WEBAUTHN-REVOKE-001",
        "webauthn-revoke@prophetica.local",
        "0912345613",
        "member",
    )
    .expect("fixture account is valid")
    .with_profile("manager", None)
    .with_webauthn_user_uuid(Some("018f6f70-1c40-7000-8000-000000000003".to_owned()));
    let app = auth_router_with_config(
        AuthConfig::new(vec![account])
            .with_webauthn_config(WebauthnRuntimeConfig {
                rp_id: "prophetica.local".to_owned(),
                rp_origin: "https://prophetica.local".to_owned(),
            })
            .with_webauthn_store(Arc::new(store.clone())),
    );
    let login_response = post_login(
        app.clone(),
        json!({
            "identifier": "webauthn-revoke@prophetica.local",
            "identifier_kind": "email",
            "password": "member"
        }),
    )
    .await;
    let token = login_response.body["session_token"]
        .as_str()
        .expect("session token should be present");

    store
        .put_credential(WebauthnCredentialRecord {
            credential_id: "credential-revoke-1".to_owned(),
            user_id: "U-WEBAUTHN-REVOKE-001".to_owned(),
            user_webauthn_uuid: "018f6f70-1c40-7000-8000-000000000003".to_owned(),
            display_name: "webauthn-revoke@prophetica.local".to_owned(),
            public_key_cose: json!({ "fixture": "passkey" }),
            signature_counter: 0,
            transports: json!([]),
            user_verified: true,
            backup_eligible: false,
            backup_state: false,
            registration_policy: "required".to_owned(),
            attestation_format: None,
            attestation: json!({}),
            created_at_epoch_seconds: 1000,
            last_used_at_epoch_seconds: None,
            revoked_at_epoch_seconds: None,
            metadata: json!({}),
        })
        .await
        .expect("fixture credential should be stored");

    let missing_session = request_with_optional_bearer_json(
        app.clone(),
        "POST",
        "/v1/auth/webauthn/credentials/revoke",
        None,
        json!({ "credential_id": "credential-revoke-1" }),
    )
    .await;
    assert_eq!(missing_session.status, StatusCode::UNAUTHORIZED);

    let revoke_response = request_with_optional_bearer_json(
        app.clone(),
        "POST",
        "/v1/auth/webauthn/credentials/revoke",
        Some(token),
        json!({ "credential_id": "credential-revoke-1" }),
    )
    .await;
    assert_eq!(revoke_response.status, StatusCode::OK);
    assert_eq!(revoke_response.body["revoked"], json!(true));

    let assert_start_response =
        request_with_optional_bearer(app, "POST", "/v1/auth/webauthn/assert/start", Some(token))
            .await;
    assert_eq!(assert_start_response.status, StatusCode::CONFLICT);
    assert_eq!(
        assert_start_response.body["error"],
        json!("ERROR_CODE_WEBAUTHN_CREDENTIAL_REQUIRED")
    );
}

#[tokio::test]
async fn me_rejects_missing_or_invalid_bearer_token() {
    let app = auth_router_with_config(config_with_fixture());
    let missing = request_with_optional_bearer(app.clone(), "GET", "/v1/auth/me", None).await;
    let invalid =
        request_with_optional_bearer(app, "GET", "/v1/auth/me", Some("not-a-token")).await;

    assert_eq!(missing.status, StatusCode::UNAUTHORIZED);
    assert_eq!(invalid.status, StatusCode::UNAUTHORIZED);
}

#[derive(Debug)]
struct LoginTestResponse {
    status: StatusCode,
    body: Value,
}

struct SlowGeoProvider;

struct UnavailableLoginEventStore;

#[async_trait]
impl LoginEventStore for UnavailableLoginEventStore {
    async fn write_event(
        &self,
        _event: LoginEventInput,
    ) -> Result<LoginEventRecord, LoginEventStoreError> {
        Err(LoginEventStoreError::Unavailable)
    }

    async fn list_for_principal(
        &self,
        _principal_id: &str,
        _limit: usize,
    ) -> Result<Vec<LoginEventRecord>, LoginEventStoreError> {
        Err(LoginEventStoreError::Unavailable)
    }

    async fn list_recent(
        &self,
        _limit: usize,
    ) -> Result<Vec<LoginEventRecord>, LoginEventStoreError> {
        Err(LoginEventStoreError::Unavailable)
    }

    async fn list_admin(
        &self,
        _filter: AdminLoginEventFilter,
    ) -> Result<Vec<LoginEventRecord>, LoginEventStoreError> {
        Err(LoginEventStoreError::Unavailable)
    }

    async fn get_by_id(&self, _id: &str) -> Result<Option<LoginEventRecord>, LoginEventStoreError> {
        Err(LoginEventStoreError::Unavailable)
    }

    async fn delete_older_than(
        &self,
        cutoff_epoch_seconds: u64,
        dry_run: bool,
    ) -> Result<LoginEventRetentionResult, LoginEventStoreError> {
        Ok(LoginEventRetentionResult {
            dry_run,
            cutoff_epoch_seconds,
            matched_events: 0,
            deleted_events: 0,
        })
    }
}

#[async_trait]
impl LoginGeoProvider for SlowGeoProvider {
    async fn lookup(&self, _ip_address: IpAddr) -> Result<GeoLookupResult, GeoLookupError> {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(GeoLookupResult {
            country: Some("TW".to_owned()),
            region: Some("Taipei".to_owned()),
            city: Some("Taipei".to_owned()),
            source: Some("slow-test-provider".to_owned()),
            accuracy: "approximate".to_owned(),
            asn: None,
            isp: None,
        })
    }
}

fn config_with_fixture() -> AuthConfig {
    let account = LocalLoginAccount::new("user_42", "member@example.com", "0912345698", "member")
        .expect("fixture account is valid")
        .with_profile("manager", None);

    AuthConfig::new(vec![account])
}

fn argon2id_hash(password: &str, memory_cost: u32, time_cost: u32, parallelism: u32) -> String {
    let params = Params::new(memory_cost, time_cost, parallelism, None)
        .unwrap_or_else(|error| panic!("argon2 params should build: {error}"));
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let salt = SaltString::from_b64("cHJvcGhldGljYXNhbHQ")
        .unwrap_or_else(|error| panic!("salt should parse: {error}"));
    argon2
        .hash_password(password.as_bytes(), &salt)
        .unwrap_or_else(|error| panic!("password should hash: {error}"))
        .to_string()
}

fn jwt_payload(token: &str) -> Value {
    let payload = token
        .split('.')
        .nth(1)
        .expect("jwt payload segment should exist");
    serde_json::from_slice(&base64_url_decode(payload)).expect("jwt payload should be json")
}

fn base64_url_decode(value: &str) -> Vec<u8> {
    let mut bits = 0_u32;
    let mut bit_count = 0_u8;
    let mut out = Vec::new();
    for byte in value.bytes() {
        let six_bits = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            b'=' => continue,
            _ => panic!("invalid base64url byte"),
        };
        bits = (bits << 6) | u32::from(six_bits);
        bit_count += 6;
        if bit_count >= 8 {
            bit_count -= 8;
            out.push(((bits >> bit_count) & 0xff) as u8);
        }
    }
    out
}

fn current_totp_code(secret_base32: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs();
    generate_totp_code(&base32_decode(secret_base32), now / 30)
}

fn generate_totp_code(secret: &[u8], counter: u64) -> String {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).expect("hmac accepts any key length");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = usize::from(digest[19] & 0x0f);
    let binary = ((u32::from(digest[offset]) & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    format!("{:06}", binary % 1_000_000)
}

fn base32_decode(input: &str) -> Vec<u8> {
    let mut bits = 0_u32;
    let mut bit_count = 0_u8;
    let mut out = Vec::new();
    for byte in input.bytes() {
        let value = match byte.to_ascii_uppercase() {
            b'A'..=b'Z' => byte.to_ascii_uppercase() - b'A',
            b'2'..=b'7' => byte - b'2' + 26,
            b'=' | b' ' | b'-' => continue,
            _ => panic!("invalid base32 byte"),
        };
        bits = (bits << 5) | u32::from(value);
        bit_count += 5;
        if bit_count >= 8 {
            bit_count -= 8;
            out.push(((bits >> bit_count) & 0xff) as u8);
        }
    }
    out
}

async fn post_login(app: axum::Router, body: Value) -> LoginTestResponse {
    post_login_with_headers(app, body, &[]).await
}

async fn post_login_with_headers(
    app: axum::Router,
    body: Value,
    headers: &[(&str, &str)],
) -> LoginTestResponse {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json");
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let response = app
        .oneshot(
            builder
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

async fn request_with_optional_bearer(
    app: axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
) -> LoginTestResponse {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .oneshot(builder.body(Body::empty()).expect("request builds"))
        .await
        .expect("request handled");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let body = if bytes.is_empty() {
        json!(null)
    } else {
        serde_json::from_slice(&bytes).expect("json response body")
    };

    LoginTestResponse { status, body }
}

async fn request_with_optional_bearer_json(
    app: axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Value,
) -> LoginTestResponse {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .oneshot(
            builder
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
    let body = if bytes.is_empty() {
        json!(null)
    } else {
        serde_json::from_slice(&bytes).expect("json response body")
    };

    LoginTestResponse { status, body }
}
