use sha2::{Digest, Sha256};
use thiserror::Error;

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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReconciliationError {
    #[error(
        "player liability mismatch: expected {expected_minor_units}, actual {actual_minor_units}"
    )]
    PlayerLiabilityMismatch {
        expected_minor_units: i64,
        actual_minor_units: i64,
    },
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
        validate_player_liability,
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

        assert_eq!(
            error,
            ReconciliationError::PlayerLiabilityMismatch {
                expected_minor_units: -13_499,
                actual_minor_units: -13_500,
            }
        );
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
