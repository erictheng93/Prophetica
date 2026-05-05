pub const LEDGER_BOOTSTRAP_SCOPE: &str = "stub-only";

use thiserror::Error;

/// Ledger amount stored in SPEC-002 internal minor units.
///
/// One player-facing point equals 100 minor units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerAmount {
    minor_units: i64,
}

impl LedgerAmount {
    /// Creates an amount from player-facing points.
    #[must_use]
    pub const fn from_points(points: i64) -> Self {
        Self {
            minor_units: points * 100,
        }
    }

    /// Creates an amount from internal minor units.
    #[must_use]
    pub const fn from_minor_units(minor_units: i64) -> Self {
        Self { minor_units }
    }

    /// Returns the amount in internal minor units.
    #[must_use]
    pub const fn minor_units(self) -> i64 {
        self.minor_units
    }
}

/// A ledger entry before it is committed to storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntryDraft {
    account_code: String,
    amount: LedgerAmount,
}

impl LedgerEntryDraft {
    /// Creates an uncommitted ledger entry for an account code.
    pub fn new(account_code: impl Into<String>, amount: LedgerAmount) -> Self {
        Self {
            account_code: account_code.into(),
            amount,
        }
    }

    /// Returns the target account code.
    #[must_use]
    pub fn account_code(&self) -> &str {
        &self.account_code
    }

    /// Returns the entry amount.
    #[must_use]
    pub const fn amount(&self) -> LedgerAmount {
        self.amount
    }

    /// Returns the DB storage direction: `1` for debit, `-1` for credit.
    #[must_use]
    pub const fn storage_direction(&self) -> i16 {
        if self.amount.minor_units() >= 0 {
            1
        } else {
            -1
        }
    }

    /// Returns the positive DB storage amount in internal minor units.
    #[must_use]
    pub fn storage_amount_minor_units(&self) -> i64 {
        self.amount.minor_units().abs()
    }
}

/// A ledger transaction draft containing all entries to commit atomically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerTransactionDraft {
    entries: Vec<LedgerEntryDraft>,
}

impl LedgerTransactionDraft {
    /// Creates a transaction draft from entry drafts.
    pub fn new(entries: impl IntoIterator<Item = LedgerEntryDraft>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    /// Validates SPEC-002 INV-1: every transaction's entry sum must equal zero.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerValidationError::Imbalanced`] when entry amounts do not sum to zero.
    pub fn validate_balanced(&self) -> Result<(), LedgerValidationError> {
        if self.entries.len() < 2 {
            return Err(LedgerValidationError::TooFewEntries);
        }

        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.amount.minor_units() == 0)
        {
            return Err(LedgerValidationError::ZeroAmount {
                account_code: entry.account_code.clone(),
            });
        }

        let sum = self
            .entries
            .iter()
            .map(|entry| i128::from(entry.amount.minor_units()))
            .sum::<i128>();

        if sum == 0 {
            Ok(())
        } else {
            Err(LedgerValidationError::Imbalanced { difference: sum })
        }
    }

    /// Returns the transaction entries.
    #[must_use]
    pub fn entries(&self) -> &[LedgerEntryDraft] {
        &self.entries
    }
}

/// Errors raised before a ledger transaction can be committed.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LedgerValidationError {
    /// Transaction does not have enough entries to be double-entry.
    #[error("ledger transaction must contain at least two entries")]
    TooFewEntries,
    /// Entry amount is zero and cannot be persisted to ledger storage.
    #[error("ledger entry amount must be non-zero for account {account_code}")]
    ZeroAmount {
        /// Account code attached to the zero-value entry.
        account_code: String,
    },
    /// Entry amounts do not sum to zero.
    #[error("ledger transaction is imbalanced by {difference} minor units")]
    Imbalanced {
        /// Algebraic difference in internal minor units.
        difference: i128,
    },
}

#[cfg(test)]
mod tests {
    use super::{LedgerAmount, LedgerEntryDraft, LedgerTransactionDraft};

    #[test]
    fn balanced_transaction_is_accepted() {
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new("Bank.TrustAccount.TWD", LedgerAmount::from_points(1_000)),
            LedgerEntryDraft::new("Player.Wallet.user_01", LedgerAmount::from_points(-1_000)),
        ]);

        assert!(transaction.validate_balanced().is_ok());
    }

    #[test]
    fn imbalanced_transaction_is_rejected() {
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Bank.TrustAccount.TWD",
                LedgerAmount::from_minor_units(100_000),
            ),
            LedgerEntryDraft::new(
                "Player.Wallet.user_01",
                LedgerAmount::from_minor_units(-99_999),
            ),
        ]);

        let Err(error) = transaction.validate_balanced() else {
            panic!("imbalanced transaction should be rejected");
        };

        assert_eq!(
            error.to_string(),
            "ledger transaction is imbalanced by 1 minor units"
        );
    }

    #[test]
    fn points_are_stored_as_hundred_minor_units() {
        assert_eq!(LedgerAmount::from_points(42).minor_units(), 4_200);
    }

    #[test]
    fn entry_draft_splits_signed_amount_into_storage_direction_and_amount() {
        let debit =
            LedgerEntryDraft::new("Bank.TrustAccount.TWD", LedgerAmount::from_minor_units(10));
        let credit =
            LedgerEntryDraft::new("Player.Wallet.user_01", LedgerAmount::from_minor_units(-10));

        assert_eq!(debit.storage_direction(), 1);
        assert_eq!(debit.storage_amount_minor_units(), 10);
        assert_eq!(credit.storage_direction(), -1);
        assert_eq!(credit.storage_amount_minor_units(), 10);
    }

    #[test]
    fn transaction_requires_at_least_two_entries() {
        let transaction = LedgerTransactionDraft::new([LedgerEntryDraft::new(
            "Bank.TrustAccount.TWD",
            LedgerAmount::from_minor_units(10),
        )]);

        let Err(error) = transaction.validate_balanced() else {
            panic!("single-entry transaction should be rejected");
        };

        assert_eq!(
            error.to_string(),
            "ledger transaction must contain at least two entries"
        );
    }

    #[test]
    fn transaction_rejects_zero_amount_entries() {
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new("Bank.TrustAccount.TWD", LedgerAmount::from_minor_units(0)),
            LedgerEntryDraft::new("Player.Wallet.user_01", LedgerAmount::from_minor_units(0)),
        ]);

        let Err(error) = transaction.validate_balanced() else {
            panic!("zero-amount transaction should be rejected");
        };

        assert_eq!(
            error.to_string(),
            "ledger entry amount must be non-zero for account Bank.TrustAccount.TWD"
        );
    }
}
