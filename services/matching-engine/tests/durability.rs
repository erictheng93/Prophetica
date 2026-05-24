#![allow(clippy::expect_used)]

use std::time::Duration;

use matching_engine::{
    chaos::{ChaosRtoHarness, ChaosScenario},
    durability::{DurabilityEvent, EngineDurabilityGuard},
    service::InMemoryEngineService,
};
use proto::proph::v1::{
    AuditMeta, MarketId, OrderType, PlaceOrderRequest, Side, TimeInForce, UserId,
    engine_service_server::EngineService,
};
use sdk::feature_flags::{EngineHaltFlagReader, EngineHaltFlagWriter, InMemoryFeatureFlagStore};

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
        fee_curve_bps: 0,
    }
}

#[tokio::test]
async fn synchronous_standby_loss_makes_primary_fail_closed_and_alerts() {
    let guard = EngineDurabilityGuard::default();
    let service = InMemoryEngineService::with_durability_guard(guard.clone());

    guard.mark_synchronous_standby_lost();

    let error = service
        .place_order(tonic::Request::new(place_order_request()))
        .await
        .expect_err("primary should reject writes when synchronous standby is lost");

    assert_eq!(error.code(), tonic::Code::FailedPrecondition);
    assert_eq!(error.message(), "engine primary is read-only");
    assert_eq!(
        error
            .metadata()
            .get("x-error-code")
            .and_then(|value| value.to_str().ok()),
        Some("ERROR_CODE_ENGINE_READ_ONLY")
    );
    assert_eq!(
        guard.events(),
        vec![DurabilityEvent::SynchronousStandbyLost {
            alert: "engine.synchronous_standby_lost"
        }]
    );
}

#[tokio::test]
async fn synchronous_standby_recovery_allows_writes_again() {
    let guard = EngineDurabilityGuard::default();
    let service = InMemoryEngineService::with_durability_guard(guard.clone());

    guard.mark_synchronous_standby_lost();
    guard.mark_synchronous_standby_healthy();

    service
        .place_order(tonic::Request::new(place_order_request()))
        .await
        .expect("primary should accept writes after synchronous standby recovers");
}

#[tokio::test]
async fn engine_halt_flag_rejects_new_orders_but_keeps_reads_available() {
    let guard = EngineDurabilityGuard::default();
    let service = InMemoryEngineService::with_durability_guard(guard.clone());
    guard.mark_engine_halted();

    let error = service
        .place_order(tonic::Request::new(place_order_request()))
        .await
        .expect_err("halted engine should reject new orders");
    assert_eq!(error.code(), tonic::Code::FailedPrecondition);
    assert_eq!(error.message(), "engine primary is read-only");
    assert_eq!(
        error
            .metadata()
            .get("x-error-code")
            .and_then(|value| value.to_str().ok()),
        Some("ERROR_CODE_MARKET_HALTED")
    );

    service
        .get_book_snapshot(tonic::Request::new(
            proto::proph::v1::GetBookSnapshotRequest {
                market_id: Some(proto::proph::v1::MarketId {
                    value: "mkt-election".to_owned(),
                }),
                depth: 10,
            },
        ))
        .await
        .expect("halted engine should keep broadcast/read surfaces available");
}

#[tokio::test]
async fn engine_applies_shared_halt_flag_snapshot_from_control_plane() {
    let flags = InMemoryFeatureFlagStore::default();
    let guard = EngineDurabilityGuard::default();
    flags.halt_engine("incident-123", "ops incident", "senior-1");

    guard.apply_engine_halt_reader(&flags);

    assert!(guard.is_read_only());
    assert!(guard.events().contains(&DurabilityEvent::EngineHalted));

    flags.release_engine_halt("cto-1");
    guard.apply_engine_halt_snapshot(&flags.engine_halt_snapshot());

    assert!(!guard.is_read_only());
}

#[tokio::test]
async fn releasing_shared_halt_flag_does_not_clear_synchronous_standby_read_only() {
    let flags = InMemoryFeatureFlagStore::default();
    let guard = EngineDurabilityGuard::default();
    guard.mark_synchronous_standby_lost();
    flags.release_engine_halt("cto-1");

    guard.apply_engine_halt_snapshot(&flags.engine_halt_snapshot());

    assert!(guard.is_read_only());
}

#[test]
fn chaos_rto_harness_covers_spec_008_failure_table() {
    let harness = ChaosRtoHarness;
    let outcomes = [
        (ChaosScenario::EnginePodCrash, Duration::from_secs(29)),
        (ChaosScenario::HostPowerLoss, Duration::from_secs(4 * 60)),
        (ChaosScenario::RegionFailure, Duration::from_secs(4 * 60)),
        (ChaosScenario::WalDiskFailure, Duration::from_secs(9 * 60)),
        (
            ChaosScenario::KafkaClusterFailure,
            Duration::from_secs(4 * 60),
        ),
    ];

    for (scenario, measured_rto) in outcomes {
        harness
            .record_rto(scenario, measured_rto)
            .expect("scenario RTO should satisfy SPEC-008 target");
    }
}

#[test]
fn chaos_rto_harness_rejects_slo_violations() {
    let harness = ChaosRtoHarness;

    let error = harness
        .record_rto(ChaosScenario::EnginePodCrash, Duration::from_secs(31))
        .expect_err("engine pod crash over 30s should violate RTO");

    assert_eq!(error.scenario, ChaosScenario::EnginePodCrash);
    assert_eq!(error.target, Duration::from_secs(30));
    assert_eq!(error.measured, Duration::from_secs(31));
}
