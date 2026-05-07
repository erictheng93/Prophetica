use sdk::feature_flags::{
    ENGINE_HALTED_FLAG, EngineHaltFlagPoller, EngineHaltFlagReader, EngineHaltFlagWriter,
    InMemoryFeatureFlagStore,
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
fn pg_feature_flag_queries_target_engine_halted_row() {
    assert!(
        sdk::feature_flags::PG_SELECT_ENGINE_HALTED_SQL.contains("WHERE key = 'engine.halted'")
    );
    assert!(sdk::feature_flags::PG_HALT_ENGINE_SQL.contains("ON CONFLICT (key) DO UPDATE"));
    assert!(sdk::feature_flags::PG_RELEASE_ENGINE_HALT_SQL.contains("WHERE key = 'engine.halted'"));
}
