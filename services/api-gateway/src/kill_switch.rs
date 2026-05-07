use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use serde::{Deserialize, Serialize};

pub trait HaltFlagStore {
    /// Sets `engine.halted=true` with a reason.
    ///
    /// # Errors
    ///
    /// Returns an error when the flag store cannot be updated.
    fn halt(&self, reason: &str) -> Result<(), HaltFlagStoreError>;

    fn is_engine_halted(&self) -> bool;
}

#[derive(Debug, Deserialize)]
struct HaltRequest {
    reason: String,
}

#[derive(Debug, Serialize)]
struct HaltResponse {
    engine_halted: bool,
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

pub fn internal_admin_router<S>(store: S) -> Router
where
    S: HaltFlagStore + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/internal/admin/halt", post(halt::<S>))
        .route("/api/internal/admin/halt/release", post(release::<S>))
        .with_state(store)
}

async fn halt<S>(
    State(store): State<S>,
    Json(request): Json<HaltRequest>,
) -> Result<Json<HaltResponse>, (StatusCode, Json<ErrorResponse>)>
where
    S: HaltFlagStore,
{
    store.halt(&request.reason).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "ERROR_CODE_INTERNAL",
            }),
        )
    })?;
    Ok(Json(HaltResponse {
        engine_halted: store.is_engine_halted(),
    }))
}

async fn release<S>(
    State(_store): State<S>,
    Json(request): Json<ReleaseRequest>,
) -> Result<Json<AuditLogMetadata>, (StatusCode, Json<ErrorResponse>)>
where
    S: HaltFlagStore,
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
    release.evaluate().map(Json).map_err(|_| multisig_error())
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
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryHaltFlagStore {
    state: Arc<Mutex<HaltFlagState>>,
}

#[derive(Debug, Default)]
struct HaltFlagState {
    engine_halted: bool,
    reason: Option<String>,
}

impl InMemoryHaltFlagStore {
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
        let mut state = self.state.lock().map_err(|error| HaltFlagStoreError {
            message: error.to_string(),
        })?;
        state.engine_halted = true;
        state.reason = Some(reason.to_owned());
        Ok(())
    }

    fn is_engine_halted(&self) -> bool {
        self.state.lock().map_or(true, |state| state.engine_halted)
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
