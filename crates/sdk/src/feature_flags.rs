use std::sync::{Arc, Mutex};

pub const ENGINE_HALTED_FLAG: &str = "engine.halted";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HaltFlagSnapshot {
    pub key: &'static str,
    pub enabled: bool,
    pub subject_id: Option<String>,
    pub reason: Option<String>,
    pub updated_by: Option<String>,
}

impl HaltFlagSnapshot {
    #[must_use]
    pub fn enabled(
        subject_id: impl Into<String>,
        reason: impl Into<String>,
        updated_by: impl Into<String>,
    ) -> Self {
        Self {
            key: ENGINE_HALTED_FLAG,
            enabled: true,
            subject_id: Some(subject_id.into()),
            reason: Some(reason.into()),
            updated_by: Some(updated_by.into()),
        }
    }

    #[must_use]
    pub fn disabled(updated_by: impl Into<String>) -> Self {
        Self {
            key: ENGINE_HALTED_FLAG,
            enabled: false,
            subject_id: None,
            reason: None,
            updated_by: Some(updated_by.into()),
        }
    }
}

impl Default for HaltFlagSnapshot {
    fn default() -> Self {
        Self {
            key: ENGINE_HALTED_FLAG,
            enabled: false,
            subject_id: None,
            reason: None,
            updated_by: None,
        }
    }
}

pub trait EngineHaltFlagReader {
    fn engine_halt_snapshot(&self) -> HaltFlagSnapshot;
}

pub trait EngineHaltFlagWriter {
    fn halt_engine(&self, subject_id: &str, reason: &str, updated_by: &str);
    fn release_engine_halt(&self, updated_by: &str);
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryFeatureFlagStore {
    state: Arc<Mutex<HaltFlagSnapshot>>,
}

impl EngineHaltFlagReader for InMemoryFeatureFlagStore {
    fn engine_halt_snapshot(&self) -> HaltFlagSnapshot {
        self.state.lock().map_or_else(
            |poisoned| poisoned.into_inner().clone(),
            |snapshot| snapshot.clone(),
        )
    }
}

impl EngineHaltFlagWriter for InMemoryFeatureFlagStore {
    fn halt_engine(&self, subject_id: &str, reason: &str, updated_by: &str) {
        if let Ok(mut snapshot) = self.state.lock() {
            *snapshot = HaltFlagSnapshot::enabled(subject_id, reason, updated_by);
        }
    }

    fn release_engine_halt(&self, updated_by: &str) {
        if let Ok(mut snapshot) = self.state.lock() {
            *snapshot = HaltFlagSnapshot::disabled(updated_by);
        }
    }
}
