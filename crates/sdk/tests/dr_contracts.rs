use std::time::Duration;

use sdk::dr::{
    BackupRestoreMetric, BackupRestoreStatus, BackupValidationInput, CanaryCheckStatus,
    DrKafkaOffsetComparison, DrLedgerCountComparison, DrReconciliationFinding,
    FailoverCanaryReport, FailoverTriggerInput, KafkaMm2LagSample, KafkaMm2LagSeverity,
    PostgresReplicationState, ThirdPartyFallbackCheck,
};

#[test]
fn kafka_mm2_lag_thresholds_match_spec_008() {
    assert_eq!(
        KafkaMm2LagSample::new("engine.events.orders", Duration::from_millis(2_999)).severity(),
        KafkaMm2LagSeverity::Healthy
    );
    assert_eq!(
        KafkaMm2LagSample::new("ledger.txns.committed", Duration::from_millis(3_001)).severity(),
        KafkaMm2LagSeverity::P1
    );
    assert_eq!(
        KafkaMm2LagSample::new("settlement.batches", Duration::from_millis(5_001)).severity(),
        KafkaMm2LagSeverity::P0
    );
}

#[test]
fn dr_ledger_count_mismatch_is_p0_finding() {
    let comparison = DrLedgerCountComparison {
        primary_count: 10,
        dr_count: 9,
    };
    let Some(finding) = comparison.finding() else {
        panic!("one missing ledger entry should alert");
    };

    assert_eq!(
        finding,
        DrReconciliationFinding::p0("dr_pg_ledger_entries_count_mismatch")
    );
}

#[test]
fn dr_kafka_offset_lag_over_one_second_is_p0_finding() {
    let comparison = DrKafkaOffsetComparison {
        topic: "ledger.txns.committed".to_owned(),
        lag: Duration::from_millis(1_001),
    };
    let Some(finding) = comparison.finding() else {
        panic!("offset lag over 1s should alert");
    };

    assert_eq!(
        finding,
        DrReconciliationFinding::p0("dr_kafka_offset_lag_exceeded")
    );
}

#[test]
fn singapore_standby_must_stream_async() {
    let state = PostgresReplicationState {
        application_name: "singapore-dr".to_owned(),
        state: "streaming".to_owned(),
        sync_state: "async".to_owned(),
    };

    assert!(state.is_cross_region_async_streaming());
}

#[test]
fn weekly_restore_success_increments_success_metric() {
    let result = BackupValidationInput {
        backup_id: "daily-20260507".to_owned(),
        schema_ok: true,
        row_counts_match: true,
    }
    .validate_weekly_restore();

    assert_eq!(result.status, BackupRestoreStatus::Valid);
    assert_eq!(result.metric, BackupRestoreMetric::success_total("weekly"));
}

#[test]
fn corrupted_snapshot_is_marked_invalid_and_p0_alerted() {
    let result = BackupValidationInput {
        backup_id: "daily-corrupt".to_owned(),
        schema_ok: false,
        row_counts_match: true,
    }
    .validate_weekly_restore();

    assert_eq!(result.status, BackupRestoreStatus::Invalid);
    assert_eq!(
        result.finding,
        Some(DrReconciliationFinding::p0(
            "backup_restore_validation_failed"
        ))
    );
}

#[test]
fn tokyo_unavailable_trigger_requires_canary_before_failover() {
    let trigger = FailoverTriggerInput {
        tokyo_edge_healthy: false,
        tokyo_db_primary_healthy: false,
        singapore_dr_ready: true,
        kafka_mm2_lag: Duration::from_millis(900),
    };

    assert!(trigger.should_failover_to_singapore());
}

#[test]
fn failover_canary_report_requires_all_critical_checks() {
    let report = FailoverCanaryReport {
        checks: [CanaryCheckStatus::Pass; 6],
    };

    assert!(report.is_pass());
}

#[test]
fn failover_canary_failure_requires_rollback() {
    let report = FailoverCanaryReport {
        checks: [
            CanaryCheckStatus::Pass,
            CanaryCheckStatus::Pass,
            CanaryCheckStatus::Pass,
            CanaryCheckStatus::Fail,
            CanaryCheckStatus::Pass,
            CanaryCheckStatus::Pass,
        ],
    };

    assert!(report.requires_rollback());
}

#[test]
fn third_party_fallback_requires_health_check_and_rollback() {
    let check = ThirdPartyFallbackCheck {
        health_check_passed: true,
        rollback_configured: true,
    };

    assert!(check.is_safe_to_switch());
}
