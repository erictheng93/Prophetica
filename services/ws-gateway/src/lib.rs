pub const SERVICE_NAME: &str = "ws-gateway";
pub const DEFAULT_PORT: u16 = 8083;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementStatusPayload {
    pub event: &'static str,
    pub market_id: String,
    pub status: String,
    pub result: Option<String>,
    pub dispute_until: Option<String>,
    pub your_payout: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineStatePayload {
    pub event: &'static str,
    pub state: &'static str,
}

#[must_use]
pub const fn halted_state_payload() -> EngineStatePayload {
    EngineStatePayload {
        event: "engine_state",
        state: "HALTED",
    }
}

#[must_use]
pub const fn resumed_state_payload() -> EngineStatePayload {
    EngineStatePayload {
        event: "engine_state",
        state: "OPEN",
    }
}

#[must_use]
pub const fn engine_state_payload_from_halt_snapshot(
    snapshot: &sdk::feature_flags::HaltFlagSnapshot,
) -> Option<EngineStatePayload> {
    if snapshot.enabled {
        Some(halted_state_payload())
    } else {
        None
    }
}

#[must_use]
pub fn settlement_status_payload(
    market_id: impl Into<String>,
    status: impl Into<String>,
    result: Option<impl Into<String>>,
    dispute_until: Option<impl Into<String>>,
    your_payout: Option<i64>,
) -> SettlementStatusPayload {
    SettlementStatusPayload {
        event: "state_change",
        market_id: market_id.into(),
        status: status.into(),
        result: result.map(Into::into),
        dispute_until: dispute_until.map(Into::into),
        your_payout,
    }
}

#[cfg(test)]
mod tests {
    use super::{halted_state_payload, settlement_status_payload};

    #[test]
    fn pending_payload_includes_result_and_dispute_deadline() {
        let payload = settlement_status_payload(
            "mkt-election",
            "PENDING",
            Some("YES"),
            Some("2026-06-02T00:00:00+08:00"),
            None,
        );

        assert_eq!(payload.event, "state_change");
        assert_eq!(payload.market_id, "mkt-election");
        assert_eq!(payload.status, "PENDING");
        assert_eq!(payload.result.as_deref(), Some("YES"));
        assert_eq!(
            payload.dispute_until.as_deref(),
            Some("2026-06-02T00:00:00+08:00")
        );
    }

    #[test]
    fn settled_payload_includes_user_payout() {
        let payload = settlement_status_payload(
            "mkt-election",
            "SETTLED",
            Option::<String>::None,
            Option::<String>::None,
            Some(6_500),
        );

        assert_eq!(payload.status, "SETTLED");
        assert_eq!(payload.your_payout, Some(6_500));
    }

    #[test]
    fn halted_state_payload_notifies_clients() {
        let payload = halted_state_payload();

        assert_eq!(payload.event, "engine_state");
        assert_eq!(payload.state, "HALTED");
    }

    #[test]
    fn shared_halt_snapshot_drives_engine_state_payload() {
        let snapshot = sdk::feature_flags::HaltFlagSnapshot::enabled(
            "incident-123",
            "ops incident",
            "senior-1",
        );

        let Some(payload) = super::engine_state_payload_from_halt_snapshot(&snapshot) else {
            panic!("enabled halt snapshot should produce a broadcast payload");
        };

        assert_eq!(payload.event, "engine_state");
        assert_eq!(payload.state, "HALTED");

        assert!(
            super::engine_state_payload_from_halt_snapshot(
                &sdk::feature_flags::HaltFlagSnapshot::disabled("cto-1")
            )
            .is_none()
        );
    }
}
