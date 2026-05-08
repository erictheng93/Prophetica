use std::sync::{Arc, Mutex};

use sdk::feature_flags::{EngineHaltFlagReader, HaltFlagSnapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurabilityEvent {
    SynchronousStandbyLost { alert: &'static str },
    SynchronousStandbyRecovered,
    EngineHalted,
}

#[derive(Debug, Clone)]
pub struct EngineDurabilityGuard {
    state: Arc<Mutex<DurabilityState>>,
}

#[derive(Debug, Default)]
struct DurabilityState {
    synchronous_standby_lost: bool,
    engine_halted: bool,
    events: Vec<DurabilityEvent>,
}

impl Default for EngineDurabilityGuard {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(DurabilityState::default())),
        }
    }
}

impl EngineDurabilityGuard {
    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.state.lock().map_or(true, |state| {
            state.synchronous_standby_lost || state.engine_halted
        })
    }

    pub fn mark_synchronous_standby_lost(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.synchronous_standby_lost = true;
            state.events.push(DurabilityEvent::SynchronousStandbyLost {
                alert: "engine.synchronous_standby_lost",
            });
        }
    }

    pub fn mark_synchronous_standby_healthy(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.synchronous_standby_lost = false;
            state
                .events
                .push(DurabilityEvent::SynchronousStandbyRecovered);
        }
    }

    pub fn mark_engine_halted(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.engine_halted = true;
            state.events.push(DurabilityEvent::EngineHalted);
        }
    }

    pub fn apply_engine_halt_snapshot(&self, snapshot: &HaltFlagSnapshot) {
        if let Ok(mut state) = self.state.lock() {
            if snapshot.enabled && !state.engine_halted {
                state.events.push(DurabilityEvent::EngineHalted);
            }
            state.engine_halted = snapshot.enabled;
        }
    }

    pub fn apply_engine_halt_reader(&self, reader: &impl EngineHaltFlagReader) {
        self.apply_engine_halt_snapshot(&reader.engine_halt_snapshot());
    }

    #[must_use]
    pub fn is_engine_halted(&self) -> bool {
        self.state.lock().map_or(true, |state| state.engine_halted)
    }

    #[must_use]
    pub fn events(&self) -> Vec<DurabilityEvent> {
        self.state.lock().map_or_else(
            |poisoned| poisoned.into_inner().events.clone(),
            |state| state.events.clone(),
        )
    }
}
