use std::sync::{Arc, Mutex};

use axum::{Json, Router, extract::Query, extract::State, routing::get};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrillLogDocument {
    pub pre_plan: String,
    pub execution_record: String,
    pub improvement_actions_24h: String,
}

impl DrillLogDocument {
    #[must_use]
    pub fn is_complete_for_regulator(&self) -> bool {
        !self.pre_plan.trim().is_empty()
            && !self.execution_record.trim().is_empty()
            && !self.improvement_actions_24h.trim().is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DrillLogType {
    EnginePodKill,
    DbPrimaryFailover,
    RegionFailover,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DrillLog {
    pub drill_id: String,
    pub drill_type: DrillLogType,
    pub scheduled_at_epoch_seconds: i64,
    pub executed_at_epoch_seconds: i64,
    pub rto_ms: i64,
    pub rpo_lost_events: i64,
    pub deviation_notes: String,
    pub improvement_actions: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct DrillLogQuery {
    pub from: i64,
    pub to: i64,
}

#[derive(Debug, Clone, Serialize)]
struct DrillLogListResponse {
    logs: Vec<DrillLog>,
}

pub trait DrillLogStore {
    /// Appends one drill log entry.
    ///
    /// # Errors
    ///
    /// Returns an error when the store cannot persist the entry.
    fn append(&self, log: DrillLog) -> Result<(), DrillLogStoreError>;

    fn query(&self, query: DrillLogQuery) -> Vec<DrillLog>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrillLogStoreError {
    message: String,
}

impl DrillLogStoreError {
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryDrillLogStore {
    logs: Arc<Mutex<Vec<DrillLog>>>,
}

impl DrillLogStore for InMemoryDrillLogStore {
    fn append(&self, log: DrillLog) -> Result<(), DrillLogStoreError> {
        let mut logs = self.logs.lock().map_err(|error| DrillLogStoreError {
            message: error.to_string(),
        })?;
        logs.push(log);
        Ok(())
    }

    fn query(&self, query: DrillLogQuery) -> Vec<DrillLog> {
        self.logs.lock().map_or_else(
            |poisoned| filter_logs(&poisoned.into_inner(), query),
            |logs| filter_logs(&logs, query),
        )
    }
}

pub fn internal_audit_router<S>(store: S) -> Router
where
    S: DrillLogStore + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/internal/audit/drill-logs", get(list_drill_logs::<S>))
        .with_state(store)
}

async fn list_drill_logs<S>(
    State(store): State<S>,
    Query(query): Query<DrillLogQuery>,
) -> Json<DrillLogListResponse>
where
    S: DrillLogStore,
{
    Json(DrillLogListResponse {
        logs: store.query(query),
    })
}

fn filter_logs(logs: &[DrillLog], query: DrillLogQuery) -> Vec<DrillLog> {
    logs.iter()
        .filter(|log| {
            log.executed_at_epoch_seconds >= query.from && log.executed_at_epoch_seconds <= query.to
        })
        .cloned()
        .collect()
}
