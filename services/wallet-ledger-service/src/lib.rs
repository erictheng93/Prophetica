pub const SERVICE_NAME: &str = "wallet-ledger-service";
pub const DEFAULT_PORT: u16 = 8086;

pub mod api;
pub mod ledger;
pub mod reconciliation;

use sdk::ledger::{LedgerTransactionDraft, LedgerValidationError};

/// Validates a transaction draft before the service persists it.
///
/// # Errors
///
/// Returns [`LedgerValidationError`] when the draft violates SPEC-002 commit rules.
pub fn validate_transaction_for_commit(
    transaction: &LedgerTransactionDraft,
) -> Result<(), LedgerValidationError> {
    transaction.validate_balanced()
}

#[cfg(test)]
mod tests {
    use sdk::ledger::{LedgerAmount, LedgerEntryDraft, LedgerTransactionDraft};

    #[test]
    fn service_accepts_balanced_transaction_draft() {
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new("Bank.TrustAccount.TWD", LedgerAmount::from_points(100)),
            LedgerEntryDraft::new("Player.Wallet.user_01", LedgerAmount::from_points(-100)),
        ]);

        assert!(super::validate_transaction_for_commit(&transaction).is_ok());
    }

    #[test]
    fn service_rejects_imbalanced_transaction_draft() {
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Bank.TrustAccount.TWD",
                LedgerAmount::from_minor_units(10_000),
            ),
            LedgerEntryDraft::new(
                "Player.Wallet.user_01",
                LedgerAmount::from_minor_units(-9_999),
            ),
        ]);

        let Err(error) = super::validate_transaction_for_commit(&transaction) else {
            panic!("imbalanced transaction should be rejected");
        };

        assert_eq!(
            error.to_string(),
            "ledger transaction is imbalanced by 1 minor units"
        );
    }
}
