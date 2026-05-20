use sdk::feature_flags::{
    DEFAULT_ENGINE_HALT_POLL_INTERVAL, DEFAULT_PAYMENTS_REAL_MONEY_POLL_INTERVAL,
    ENGINE_HALTED_FLAG, EngineHaltFlagPoller, EngineHaltFlagReader, EngineHaltFlagWriter,
    InMemoryFeatureFlagStore, PAYMENTS_REAL_MONEY_ENABLED_FLAG, PaymentsRealMoneyFlagReader,
};

#[test]
fn engine_halt_flag_store_fans_out_current_snapshot() {
    let store = InMemoryFeatureFlagStore::default();

    store.halt_engine("incident-123", "ops incident", "senior-1");
    let halted = store.engine_halt_snapshot();

    assert_eq!(halted.key, ENGINE_HALTED_FLAG);
    assert!(halted.enabled);
    assert_eq!(halted.subject_id.as_deref(), Some("incident-123"));
    assert_eq!(halted.reason.as_deref(), Some("ops incident"));
    assert_eq!(halted.updated_by.as_deref(), Some("senior-1"));

    store.release_engine_halt("cto-1");
    let released = store.engine_halt_snapshot();

    assert_eq!(released.key, ENGINE_HALTED_FLAG);
    assert!(!released.enabled);
    assert_eq!(released.updated_by.as_deref(), Some("cto-1"));
}

#[test]
fn poller_applies_latest_snapshot_to_data_plane() {
    let store = InMemoryFeatureFlagStore::default();
    let poller = EngineHaltFlagPoller::new(store.clone());
    store.halt_engine("incident-123", "ops incident", "senior-1");
    let mut applied = Vec::new();

    poller.poll_once(|snapshot| applied.push(snapshot));

    assert_eq!(applied.len(), 1);
    assert!(applied[0].enabled);
    assert_eq!(applied[0].subject_id.as_deref(), Some("incident-123"));
}

#[test]
fn payments_real_money_flag_defaults_to_disabled_and_can_be_enabled() {
    let store = InMemoryFeatureFlagStore::default();

    assert_eq!(
        PAYMENTS_REAL_MONEY_ENABLED_FLAG,
        "payments.real_money_enabled"
    );
    assert!(!store.payments_real_money_enabled());

    store.set_payments_real_money_enabled(true);
    assert!(store.payments_real_money_enabled());
}

#[test]
fn payments_real_money_poll_interval_matches_engine_halt_cadence() {
    // SPEC-023 plan §11 D-6: flag flips must take effect on the same cadence
    // as engine halt for consistent operator expectations.
    assert_eq!(
        DEFAULT_PAYMENTS_REAL_MONEY_POLL_INTERVAL,
        DEFAULT_ENGINE_HALT_POLL_INTERVAL
    );
    assert!(DEFAULT_PAYMENTS_REAL_MONEY_POLL_INTERVAL.as_secs() <= 60);
}

#[test]
fn pg_feature_flag_queries_target_engine_halted_row() {
    assert!(
        sdk::feature_flags::PG_SELECT_ENGINE_HALTED_SQL.contains("WHERE key = 'engine.halted'")
    );
    assert!(sdk::feature_flags::PG_HALT_ENGINE_SQL.contains("ON CONFLICT (key) DO UPDATE"));
    assert!(sdk::feature_flags::PG_RELEASE_ENGINE_HALT_SQL.contains("WHERE key = 'engine.halted'"));
    assert!(
        sdk::feature_flags::PG_SELECT_PAYMENTS_REAL_MONEY_ENABLED_SQL
            .contains("WHERE key = 'payments.real_money_enabled'")
    );
}
