use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

pub const ENGINE_HALTED_FLAG: &str = "engine.halted";
pub const PAYMENTS_REAL_MONEY_ENABLED_FLAG: &str = "payments.real_money_enabled";
pub const DEFAULT_ENGINE_HALT_POLL_INTERVAL: Duration = Duration::from_secs(30);
pub const DEFAULT_PAYMENTS_REAL_MONEY_POLL_INTERVAL: Duration = Duration::from_secs(30);
pub const PG_SELECT_ENGINE_HALTED_SQL: &str = "\
SELECT enabled, subject_id, reason, updated_by \
FROM feature_flags \
WHERE key = 'engine.halted'";
pub const PG_SELECT_PAYMENTS_REAL_MONEY_ENABLED_SQL: &str = "\
SELECT enabled \
FROM feature_flags \
WHERE key = 'payments.real_money_enabled'";
pub const PG_HALT_ENGINE_SQL: &str = "\
INSERT INTO feature_flags (key, enabled, subject_id, reason, updated_by, updated_at) \
VALUES ('engine.halted', TRUE, $1, $2, $3, now()) \
ON CONFLICT (key) DO UPDATE \
SET enabled = EXCLUDED.enabled, \
    subject_id = EXCLUDED.subject_id, \
    reason = EXCLUDED.reason, \
    updated_by = EXCLUDED.updated_by, \
    updated_at = now()";
pub const PG_RELEASE_ENGINE_HALT_SQL: &str = "\
UPDATE feature_flags \
SET enabled = FALSE, subject_id = NULL, reason = NULL, updated_by = $1, updated_at = now() \
WHERE key = 'engine.halted'";

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

pub trait PaymentsRealMoneyFlagReader {
    fn payments_real_money_enabled(&self) -> bool;
}

#[derive(Debug, Clone)]
pub struct EngineHaltFlagPoller<R> {
    reader: R,
    interval: Duration,
}

impl<R> EngineHaltFlagPoller<R>
where
    R: EngineHaltFlagReader,
{
    #[must_use]
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            interval: DEFAULT_ENGINE_HALT_POLL_INTERVAL,
        }
    }

    #[must_use]
    pub const fn with_interval(reader: R, interval: Duration) -> Self {
        Self { reader, interval }
    }

    pub fn poll_once(&self, mut apply: impl FnMut(HaltFlagSnapshot)) {
        apply(self.reader.engine_halt_snapshot());
    }

    pub async fn run(self, mut apply: impl FnMut(HaltFlagSnapshot) + Send + 'static)
    where
        R: Send + Sync + 'static,
    {
        let mut interval = tokio::time::interval(self.interval);
        loop {
            interval.tick().await;
            apply(self.reader.engine_halt_snapshot());
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryFeatureFlagStore {
    state: Arc<Mutex<FeatureFlagState>>,
}

#[derive(Debug, Clone, Default)]
struct FeatureFlagState {
    engine_halt: HaltFlagSnapshot,
    payments_real_money_enabled: bool,
}

impl EngineHaltFlagReader for InMemoryFeatureFlagStore {
    fn engine_halt_snapshot(&self) -> HaltFlagSnapshot {
        self.state.lock().map_or_else(
            |poisoned| poisoned.into_inner().engine_halt.clone(),
            |state| state.engine_halt.clone(),
        )
    }
}

impl EngineHaltFlagWriter for InMemoryFeatureFlagStore {
    fn halt_engine(&self, subject_id: &str, reason: &str, updated_by: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.engine_halt = HaltFlagSnapshot::enabled(subject_id, reason, updated_by);
        }
    }

    fn release_engine_halt(&self, updated_by: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.engine_halt = HaltFlagSnapshot::disabled(updated_by);
        }
    }
}

impl PaymentsRealMoneyFlagReader for InMemoryFeatureFlagStore {
    fn payments_real_money_enabled(&self) -> bool {
        self.state
            .lock()
            .is_ok_and(|state| state.payments_real_money_enabled)
    }
}

impl InMemoryFeatureFlagStore {
    pub fn set_payments_real_money_enabled(&self, enabled: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.payments_real_money_enabled = enabled;
        }
    }
}

#[derive(Debug, Clone)]
pub struct PgFeatureFlagStore {
    pool: sqlx::PgPool,
}

impl PgFeatureFlagStore {
    #[must_use]
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Loads the current `engine.halted` row from Postgres.
    ///
    /// # Errors
    ///
    /// Returns [`sqlx::Error`] when the query fails.
    pub async fn load_engine_halt_snapshot(&self) -> Result<HaltFlagSnapshot, sqlx::Error> {
        let row = sqlx::query_as::<_, (bool, Option<String>, Option<String>, Option<String>)>(
            PG_SELECT_ENGINE_HALTED_SQL,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map_or_else(
            HaltFlagSnapshot::default,
            |(enabled, subject_id, reason, updated_by)| HaltFlagSnapshot {
                key: ENGINE_HALTED_FLAG,
                enabled,
                subject_id,
                reason,
                updated_by,
            },
        ))
    }

    /// Loads the current `payments.real_money_enabled` row from Postgres.
    ///
    /// # Errors
    ///
    /// Returns [`sqlx::Error`] when the query fails.
    pub async fn load_payments_real_money_enabled(&self) -> Result<bool, sqlx::Error> {
        let row = sqlx::query_as::<_, (bool,)>(PG_SELECT_PAYMENTS_REAL_MONEY_ENABLED_SQL)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.is_some_and(|(enabled,)| enabled))
    }

    /// Sets `engine.halted=true` in Postgres.
    ///
    /// # Errors
    ///
    /// Returns [`sqlx::Error`] when the write fails.
    pub async fn halt_engine_pg(
        &self,
        subject_id: &str,
        reason: &str,
        updated_by: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(PG_HALT_ENGINE_SQL)
            .bind(subject_id)
            .bind(reason)
            .bind(updated_by)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Sets `engine.halted=false` in Postgres.
    ///
    /// # Errors
    ///
    /// Returns [`sqlx::Error`] when the write fails.
    pub async fn release_engine_halt_pg(&self, updated_by: &str) -> Result<(), sqlx::Error> {
        sqlx::query(PG_RELEASE_ENGINE_HALT_SQL)
            .bind(updated_by)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
