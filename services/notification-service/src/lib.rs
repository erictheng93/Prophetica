pub const SERVICE_NAME: &str = "notification-service";
pub const DEFAULT_PORT: u16 = 8093;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementNotificationChannel {
    Email,
    InApp,
    WebSocket,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementNotificationReceipt {
    pub user_id: String,
    pub channel: SettlementNotificationChannel,
    pub delivered_within_24h: bool,
}

impl SettlementNotificationReceipt {
    #[must_use]
    pub fn delivered(user_id: impl Into<String>, channel: SettlementNotificationChannel) -> Self {
        Self {
            user_id: user_id.into(),
            channel,
            delivered_within_24h: true,
        }
    }

    #[must_use]
    pub fn missed(user_id: impl Into<String>, channel: SettlementNotificationChannel) -> Self {
        Self {
            user_id: user_id.into(),
            channel,
            delivered_within_24h: false,
        }
    }
}

#[must_use]
pub fn settlement_announcement_delivery_rate_bps(
    receipts: &[SettlementNotificationReceipt],
) -> u32 {
    if receipts.is_empty() {
        return 10_000;
    }

    let delivered = receipts
        .iter()
        .filter(|receipt| receipt.delivered_within_24h)
        .count();
    u32::try_from((delivered * 10_000) / receipts.len()).unwrap_or(10_000)
}

#[must_use]
pub fn meets_settlement_announcement_slo(receipts: &[SettlementNotificationReceipt]) -> bool {
    if receipts.is_empty() {
        return true;
    }

    let delivered = receipts
        .iter()
        .filter(|receipt| receipt.delivered_within_24h)
        .count();
    delivered * 1_000 >= receipts.len() * 999
}

#[cfg(test)]
mod tests {
    use super::{
        SettlementNotificationChannel, SettlementNotificationReceipt,
        meets_settlement_announcement_slo, settlement_announcement_delivery_rate_bps,
    };

    #[test]
    fn settlement_announcement_delivery_rate_counts_24h_receipts() {
        let receipts = [
            SettlementNotificationReceipt::delivered(
                "user_01",
                SettlementNotificationChannel::Email,
            ),
            SettlementNotificationReceipt::delivered(
                "user_01",
                SettlementNotificationChannel::InApp,
            ),
            SettlementNotificationReceipt::missed(
                "user_02",
                SettlementNotificationChannel::WebSocket,
            ),
        ];

        assert_eq!(settlement_announcement_delivery_rate_bps(&receipts), 6_666);
    }

    #[test]
    fn settlement_announcement_slo_requires_99_9_percent_delivery() {
        let mut receipts = (0..1_000)
            .map(|i| {
                SettlementNotificationReceipt::delivered(
                    format!("user_{i}"),
                    SettlementNotificationChannel::Email,
                )
            })
            .collect::<Vec<_>>();

        assert!(meets_settlement_announcement_slo(&receipts));

        receipts.push(SettlementNotificationReceipt::missed(
            "late_user",
            SettlementNotificationChannel::Email,
        ));
        receipts.push(SettlementNotificationReceipt::missed(
            "late_user_2",
            SettlementNotificationChannel::Email,
        ));
        assert!(!meets_settlement_announcement_slo(&receipts));
    }
}
