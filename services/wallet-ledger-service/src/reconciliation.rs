use sha2::{Digest, Sha256};
use sqlx::PgPool;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconciliationLayer {
    PlayerLiability = 2,
    BankTrust = 3,
    GatewaySettlement = 4,
}

impl ReconciliationLayer {
    #[must_use]
    pub const fn as_i16(self) -> i16 {
        self as i16
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationFinding {
    pub severity: String,
    pub finding_type: String,
    pub account_code: Option<String>,
    pub expected_amount: i64,
    pub actual_amount: i64,
    pub difference_amount: i64,
    pub details: serde_json::Value,
}

impl ReconciliationFinding {
    #[must_use]
    pub fn mismatch(
        finding_type: impl Into<String>,
        account_code: Option<String>,
        expected_amount: i64,
        actual_amount: i64,
    ) -> Self {
        Self {
            severity: "critical".to_owned(),
            finding_type: finding_type.into(),
            account_code,
            expected_amount,
            actual_amount,
            difference_amount: actual_amount - expected_amount,
            details: serde_json::json!({}),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerLiabilitySnapshot {
    pub wallet_minor_units: i64,
    pub locked_minor_units: i64,
    pub frozen_minor_units: i64,
}

impl PlayerLiabilitySnapshot {
    #[must_use]
    pub const fn total_minor_units(self) -> i64 {
        self.wallet_minor_units + self.locked_minor_units + self.frozen_minor_units
    }
}

#[derive(Debug, Error)]
pub enum ReconciliationError {
    #[error(
        "player liability mismatch: expected {expected_minor_units}, actual {actual_minor_units}"
    )]
    PlayerLiabilityMismatch {
        expected_minor_units: i64,
        actual_minor_units: i64,
    },
    #[error(transparent)]
    Store(#[from] sqlx::Error),
}

/// Validates SPEC-002 INV-2 for the signed ledger representation.
///
/// Player liabilities are stored as negative ledger balances because player wallet,
/// locked, and frozen accounts are liability accounts.
///
/// # Errors
///
/// Returns [`ReconciliationError::PlayerLiabilityMismatch`] when the ledger total differs
/// from the expected signed liability amount.
pub fn validate_player_liability(
    snapshot: PlayerLiabilitySnapshot,
    expected_signed_minor_units: i64,
) -> Result<(), ReconciliationError> {
    let actual_minor_units = snapshot.total_minor_units();
    if actual_minor_units == expected_signed_minor_units {
        Ok(())
    } else {
        Err(ReconciliationError::PlayerLiabilityMismatch {
            expected_minor_units: expected_signed_minor_units,
            actual_minor_units,
        })
    }
}

/// Aggregates Layer 2 player liability balances directly from ledger entries.
///
/// Player liabilities are signed balances under `Player.Wallet.*`, `Player.Locked.*`,
/// and `Player.Frozen.*`.
///
/// # Errors
///
/// Returns [`ReconciliationError::Store`] when Postgres cannot run the aggregation.
pub async fn load_player_liability_snapshot(
    pool: &PgPool,
) -> Result<PlayerLiabilitySnapshot, ReconciliationError> {
    let (wallet_minor_units,) = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(SUM(e.amount * e.direction), 0)::BIGINT \
         FROM ledger_entries e \
         JOIN ledger_accounts a ON a.id = e.account_id \
         WHERE a.code LIKE 'Player.Wallet.%'",
    )
    .fetch_one(pool)
    .await?;
    let (locked_minor_units,) = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(SUM(e.amount * e.direction), 0)::BIGINT \
         FROM ledger_entries e \
         JOIN ledger_accounts a ON a.id = e.account_id \
         WHERE a.code LIKE 'Player.Locked.%'",
    )
    .fetch_one(pool)
    .await?;
    let (frozen_minor_units,) = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(SUM(e.amount * e.direction), 0)::BIGINT \
         FROM ledger_entries e \
         JOIN ledger_accounts a ON a.id = e.account_id \
         WHERE a.code LIKE 'Player.Frozen.%'",
    )
    .fetch_one(pool)
    .await?;

    Ok(PlayerLiabilitySnapshot {
        wallet_minor_units,
        locked_minor_units,
        frozen_minor_units,
    })
}

#[must_use]
pub fn player_liability_finding(
    snapshot: PlayerLiabilitySnapshot,
    expected_signed_minor_units: i64,
) -> Option<ReconciliationFinding> {
    let actual_minor_units = snapshot.total_minor_units();
    (actual_minor_units != expected_signed_minor_units).then(|| {
        ReconciliationFinding::mismatch(
            "player_liability_mismatch",
            Some("Player.*".to_owned()),
            expected_signed_minor_units,
            actual_minor_units,
        )
    })
}

/// Starts a reconciliation run row and returns its id.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the row cannot be inserted.
pub async fn start_reconciliation_run(
    pool: &PgPool,
    layer: ReconciliationLayer,
) -> Result<i64, sqlx::Error> {
    let (run_id,) = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO reconciliation_runs (layer, status) \
         VALUES ($1, 'started') \
         RETURNING id",
    )
    .bind(layer.as_i16())
    .fetch_one(pool)
    .await?;

    Ok(run_id)
}

/// Persists one reconciliation finding.
///
/// # Errors
///
/// Returns [`sqlx::Error`] when the finding cannot be inserted.
pub async fn persist_reconciliation_finding(
    pool: &PgPool,
    run_id: i64,
    finding: &ReconciliationFinding,
) -> Result<i64, sqlx::Error> {
    let (finding_id,) = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO reconciliation_findings \
         (run_id, severity, finding_type, account_code, expected_amount, actual_amount, \
          difference_amount, details) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         RETURNING id",
    )
    .bind(run_id)
    .bind(&finding.severity)
    .bind(&finding.finding_type)
    .bind(finding.account_code.as_deref())
    .bind(finding.expected_amount)
    .bind(finding.actual_amount)
    .bind(finding.difference_amount)
    .bind(&finding.details)
    .fetch_one(pool)
    .await?;

    Ok(finding_id)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HashChainEntry {
    pub txn_id: i64,
    pub account_code: String,
    pub direction: i16,
    pub amount_minor_units: i64,
    pub balance_after_minor_units: i64,
}

impl HashChainEntry {
    #[must_use]
    pub fn canonical_line(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.txn_id,
            self.account_code,
            self.direction,
            self.amount_minor_units,
            self.balance_after_minor_units
        )
    }
}

#[must_use]
pub fn compute_daily_hash(previous_hash_hex: &str, entries: &[HashChainEntry]) -> String {
    let mut sorted_entries = entries.to_vec();
    sorted_entries.sort();

    let mut hasher = Sha256::new();
    hasher.update(previous_hash_hex.as_bytes());
    for entry in sorted_entries {
        hasher.update(b"\n");
        hasher.update(entry.canonical_line().as_bytes());
    }

    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        HashChainEntry, PlayerLiabilitySnapshot, ReconciliationError, compute_daily_hash,
        player_liability_finding, validate_player_liability,
    };

    #[test]
    fn validates_matching_player_liability_snapshot() {
        let snapshot = PlayerLiabilitySnapshot {
            wallet_minor_units: -10_000,
            locked_minor_units: -2_500,
            frozen_minor_units: -1_000,
        };

        assert!(validate_player_liability(snapshot, -13_500).is_ok());
    }

    #[test]
    fn rejects_mismatched_player_liability_snapshot() {
        let snapshot = PlayerLiabilitySnapshot {
            wallet_minor_units: -10_000,
            locked_minor_units: -2_500,
            frozen_minor_units: -1_000,
        };

        let error = match validate_player_liability(snapshot, -13_499) {
            Ok(()) => panic!("mismatched liability should fail"),
            Err(error) => error,
        };

        match error {
            ReconciliationError::PlayerLiabilityMismatch {
                expected_minor_units,
                actual_minor_units,
            } => {
                assert_eq!(expected_minor_units, -13_499);
                assert_eq!(actual_minor_units, -13_500);
            }
            ReconciliationError::Store(error) => {
                panic!("unexpected store error: {error}");
            }
        }
    }

    #[test]
    fn builds_player_liability_mismatch_finding() {
        let snapshot = PlayerLiabilitySnapshot {
            wallet_minor_units: -10_000,
            locked_minor_units: -2_500,
            frozen_minor_units: -1_000,
        };

        let Some(finding) = player_liability_finding(snapshot, -13_499) else {
            panic!("mismatch should produce a finding");
        };

        assert_eq!(finding.severity, "critical");
        assert_eq!(finding.finding_type, "player_liability_mismatch");
        assert_eq!(finding.account_code, Some("Player.*".to_owned()));
        assert_eq!(finding.expected_amount, -13_499);
        assert_eq!(finding.actual_amount, -13_500);
        assert_eq!(finding.difference_amount, -1);
    }

    #[test]
    fn daily_hash_is_order_independent_for_same_entries() {
        let first = sample_entries();
        let mut second = sample_entries();
        second.reverse();

        assert_eq!(
            compute_daily_hash("previous", &first),
            compute_daily_hash("previous", &second)
        );
    }

    #[test]
    fn daily_hash_changes_when_previous_hash_changes() {
        let entries = sample_entries();

        assert_ne!(
            compute_daily_hash("previous-a", &entries),
            compute_daily_hash("previous-b", &entries)
        );
    }

    fn sample_entries() -> Vec<HashChainEntry> {
        vec![
            HashChainEntry {
                txn_id: 2,
                account_code: "Player.Wallet.user_01".to_owned(),
                direction: -1,
                amount_minor_units: 10_000,
                balance_after_minor_units: -10_000,
            },
            HashChainEntry {
                txn_id: 1,
                account_code: "Bank.TrustAccount.TWD".to_owned(),
                direction: 1,
                amount_minor_units: 10_000,
                balance_after_minor_units: 10_000,
            },
        ]
    }
}
