use async_trait::async_trait;
use thiserror::Error;

use crate::reconciliation::ReconciliationFinding;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettledPayment {
    pub provider: String,
    pub provider_transaction_id: String,
    pub settled_date: String,
    pub amount_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettledReversal {
    pub provider: String,
    pub provider_transaction_id: String,
    pub settled_date: String,
    pub amount_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedWebhook {
    pub provider: String,
    pub provider_transaction_id: String,
    pub idempotency_key: String,
    pub amount_minor_units: i64,
}

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("invalid webhook signature")]
    InvalidSignature,
    #[error("gateway adapter error: {0}")]
    Adapter(String),
}

#[async_trait]
pub trait PaymentGatewayAdapter {
    async fn list_settled_payments(&self, date: &str) -> Result<Vec<SettledPayment>, GatewayError>;

    async fn list_settled_reversals(
        &self,
        date: &str,
    ) -> Result<Vec<SettledReversal>, GatewayError>;

    async fn verify_webhook(
        &self,
        signature: &str,
        payload: &[u8],
    ) -> Result<VerifiedWebhook, GatewayError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerSettlementTotals {
    pub deposit_minor_units: i64,
    pub reversal_minor_units: i64,
}

pub fn reconcile_gateway_settlement(
    date: &str,
    payments: &[SettledPayment],
    reversals: &[SettledReversal],
    ledger_totals: LedgerSettlementTotals,
) -> Vec<ReconciliationFinding> {
    let payment_total = payments
        .iter()
        .map(|payment| payment.amount_minor_units)
        .sum::<i64>();
    let reversal_total = reversals
        .iter()
        .map(|reversal| reversal.amount_minor_units)
        .sum::<i64>();
    let mut findings = Vec::new();

    if payment_total != ledger_totals.deposit_minor_units {
        let mut finding = ReconciliationFinding::mismatch(
            "gateway_deposit_total_mismatch",
            Some("Bank.TrustAccount.TWD".to_owned()),
            payment_total,
            ledger_totals.deposit_minor_units,
        );
        finding.details = serde_json::json!({ "settled_date": date });
        findings.push(finding);
    }

    if reversal_total != ledger_totals.reversal_minor_units {
        let mut finding = ReconciliationFinding::mismatch(
            "gateway_reversal_total_mismatch",
            Some("Player.Wallet.*".to_owned()),
            reversal_total,
            ledger_totals.reversal_minor_units,
        );
        finding.details = serde_json::json!({ "settled_date": date });
        findings.push(finding);
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::{
        LedgerSettlementTotals, SettledPayment, SettledReversal, reconcile_gateway_settlement,
    };

    #[test]
    fn gateway_reconciliation_passes_when_totals_match() {
        let findings = reconcile_gateway_settlement(
            "2026-05-06",
            &[payment("gw-1", 10_000), payment("gw-2", 20_000)],
            &[reversal("rv-1", 5_000)],
            LedgerSettlementTotals {
                deposit_minor_units: 30_000,
                reversal_minor_units: 5_000,
            },
        );

        assert!(findings.is_empty());
    }

    #[test]
    fn gateway_reconciliation_reports_deposit_and_reversal_mismatches() {
        let findings = reconcile_gateway_settlement(
            "2026-05-06",
            &[payment("gw-1", 10_000), payment("gw-2", 20_000)],
            &[reversal("rv-1", 5_000)],
            LedgerSettlementTotals {
                deposit_minor_units: 29_000,
                reversal_minor_units: 4_000,
            },
        );

        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].finding_type, "gateway_deposit_total_mismatch");
        assert_eq!(findings[0].expected_amount, 30_000);
        assert_eq!(findings[0].actual_amount, 29_000);
        assert_eq!(findings[0].difference_amount, -1_000);
        assert_eq!(findings[1].finding_type, "gateway_reversal_total_mismatch");
        assert_eq!(findings[1].expected_amount, 5_000);
        assert_eq!(findings[1].actual_amount, 4_000);
        assert_eq!(findings[1].difference_amount, -1_000);
    }

    fn payment(provider_transaction_id: &str, amount_minor_units: i64) -> SettledPayment {
        SettledPayment {
            provider: "mock".to_owned(),
            provider_transaction_id: provider_transaction_id.to_owned(),
            settled_date: "2026-05-06".to_owned(),
            amount_minor_units,
        }
    }

    fn reversal(provider_transaction_id: &str, amount_minor_units: i64) -> SettledReversal {
        SettledReversal {
            provider: "mock".to_owned(),
            provider_transaction_id: provider_transaction_id.to_owned(),
            settled_date: "2026-05-06".to_owned(),
            amount_minor_units,
        }
    }
}
