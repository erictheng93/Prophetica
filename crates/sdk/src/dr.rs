use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KafkaMm2LagSample {
    pub topic: String,
    pub lag: Duration,
}

impl KafkaMm2LagSample {
    #[must_use]
    pub fn new(topic: impl Into<String>, lag: Duration) -> Self {
        Self {
            topic: topic.into(),
            lag,
        }
    }

    #[must_use]
    pub fn severity(&self) -> KafkaMm2LagSeverity {
        if self.lag > Duration::from_secs(5) {
            KafkaMm2LagSeverity::P0
        } else if self.lag > Duration::from_secs(3) {
            KafkaMm2LagSeverity::P1
        } else {
            KafkaMm2LagSeverity::Healthy
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KafkaMm2LagSeverity {
    Healthy,
    P1,
    P0,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrReconciliationFinding {
    pub severity: &'static str,
    pub finding_type: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupRestoreStatus {
    Valid,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRestoreMetric {
    pub name: &'static str,
    pub backup_type: &'static str,
}

impl BackupRestoreMetric {
    #[must_use]
    pub const fn success_total(backup_type: &'static str) -> Self {
        Self {
            name: "backup.restore.success_total",
            backup_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupValidationInput {
    pub backup_id: String,
    pub schema_ok: bool,
    pub row_counts_match: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupValidationResult {
    pub backup_id: String,
    pub status: BackupRestoreStatus,
    pub metric: BackupRestoreMetric,
    pub finding: Option<DrReconciliationFinding>,
}

impl BackupValidationInput {
    #[must_use]
    pub fn validate_weekly_restore(&self) -> BackupValidationResult {
        let valid = self.schema_ok && self.row_counts_match;
        BackupValidationResult {
            backup_id: self.backup_id.clone(),
            status: if valid {
                BackupRestoreStatus::Valid
            } else {
                BackupRestoreStatus::Invalid
            },
            metric: BackupRestoreMetric::success_total("weekly"),
            finding: (!valid)
                .then(|| DrReconciliationFinding::p0("backup_restore_validation_failed")),
        }
    }
}

impl DrReconciliationFinding {
    #[must_use]
    pub const fn p0(finding_type: &'static str) -> Self {
        Self {
            severity: "P0",
            finding_type,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrLedgerCountComparison {
    pub primary_count: i64,
    pub dr_count: i64,
}

impl DrLedgerCountComparison {
    #[must_use]
    pub fn finding(&self) -> Option<DrReconciliationFinding> {
        (self.primary_count != self.dr_count)
            .then(|| DrReconciliationFinding::p0("dr_pg_ledger_entries_count_mismatch"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrKafkaOffsetComparison {
    pub topic: String,
    pub lag: Duration,
}

impl DrKafkaOffsetComparison {
    #[must_use]
    pub fn finding(&self) -> Option<DrReconciliationFinding> {
        (self.lag > Duration::from_secs(1))
            .then(|| DrReconciliationFinding::p0("dr_kafka_offset_lag_exceeded"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostgresReplicationState {
    pub application_name: String,
    pub state: String,
    pub sync_state: String,
}

impl PostgresReplicationState {
    #[must_use]
    pub fn is_cross_region_async_streaming(&self) -> bool {
        self.state == "streaming" && self.sync_state == "async"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailoverTriggerInput {
    pub tokyo_edge_healthy: bool,
    pub tokyo_db_primary_healthy: bool,
    pub singapore_dr_ready: bool,
    pub kafka_mm2_lag: Duration,
}

impl FailoverTriggerInput {
    #[must_use]
    pub fn should_failover_to_singapore(&self) -> bool {
        (!self.tokyo_edge_healthy || !self.tokyo_db_primary_healthy)
            && self.singapore_dr_ready
            && self.kafka_mm2_lag <= Duration::from_secs(5)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanaryCheckStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailoverCanaryReport {
    pub checks: [CanaryCheckStatus; 6],
}

impl FailoverCanaryReport {
    #[must_use]
    pub fn is_pass(&self) -> bool {
        self.checks
            .iter()
            .all(|check| matches!(check, CanaryCheckStatus::Pass))
    }

    #[must_use]
    pub fn requires_rollback(&self) -> bool {
        !self.is_pass()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThirdPartyFallbackCheck {
    pub health_check_passed: bool,
    pub rollback_configured: bool,
}

impl ThirdPartyFallbackCheck {
    #[must_use]
    pub const fn is_safe_to_switch(&self) -> bool {
        self.health_check_passed && self.rollback_configured
    }
}
