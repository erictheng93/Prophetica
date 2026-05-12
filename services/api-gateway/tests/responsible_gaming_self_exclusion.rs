#![allow(clippy::expect_used)]

use std::sync::{Arc, Mutex};

use api_gateway::{
    ApiError, WalletBalance, WalletLedgerClient,
    responsible_gaming::{
        InMemorySelfExclusionStore, ResponsibleGamingClock, responsible_gaming_router,
    },
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn self_exclusion_submit_freezes_available_wallet_and_records_status() {
    let wallet = RecordingWalletLedgerClient::with_available_minor_units(-12_500);
    let app = responsible_gaming_router(
        wallet.clone(),
        InMemorySelfExclusionStore::default(),
        ResponsibleGamingClock::fixed(1_780_000_000),
    );

    let response = app
        .clone()
        .oneshot(json_post(
            "/v1/responsible-gaming/self-exclusion",
            "rg-self-exclusion:user_42:001",
            &json!({
                "mode": "timeout",
                "period": "six_months",
                "consent": true,
                "reason": "need a long break"
            }),
        ))
        .await
        .expect("self-exclusion request should be handled");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["record"]["user_id"], json!("user_42"));
    assert_eq!(body["record"]["mode"], json!("timeout"));
    assert_eq!(body["record"]["period"], json!("six_months"));
    assert_eq!(
        body["record"]["starts_at_epoch_seconds"],
        json!(1_780_000_000)
    );
    assert_eq!(
        body["record"]["ends_at_epoch_seconds"],
        json!(1_795_811_200)
    );
    assert_eq!(body["record"]["frozen_minor_units"], json!(12_500));
    assert_eq!(body["replayed"], json!(false));
    assert_eq!(
        wallet.freeze_requests(),
        vec![(
            "user_42".to_owned(),
            12_500,
            "self-exclusion:user_42:rg-self-exclusion:user_42:001".to_owned(),
        )]
    );

    let status = app
        .oneshot(empty_get("/v1/responsible-gaming/self-exclusion"))
        .await
        .expect("status request should be handled");

    assert_eq!(status.status(), StatusCode::OK);
    let status_body = response_json(status).await;
    assert_eq!(status_body["active"], json!(true));
    assert_eq!(status_body["record"]["mode"], json!("timeout"));
}

#[tokio::test]
async fn self_exclusion_replays_same_idempotency_key_without_second_freeze() {
    let wallet = RecordingWalletLedgerClient::with_available_minor_units(-12_500);
    let app = responsible_gaming_router(
        wallet.clone(),
        InMemorySelfExclusionStore::default(),
        ResponsibleGamingClock::fixed(1_780_000_000),
    );
    let request = json!({
        "mode": "permanent",
        "points_disposition": "hold",
        "consent": true
    });

    let first = app
        .clone()
        .oneshot(json_post(
            "/v1/responsible-gaming/self-exclusion",
            "rg-self-exclusion:user_42:permanent",
            &request,
        ))
        .await
        .expect("first request should be handled");
    let second = app
        .oneshot(json_post(
            "/v1/responsible-gaming/self-exclusion",
            "rg-self-exclusion:user_42:permanent",
            &request,
        ))
        .await
        .expect("replay request should be handled");

    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(response_json(second).await["replayed"], json!(true));
    assert_eq!(wallet.freeze_requests().len(), 1);
}

#[tokio::test]
async fn self_exclusion_rejects_missing_consent() {
    let app = responsible_gaming_router(
        RecordingWalletLedgerClient::with_available_minor_units(-12_500),
        InMemorySelfExclusionStore::default(),
        ResponsibleGamingClock::fixed(1_780_000_000),
    );

    let response = app
        .oneshot(json_post(
            "/v1/responsible-gaming/self-exclusion",
            "rg-self-exclusion:user_42:missing-consent",
            &json!({
                "mode": "timeout",
                "period": "six_months",
                "consent": false
            }),
        ))
        .await
        .expect("request should be handled");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await["error"],
        json!("self_exclusion_consent_required")
    );
}

#[derive(Clone)]
struct RecordingWalletLedgerClient {
    available_minor_units: i64,
    freezes: Arc<Mutex<Vec<(String, i64, String)>>>,
}

impl RecordingWalletLedgerClient {
    fn with_available_minor_units(available_minor_units: i64) -> Self {
        Self {
            available_minor_units,
            freezes: Arc::default(),
        }
    }

    fn freeze_requests(&self) -> Vec<(String, i64, String)> {
        self.freezes
            .lock()
            .expect("freeze requests lock should not poison")
            .clone()
    }
}

#[async_trait::async_trait]
impl WalletLedgerClient for RecordingWalletLedgerClient {
    async fn get_wallet_balance(&self, user_id: &str) -> Result<WalletBalance, ApiError> {
        Ok(WalletBalance {
            user_id: user_id.to_owned(),
            available_minor_units: self.available_minor_units,
            locked_minor_units: 0,
            frozen_minor_units: 0,
        })
    }

    async fn list_entries(
        &self,
        _account_code: &str,
        _limit: i64,
    ) -> Result<Vec<api_gateway::LedgerEntry>, ApiError> {
        Ok(Vec::new())
    }

    async fn freeze_player_funds(
        &self,
        user_id: &str,
        amount_minor_units: i64,
        idempotency_key: &str,
    ) -> Result<(), ApiError> {
        self.freezes
            .lock()
            .expect("freeze requests lock should not poison")
            .push((
                user_id.to_owned(),
                amount_minor_units,
                idempotency_key.to_owned(),
            ));
        Ok(())
    }
}

fn json_post(uri: &str, idempotency_key: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("x-user-id", "user_42")
        .header("idempotency-key", idempotency_key)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request should build")
}

fn empty_get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("x-user-id", "user_42")
        .body(Body::empty())
        .expect("request should build")
}

async fn response_json(response: axum::response::Response) -> Value {
    let collected = response
        .into_body()
        .collect()
        .await
        .expect("response body should collect");
    serde_json::from_slice(&collected.to_bytes()).expect("response should be json")
}
