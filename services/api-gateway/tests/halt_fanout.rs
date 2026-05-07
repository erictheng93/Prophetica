#![allow(clippy::expect_used)]

use api_gateway::kill_switch::{
    InMemoryAuditLogStore, InMemoryHaltFlagStore, internal_admin_router,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use matching_engine::{durability::EngineDurabilityGuard, service::InMemoryEngineService};
use proto::proph::v1::{
    AuditMeta, MarketId, OrderType, PlaceOrderRequest, Side, TimeInForce, UserId,
    engine_service_server::EngineService,
};
use sdk::feature_flags::EngineHaltFlagPoller;
use serde_json::{Value, json};
use settlement_worker::{
    Outcome, Position, SettlementBatchRunner, SettlementProgress, build_settlement_batches,
};
use tower::ServiceExt;

#[tokio::test]
async fn admin_halt_fans_out_to_engine_ws_and_settlement_data_planes() {
    let halt_store = InMemoryHaltFlagStore::default();
    let flags = halt_store.feature_flags();
    let app = internal_admin_router(halt_store, InMemoryAuditLogStore::default());

    let response = app
        .oneshot(json_post(
            "/api/internal/admin/halt",
            &json!({ "reason": "incident-123" }),
        ))
        .await
        .expect("halt request should be handled");
    assert_eq!(response.status(), StatusCode::OK);

    let guard = EngineDurabilityGuard::default();
    let service = InMemoryEngineService::with_durability_guard(guard.clone());
    EngineHaltFlagPoller::new(flags.clone()).poll_once(|_snapshot| {
        guard.apply_engine_halt_reader(&flags);
    });

    let rejected = service
        .place_order(tonic::Request::new(place_order_request()))
        .await
        .expect_err("engine should reject orders after halt fan-out");
    assert_eq!(rejected.code(), tonic::Code::FailedPrecondition);
    assert_eq!(
        rejected
            .metadata()
            .get("x-error-code")
            .and_then(|value| value.to_str().ok()),
        Some("ERROR_CODE_MARKET_HALTED")
    );

    let Some(payload) = ws_gateway::engine_state_payload_from_halt_reader(&flags) else {
        panic!("halt snapshot should produce HALTED payload");
    };
    assert_eq!(payload.state, "HALTED");

    let batches = build_settlement_batches(
        "mkt-election",
        [Position::new("pos-1", "user_01", Outcome::Yes, 1, 50)],
    );
    let mut executed = 0;
    let progress = SettlementBatchRunner::new(batches).run_with_retry_and_halt_reader(
        SettlementProgress::default(),
        &flags,
        |_batch| {
            executed += 1;
            Ok(())
        },
    );

    assert_eq!(executed, 0);
    assert!(progress.records.is_empty());
}

fn json_post(uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|error| panic!("request should build: {error}"))
}

fn place_order_request() -> PlaceOrderRequest {
    PlaceOrderRequest {
        audit: Some(AuditMeta::default()),
        user_id: Some(UserId { value: 10 }),
        market_id: Some(MarketId {
            value: "mkt-election".to_string(),
        }),
        side: Side::Ask.into(),
        outcome: 1,
        r#type: OrderType::Limit.into(),
        price_pts: 55,
        quantity: 3,
        tif: TimeInForce::Gtc.into(),
        post_only: false,
        client_order_id: "client-maker".to_string(),
    }
}
