use std::sync::{Arc, Mutex};

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
    read_only: bool,
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
        self.state.lock().map_or(true, |state| state.read_only)
    }

    pub fn mark_synchronous_standby_lost(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.read_only = true;
            state.events.push(DurabilityEvent::SynchronousStandbyLost {
                alert: "engine.synchronous_standby_lost",
            });
        }
    }

    pub fn mark_synchronous_standby_healthy(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.read_only = false;
            state
                .events
                .push(DurabilityEvent::SynchronousStandbyRecovered);
        }
    }

    pub fn mark_engine_halted(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.read_only = true;
            state.events.push(DurabilityEvent::EngineHalted);
        }
    }

    #[must_use]
    pub fn events(&self) -> Vec<DurabilityEvent> {
        self.state.lock().map_or_else(
            |poisoned| poisoned.into_inner().events.clone(),
            |state| state.events.clone(),
        )
    }
}
