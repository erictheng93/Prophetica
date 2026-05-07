use sdk::feature_flags::{
    ENGINE_HALTED_FLAG, EngineHaltFlagReader, EngineHaltFlagWriter, InMemoryFeatureFlagStore,
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
