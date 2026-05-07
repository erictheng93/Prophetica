pub const SERVICE_NAME: &str = "wallet-ledger-service";
pub const DEFAULT_PORT: u16 = 8086;

pub mod api;
pub mod archive;
pub mod gateway;
pub mod ledger;
pub mod reconciliation;

use sdk::ledger::{LedgerTransactionDraft, LedgerTxnKind, LedgerValidationError};

/// Maps `PostTransactionDraft::txn_type` string to typed [`LedgerTxnKind`].
/// Unknown `txn_type` defaults to [`LedgerTxnKind::Other`] (most restrictive
/// INV-6 mode); callers that need `Player.Wallet → Player.Wallet` settlement
/// **must** declare `txn_type = "trade"` or `"settle"`.
#[must_use]
pub fn map_txn_type_to_kind(txn_type: &str) -> LedgerTxnKind {
    match txn_type {
        "deposit" => LedgerTxnKind::Deposit,
        "order_lock" => LedgerTxnKind::OrderLock,
        "trade" => LedgerTxnKind::Trade,
        "settle" => LedgerTxnKind::Settle,
        "fee" => LedgerTxnKind::Fee,
        "freeze" => LedgerTxnKind::Freeze,
        "unfreeze" => LedgerTxnKind::Unfreeze,
        "cs_compensation" => LedgerTxnKind::Compensation,
        "chargeback" => LedgerTxnKind::Chargeback,
        _ => LedgerTxnKind::Other,
    }
}

/// Validates a transaction draft before the service persists it.
///
/// Runs INV-1 (balanced) and INV-6 (`Player.Wallet.*` outflow whitelist) — the
/// subset of SPEC-002 §5 invariants that can be checked statically before DB
/// commit. INV-2 / INV-3 / INV-4 are enforced by the reconciliation worker;
/// INV-5 is enforced by DB role permissions.
///
/// # Errors
///
/// Returns [`LedgerValidationError`] when the draft violates SPEC-002 commit rules.
pub fn validate_transaction_for_commit(
    transaction: &LedgerTransactionDraft,
) -> Result<(), LedgerValidationError> {
    transaction.validate()
}

#[cfg(test)]
mod tests {
    use super::map_txn_type_to_kind;
    use sdk::ledger::{
        LedgerAmount, LedgerEntryDraft, LedgerTransactionDraft, LedgerTxnKind,
        LedgerValidationError,
    };

    #[test]
    fn service_accepts_balanced_deposit_transaction() {
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new("Bank.TrustAccount.TWD", LedgerAmount::from_points(100)),
            LedgerEntryDraft::new("Player.Wallet.user_01", LedgerAmount::from_points(-100)),
        ])
        .with_kind(LedgerTxnKind::Deposit);

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

    #[test]
    fn service_rejects_inv6_wallet_to_accounts_payable_at_commit() {
        // R-1 紅線：即便 transaction balanced，禁止 Player.Wallet → AccountsPayable
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new("Player.Wallet.user_42", LedgerAmount::from_points(1_000)),
            LedgerEntryDraft::new(
                "Liability.AccountsPayable.partner_007",
                LedgerAmount::from_points(-1_000),
            ),
        ]);

        assert!(matches!(
            super::validate_transaction_for_commit(&transaction),
            Err(LedgerValidationError::Inv6Violation { .. })
        ));
    }

    #[test]
    fn txn_type_string_maps_to_typed_kind() {
        assert_eq!(map_txn_type_to_kind("deposit"), LedgerTxnKind::Deposit);
        assert_eq!(map_txn_type_to_kind("trade"), LedgerTxnKind::Trade);
        assert_eq!(map_txn_type_to_kind("settle"), LedgerTxnKind::Settle);
        assert_eq!(map_txn_type_to_kind("freeze"), LedgerTxnKind::Freeze);
        assert_eq!(map_txn_type_to_kind("unknown_type"), LedgerTxnKind::Other);
    }
}
