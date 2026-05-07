use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use sdk::feature_flags::{EngineHaltFlagWriter, InMemoryFeatureFlagStore};
use serde::{Deserialize, Serialize};

pub trait HaltFlagStore {
    /// Sets `engine.halted=true` with a reason.
    ///
    /// # Errors
    ///
    /// Returns an error when the flag store cannot be updated.
    fn halt(&self, reason: &str) -> Result<(), HaltFlagStoreError>;

    /// Clears `engine.halted` after MS-9 approval and returns the original halt subject.
    ///
    /// # Errors
    ///
    /// Returns an error when the flag store cannot be updated or no halt is active.
    fn release(&self) -> Result<String, HaltFlagStoreError>;

    /// Returns the incident or subject associated with the active halt.
    ///
    /// # Errors
    ///
    /// Returns an error when no halt subject is available.
    fn current_subject_id(&self) -> Result<String, HaltFlagStoreError>;

    fn is_engine_halted(&self) -> bool;
}

pub trait AuditLogStore {
    /// Persists a kill-switch governance audit log.
    ///
    /// # Errors
    ///
    /// Returns an error when the audit log cannot be written.
    fn append(&self, record: AuditLogRecord) -> Result<(), AuditLogStoreError>;
}

#[derive(Debug, Deserialize)]
struct HaltRequest {
    reason: String,
}

#[derive(Debug, Serialize)]
struct HaltResponse {
    engine_halted: bool,
}

#[derive(Debug, Serialize)]
struct ReleaseResponse {
    engine_halted: bool,
    audit_log: AuditLogRecord,
}

#[derive(Debug, Deserialize)]
struct ReleaseRequest {
    reason: String,
    evaluated_at_epoch_seconds: u64,
    signers: Vec<ReleaseSignerRequest>,
}

#[derive(Debug, Deserialize)]
struct ReleaseSignerRequest {
    user_id: String,
    role: String,
    hardware_mfa: bool,
    mfa_verified_at_epoch_seconds: u64,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[derive(Clone)]
struct KillSwitchState<H, A> {
    halt_store: H,
    audit_store: A,
}

pub fn internal_admin_router<H, A>(halt_store: H, audit_store: A) -> Router
where
    H: HaltFlagStore + Clone + Send + Sync + 'static,
    A: AuditLogStore + Clone + Send + Sync + 'static,
{
    let state = KillSwitchState {
        halt_store,
        audit_store,
    };
    Router::new()
        .route("/api/internal/admin/halt", post(halt::<H, A>))
        .route("/api/internal/admin/halt/release", post(release::<H, A>))
        .with_state(state)
}

async fn halt<H, A>(
    State(state): State<KillSwitchState<H, A>>,
    Json(request): Json<HaltRequest>,
) -> Result<Json<HaltResponse>, (StatusCode, Json<ErrorResponse>)>
where
    H: HaltFlagStore,
    A: AuditLogStore,
{
    state.halt_store.halt(&request.reason).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "ERROR_CODE_INTERNAL",
            }),
        )
    })?;
    Ok(Json(HaltResponse {
        engine_halted: state.halt_store.is_engine_halted(),
    }))
}

async fn release<H, A>(
    State(state): State<KillSwitchState<H, A>>,
    Json(request): Json<ReleaseRequest>,
) -> Result<Json<ReleaseResponse>, (StatusCode, Json<ErrorResponse>)>
where
    H: HaltFlagStore,
    A: AuditLogStore,
{
    let signers = request
        .signers
        .into_iter()
        .map(ReleaseSignerRequest::try_into_signer)
        .collect::<Result<Vec<_>, _>>()?;
    let release = MultiSigReleaseRequest::new(
        "MS-9",
        signers,
        SystemTime::UNIX_EPOCH + Duration::from_secs(request.evaluated_at_epoch_seconds),
        &request.reason,
    );
    let metadata = release.evaluate().map_err(|_| multisig_error())?;
    let subject_id = state.halt_store.current_subject_id().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "ERROR_CODE_INTERNAL",
            }),
        )
    })?;
    let audit_log = AuditLogRecord {
        action: "kill_switch_release".to_owned(),
        subject_id,
        metadata,
    };
    state.audit_store.append(audit_log.clone()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "ERROR_CODE_INTERNAL",
            }),
        )
    })?;
    state.halt_store.release().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "ERROR_CODE_INTERNAL",
            }),
        )
    })?;
    Ok(Json(ReleaseResponse {
        engine_halted: state.halt_store.is_engine_halted(),
        audit_log,
    }))
}

impl ReleaseSignerRequest {
    fn try_into_signer(self) -> Result<SignerApproval, (StatusCode, Json<ErrorResponse>)> {
        let role = match self.role.as_str() {
            "CTO" => MultiSigRole::Cto,
            "Compliance" => MultiSigRole::Compliance,
            "SeniorEngineer" => MultiSigRole::SeniorEngineer,
            _ => return Err(multisig_error()),
        };
        let verified_at =
            SystemTime::UNIX_EPOCH + Duration::from_secs(self.mfa_verified_at_epoch_seconds);
        if self.hardware_mfa {
            Ok(SignerApproval::hardware_mfa(
                &self.user_id,
                role,
                verified_at,
            ))
        } else {
            Ok(SignerApproval::software_mfa(
                &self.user_id,
                role,
                verified_at,
            ))
        }
    }
}

fn multisig_error() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::FORBIDDEN,
        Json(ErrorResponse {
            error: "ERROR_CODE_MULTISIG_REJECTED",
        }),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HaltFlagStoreError {
    message: String,
}

impl HaltFlagStoreError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLogStoreError {
    message: String,
}

impl AuditLogStoreError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryHaltFlagStore {
    state: Arc<Mutex<HaltFlagState>>,
    feature_flags: InMemoryFeatureFlagStore,
}

#[derive(Debug, Default)]
struct HaltFlagState {
    engine_halted: bool,
    reason: Option<String>,
}

impl InMemoryHaltFlagStore {
    #[must_use]
    pub fn with_feature_flags(feature_flags: InMemoryFeatureFlagStore) -> Self {
        Self {
            state: Arc::new(Mutex::new(HaltFlagState::default())),
            feature_flags,
        }
    }

    #[must_use]
    pub fn feature_flags(&self) -> InMemoryFeatureFlagStore {
        self.feature_flags.clone()
    }

    #[must_use]
    pub fn reason(&self) -> Option<String> {
        self.state.lock().map_or_else(
            |poisoned| poisoned.into_inner().reason.clone(),
            |state| state.reason.clone(),
        )
    }
}

impl HaltFlagStore for InMemoryHaltFlagStore {
    fn halt(&self, reason: &str) -> Result<(), HaltFlagStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| HaltFlagStoreError::new(error.to_string()))?;
        state.engine_halted = true;
        state.reason = Some(reason.to_owned());
        self.feature_flags
            .halt_engine(reason, reason, "api-gateway:kill-switch");
        Ok(())
    }

    fn release(&self) -> Result<String, HaltFlagStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| HaltFlagStoreError::new(error.to_string()))?;
        if !state.engine_halted {
            return Err(HaltFlagStoreError::new("engine halt is not active"));
        }
        let subject_id = state
            .reason
            .clone()
            .ok_or_else(|| HaltFlagStoreError::new("engine halt subject is missing"))?;
        state.engine_halted = false;
        self.feature_flags
            .release_engine_halt("api-gateway:kill-switch");
        Ok(subject_id)
    }

    fn current_subject_id(&self) -> Result<String, HaltFlagStoreError> {
        let state = self
            .state
            .lock()
            .map_err(|error| HaltFlagStoreError::new(error.to_string()))?;
        if !state.engine_halted {
            return Err(HaltFlagStoreError::new("engine halt is not active"));
        }
        state
            .reason
            .clone()
            .ok_or_else(|| HaltFlagStoreError::new("engine halt subject is missing"))
    }

    fn is_engine_halted(&self) -> bool {
        self.state.lock().map_or(true, |state| state.engine_halted)
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAuditLogStore {
    records: Arc<Mutex<Vec<AuditLogRecord>>>,
}

impl InMemoryAuditLogStore {
    #[must_use]
    pub fn records(&self) -> Vec<AuditLogRecord> {
        self.records.lock().map_or_else(
            |poisoned| poisoned.into_inner().clone(),
            |records| records.clone(),
        )
    }
}

impl AuditLogStore for InMemoryAuditLogStore {
    fn append(&self, record: AuditLogRecord) -> Result<(), AuditLogStoreError> {
        let mut records = self
            .records
            .lock()
            .map_err(|error| AuditLogStoreError::new(error.to_string()))?;
        records.push(record);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MultiSigRole {
    Cto,
    Compliance,
    SeniorEngineer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignerApproval {
    user_id: String,
    role: MultiSigRole,
    hardware_mfa: bool,
    mfa_verified_at: SystemTime,
}

impl SignerApproval {
    #[must_use]
    pub fn hardware_mfa(user_id: &str, role: MultiSigRole, mfa_verified_at: SystemTime) -> Self {
        Self {
            user_id: user_id.to_owned(),
            role,
            hardware_mfa: true,
            mfa_verified_at,
        }
    }

    #[must_use]
    pub fn software_mfa(user_id: &str, role: MultiSigRole, mfa_verified_at: SystemTime) -> Self {
        Self {
            user_id: user_id.to_owned(),
            role,
            hardware_mfa: false,
            mfa_verified_at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MultiSigDecision {
    Approved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditLogMetadata {
    pub ms_id: String,
    pub signers: Vec<String>,
    pub decision: MultiSigDecision,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditLogRecord {
    pub action: String,
    pub subject_id: String,
    pub metadata: AuditLogMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiSigError {
    code: &'static str,
}

impl MultiSigError {
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiSigReleaseRequest {
    ms_id: String,
    signers: Vec<SignerApproval>,
    evaluated_at: SystemTime,
    reason: String,
}

impl MultiSigReleaseRequest {
    #[must_use]
    pub fn new(
        ms_id: &str,
        signers: Vec<SignerApproval>,
        evaluated_at: SystemTime,
        reason: &str,
    ) -> Self {
        Self {
            ms_id: ms_id.to_owned(),
            signers,
            evaluated_at,
            reason: reason.to_owned(),
        }
    }

    /// Evaluates SPEC-010 MS-9 release governance.
    ///
    /// # Errors
    ///
    /// Returns [`MultiSigError`] when the release does not satisfy 2-of-3, signer uniqueness,
    /// role eligibility, or hardware MFA freshness.
    pub fn evaluate(&self) -> Result<AuditLogMetadata, MultiSigError> {
        let mut unique_users = HashSet::new();
        let mut roles = HashSet::new();
        let mut signers = Vec::new();

        for signer in &self.signers {
            if !unique_users.insert(signer.user_id.clone()) {
                return Err(multisig_rejected());
            }
            if !matches!(
                signer.role,
                MultiSigRole::Cto | MultiSigRole::Compliance | MultiSigRole::SeniorEngineer
            ) {
                return Err(multisig_rejected());
            }
            if !signer.hardware_mfa {
                return Err(multisig_rejected());
            }
            let age = self
                .evaluated_at
                .duration_since(signer.mfa_verified_at)
                .map_err(|_| multisig_rejected())?;
            if age > Duration::from_secs(30 * 60) {
                return Err(multisig_rejected());
            }

            roles.insert(signer.role);
            signers.push(signer.user_id.clone());
        }

        if roles.len() < 2 {
            return Err(multisig_rejected());
        }

        Ok(AuditLogMetadata {
            ms_id: self.ms_id.clone(),
            signers,
            decision: MultiSigDecision::Approved,
            reason: self.reason.clone(),
        })
    }
}

const fn multisig_rejected() -> MultiSigError {
    MultiSigError {
        code: "ERROR_CODE_MULTISIG_REJECTED",
    }
}
