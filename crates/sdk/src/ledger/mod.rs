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
    txn_kind: LedgerTxnKind,
}

impl LedgerTransactionDraft {
    /// Creates a transaction draft from entry drafts. Kind defaults to
    /// [`LedgerTxnKind::Other`] (the most restrictive INV-6 mode); use
    /// [`Self::with_kind`] to declare a specific transaction type when
    /// `Player.Wallet → Player.Wallet` is required (i.e. trade settlement).
    pub fn new(entries: impl IntoIterator<Item = LedgerEntryDraft>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
            txn_kind: LedgerTxnKind::Other,
        }
    }

    /// Sets the transaction kind. Builder style; consumes and returns the draft.
    #[must_use]
    pub fn with_kind(mut self, kind: LedgerTxnKind) -> Self {
        self.txn_kind = kind;
        self
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

    /// Validates SPEC-002 INV-6 / ADR-0006 §4.1: `Player.Wallet.*` outflow whitelist.
    ///
    /// Whenever the transaction contains an outflow on a `Player.Wallet.*` account
    /// (debit, i.e. `minor_units > 0` since wallet is a liability), every credit
    /// counterpart (`minor_units < 0`) must point to an allow-listed account:
    /// - `Player.Locked.*` (lock for resting orders)
    /// - `Player.Frozen.*` (self-exclusion / compliance / EDD)
    /// - `Revenue.TradingFee` / `Revenue.SettlementFee`
    /// - `Player.Wallet.*` (only when [`Self::txn_kind`] is [`LedgerTxnKind::Trade`]
    ///   or [`LedgerTxnKind::Settle`])
    ///
    /// Any other counterpart account (e.g. `Liability.AccountsPayable.*`,
    /// `Bank.OperatingAccount.*`, `Expense.AffiliatePartnerFee`, any banking /
    /// payment-rail account) violates the no-cash-out red line.
    ///
    /// This is the application-layer mirror of the DB CONSTRAINT TRIGGER defined
    /// in `migrations/202605070004_inv6_outflow_whitelist.sql`. Both layers must
    /// remain consistent — see ADR-0006 §4.2.
    ///
    /// # Errors
    ///
    /// Returns [`LedgerValidationError::Inv6Violation`] when a `Player.Wallet.*`
    /// outflow's counterpart is outside the allow-list.
    pub fn validate_player_wallet_outflow_whitelist(&self) -> Result<(), LedgerValidationError> {
        // 是否有任一 Player.Wallet.* 為 outflow（debit，minor_units > 0）
        let has_wallet_outflow = self.entries.iter().any(|entry| {
            is_player_wallet_account(&entry.account_code) && entry.amount.minor_units() > 0
        });

        if !has_wallet_outflow {
            return Ok(());
        }

        // 任一 credit（minor_units < 0）之 account_code 必須在白名單中
        for entry in &self.entries {
            if entry.amount.minor_units() >= 0 {
                continue;
            }
            if !self.is_inv6_whitelisted_target(&entry.account_code) {
                return Err(LedgerValidationError::Inv6Violation {
                    account_code: entry.account_code.clone(),
                });
            }
        }

        Ok(())
    }

    /// Validates the full ledger invariant set required at draft time:
    /// INV-1 (balanced) + INV-6 (`Player.Wallet.*` outflow whitelist).
    ///
    /// INV-2 (account-sum reconciliation), INV-3 (`TrustAccount` equality),
    /// INV-4 (non-negative wallet), and INV-5 (append-only) are enforced at
    /// reconciliation / commit time elsewhere; this method covers the subset
    /// that can be checked statically before the DB sees the transaction.
    ///
    /// # Errors
    ///
    /// Propagates the first invariant violation encountered.
    pub fn validate(&self) -> Result<(), LedgerValidationError> {
        self.validate_balanced()?;
        self.validate_player_wallet_outflow_whitelist()?;
        Ok(())
    }

    /// Returns the transaction entries.
    #[must_use]
    pub fn entries(&self) -> &[LedgerEntryDraft] {
        &self.entries
    }

    /// Returns the declared transaction kind, used by INV-6 to allow
    /// `Player.Wallet → Player.Wallet` only on `Trade` / `Settle`.
    #[must_use]
    pub const fn txn_kind(&self) -> LedgerTxnKind {
        self.txn_kind
    }

    fn is_inv6_whitelisted_target(&self, code: &str) -> bool {
        if code.starts_with("Player.Locked.") || code.starts_with("Player.Frozen.") {
            return true;
        }
        if code == "Revenue.TradingFee" || code == "Revenue.SettlementFee" {
            return true;
        }
        // Player.Wallet → Player.Wallet 僅在 trade / settle 時允許（呼應 ADR-0006 §4.1）
        if is_player_wallet_account(code) {
            return matches!(self.txn_kind, LedgerTxnKind::Trade | LedgerTxnKind::Settle);
        }
        // Player.Wallet → Bank.TrustAccount.* 僅在 chargeback reversal 時允許
        // （呼應 SPEC-002 §8.1：玩家信用卡爭議 → 點數 reverse + 託管帳戶退款給卡組織）。
        // 這**不是**「點數變現」之路徑（玩家不取得現金），而是 deposit 之反向沖正，
        // 故不屬於點數變現路徑。
        if code.starts_with("Bank.TrustAccount.") {
            return matches!(self.txn_kind, LedgerTxnKind::Chargeback);
        }
        false
    }
}

/// Ledger transaction kind. Carried into invariant checks so that INV-6 can
/// distinguish trade / settle (where `Player.Wallet → Player.Wallet` is legal)
/// from deposit / lock / fee / refund / reversal (where it is not relevant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LedgerTxnKind {
    /// Deposit from payment gateway → player wallet.
    Deposit,
    /// Funds locked for a resting order (Player.Wallet → Player.Locked).
    OrderLock,
    /// Trade match settlement (Player.Wallet → Player.Wallet).
    Trade,
    /// Market settlement (Player.Locked → Player.Wallet of winners).
    Settle,
    /// Fee deduction (Player.Wallet → Revenue.*).
    Fee,
    /// Self-exclusion / compliance freeze (Player.Wallet → Player.Frozen).
    Freeze,
    /// Unfreeze on expiry / clearance (Player.Frozen → Player.Wallet).
    Unfreeze,
    /// Customer-service compensation reversal (4-eye, MS-6).
    Compensation,
    /// Chargeback reversal.
    Chargeback,
    /// Catch-all for non-classified drafts (defaults to most restrictive INV-6 mode).
    #[default]
    Other,
}

#[inline]
fn is_player_wallet_account(code: &str) -> bool {
    code.starts_with("Player.Wallet.")
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
    /// SPEC-002 §5 INV-6 / ADR-0006 §4.1: a `Player.Wallet.*` outflow targets
    /// an account outside the allow-list (e.g. `Liability.AccountsPayable.*`,
    /// `Bank.OperatingAccount.*`, `Expense.AffiliatePartnerFee`).
    /// Connects to the no-cash-out red line and SPEC-011 §6.3.
    #[error(
        "INV-6 violation: Player.Wallet outflow targets non-whitelisted account {account_code} \
         (see ADR-0006 §4.1 / SPEC-011 §6.3)"
    )]
    Inv6Violation {
        /// Account code that is not in the INV-6 whitelist.
        account_code: String,
    },
}

#[cfg(test)]
mod tests {
    use super::{
        LedgerAmount, LedgerEntryDraft, LedgerTransactionDraft, LedgerTxnKind,
        LedgerValidationError,
    };

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

    // -------------------------------------------------------------------
    // INV-6 / ADR-0006 §4.1: Player.Wallet.* outflow whitelist tests
    // -------------------------------------------------------------------

    #[test]
    fn inv6_deposit_has_no_wallet_outflow_so_passes() {
        // Deposit: Bank.TrustAccount debit / Player.Wallet credit.
        // Wallet 是 inflow（minor_units < 0），不觸發 INV-6。
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Bank.TrustAccount.TWD",
                LedgerAmount::from_minor_units(100_000),
            ),
            LedgerEntryDraft::new(
                "Player.Wallet.user_42",
                LedgerAmount::from_minor_units(-100_000),
            ),
        ])
        .with_kind(LedgerTxnKind::Deposit);

        assert!(transaction.validate().is_ok());
    }

    #[test]
    fn inv6_order_lock_to_player_locked_passes() {
        // OrderLock: Player.Wallet outflow → Player.Locked inflow（白名單）
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Player.Wallet.user_42",
                LedgerAmount::from_minor_units(50_000),
            ),
            LedgerEntryDraft::new(
                "Player.Locked.user_42",
                LedgerAmount::from_minor_units(-50_000),
            ),
        ])
        .with_kind(LedgerTxnKind::OrderLock);

        assert!(transaction.validate().is_ok());
    }

    #[test]
    fn inv6_fee_to_revenue_trading_fee_passes() {
        // Fee: Player.Wallet outflow → Revenue.TradingFee inflow（白名單）
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new("Player.Wallet.user_42", LedgerAmount::from_minor_units(130)),
            LedgerEntryDraft::new("Revenue.TradingFee", LedgerAmount::from_minor_units(-130)),
        ])
        .with_kind(LedgerTxnKind::Fee);

        assert!(transaction.validate().is_ok());
    }

    #[test]
    fn inv6_freeze_to_player_frozen_passes() {
        // Freeze: Player.Wallet outflow → Player.Frozen inflow（白名單）
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Player.Wallet.user_42",
                LedgerAmount::from_minor_units(10_000),
            ),
            LedgerEntryDraft::new(
                "Player.Frozen.user_42",
                LedgerAmount::from_minor_units(-10_000),
            ),
        ])
        .with_kind(LedgerTxnKind::Freeze);

        assert!(transaction.validate().is_ok());
    }

    #[test]
    fn inv6_trade_settlement_player_to_player_passes() {
        // Trade: Player.Wallet.A outflow → Player.Wallet.B inflow（限 Trade/Settle 白名單）
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Player.Wallet.user_alice",
                LedgerAmount::from_minor_units(6_500),
            ),
            LedgerEntryDraft::new(
                "Player.Wallet.user_bob",
                LedgerAmount::from_minor_units(-6_500),
            ),
        ])
        .with_kind(LedgerTxnKind::Trade);

        assert!(transaction.validate().is_ok());
    }

    #[test]
    fn inv6_settle_player_to_player_passes() {
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Player.Wallet.user_alice",
                LedgerAmount::from_minor_units(10_000),
            ),
            LedgerEntryDraft::new(
                "Player.Wallet.user_bob",
                LedgerAmount::from_minor_units(-10_000),
            ),
        ])
        .with_kind(LedgerTxnKind::Settle);

        assert!(transaction.validate().is_ok());
    }

    #[test]
    fn inv6_rejects_wallet_to_accounts_payable() {
        // R-1 紅線：禁止 Player.Wallet → Liability.AccountsPayable.*
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Player.Wallet.user_42",
                LedgerAmount::from_minor_units(100_000),
            ),
            LedgerEntryDraft::new(
                "Liability.AccountsPayable.partner_007",
                LedgerAmount::from_minor_units(-100_000),
            ),
        ])
        .with_kind(LedgerTxnKind::Other);

        let Err(error) = transaction.validate() else {
            panic!("Player.Wallet → AccountsPayable must violate INV-6");
        };

        assert_eq!(
            error,
            LedgerValidationError::Inv6Violation {
                account_code: "Liability.AccountsPayable.partner_007".to_owned(),
            }
        );
    }

    #[test]
    fn inv6_rejects_wallet_to_bank_operating_account() {
        // R-1 紅線：禁止 Player.Wallet → Bank.OperatingAccount.*
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Player.Wallet.user_42",
                LedgerAmount::from_minor_units(50_000),
            ),
            LedgerEntryDraft::new(
                "Bank.OperatingAccount.TWD",
                LedgerAmount::from_minor_units(-50_000),
            ),
        ]);

        let Err(LedgerValidationError::Inv6Violation { account_code }) = transaction.validate()
        else {
            panic!("Player.Wallet → Bank.OperatingAccount must violate INV-6");
        };
        assert_eq!(account_code, "Bank.OperatingAccount.TWD");
    }

    #[test]
    fn inv6_rejects_wallet_to_affiliate_partner_fee() {
        // R-1 紅線：禁止 Player.Wallet → Expense.AffiliatePartnerFee
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Player.Wallet.user_42",
                LedgerAmount::from_minor_units(20_000),
            ),
            LedgerEntryDraft::new(
                "Expense.AffiliatePartnerFee",
                LedgerAmount::from_minor_units(-20_000),
            ),
        ]);

        assert!(matches!(
            transaction.validate(),
            Err(LedgerValidationError::Inv6Violation { .. })
        ));
    }

    #[test]
    fn inv6_rejects_wallet_to_wallet_when_kind_is_not_trade_or_settle() {
        // 即便 Player.Wallet → Player.Wallet 也不允許（除 Trade / Settle）
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Player.Wallet.user_alice",
                LedgerAmount::from_minor_units(10_000),
            ),
            LedgerEntryDraft::new(
                "Player.Wallet.user_bob",
                LedgerAmount::from_minor_units(-10_000),
            ),
        ])
        .with_kind(LedgerTxnKind::Other);

        let Err(LedgerValidationError::Inv6Violation { account_code }) = transaction.validate()
        else {
            panic!("Player.Wallet → Player.Wallet must require Trade/Settle kind");
        };
        assert_eq!(account_code, "Player.Wallet.user_bob");
    }

    #[test]
    fn inv6_default_kind_is_other_and_strictest() {
        // new() 預設 LedgerTxnKind::Other，撮合 / 結算之外的 transaction 必須走 with_kind 顯式宣告
        let draft = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Player.Wallet.user_42",
                LedgerAmount::from_minor_units(1_000),
            ),
            LedgerEntryDraft::new(
                "Player.Wallet.user_99",
                LedgerAmount::from_minor_units(-1_000),
            ),
        ]);
        assert_eq!(draft.txn_kind(), LedgerTxnKind::Other);
        assert!(matches!(
            draft.validate(),
            Err(LedgerValidationError::Inv6Violation { .. })
        ));
    }

    #[test]
    fn inv6_chargeback_to_bank_trust_account_passes() {
        // SPEC-002 §8.1 chargeback：玩家信用卡爭議 → 點數 reverse + 託管帳戶退款給卡組織
        // Player.Wallet outflow → Bank.TrustAccount inflow（限 LedgerTxnKind::Chargeback）
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new("Player.Wallet.user_42", LedgerAmount::from_minor_units(1)),
            LedgerEntryDraft::new("Bank.TrustAccount.TWD", LedgerAmount::from_minor_units(-1)),
        ])
        .with_kind(LedgerTxnKind::Chargeback);

        assert!(
            transaction.validate().is_ok(),
            "chargeback Player.Wallet → Bank.TrustAccount should be allowed (SPEC-002 §8.1)"
        );
    }

    #[test]
    fn inv6_rejects_wallet_to_bank_trust_account_without_chargeback_kind() {
        // 沒標 Chargeback kind 時，Player.Wallet → Bank.TrustAccount 仍應被擋下
        // 防止「假裝是 trade」之類混用攻擊
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new("Player.Wallet.user_42", LedgerAmount::from_points(1_000)),
            LedgerEntryDraft::new("Bank.TrustAccount.TWD", LedgerAmount::from_points(-1_000)),
        ])
        .with_kind(LedgerTxnKind::Trade); // 錯誤宣告為 trade

        assert!(matches!(
            transaction.validate(),
            Err(LedgerValidationError::Inv6Violation { .. })
        ));
    }

    #[test]
    fn inv6_unfreeze_player_frozen_to_wallet_passes() {
        // Unfreeze: Player.Frozen outflow → Player.Wallet inflow，**Player.Wallet 是 credit**
        // 所以 INV-6 不觸發（INV-6 只看 Player.Wallet outflow）
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Player.Frozen.user_42",
                LedgerAmount::from_minor_units(10_000),
            ),
            LedgerEntryDraft::new(
                "Player.Wallet.user_42",
                LedgerAmount::from_minor_units(-10_000),
            ),
        ])
        .with_kind(LedgerTxnKind::Unfreeze);

        assert!(transaction.validate().is_ok());
    }

    #[test]
    fn inv6_balanced_check_runs_first_in_validate() {
        // 如果 transaction 失衡，應該先回 Imbalanced，不會走到 INV-6 check
        let transaction = LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Player.Wallet.user_42",
                LedgerAmount::from_minor_units(100_000),
            ),
            LedgerEntryDraft::new(
                "Liability.AccountsPayable.partner_007",
                LedgerAmount::from_minor_units(-99_000), // 失衡 1000 minor units
            ),
        ]);

        let Err(error) = transaction.validate() else {
            panic!("imbalanced transaction must be rejected before INV-6");
        };

        // 應該先觸發 Imbalanced 而非 Inv6Violation
        assert!(matches!(error, LedgerValidationError::Imbalanced { .. }));
    }
}
