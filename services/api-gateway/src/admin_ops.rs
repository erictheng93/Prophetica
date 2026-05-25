use std::{
    env,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use axum::{
    Json, Router,
    body::Body,
    extract::{OriginalUri, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    routing::{delete, get, patch, post},
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, QueryBuilder, Row};

#[async_trait]
pub trait AdminOpsStore {
    /// Lists members that need operational attention.
    ///
    /// # Errors
    ///
    /// Returns an error when the backing store cannot be read.
    async fn pending_member_cases(
        &self,
        query: PendingMemberCasesQuery,
    ) -> Result<PendingMemberCasesResponse, AdminOpsStoreError>;

    async fn list_members(&self, query: MemberListQuery) -> Result<Value, AdminOpsStoreError>;

    async fn list_role_accounts(
        &self,
        query: RoleAccountListQuery,
        context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError>;

    /// Builds the internal admin member summary facade.
    ///
    /// # Errors
    ///
    /// Returns an error when the backing store cannot be read.
    async fn member_summary(&self, user_id: &str) -> Result<Value, AdminOpsStoreError>;

    async fn create_member(
        &self,
        request: Value,
        context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError>;

    async fn update_member(
        &self,
        user_id: &str,
        request: Value,
        context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError>;

    async fn delete_member(
        &self,
        user_id: &str,
        context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError>;

    async fn list_member_actions(
        &self,
        query: MemberActionListQuery,
    ) -> Result<MemberActionListResponse, AdminOpsStoreError>;

    async fn create_member_action(
        &self,
        request: MemberActionRequest,
        context: AdminOpsRequestContext,
    ) -> Result<MemberActionResponse, AdminOpsStoreError>;

    async fn review_member_action(
        &self,
        action_id: &str,
        request: MemberActionReviewRequest,
        context: AdminOpsRequestContext,
    ) -> Result<MemberActionRecord, AdminOpsStoreError>;

    async fn list_compensation_proposals(
        &self,
        query: CompensationProposalListQuery,
    ) -> Result<CompensationProposalListResponse, AdminOpsStoreError>;

    async fn compensation_proposal_detail(
        &self,
        proposal_id: &str,
    ) -> Result<CompensationProposalDetailResponse, AdminOpsStoreError>;

    async fn review_compensation_proposal(
        &self,
        proposal_id: &str,
        request: CompensationReviewRequest,
        context: AdminOpsRequestContext,
    ) -> Result<CompensationProposalReviewResponse, AdminOpsStoreError>;

    async fn retry_compensation_posting(
        &self,
        proposal_id: &str,
        context: AdminOpsRequestContext,
    ) -> Result<CompensationPostingRetryResponse, AdminOpsStoreError>;

    /// Persists a compensation proposal without posting a ledger transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when the proposal or audit record cannot be persisted.
    async fn create_compensation_proposal(
        &self,
        user_id: &str,
        request: CompensationProposalRequest,
        _context: AdminOpsRequestContext,
    ) -> Result<CompensationProposalResponse, AdminOpsStoreError>;

    /// Returns the HMAC secret used to verify admin internal-call signatures for
    /// the KYC document proxy. Reads `ADMIN_INTERNAL_CALL_SECRET` from the process
    /// environment by default; implementors may override this for testing.
    fn kyc_proxy_secret(&self) -> Option<String> {
        env::var("ADMIN_INTERNAL_CALL_SECRET").ok()
    }

    /// Returns the base URL of the KYC service for the document proxy.
    /// Reads `KYC_SERVICE_BASE_URL` / `KYC_SERVICE_URL` from the process
    /// environment by default.
    fn kyc_service_base_url(&self) -> String {
        crate::kyc_service_base_url_from_env()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminOpsRequestContext {
    pub actor_user_id: String,
    pub actor_role: String,
    pub request_id: Option<String>,
    pub idempotency_key: Option<String>,
    pub compensation_referral_relationship: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PendingMemberCasesQuery {
    pub q: Option<String>,
    pub status: Option<String>,
    pub restriction: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RoleAccountListQuery {
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MemberListQuery {
    pub q: Option<String>,
    pub affiliate_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MemberActionListQuery {
    pub status: Option<String>,
    pub target_user_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingMemberCasesResponse {
    pub items: Vec<PendingMemberCase>,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingMemberCase {
    pub member_id: String,
    pub display_name: String,
    pub restriction_reason: String,
    pub contact: PendingMemberContact,
    pub status: String,
    pub kyc_status: String,
    pub affiliate_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingMemberContact {
    pub email_masked: String,
    pub phone_masked: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemberSummaryResponse {
    pub member: MemberIdentitySummary,
    pub wallet: WalletOpsSummary,
    pub kyc_risk: KycRiskSummary,
    pub affiliate: AffiliateOpsSummary,
    pub open_cases: Vec<OpenCaseSummary>,
    pub audit: AuditPostureSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemberIdentitySummary {
    pub user_id: String,
    pub display_name: String,
    pub email_masked: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WalletOpsSummary {
    pub balance: WalletBalanceSummary,
    pub entries_summary: WalletEntriesSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WalletBalanceSummary {
    pub available_minor_units: i64,
    pub locked_minor_units: i64,
    pub frozen_minor_units: i64,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WalletEntriesSummary {
    pub recent_entries_count: u32,
    pub last_entry_at_epoch_seconds: Option<i64>,
    pub pending_adjustments_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KycRiskSummary {
    pub kyc_status: String,
    pub risk_level: String,
    pub risk_score: Option<u32>,
    pub last_reviewed_at_epoch_seconds: Option<i64>,
    pub restrictions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AffiliateOpsSummary {
    pub referral_code: Option<String>,
    pub successful_referrals_count: u32,
    pub total_rewards_minor_units: i64,
    pub pending_rewards_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenCaseSummary {
    pub case_id: String,
    pub case_type: String,
    pub status: String,
    pub owner_role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuditPostureSummary {
    pub correlation_id: String,
    pub source: String,
    pub audit_required: bool,
    pub latest_admin_action_at_epoch_seconds: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CompensationProposalRequest {
    pub ticket_id: Option<String>,
    pub reason: Option<String>,
    pub amount_minor_units: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CompensationProposalListQuery {
    pub status: Option<String>,
    pub proposer_kind: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CompensationReviewRequest {
    pub decision: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct MemberActionRequest {
    pub target_user_id: String,
    pub action_type: String,
    pub ticket_id: String,
    pub reason: String,
    pub correlation_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct MemberActionReviewRequest {
    pub decision: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberActionResponse {
    pub action_id: String,
    pub target_user_id: String,
    pub action_type: String,
    pub status: String,
    pub requires_second_approval: bool,
    pub audit: MemberActionAuditMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberActionListResponse {
    pub actions: Vec<MemberActionRecord>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberActionRecord {
    pub action_id: String,
    pub target_user_id: String,
    pub action_type: String,
    pub ticket_id: String,
    pub reason: String,
    pub status: String,
    pub requires_second_approval: bool,
    pub actor_user_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub review: Option<MemberActionReviewSummary>,
    pub side_effects: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberActionReviewSummary {
    pub reviewer_user_id: String,
    pub reviewer_role: String,
    pub decision: String,
    pub reason: String,
    pub reviewed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberActionAuditMetadata {
    pub actor_user_id: String,
    pub actor_role: String,
    pub target_user_id: String,
    pub ticket_id: String,
    pub correlation_id: String,
    pub ledger_transaction_posted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompensationApprovalRequest {
    pub approver_user_id: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompensationReviewSummary {
    pub reviewer_user_id: String,
    pub reviewer_role: String,
    pub decision: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompensationProposalResponse {
    pub proposal: CompensationProposal,
    pub audit: CompensationAuditMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompensationProposalListResponse {
    pub proposals: Vec<CompensationProposal>,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompensationProposalDetailResponse {
    pub proposal: CompensationProposal,
    pub approvals: Vec<CompensationApprovalRequest>,
    pub recent_audit_events: Vec<CompensationAuditEventSummary>,
    pub posting_attempt_count: i32,
    pub last_posting_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompensationProposalReviewResponse {
    pub proposal: CompensationProposal,
    pub review: CompensationReviewSummary,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompensationPostingRetryResponse {
    pub proposal: CompensationProposal,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompensationProposal {
    pub proposal_id: String,
    pub user_id: String,
    pub ticket_id: String,
    pub reason: String,
    pub amount_minor_units: i64,
    pub status: CompensationProposalStatus,
    pub proposer_kind: String,
    pub proposer_user_id: String,
    pub approvals: Vec<CompensationApprovalRequest>,
    pub ledger_transaction_posted: bool,
    pub ledger_idempotency_key: Option<String>,
    pub ledger_transaction_id: Option<i64>,
    pub posting_attempt_count: i32,
    pub last_posting_error: Option<String>,
    pub approved_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompensationPostingDraft {
    proposal_id: String,
    subject_user_id: String,
    ticket_id: String,
    amount_minor_units: i64,
    correlation_id: String,
    ledger_idempotency_key: String,
    posting_attempt_count: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CompensationLedgerPostOutcome {
    Posted {
        txn_id: i64,
    },
    Failed {
        error_class: String,
        message: String,
    },
    Unknown {
        error_class: String,
        message: String,
    },
}

#[derive(Debug, Deserialize)]
struct LedgerPostResponse {
    txn_id: i64,
}

#[derive(Debug, Deserialize)]
struct LedgerLookupResponse {
    txn_id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompensationProposalStatus {
    Proposed,
    Approved,
    Posting,
    Posted,
    PostingFailed,
    PostingUnknown,
    Rejected,
    Cancelled,
    PendingApproval,
    ApprovedReadyForLedger,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompensationAuditMetadata {
    pub action: String,
    pub subject_user_id: String,
    pub ticket_id: String,
    pub correlation_id: String,
    pub approval_roles: Vec<String>,
    pub proposer_kind: String,
    pub ledger_transaction_posted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompensationAuditEventSummary {
    pub action: String,
    pub actor_user_id: String,
    pub actor_role: String,
    pub correlation_id: String,
    pub metadata: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ReferralScopeResponse {
    in_scope: bool,
    relationship: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct UpstreamError {
    error: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminOpsStoreError {
    message: String,
    upstream_status: Option<u16>,
    upstream_code: Option<String>,
}

impl AdminOpsStoreError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            upstream_status: None,
            upstream_code: None,
        }
    }

    #[must_use]
    pub fn from_upstream(status: u16, code: Option<String>, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            upstream_status: Some(status),
            upstream_code: code,
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn upstream_status(&self) -> Option<u16> {
        self.upstream_status
    }

    #[must_use]
    pub fn upstream_code(&self) -> Option<&str> {
        self.upstream_code.as_deref()
    }
}

pub fn internal_admin_router<S>(store: S) -> Router
where
    S: AdminOpsStore + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/internal/admin/members", get(list_members::<S>))
        .route(
            "/api/internal/admin/role-accounts",
            get(list_role_accounts::<S>),
        )
        .route("/api/internal/admin/members", post(create_member::<S>))
        .route(
            "/api/internal/admin/members/pending-cases",
            get(pending_member_cases::<S>),
        )
        .route(
            "/api/internal/admin/members/:user_id/summary",
            get(member_summary::<S>),
        )
        .route(
            "/api/internal/admin/members/:user_id",
            patch(update_member::<S>),
        )
        .route(
            "/api/internal/admin/members/:user_id",
            delete(delete_member::<S>),
        )
        .route(
            "/api/internal/admin/members/actions",
            get(list_member_actions::<S>),
        )
        .route(
            "/api/internal/admin/members/actions",
            post(create_member_action::<S>),
        )
        .route(
            "/api/internal/admin/members/actions/:action_id/reviews",
            post(review_member_action::<S>),
        )
        .route(
            "/api/internal/admin/compensation-proposals",
            get(list_compensation_proposals::<S>),
        )
        .route(
            "/api/internal/admin/compensation-proposals/:proposal_id",
            get(get_compensation_proposal_detail::<S>),
        )
        .route(
            "/api/internal/admin/compensation-proposals/:proposal_id/review",
            post(review_compensation_proposal::<S>),
        )
        .route(
            "/api/internal/admin/compensation-proposals/:proposal_id/retry-posting",
            post(retry_compensation_posting::<S>),
        )
        .route(
            "/api/internal/admin/members/:user_id/compensation-proposals",
            post(create_compensation_proposal::<S>),
        )
        .route(
            "/api/internal/admin/members/:user_id/kyc-documents",
            get(proxy_kyc_document_list::<S>),
        )
        .route(
            "/api/internal/admin/members/:user_id/kyc-documents/:doc_id",
            get(proxy_kyc_document_image::<S>),
        )
        .with_state(store)
}

async fn list_members<S>(
    State(store): State<S>,
    Query(query): Query<MemberListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    store
        .list_members(query)
        .await
        .map(Json)
        .map_err(|_| internal_error())
}

async fn pending_member_cases<S>(
    State(store): State<S>,
    Query(query): Query<PendingMemberCasesQuery>,
) -> Result<Json<PendingMemberCasesResponse>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    store
        .pending_member_cases(query)
        .await
        .map(Json)
        .map_err(|error| pending_member_cases_error(&error))
}

async fn list_role_accounts<S>(
    State(store): State<S>,
    headers: HeaderMap,
    Query(query): Query<RoleAccountListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    let context = request_context(&headers)?;
    if context.actor_role != "admin" {
        return Err(forbidden_error("ERROR_CODE_ROLE_ACCOUNT_FORBIDDEN"));
    }
    store
        .list_role_accounts(query, context)
        .await
        .map(Json)
        .map_err(|_| internal_error())
}

async fn member_summary<S>(
    State(store): State<S>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    store
        .member_summary(&user_id)
        .await
        .map(Json)
        .map_err(|error| member_summary_error(&error))
}

async fn create_member<S>(
    State(store): State<S>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    let context = request_context(&headers)?;
    if context.actor_role != "admin" && targets_manager_account_role(&request) {
        return Err(forbidden_error("ERROR_CODE_ROLE_ACCOUNT_FORBIDDEN"));
    }
    store
        .create_member(request, context)
        .await
        .map(|body| (StatusCode::CREATED, Json(body)))
        .map_err(|error| member_mutation_error(&error))
}

async fn update_member<S>(
    State(store): State<S>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    let context = request_context(&headers)?;
    if context.actor_role != "admin" && targets_manager_account_role(&request) {
        return Err(forbidden_error("ERROR_CODE_ROLE_ACCOUNT_FORBIDDEN"));
    }
    store
        .update_member(&user_id, request, context)
        .await
        .map(Json)
        .map_err(|error| member_mutation_error(&error))
}

async fn delete_member<S>(
    State(store): State<S>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    let context = request_context(&headers)?;
    store
        .delete_member(&user_id, context)
        .await
        .map(Json)
        .map_err(|error| member_mutation_error(&error))
}

async fn create_member_action<S>(
    State(store): State<S>,
    headers: HeaderMap,
    Json(request): Json<MemberActionRequest>,
) -> Result<(StatusCode, Json<MemberActionResponse>), (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    validate_member_action(&request)?;
    let context = request_context(&headers)?;
    store
        .create_member_action(request, context)
        .await
        .map(|body| (StatusCode::ACCEPTED, Json(body)))
        .map_err(|_| internal_error())
}

async fn list_member_actions<S>(
    State(store): State<S>,
    Query(query): Query<MemberActionListQuery>,
) -> Result<Json<MemberActionListResponse>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    store
        .list_member_actions(query)
        .await
        .map(Json)
        .map_err(|_| internal_error())
}

async fn review_member_action<S>(
    State(store): State<S>,
    headers: HeaderMap,
    Path(action_id): Path<String>,
    Json(request): Json<MemberActionReviewRequest>,
) -> Result<Json<MemberActionRecord>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    validate_member_action_review(&request)?;
    let context = request_context(&headers)?;
    store
        .review_member_action(&action_id, request, context)
        .await
        .map(Json)
        .map_err(|error| review_error(&error))
}

async fn list_compensation_proposals<S>(
    State(store): State<S>,
    Query(query): Query<CompensationProposalListQuery>,
) -> Result<Json<CompensationProposalListResponse>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    store
        .list_compensation_proposals(query)
        .await
        .map(Json)
        .map_err(|_| internal_error())
}

async fn get_compensation_proposal_detail<S>(
    State(store): State<S>,
    Path(proposal_id): Path<String>,
) -> Result<Json<CompensationProposalDetailResponse>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    store
        .compensation_proposal_detail(&proposal_id)
        .await
        .map(Json)
        .map_err(|error| {
            if error.message() == "compensation proposal not found" {
                (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "ERROR_CODE_COMPENSATION_PROPOSAL_NOT_FOUND",
                    }),
                )
            } else {
                internal_error()
            }
        })
}

async fn review_compensation_proposal<S>(
    State(store): State<S>,
    headers: HeaderMap,
    Path(proposal_id): Path<String>,
    Json(request): Json<CompensationReviewRequest>,
) -> Result<(StatusCode, Json<CompensationProposalReviewResponse>), (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    validate_compensation_review(&request)?;
    let context = request_context(&headers)?;
    store
        .review_compensation_proposal(&proposal_id, request, context)
        .await
        .map(|response| {
            let status = if matches!(
                response.proposal.status,
                CompensationProposalStatus::PostingFailed
                    | CompensationProposalStatus::PostingUnknown
            ) {
                StatusCode::ACCEPTED
            } else {
                StatusCode::OK
            };
            (status, Json(response))
        })
        .map_err(|error| compensation_review_error(&error))
}

async fn retry_compensation_posting<S>(
    State(store): State<S>,
    headers: HeaderMap,
    Path(proposal_id): Path<String>,
) -> Result<Json<CompensationPostingRetryResponse>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    let context = request_context(&headers)?;
    store
        .retry_compensation_posting(&proposal_id, context)
        .await
        .map(Json)
        .map_err(|error| compensation_retry_error(&error))
}

async fn create_compensation_proposal<S>(
    State(store): State<S>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<CompensationProposalRequest>,
) -> Result<Json<CompensationProposalResponse>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    validate_compensation_proposal(&request)?;
    let context = request_context(&headers)?;
    store
        .create_compensation_proposal(&user_id, request, context)
        .await
        .map(Json)
        .map_err(|error| compensation_error(&error))
}

/// Proxies a KYC document list request to kyc-service, after verifying the
/// signed internal-call headers. SPEC-005 §5.3.
async fn proxy_kyc_document_list<S>(
    State(store): State<S>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Path(user_id): Path<String>,
) -> Response
where
    S: AdminOpsStore,
{
    let Some(secret) = store.kyc_proxy_secret() else {
        tracing::error!("ADMIN_INTERNAL_CALL_SECRET is not set; rejecting KYC proxy request");
        return kyc_proxy_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "kyc_proxy_secret_unconfigured",
        );
    };
    let request_path = uri.path();
    if let Err(auth_err) = verify_kyc_proxy_auth_with_secret(&headers, "GET", request_path, &secret)
    {
        return auth_err;
    }

    let kyc_base = store.kyc_service_base_url();
    let Ok((kyc_url, client)) = kyc_proxy_upstream_from(&kyc_base) else {
        return kyc_proxy_error(StatusCode::INTERNAL_SERVER_ERROR, "kyc_proxy_bad_url");
    };

    let upstream_path = format!(
        "/v1/internal/admin/kyc/members/{}/documents",
        url_encode_path_segment(&user_id)
    );
    let Ok(upstream_url) = kyc_url.join(&upstream_path) else {
        return kyc_proxy_error(StatusCode::INTERNAL_SERVER_ERROR, "kyc_proxy_bad_url");
    };

    let mut fwd_request = client.get(upstream_url);
    fwd_request = forward_proxy_headers(fwd_request, &headers);

    kyc_proxy_stream(fwd_request, false).await
}

/// Proxies a KYC document image request to kyc-service, streaming the response
/// bytes back to the caller. SPEC-005 §5.3.
async fn proxy_kyc_document_image<S>(
    State(store): State<S>,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Path((user_id, doc_id)): Path<(String, String)>,
) -> Response
where
    S: AdminOpsStore,
{
    let Some(secret) = store.kyc_proxy_secret() else {
        tracing::error!("ADMIN_INTERNAL_CALL_SECRET is not set; rejecting KYC proxy request");
        return kyc_proxy_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "kyc_proxy_secret_unconfigured",
        );
    };
    let request_path = uri.path();
    if let Err(auth_err) = verify_kyc_proxy_auth_with_secret(&headers, "GET", request_path, &secret)
    {
        return auth_err;
    }

    let kyc_base = store.kyc_service_base_url();
    let Ok((kyc_url, client)) = kyc_proxy_upstream_from(&kyc_base) else {
        return kyc_proxy_error(StatusCode::INTERNAL_SERVER_ERROR, "kyc_proxy_bad_url");
    };

    let upstream_path = format!(
        "/v1/internal/admin/kyc/members/{}/documents/{}",
        url_encode_path_segment(&user_id),
        url_encode_path_segment(&doc_id),
    );
    let Ok(upstream_url) = kyc_url.join(&upstream_path) else {
        return kyc_proxy_error(StatusCode::INTERNAL_SERVER_ERROR, "kyc_proxy_bad_url");
    };

    let mut fwd_request = client.get(upstream_url);
    fwd_request = forward_proxy_headers(fwd_request, &headers);

    kyc_proxy_stream(fwd_request, true).await
}

/// Verifies the internal-call HMAC signature for a KYC proxy request.
///
/// Returns `Err(Response)` with 401 when verification fails.
fn verify_kyc_proxy_auth_with_secret(
    headers: &HeaderMap,
    method: &str,
    path: &str,
    secret: &str,
) -> Result<(), Response> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    // The signed canonical string covers the operator role, so a valid
    // signature authenticates the `x-operator-role` header that the gateway
    // forwards. KYC review permission enforcement and denied-access auditing
    // live in kyc-service (SPEC-005 §5.3.2 / §8), which owns the
    // kyc_document_access_log table — so a role-rejected request must reach
    // kyc-service rather than being short-circuited here.
    crate::internal_call_auth::verify_internal_call(headers, method, path, b"", secret, now)
        .map(|_actor| ())
        .map_err(|_| kyc_proxy_error(StatusCode::UNAUTHORIZED, "kyc_proxy_unauthorized"))
}

fn current_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs()).unwrap_or(i64::MAX)
        })
}

/// Resolves the KYC-service base URL and a reqwest client for the proxy.
///
/// Returns `Err(Response)` with 500 when the URL cannot be parsed.
fn kyc_proxy_upstream_from(raw_url: &str) -> Result<(Url, reqwest::Client), Response> {
    let url = Url::parse(raw_url)
        .map_err(|_| kyc_proxy_error(StatusCode::INTERNAL_SERVER_ERROR, "kyc_proxy_bad_url"))?;
    Ok((url, reqwest::Client::new()))
}

/// Forwards actor / operator / request-id headers to the upstream reqwest request.
fn forward_proxy_headers(
    mut request: reqwest::RequestBuilder,
    headers: &HeaderMap,
) -> reqwest::RequestBuilder {
    for name in &["x-actor-id", "x-operator-role", "x-request-id"] {
        if let Some(value) = headers.get(*name) {
            request = request.header(*name, value.clone());
        }
    }
    request
}

/// Sends the upstream reqwest request and streams its response body back as an
/// axum [`Response`], passing through the upstream status and `Content-Type`.
/// When `add_cache_control` is `true` a `Cache-Control: private, no-store`
/// header is also appended (used for image responses).
async fn kyc_proxy_stream(request: reqwest::RequestBuilder, add_cache_control: bool) -> Response {
    let upstream_response = match request.send().await {
        Ok(r) => r,
        Err(err) => {
            tracing::warn!(%err, "KYC document proxy upstream request failed");
            return kyc_proxy_error(StatusCode::BAD_GATEWAY, "kyc_proxy_upstream_failed");
        }
    };

    let status = upstream_response.status();
    let axum_status =
        StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let content_type = upstream_response
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned();

    let stream = upstream_response.bytes_stream();
    let body = Body::from_stream(stream);
    let mut response = Response::new(body);
    *response.status_mut() = axum_status;
    if let Some(ct) = content_type {
        response.headers_mut().insert(header::CONTENT_TYPE, ct);
    }
    if add_cache_control {
        if let Ok(value) = "private, no-store".parse() {
            response.headers_mut().insert(header::CACHE_CONTROL, value);
        }
    }
    response
}

fn kyc_proxy_error(status: StatusCode, code: &'static str) -> Response {
    let body = format!("{{\"error\":\"{code}\"}}");
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    if let Ok(value) = "application/json".parse() {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    response
}

fn validate_member_action_review(
    request: &MemberActionReviewRequest,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if request.reason.trim().is_empty()
        || !matches!(request.decision.as_str(), "approve" | "reject")
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "ERROR_CODE_INVALID_MEMBER_ACTION_REVIEW",
            }),
        ));
    }
    Ok(())
}

fn validate_member_action(
    request: &MemberActionRequest,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if request.target_user_id.trim().is_empty()
        || request.ticket_id.trim().is_empty()
        || request.reason.trim().is_empty()
        || !matches!(
            request.action_type.as_str(),
            "add_note" | "request_kyc_review" | "restrict_withdrawals" | "escalate_case"
        )
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "ERROR_CODE_INVALID_MEMBER_ACTION",
            }),
        ));
    }
    Ok(())
}

fn targets_manager_account_role(request: &Value) -> bool {
    request
        .get("role")
        .and_then(Value::as_str)
        .is_some_and(|role| matches!(role, "admin" | "manager"))
}

fn request_context(
    headers: &HeaderMap,
) -> Result<AdminOpsRequestContext, (StatusCode, Json<ErrorResponse>)> {
    let actor_user_id = required_header(headers, "x-actor-id")?;
    let actor_role = required_header(headers, "x-operator-role")?;
    Ok(AdminOpsRequestContext {
        actor_user_id,
        actor_role,
        request_id: optional_header(headers, "x-request-id"),
        idempotency_key: optional_header(headers, "idempotency-key"),
        compensation_referral_relationship: None,
    })
}

fn required_header(
    headers: &HeaderMap,
    name: &'static str,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    optional_header(headers, name).ok_or((
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            error: "ERROR_CODE_MISSING_ADMIN_CONTEXT",
        }),
    ))
}

fn optional_header(headers: &HeaderMap, name: &'static str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn validate_compensation_proposal(
    request: &CompensationProposalRequest,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if request
        .ticket_id
        .as_deref()
        .is_none_or(|ticket_id| ticket_id.trim().is_empty())
        || request
            .reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty())
        || request.amount_minor_units.is_none_or(|amount| amount <= 0)
    {
        return Err(invalid_proposal());
    }
    Ok(())
}

fn validate_compensation_review(
    request: &CompensationReviewRequest,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if !matches!(request.decision.as_str(), "approved" | "rejected") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "ERROR_CODE_INVALID_COMPENSATION_REVIEW",
            }),
        ));
    }
    Ok(())
}

fn invalid_proposal() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "ERROR_CODE_INVALID_COMPENSATION_PROPOSAL",
        }),
    )
}

fn internal_error() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "ERROR_CODE_INTERNAL",
        }),
    )
}

fn forbidden_error(code: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (StatusCode::FORBIDDEN, Json(ErrorResponse { error: code }))
}

fn pending_member_cases_error(error: &AdminOpsStoreError) -> (StatusCode, Json<ErrorResponse>) {
    let message = error.message();
    if message.contains("error sending request")
        || message.contains("connection refused")
        || message.contains("Connection refused")
    {
        return (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "ERROR_CODE_KYC_SERVICE_UNAVAILABLE",
            }),
        );
    }
    internal_error()
}

fn member_summary_error(error: &AdminOpsStoreError) -> (StatusCode, Json<ErrorResponse>) {
    if let Some(upstream_status) = error.upstream_status() {
        let status =
            StatusCode::from_u16(upstream_status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let code = error
            .upstream_code()
            .and_then(member_summary_upstream_error_code)
            .unwrap_or_else(|| member_summary_status_error_code(status));
        return (status, Json(ErrorResponse { error: code }));
    }

    if is_transport_error(error) {
        return (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "ERROR_CODE_USER_SERVICE_UNAVAILABLE",
            }),
        );
    }

    internal_error()
}

fn member_mutation_error(error: &AdminOpsStoreError) -> (StatusCode, Json<ErrorResponse>) {
    if let Some(upstream_status) = error.upstream_status() {
        let status =
            StatusCode::from_u16(upstream_status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let code = error
            .upstream_code()
            .and_then(member_summary_upstream_error_code)
            .unwrap_or_else(|| member_summary_status_error_code(status));
        return (status, Json(ErrorResponse { error: code }));
    }
    if is_transport_error(error) {
        return (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "ERROR_CODE_USER_SERVICE_UNAVAILABLE",
            }),
        );
    }
    internal_error()
}

fn member_summary_upstream_error_code(code: &str) -> Option<&'static str> {
    match code {
        "ERROR_CODE_MEMBER_NOT_FOUND" => Some("ERROR_CODE_MEMBER_NOT_FOUND"),
        "ERROR_CODE_INVALID_MEMBER" => Some("ERROR_CODE_INVALID_MEMBER"),
        "ERROR_CODE_MEMBER_EXISTS" => Some("ERROR_CODE_MEMBER_EXISTS"),
        "ERROR_CODE_MANAGER_HAS_AFFILIATES" => Some("ERROR_CODE_MANAGER_HAS_AFFILIATES"),
        "ERROR_CODE_INTERNAL" => Some("ERROR_CODE_INTERNAL"),
        _ => None,
    }
}

fn member_summary_status_error_code(status: StatusCode) -> &'static str {
    match status {
        StatusCode::NOT_FOUND => "ERROR_CODE_MEMBER_NOT_FOUND",
        StatusCode::BAD_REQUEST => "ERROR_CODE_INVALID_MEMBER",
        StatusCode::CONFLICT => "ERROR_CODE_MEMBER_EXISTS",
        StatusCode::FORBIDDEN => "ERROR_CODE_MEMBER_ACCOUNT_FORBIDDEN",
        _ => "ERROR_CODE_INTERNAL",
    }
}

fn is_transport_error(error: &AdminOpsStoreError) -> bool {
    let message = error.message();
    message.contains("error sending request")
        || message.contains("connection refused")
        || message.contains("Connection refused")
}

fn review_error(error: &AdminOpsStoreError) -> (StatusCode, Json<ErrorResponse>) {
    let (status, code) = match error.message() {
        "member action self approval is not allowed" => (
            StatusCode::CONFLICT,
            "ERROR_CODE_MEMBER_ACTION_SELF_APPROVAL",
        ),
        "member action cannot be reviewed in its current status" => (
            StatusCode::CONFLICT,
            "ERROR_CODE_MEMBER_ACTION_NOT_REVIEWABLE",
        ),
        "member action not found" => (StatusCode::NOT_FOUND, "ERROR_CODE_MEMBER_ACTION_NOT_FOUND"),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "ERROR_CODE_INTERNAL"),
    };
    (status, Json(ErrorResponse { error: code }))
}

fn compensation_error(error: &AdminOpsStoreError) -> (StatusCode, Json<ErrorResponse>) {
    match error.message() {
        "compensation subject out of scope" => {
            return forbidden_error("ERROR_CODE_COMPENSATION_SUBJECT_OUT_OF_SCOPE");
        }
        "referral scope unavailable" => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: "ERROR_CODE_REFERRAL_SCOPE_UNAVAILABLE",
                }),
            );
        }
        "compensation amount over affiliate limit" => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorResponse {
                    error: "ERROR_CODE_COMPENSATION_AMOUNT_OVER_LIMIT",
                }),
            );
        }
        _ => {}
    }
    if let Some(upstream_status) = error.upstream_status() {
        let status =
            StatusCode::from_u16(upstream_status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        if status == StatusCode::NOT_FOUND {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "ERROR_CODE_MEMBER_NOT_FOUND",
                }),
            );
        }
    }
    if is_transport_error(error) {
        return (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "ERROR_CODE_USER_SERVICE_UNAVAILABLE",
            }),
        );
    }
    internal_error()
}

fn compensation_review_error(error: &AdminOpsStoreError) -> (StatusCode, Json<ErrorResponse>) {
    let (status, code) = match error.message() {
        "compensation review forbidden" => (
            StatusCode::FORBIDDEN,
            "ERROR_CODE_COMPENSATION_REVIEW_FORBIDDEN",
        ),
        "compensation self review" => {
            (StatusCode::FORBIDDEN, "ERROR_CODE_COMPENSATION_SELF_REVIEW")
        }
        "compensation proposal not pending" => {
            (StatusCode::CONFLICT, "ERROR_CODE_COMPENSATION_NOT_PENDING")
        }
        "compensation proposal not found" => (
            StatusCode::NOT_FOUND,
            "ERROR_CODE_COMPENSATION_PROPOSAL_NOT_FOUND",
        ),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "ERROR_CODE_INTERNAL"),
    };
    (status, Json(ErrorResponse { error: code }))
}

fn compensation_retry_error(error: &AdminOpsStoreError) -> (StatusCode, Json<ErrorResponse>) {
    let (status, code) = match error.message() {
        "compensation retry forbidden" => (
            StatusCode::FORBIDDEN,
            "ERROR_CODE_COMPENSATION_RETRY_FORBIDDEN",
        ),
        "compensation proposal not found" => (
            StatusCode::NOT_FOUND,
            "ERROR_CODE_COMPENSATION_PROPOSAL_NOT_FOUND",
        ),
        "compensation retry not available" => (
            StatusCode::CONFLICT,
            "ERROR_CODE_COMPENSATION_RETRY_NOT_AVAILABLE",
        ),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "ERROR_CODE_INTERNAL"),
    };
    (status, Json(ErrorResponse { error: code }))
}

fn push_compensation_filters(
    builder: &mut QueryBuilder<'_, sqlx::Postgres>,
    query: &CompensationProposalListQuery,
) {
    let has_status = query
        .status
        .as_deref()
        .map(str::trim)
        .is_some_and(|status| !status.is_empty());
    let has_proposer_kind = query
        .proposer_kind
        .as_deref()
        .map(str::trim)
        .is_some_and(|proposer_kind| !proposer_kind.is_empty());
    if has_status || has_proposer_kind {
        builder.push(" WHERE ");
    }
    let mut separated = builder.separated(" AND ");
    if let Some(status) = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|status| !status.is_empty())
    {
        if status == "all_active" {
            separated.push(
                "status IN ('proposed', 'approved', 'posting', 'posting_failed', 'posting_unknown')",
            );
        } else {
            separated.push("status = ");
            separated.push_bind(status.to_owned());
        }
    }
    if let Some(proposer_kind) = query
        .proposer_kind
        .as_deref()
        .map(str::trim)
        .filter(|proposer_kind| !proposer_kind.is_empty())
    {
        separated.push("proposer_kind = ");
        separated.push_bind(proposer_kind.to_owned());
    }
}

#[derive(Debug, Clone)]
pub struct HttpAdminOpsStore {
    client: reqwest::Client,
    kyc_service_url: Url,
    user_service_url: Url,
    wallet_ledger_url: Option<Url>,
    risk_service_url: Option<Url>,
    referral_service_url: Option<Url>,
    notification_service_url: Option<Url>,
}

impl HttpAdminOpsStore {
    /// Creates an admin ops store backed by production service APIs.
    ///
    /// # Errors
    ///
    /// Returns an error when the upstream base URL is invalid.
    pub fn new(
        kyc_service_url: impl AsRef<str>,
        user_service_url: impl AsRef<str>,
    ) -> Result<Self, AdminOpsStoreError> {
        let kyc_service_url = Url::parse(kyc_service_url.as_ref())
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let user_service_url = Url::parse(user_service_url.as_ref())
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(Self {
            client: reqwest::Client::new(),
            kyc_service_url,
            user_service_url,
            wallet_ledger_url: None,
            risk_service_url: None,
            referral_service_url: None,
            notification_service_url: None,
        })
    }

    /// Creates an admin ops store with optional write-side-effect services.
    ///
    /// # Errors
    ///
    /// Returns an error when any configured upstream base URL is invalid.
    pub fn with_side_effect_services(
        kyc_service_url: impl AsRef<str>,
        user_service_url: impl AsRef<str>,
        wallet_ledger_url: Option<impl AsRef<str>>,
        risk_service_url: Option<impl AsRef<str>>,
    ) -> Result<Self, AdminOpsStoreError> {
        let mut store = Self::new(kyc_service_url, user_service_url)?;
        store.wallet_ledger_url = wallet_ledger_url
            .map(|url| Url::parse(url.as_ref()))
            .transpose()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        store.risk_service_url = risk_service_url
            .map(|url| Url::parse(url.as_ref()))
            .transpose()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(store)
    }

    /// Adds referral-service for manager orphan-affiliate checks.
    ///
    /// # Errors
    ///
    /// Returns an error when the referral-service base URL is invalid.
    pub fn with_referral_service(
        mut self,
        referral_service_url: impl AsRef<str>,
    ) -> Result<Self, AdminOpsStoreError> {
        self.referral_service_url = Some(
            Url::parse(referral_service_url.as_ref())
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        );
        Ok(self)
    }

    /// Adds notification-service as the persisted reminder outbox.
    ///
    /// # Errors
    ///
    /// Returns an error when the upstream base URL is invalid.
    pub fn with_notification_service(
        mut self,
        notification_service_url: impl AsRef<str>,
    ) -> Result<Self, AdminOpsStoreError> {
        self.notification_service_url = Some(
            Url::parse(notification_service_url.as_ref())
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        );
        Ok(self)
    }

    /// Loads upstream service URLs from the process environment.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured KYC service URL is invalid.
    pub fn from_env() -> Result<Self, AdminOpsStoreError> {
        let kyc_service_url = crate::kyc_service_base_url_from_env();
        let user_service_url =
            env::var("USER_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8084".to_owned());
        let store = Self::with_side_effect_services(
            kyc_service_url,
            user_service_url,
            env::var("WALLET_LEDGER_BASE_URL").ok(),
            env::var("RISK_SERVICE_URL").ok(),
        )?;
        let store = match env::var("REFERRAL_SERVICE_URL") {
            Ok(url) => store.with_referral_service(url)?,
            Err(_) => store,
        };
        match env::var("NOTIFICATION_SERVICE_URL") {
            Ok(url) => store.with_notification_service(url),
            Err(_) if env::var("PROPHETICA_ENV").is_ok_and(|value| value == "production") => Err(
                AdminOpsStoreError::new("NOTIFICATION_SERVICE_URL is required in production"),
            ),
            Err(_) => Ok(store),
        }
    }

    fn pending_cases_url(
        &self,
        query: &PendingMemberCasesQuery,
    ) -> Result<Url, AdminOpsStoreError> {
        let mut url = self
            .kyc_service_url
            .join("/v1/internal/admin/members/pending-cases")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(value) = query.q.as_deref() {
                pairs.append_pair("q", value);
            }
            if let Some(value) = query.status.as_deref() {
                pairs.append_pair("status", value);
            }
            if let Some(value) = query.restriction.as_deref() {
                pairs.append_pair("restriction", value);
            }
            if let Some(value) = query.page {
                pairs.append_pair("page", &value.to_string());
            }
            if let Some(value) = query.page_size {
                pairs.append_pair("page_size", &value.to_string());
            }
        }
        Ok(url)
    }

    fn user_members_url(&self, path: &str) -> Result<Url, AdminOpsStoreError> {
        self.user_service_url
            .join(path)
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))
    }

    fn list_members_url(&self, query: &MemberListQuery) -> Result<Url, AdminOpsStoreError> {
        let mut url = self.user_members_url("/v1/internal/admin/members")?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(value) = query.q.as_deref() {
                pairs.append_pair("q", value);
            }
            if let Some(value) = query.affiliate_id.as_deref() {
                pairs.append_pair("affiliate_id", value);
            }
        }
        Ok(url)
    }

    fn list_role_accounts_url(
        &self,
        query: &RoleAccountListQuery,
    ) -> Result<Url, AdminOpsStoreError> {
        let mut url = self.user_members_url("/v1/internal/admin/role-accounts")?;
        url.query_pairs_mut().append_pair("role", &query.role);
        Ok(url)
    }

    fn referral_admin_affiliates_url(&self) -> Result<Option<Url>, AdminOpsStoreError> {
        self.referral_service_url
            .as_ref()
            .map(|url| {
                url.join("/internal/admin/affiliates")
                    .map_err(|error| AdminOpsStoreError::new(error.to_string()))
            })
            .transpose()
    }

    fn referral_subject_scope_url(
        &self,
        affiliate_id: &str,
        subject_user_id: &str,
    ) -> Result<Url, AdminOpsStoreError> {
        let Some(base_url) = &self.referral_service_url else {
            return Err(AdminOpsStoreError::new("referral scope unavailable"));
        };
        let path = format!(
            "/internal/referrals/affiliates/{}/subjects/{}/scope",
            url_encode_path_segment(affiliate_id),
            url_encode_path_segment(subject_user_id)
        );
        base_url
            .join(&path)
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))
    }

    async fn post_compensation_ledger_transaction(
        &self,
        draft: &CompensationPostingDraft,
    ) -> CompensationLedgerPostOutcome {
        let Some(wallet_ledger_url) = &self.wallet_ledger_url else {
            return CompensationLedgerPostOutcome::Unknown {
                error_class: "network".to_owned(),
                message: "wallet ledger url is not configured".to_owned(),
            };
        };
        let transaction_url = match wallet_ledger_url.join("/internal/ledger/transactions") {
            Ok(url) => url,
            Err(error) => {
                return CompensationLedgerPostOutcome::Unknown {
                    error_class: "network".to_owned(),
                    message: error.to_string(),
                };
            }
        };
        let body = json!({
            "txn_type": "cs_compensation",
            "idempotency_key": draft.ledger_idempotency_key,
            "correlation_id": draft.correlation_id,
            "description": format!("customer support compensation {} for {}", draft.ticket_id, draft.subject_user_id),
            "metadata": {
                "proposal_id": draft.proposal_id,
                "ticket_id": draft.ticket_id,
                "subject_user_id": draft.subject_user_id,
                "ledger_idempotency_key": draft.ledger_idempotency_key,
                "posting_attempt_count": draft.posting_attempt_count,
            },
            "entries": [
                {
                    "account_code": "Expense.CustomerSupportCompensation",
                    "direction": "debit",
                    "amount_minor_units": draft.amount_minor_units
                },
                {
                    "account_code": format!("Player.Wallet.{}", draft.subject_user_id),
                    "direction": "credit",
                    "amount_minor_units": draft.amount_minor_units
                }
            ]
        });
        let response = self
            .client
            .post(transaction_url)
            .timeout(Duration::from_secs(5))
            .header("Idempotency-Key", &draft.ledger_idempotency_key)
            .json(&body)
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                return CompensationLedgerPostOutcome::Unknown {
                    error_class: if error.is_timeout() {
                        "ledger_timeout".to_owned()
                    } else {
                        "network".to_owned()
                    },
                    message: error.to_string(),
                };
            }
        };
        let status = response.status();
        if status.is_success() {
            return match response.json::<LedgerPostResponse>().await {
                Ok(posted) => CompensationLedgerPostOutcome::Posted {
                    txn_id: posted.txn_id,
                },
                Err(error) => CompensationLedgerPostOutcome::Unknown {
                    error_class: "network".to_owned(),
                    message: error.to_string(),
                },
            };
        }
        let message = response.text().await.unwrap_or_default();
        if status.is_client_error() {
            CompensationLedgerPostOutcome::Failed {
                error_class: "ledger_4xx".to_owned(),
                message,
            }
        } else {
            CompensationLedgerPostOutcome::Unknown {
                error_class: "ledger_5xx".to_owned(),
                message,
            }
        }
    }

    async fn find_compensation_ledger_transaction(
        &self,
        ledger_idempotency_key: &str,
    ) -> Result<Option<i64>, AdminOpsStoreError> {
        let Some(wallet_ledger_url) = &self.wallet_ledger_url else {
            return Ok(None);
        };
        let path = format!(
            "/internal/ledger/transactions/by-idempotency-key/{}",
            url_encode_path_segment(ledger_idempotency_key)
        );
        let url = wallet_ledger_url
            .join(&path)
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(AdminOpsStoreError::new("ledger lookup unavailable"));
        }
        response
            .json::<LedgerLookupResponse>()
            .await
            .map(|response| Some(response.txn_id))
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))
    }

    async fn referral_subject_scope(
        &self,
        affiliate_id: &str,
        subject_user_id: &str,
    ) -> Result<ReferralScopeResponse, AdminOpsStoreError> {
        let url = self.referral_subject_scope_url(affiliate_id, subject_user_id)?;
        let path = url.path().to_owned();
        let timestamp = current_epoch_seconds();
        let secret = env::var("ADMIN_INTERNAL_CALL_SECRET")
            .map_err(|_| AdminOpsStoreError::new("referral scope unavailable"))?;
        let signature = crate::internal_call_auth::sign_internal_call(
            "GET",
            &path,
            b"",
            timestamp,
            "api-gateway",
            "service",
            &secret,
        );
        let response = self
            .client
            .get(url)
            .header("x-actor-id", "api-gateway")
            .header("x-operator-role", "service")
            .header(
                "x-correlation-id",
                format!("referral-scope-{affiliate_id}-{subject_user_id}"),
            )
            .header("x-internal-call-timestamp", timestamp.to_string())
            .header("x-internal-call-signature", signature)
            .send()
            .await
            .map_err(|_| AdminOpsStoreError::new("referral scope unavailable"))?;
        if response.status().is_server_error() {
            return Err(AdminOpsStoreError::new("referral scope unavailable"));
        }
        if !response.status().is_success() {
            return Err(AdminOpsStoreError::new("compensation subject out of scope"));
        }
        response
            .json::<ReferralScopeResponse>()
            .await
            .map_err(|_| AdminOpsStoreError::new("referral scope unavailable"))
    }

    async fn user_service_json(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<Value, AdminOpsStoreError> {
        let response = request
            .send()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let upstream_code = serde_json::from_str::<UpstreamError>(&body)
                .ok()
                .map(|error| error.error);
            let message = upstream_code
                .as_deref()
                .unwrap_or("user service returned non-success status")
                .to_owned();
            return Err(AdminOpsStoreError::from_upstream(
                status.as_u16(),
                upstream_code,
                message,
            ));
        }

        response
            .json::<Value>()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))
    }

    async fn manager_has_active_affiliates(
        &self,
        manager_id: &str,
        context: &AdminOpsRequestContext,
    ) -> Result<bool, AdminOpsStoreError> {
        let Some(url) = self.referral_admin_affiliates_url()? else {
            return Ok(false);
        };
        let response = self
            .client
            .get(url)
            .headers(Self::user_service_actor_headers(context)?)
            .send()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .error_for_status()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .json::<Value>()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(response.as_array().is_some_and(|affiliates| {
            affiliates.iter().any(|affiliate| {
                let owner = affiliate
                    .get("managingManagerId")
                    .or_else(|| affiliate.get("managing_manager_id"))
                    .and_then(Value::as_str);
                let status = affiliate.get("status").and_then(Value::as_str);
                owner == Some(manager_id) && status != Some("archived")
            })
        }))
    }

    async fn role_account_is_active_manager(
        &self,
        user_id: &str,
        context: &AdminOpsRequestContext,
    ) -> Result<bool, AdminOpsStoreError> {
        let headers = Self::user_service_actor_headers(context)?;
        let accounts = self
            .user_service_json(
                self.client
                    .get(self.list_role_accounts_url(&RoleAccountListQuery {
                        role: "manager".to_owned(),
                    })?)
                    .headers(headers),
            )
            .await?;
        Ok(accounts
            .get("accounts")
            .and_then(Value::as_array)
            .is_some_and(|accounts| {
                accounts.iter().any(|account| {
                    account
                        .get("user_id")
                        .or_else(|| account.get("userId"))
                        .and_then(Value::as_str)
                        == Some(user_id)
                        && !account
                            .get("disabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(true)
                })
            }))
    }

    fn user_service_actor_headers(
        context: &AdminOpsRequestContext,
    ) -> Result<reqwest::header::HeaderMap, AdminOpsStoreError> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "x-actor-id",
            context
                .actor_user_id
                .parse()
                .map_err(|error| AdminOpsStoreError::new(format!("{error}")))?,
        );
        headers.insert(
            "x-operator-role",
            context
                .actor_role
                .parse()
                .map_err(|error| AdminOpsStoreError::new(format!("{error}")))?,
        );
        if let Some(request_id) = context.request_id.as_deref() {
            headers.insert(
                "x-request-id",
                request_id
                    .parse()
                    .map_err(|error| AdminOpsStoreError::new(format!("{error}")))?,
            );
        }
        Ok(headers)
    }

    async fn enrich_pending_contacts(
        &self,
        response: &mut PendingMemberCasesResponse,
    ) -> Result<(), AdminOpsStoreError> {
        if response.items.is_empty() {
            return Ok(());
        }
        let ids = response
            .items
            .iter()
            .map(|item| item.member_id.clone())
            .collect::<Vec<_>>();
        let contacts = self
            .client
            .post(self.user_members_url("/v1/internal/users/contacts:lookup-masked")?)
            .json(&LookupMaskedContactsRequest { ids })
            .send()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .error_for_status()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .json::<LookupMaskedContactsResponse>()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        for item in &mut response.items {
            if let Some(contact) = contacts
                .contacts
                .iter()
                .find(|contact| contact.id == item.member_id)
            {
                item.contact.email_masked = contact.email_masked.clone();
                item.contact.phone_masked = contact.phone_masked.clone();
            }
        }
        Ok(())
    }

    async fn request_kyc_review_task(
        &self,
        record: &MemberActionRecord,
    ) -> Result<(), AdminOpsStoreError> {
        let path = format!(
            "/api/v1/kyc/users/{}/upgrades",
            url_encode_path_segment(&record.target_user_id)
        );
        let url = self
            .kyc_service_url
            .join(&path)
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        self.client
            .post(url)
            .header("x-user-id", &record.target_user_id)
            .json(&json!({
                "target_tier": "TIER_2",
                "idempotency_key": format!("{}:kyc-review", record.correlation_id),
                "declared_monthly_income": null,
            }))
            .send()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .error_for_status()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(())
    }

    async fn freeze_available_wallet_balance(
        &self,
        record: &MemberActionRecord,
    ) -> Result<(), AdminOpsStoreError> {
        let Some(wallet_ledger_url) = &self.wallet_ledger_url else {
            return Ok(());
        };
        let balance_url = wallet_ledger_url
            .join(&format!(
                "/internal/ledger/wallets/{}/balance",
                url_encode_path_segment(&record.target_user_id)
            ))
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let balance = self
            .client
            .get(balance_url)
            .header("x-actor-id", &record.actor_user_id)
            .header("x-operator-role", &record.actor_role)
            .header("x-correlation-id", &record.correlation_id)
            .send()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .error_for_status()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .json::<Value>()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let available_minor_units = balance
            .get("available_minor_units")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let amount_to_freeze = available_minor_units.saturating_neg();
        if amount_to_freeze <= 0 {
            return Ok(());
        }

        let transaction_url = wallet_ledger_url
            .join("/internal/ledger/transactions")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        self.client
            .post(transaction_url)
            .header(
                "idempotency-key",
                format!("{}:wallet-freeze", record.correlation_id),
            )
            .json(&json!({
                "txn_type": "freeze",
                "correlation_id": record.correlation_id,
                "description": "admin member withdrawal restriction freeze",
                "metadata": {
                    "action_id": record.action_id,
                    "target_user_id": record.target_user_id,
                    "action_type": record.action_type,
                    "ticket_id": record.ticket_id,
                },
                "entries": [
                    {
                        "account_code": format!("Player.Wallet.{}", record.target_user_id),
                        "direction": "debit",
                        "amount_minor_units": amount_to_freeze
                    },
                    {
                        "account_code": format!("Player.Frozen.{}", record.target_user_id),
                        "direction": "credit",
                        "amount_minor_units": amount_to_freeze
                    }
                ]
            }))
            .send()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .error_for_status()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(())
    }

    async fn record_risk_member_action(
        &self,
        record: &MemberActionRecord,
    ) -> Result<(), AdminOpsStoreError> {
        let Some(risk_service_url) = &self.risk_service_url else {
            return Ok(());
        };
        let url = risk_service_url
            .join("/internal/risk/member-actions")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        self.client
            .post(url)
            .json(&json!({
                "target_user_id": record.target_user_id,
                "action_id": record.action_id,
                "action_type": record.action_type,
                "correlation_id": record.correlation_id,
                "reason": record.reason,
            }))
            .send()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .error_for_status()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(())
    }

    async fn enqueue_notification_reminder(&self, body: Value) -> Result<(), AdminOpsStoreError> {
        let Some(notification_service_url) = &self.notification_service_url else {
            return Ok(());
        };
        self.client
            .post(
                notification_service_url
                    .join("/internal/notifications/reminders")
                    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            )
            .json(&body)
            .send()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .error_for_status()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(())
    }

    async fn enqueue_pending_case_reminders(
        &self,
        response: &PendingMemberCasesResponse,
    ) -> Result<(), AdminOpsStoreError> {
        for item in &response.items {
            self.enqueue_notification_reminder(json!({
                "dedupe_key": format!(
                    "pending-case:{}:{}:{}",
                    item.member_id, item.restriction_reason, item.kyc_status
                ),
                "reminder_type": "pending_case",
                "target_user_id": &item.member_id,
                "title": "Pending member case requires review",
                "body": format!(
                    "{} ({}) requires review for {}.",
                    item.display_name, item.member_id, item.restriction_reason
                ),
                "channels": ["email", "in_app"],
                "recipients": [{
                    "recipient_role": "risk_ops",
                    "email": risk_ops_notification_email()
                }],
                "metadata": {
                    "status": &item.status,
                    "kyc_status": &item.kyc_status,
                    "affiliate_id": &item.affiliate_id
                }
            }))
            .await?;
        }
        Ok(())
    }

    async fn enqueue_member_action_approval_reminder(
        &self,
        response: &MemberActionResponse,
    ) -> Result<(), AdminOpsStoreError> {
        self.enqueue_notification_reminder(json!({
            "dedupe_key": format!("member-action:{}:approval-required", response.action_id),
            "reminder_type": "approval_required",
            "target_user_id": &response.target_user_id,
            "action_id": &response.action_id,
            "title": "Member action requires second approval",
            "body": format!(
                "{} for {} is waiting for a second approver.",
                response.action_type, response.target_user_id
            ),
            "channels": ["email", "in_app"],
            "recipients": [{
                "recipient_role": "ops_control",
                "email": ops_control_notification_email()
            }],
            "metadata": {
                "action_type": &response.action_type,
                "ticket_id": &response.audit.ticket_id,
                "actor_user_id": &response.audit.actor_user_id,
                "actor_role": &response.audit.actor_role,
                "correlation_id": &response.audit.correlation_id
            }
        }))
        .await
    }
}

#[derive(Debug, Serialize)]
struct LookupMaskedContactsRequest {
    ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LookupMaskedContactsResponse {
    contacts: Vec<LookupMaskedContact>,
}

#[derive(Debug, Deserialize)]
struct LookupMaskedContact {
    id: String,
    email_masked: String,
    phone_masked: String,
}

fn url_encode_path_segment(value: &str) -> String {
    value.replace('/', "%2F")
}

fn risk_ops_notification_email() -> String {
    env::var("NOTIFICATION_RISK_OPS_EMAIL")
        .unwrap_or_else(|_| "risk-ops@prophetica.internal".to_owned())
}

fn ops_control_notification_email() -> String {
    env::var("NOTIFICATION_OPS_CONTROL_EMAIL")
        .unwrap_or_else(|_| "ops-control@prophetica.internal".to_owned())
}

fn request_hash<T: Serialize>(request: &T) -> Result<String, AdminOpsStoreError> {
    let body =
        serde_json::to_vec(request).map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(body)))
}

fn member_action_response(
    action_id: String,
    request: MemberActionRequest,
    status: &str,
    requires_second_approval: bool,
    context: AdminOpsRequestContext,
    correlation_id: String,
) -> MemberActionResponse {
    MemberActionResponse {
        action_id,
        target_user_id: request.target_user_id.clone(),
        action_type: request.action_type,
        status: status.to_owned(),
        requires_second_approval,
        audit: MemberActionAuditMetadata {
            actor_user_id: context.actor_user_id,
            actor_role: context.actor_role,
            target_user_id: request.target_user_id,
            ticket_id: request.ticket_id,
            correlation_id,
            ledger_transaction_posted: false,
        },
    }
}

fn member_action_response_from_record(record: &MemberActionRecord) -> MemberActionResponse {
    MemberActionResponse {
        action_id: record.action_id.clone(),
        target_user_id: record.target_user_id.clone(),
        action_type: record.action_type.clone(),
        status: record.status.clone(),
        requires_second_approval: record.requires_second_approval,
        audit: MemberActionAuditMetadata {
            actor_user_id: record.actor_user_id.clone(),
            actor_role: record.actor_role.clone(),
            target_user_id: record.target_user_id.clone(),
            ticket_id: record.ticket_id.clone(),
            correlation_id: record.correlation_id.clone(),
            ledger_transaction_posted: false,
        },
    }
}

fn action_side_effects(action_type: &str) -> Vec<String> {
    match action_type {
        "add_note" => vec!["case_note:recorded".to_owned()],
        "request_kyc_review" => vec!["kyc_review_task:requested".to_owned()],
        "restrict_withdrawals" => vec![
            "member_restriction:withdrawals_locked".to_owned(),
            "wallet_freeze:available_balance".to_owned(),
            "risk_event:member_action_recorded".to_owned(),
        ],
        "escalate_case" => vec!["case_queue:escalated".to_owned()],
        _ => Vec::new(),
    }
}

fn proposer_kind_from_role(role: &str) -> &'static str {
    match role {
        "affiliate" => "affiliate",
        "manager" => "manager",
        "cs_system" => "cs_system",
        _ => "admin",
    }
}

fn affiliate_compensation_max_minor_units() -> i64 {
    env::var("AFFILIATE_COMPENSATION_MAX_MINOR_UNITS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(10_000_000)
}

async fn apply_member_action_side_effects(
    upstream: &HttpAdminOpsStore,
    record: &MemberActionRecord,
) -> Result<(), AdminOpsStoreError> {
    let summary = upstream.member_summary(&record.target_user_id).await?;
    let existing_restrictions = member_restrictions_from_summary(&summary);
    let request = match record.action_type.as_str() {
        "request_kyc_review" => {
            upstream.request_kyc_review_task(record).await?;
            Some(json!({
                "status": "restricted",
                "kyc_status": "review_required",
                "restrictions": merged_restrictions(
                    existing_restrictions,
                    &["manual_review_required"],
                ),
            }))
        }
        "restrict_withdrawals" => Some(json!({
            "status": "restricted",
            "restrictions": merged_restrictions(
                existing_restrictions,
                &["withdrawals_locked"],
            ),
        })),
        _ => None,
    };
    if record.action_type == "restrict_withdrawals" {
        upstream.freeze_available_wallet_balance(record).await?;
        upstream.record_risk_member_action(record).await?;
    }
    if let Some(request) = request {
        upstream
            .update_member(
                &record.target_user_id,
                request,
                AdminOpsRequestContext {
                    actor_user_id: record.actor_user_id.clone(),
                    actor_role: record.actor_role.clone(),
                    request_id: Some(record.correlation_id.clone()),
                    idempotency_key: None,
                    compensation_referral_relationship: None,
                },
            )
            .await?;
    }
    Ok(())
}

fn member_restrictions_from_summary(summary: &Value) -> Vec<String> {
    summary
        .get("kyc_risk")
        .and_then(|kyc_risk| kyc_risk.get("restrictions"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn merged_restrictions(existing: Vec<String>, additions: &[&str]) -> Vec<String> {
    let mut merged = existing;
    for addition in additions {
        if !merged.iter().any(|value| value == addition) {
            merged.push((*addition).to_owned());
        }
    }
    merged
}

#[derive(Debug, Clone)]
pub struct ProductionAdminOpsStore {
    upstream: HttpAdminOpsStore,
    compensation: PostgresCompensationProposalStore,
}

impl ProductionAdminOpsStore {
    #[must_use]
    pub const fn new(
        upstream: HttpAdminOpsStore,
        compensation: PostgresCompensationProposalStore,
    ) -> Self {
        Self {
            upstream,
            compensation,
        }
    }
}

#[async_trait]
impl AdminOpsStore for ProductionAdminOpsStore {
    async fn pending_member_cases(
        &self,
        query: PendingMemberCasesQuery,
    ) -> Result<PendingMemberCasesResponse, AdminOpsStoreError> {
        self.upstream.pending_member_cases(query).await
    }

    async fn list_members(&self, query: MemberListQuery) -> Result<Value, AdminOpsStoreError> {
        self.upstream.list_members(query).await
    }

    async fn list_role_accounts(
        &self,
        query: RoleAccountListQuery,
        context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError> {
        self.upstream.list_role_accounts(query, context).await
    }

    async fn member_summary(&self, user_id: &str) -> Result<Value, AdminOpsStoreError> {
        self.upstream.member_summary(user_id).await
    }

    async fn create_member(
        &self,
        request: Value,
        context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError> {
        self.upstream.create_member(request, context).await
    }

    async fn update_member(
        &self,
        user_id: &str,
        request: Value,
        context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError> {
        self.upstream.update_member(user_id, request, context).await
    }

    async fn delete_member(
        &self,
        user_id: &str,
        context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError> {
        self.upstream.delete_member(user_id, context).await
    }

    async fn list_member_actions(
        &self,
        query: MemberActionListQuery,
    ) -> Result<MemberActionListResponse, AdminOpsStoreError> {
        self.compensation.list_member_actions(query).await
    }

    async fn create_member_action(
        &self,
        request: MemberActionRequest,
        context: AdminOpsRequestContext,
    ) -> Result<MemberActionResponse, AdminOpsStoreError> {
        let response = self
            .compensation
            .create_member_action(request, context)
            .await?;
        if response.requires_second_approval {
            self.upstream
                .enqueue_member_action_approval_reminder(&response)
                .await?;
        }
        Ok(response)
    }

    async fn review_member_action(
        &self,
        action_id: &str,
        request: MemberActionReviewRequest,
        context: AdminOpsRequestContext,
    ) -> Result<MemberActionRecord, AdminOpsStoreError> {
        let record = self
            .compensation
            .review_member_action(action_id, request, context)
            .await?;
        if record.status == "approved" {
            apply_member_action_side_effects(&self.upstream, &record).await?;
        }
        Ok(record)
    }

    async fn list_compensation_proposals(
        &self,
        query: CompensationProposalListQuery,
    ) -> Result<CompensationProposalListResponse, AdminOpsStoreError> {
        self.compensation.list_compensation_proposals(query).await
    }

    async fn compensation_proposal_detail(
        &self,
        proposal_id: &str,
    ) -> Result<CompensationProposalDetailResponse, AdminOpsStoreError> {
        self.compensation
            .compensation_proposal_detail(proposal_id)
            .await
    }

    async fn review_compensation_proposal(
        &self,
        proposal_id: &str,
        request: CompensationReviewRequest,
        context: AdminOpsRequestContext,
    ) -> Result<CompensationProposalReviewResponse, AdminOpsStoreError> {
        let response = self
            .compensation
            .review_compensation_proposal(proposal_id, request, context)
            .await?;
        if response.proposal.status != CompensationProposalStatus::Approved || response.replayed {
            return Ok(response);
        }
        let draft = self
            .compensation
            .start_compensation_posting(proposal_id)
            .await?;
        let outcome = self
            .upstream
            .post_compensation_ledger_transaction(&draft)
            .await;
        let proposal = self
            .compensation
            .finish_compensation_posting(proposal_id, outcome)
            .await?;
        Ok(CompensationProposalReviewResponse {
            proposal,
            ..response
        })
    }

    async fn retry_compensation_posting(
        &self,
        proposal_id: &str,
        context: AdminOpsRequestContext,
    ) -> Result<CompensationPostingRetryResponse, AdminOpsStoreError> {
        if context.actor_role != "admin" {
            return Err(AdminOpsStoreError::new("compensation retry forbidden"));
        }
        let draft = self
            .compensation
            .prepare_compensation_manual_retry(proposal_id, &context)
            .await?;
        if let Some(txn_id) = self
            .upstream
            .find_compensation_ledger_transaction(&draft.ledger_idempotency_key)
            .await?
        {
            let proposal = self
                .compensation
                .mark_compensation_reconcile_posted(proposal_id, txn_id)
                .await?;
            return Ok(CompensationPostingRetryResponse {
                proposal,
                replayed: true,
            });
        }
        let retry_draft = self
            .compensation
            .start_compensation_retry_attempt(proposal_id)
            .await?;
        let outcome = self
            .upstream
            .post_compensation_ledger_transaction(&retry_draft)
            .await;
        let proposal = self
            .compensation
            .finish_compensation_posting(proposal_id, outcome)
            .await?;
        Ok(CompensationPostingRetryResponse {
            proposal,
            replayed: false,
        })
    }

    async fn create_compensation_proposal(
        &self,
        user_id: &str,
        request: CompensationProposalRequest,
        mut context: AdminOpsRequestContext,
    ) -> Result<CompensationProposalResponse, AdminOpsStoreError> {
        let proposer_kind = proposer_kind_from_role(&context.actor_role);
        if let Err(error) = self.upstream.member_summary(user_id).await {
            if proposer_kind == "affiliate" && error.upstream_status() == Some(404) {
                return Err(AdminOpsStoreError::new("compensation subject out of scope"));
            }
            return Err(error);
        }
        if proposer_kind == "affiliate" {
            let amount_minor_units = request
                .amount_minor_units
                .ok_or_else(|| AdminOpsStoreError::new("validated amount is missing"))?;
            if amount_minor_units > affiliate_compensation_max_minor_units() {
                return Err(AdminOpsStoreError::new(
                    "compensation amount over affiliate limit",
                ));
            }
            let scope = self
                .upstream
                .referral_subject_scope(&context.actor_user_id, user_id)
                .await?;
            if !scope.in_scope {
                return Err(AdminOpsStoreError::new("compensation subject out of scope"));
            }
            context.compensation_referral_relationship = scope.relationship;
        }
        self.compensation
            .create_compensation_proposal(user_id, request, context)
            .await
    }
}

#[derive(Debug, Clone)]
pub struct PostgresCompensationProposalStore {
    pool: PgPool,
}

impl PostgresCompensationProposalStore {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn list_compensation_proposals(
        &self,
        query: CompensationProposalListQuery,
    ) -> Result<CompensationProposalListResponse, AdminOpsStoreError> {
        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(25).clamp(1, 100);
        let offset = page.saturating_sub(1) * page_size;

        let mut count_builder =
            QueryBuilder::new("SELECT COUNT(*)::BIGINT AS total FROM admin_compensation_proposals");
        push_compensation_filters(&mut count_builder, &query);
        let total_row = count_builder
            .build()
            .fetch_one(&self.pool)
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let total_i64: i64 = total_row
            .try_get("total")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let total = usize::try_from(total_i64)
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;

        let mut list_builder = QueryBuilder::new(
            r"
            SELECT id, proposal_id, subject_user_id, ticket_id, reason, amount_minor_units,
                   status, proposer_kind, ledger_transaction_posted, ledger_transaction_id,
                   ledger_idempotency_key, posting_attempt_count, last_posting_error, correlation_id,
                   actor_user_id, approved_at::TEXT AS approved_at
            FROM admin_compensation_proposals
            ",
        );
        push_compensation_filters(&mut list_builder, &query);
        list_builder.push(" ORDER BY created_at DESC LIMIT ");
        list_builder.push_bind(
            i64::try_from(page_size).map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        );
        list_builder.push(" OFFSET ");
        list_builder.push_bind(
            i64::try_from(offset).map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        );
        let rows = list_builder
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let mut proposals = Vec::with_capacity(rows.len());
        for row in rows {
            proposals.push(
                compensation_response_from_row(&self.pool, &row)
                    .await?
                    .proposal,
            );
        }

        Ok(CompensationProposalListResponse {
            proposals,
            page,
            page_size,
            total,
            source: "api-gateway:compensation".to_owned(),
        })
    }

    async fn compensation_proposal_detail(
        &self,
        proposal_id: &str,
    ) -> Result<CompensationProposalDetailResponse, AdminOpsStoreError> {
        let row = sqlx::query(
            r"
            SELECT id, proposal_id, subject_user_id, ticket_id, reason, amount_minor_units,
                   status, proposer_kind, ledger_transaction_posted, ledger_transaction_id,
                   ledger_idempotency_key, posting_attempt_count, last_posting_error, correlation_id,
                   actor_user_id, approved_at::TEXT AS approved_at
            FROM admin_compensation_proposals
            WHERE proposal_id = $1
            ",
        )
        .bind(proposal_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
        .ok_or_else(|| AdminOpsStoreError::new("compensation proposal not found"))?;
        compensation_detail_from_row(&self.pool, &row).await
    }

    async fn review_compensation_proposal(
        &self,
        proposal_id: &str,
        request: CompensationReviewRequest,
        context: AdminOpsRequestContext,
    ) -> Result<CompensationProposalReviewResponse, AdminOpsStoreError> {
        if !matches!(context.actor_role.as_str(), "manager" | "admin") {
            return Err(AdminOpsStoreError::new("compensation review forbidden"));
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let row = sqlx::query(
            r"
            SELECT id, proposal_id, subject_user_id, ticket_id, reason, amount_minor_units,
                   status, proposer_kind, ledger_transaction_posted, ledger_transaction_id,
                   ledger_idempotency_key, posting_attempt_count, last_posting_error, correlation_id,
                   actor_user_id, approved_at::TEXT AS approved_at
            FROM admin_compensation_proposals
            WHERE proposal_id = $1
            FOR UPDATE
            ",
        )
        .bind(proposal_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
        .ok_or_else(|| AdminOpsStoreError::new("compensation proposal not found"))?;
        let row_id: i64 = row
            .try_get("id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let proposer_user_id: String = row
            .try_get("actor_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        if proposer_user_id == context.actor_user_id {
            return Err(AdminOpsStoreError::new("compensation self review"));
        }
        let status: String = row
            .try_get("status")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let existing_review = sqlx::query(
            r"
            SELECT decision, review_note
            FROM admin_compensation_proposal_approvals
            WHERE proposal_id = $1 AND approver_user_id = $2
            ",
        )
        .bind(row_id)
        .bind(&context.actor_user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        if status != "proposed" {
            if let Some(review_row) = existing_review {
                let decision = review_row
                    .try_get("decision")
                    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
                let note = review_row
                    .try_get("review_note")
                    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
                tx.commit()
                    .await
                    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
                let detail = self.compensation_proposal_detail(proposal_id).await?;
                return Ok(CompensationProposalReviewResponse {
                    proposal: detail.proposal,
                    review: CompensationReviewSummary {
                        reviewer_user_id: context.actor_user_id,
                        reviewer_role: context.actor_role,
                        decision,
                        note,
                    },
                    replayed: true,
                });
            }
            return Err(AdminOpsStoreError::new("compensation proposal not pending"));
        }
        if existing_review.is_some() {
            return Err(AdminOpsStoreError::new("compensation proposal not pending"));
        }

        sqlx::query(
            r"
            INSERT INTO admin_compensation_proposal_approvals (
                proposal_id, approver_user_id, role, decision, review_note
            )
            VALUES ($1, $2, $3, $4, $5)
            ",
        )
        .bind(row_id)
        .bind(&context.actor_user_id)
        .bind(&context.actor_role)
        .bind(&request.decision)
        .bind(request.note.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let next_status = if request.decision == "approved" {
            "approved"
        } else {
            "rejected"
        };
        sqlx::query(
            r"
            UPDATE admin_compensation_proposals
            SET status = $2,
                approved_at = CASE WHEN $2 = 'approved' THEN now() ELSE approved_at END
            WHERE id = $1
            ",
        )
        .bind(row_id)
        .bind(next_status)
        .execute(&mut *tx)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        insert_compensation_review_audit_event(&mut tx, row_id, &row, &request, &context).await?;
        tx.commit()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let detail = self.compensation_proposal_detail(proposal_id).await?;
        Ok(CompensationProposalReviewResponse {
            proposal: detail.proposal,
            review: CompensationReviewSummary {
                reviewer_user_id: context.actor_user_id,
                reviewer_role: context.actor_role,
                decision: request.decision,
                note: request.note,
            },
            replayed: false,
        })
    }

    async fn start_compensation_posting(
        &self,
        proposal_id: &str,
    ) -> Result<CompensationPostingDraft, AdminOpsStoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let ledger_idempotency_key = format!("cs-cmp-{proposal_id}");
        let row = sqlx::query(
            r"
            UPDATE admin_compensation_proposals
            SET status = 'posting',
                ledger_idempotency_key = COALESCE(ledger_idempotency_key, $2),
                posting_attempt_count = posting_attempt_count + 1,
                last_posting_error = NULL
            WHERE proposal_id = $1
              AND status = 'approved'
            RETURNING id, proposal_id, subject_user_id, ticket_id, amount_minor_units,
                      correlation_id, ledger_idempotency_key, posting_attempt_count,
                      proposer_kind, actor_user_id, actor_role
            ",
        )
        .bind(proposal_id)
        .bind(&ledger_idempotency_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
        .ok_or_else(|| AdminOpsStoreError::new("compensation proposal not pending"))?;
        let row_id: i64 = row
            .try_get("id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let subject_user_id: String = row
            .try_get("subject_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let ticket_id: String = row
            .try_get("ticket_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let amount_minor_units: i64 = row
            .try_get("amount_minor_units")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let correlation_id: String = row
            .try_get("correlation_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let stored_ledger_idempotency_key: String = row
            .try_get("ledger_idempotency_key")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let posting_attempt_count: i32 = row
            .try_get("posting_attempt_count")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let actor_user_id: String = row
            .try_get("actor_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let actor_role: String = row
            .try_get("actor_role")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let proposer_kind: String = row
            .try_get("proposer_kind")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        insert_compensation_posting_audit_event(
            &mut tx,
            row_id,
            "compensation_posting_attempt_started",
            &actor_user_id,
            &actor_role,
            &subject_user_id,
            &ticket_id,
            &correlation_id,
            json!({
                "proposer_kind": proposer_kind,
                "ledger_idempotency_key": stored_ledger_idempotency_key,
                "attempt": posting_attempt_count,
            }),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(CompensationPostingDraft {
            proposal_id: proposal_id.to_owned(),
            subject_user_id,
            ticket_id,
            amount_minor_units,
            correlation_id,
            ledger_idempotency_key: stored_ledger_idempotency_key,
            posting_attempt_count,
        })
    }

    async fn finish_compensation_posting(
        &self,
        proposal_id: &str,
        outcome: CompensationLedgerPostOutcome,
    ) -> Result<CompensationProposal, AdminOpsStoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let (next_status, ledger_transaction_id, last_error, action, error_class) = match &outcome {
            CompensationLedgerPostOutcome::Posted { txn_id } => (
                "posted",
                Some(*txn_id),
                None,
                "compensation_ledger_transaction_posted",
                None,
            ),
            CompensationLedgerPostOutcome::Failed {
                error_class,
                message,
            } => (
                "posting_failed",
                None,
                Some(format!("{error_class}: {message}")),
                "compensation_posting_attempt_failed",
                Some(error_class.clone()),
            ),
            CompensationLedgerPostOutcome::Unknown {
                error_class,
                message,
            } => (
                "posting_unknown",
                None,
                Some(format!("{error_class}: {message}")),
                "compensation_posting_attempt_failed",
                Some(error_class.clone()),
            ),
        };
        let row = sqlx::query(
            r"
            UPDATE admin_compensation_proposals
            SET status = $2,
                posted_at = CASE WHEN $2 = 'posted' THEN now() ELSE posted_at END,
                ledger_transaction_posted = CASE WHEN $2 = 'posted' THEN TRUE ELSE ledger_transaction_posted END,
                ledger_transaction_id = COALESCE($3, ledger_transaction_id),
                last_posting_error = $4
            WHERE proposal_id = $1
            RETURNING id, proposal_id, subject_user_id, ticket_id, reason, amount_minor_units,
                      status, proposer_kind, ledger_transaction_posted, ledger_transaction_id,
                      ledger_idempotency_key, posting_attempt_count, last_posting_error,
                      correlation_id, actor_user_id, actor_role, approved_at::TEXT AS approved_at
            ",
        )
        .bind(proposal_id)
        .bind(next_status)
        .bind(ledger_transaction_id)
        .bind(last_error.as_deref())
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let row_id: i64 = row
            .try_get("id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let subject_user_id: String = row
            .try_get("subject_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let ticket_id: String = row
            .try_get("ticket_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let correlation_id: String = row
            .try_get("correlation_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let actor_user_id: String = row
            .try_get("actor_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let actor_role: String = row
            .try_get("actor_role")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let proposer_kind: String = row
            .try_get("proposer_kind")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let ledger_idempotency_key: Option<String> = row
            .try_get("ledger_idempotency_key")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let posting_attempt_count: i32 = row
            .try_get("posting_attempt_count")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        insert_compensation_posting_audit_event(
            &mut tx,
            row_id,
            action,
            &actor_user_id,
            &actor_role,
            &subject_user_id,
            &ticket_id,
            &correlation_id,
            json!({
                "proposer_kind": proposer_kind,
                "ledger_idempotency_key": ledger_idempotency_key,
                "ledger_transaction_id": ledger_transaction_id,
                "attempt": posting_attempt_count,
                "error_class": error_class,
            }),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(compensation_response_from_row(&self.pool, &row)
            .await?
            .proposal)
    }

    /// Compensation reconcile algorithm shared by v1 manual retry and the deferred
    /// v1.1 worker:
    /// 1. Select proposals in posting/posting_failed/posting_unknown that are old
    ///    enough for reconciliation.
    /// 2. Read the existing ledger_idempotency_key from the proposal. If it is
    ///    NULL, audit and fail; never synthesize a retry key.
    /// 3. Query wallet-ledger by that key before any post attempt. This step is
    ///    mandatory for both posting_failed and posting_unknown because unknown
    ///    may mean "ledger committed, gateway lost the response".
    /// 4. If ledger returns an existing txn, mark the proposal posted and audit
    ///    compensation_reconcile_marked_posted.
    /// 5. If ledger does not have the txn, repost the original compensation with
    ///    the same ledger_idempotency_key, increment posting_attempt_count, then
    ///    apply the Task 6 success/failed/unknown outcome handling.
    /// The v1 worker shell is intentionally out of scope; manual retry invokes
    /// this algorithm directly.
    async fn prepare_compensation_manual_retry(
        &self,
        proposal_id: &str,
        context: &AdminOpsRequestContext,
    ) -> Result<CompensationPostingDraft, AdminOpsStoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let row = sqlx::query(
            r"
            SELECT id, proposal_id, subject_user_id, ticket_id, amount_minor_units,
                   status, proposer_kind, correlation_id, ledger_idempotency_key,
                   posting_attempt_count
            FROM admin_compensation_proposals
            WHERE proposal_id = $1
            FOR UPDATE
            ",
        )
        .bind(proposal_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
        .ok_or_else(|| AdminOpsStoreError::new("compensation proposal not found"))?;
        let status: String = row
            .try_get("status")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        if !matches!(
            status.as_str(),
            "posting" | "posting_failed" | "posting_unknown" | "posted"
        ) {
            return Err(AdminOpsStoreError::new("compensation retry not available"));
        }
        let ledger_idempotency_key: Option<String> = row
            .try_get("ledger_idempotency_key")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let Some(ledger_idempotency_key) = ledger_idempotency_key else {
            return Err(AdminOpsStoreError::new(
                "compensation ledger idempotency key missing",
            ));
        };
        let row_id: i64 = row
            .try_get("id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let subject_user_id: String = row
            .try_get("subject_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let ticket_id: String = row
            .try_get("ticket_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let amount_minor_units: i64 = row
            .try_get("amount_minor_units")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let correlation_id: String = row
            .try_get("correlation_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let proposer_kind: String = row
            .try_get("proposer_kind")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let posting_attempt_count: i32 = row
            .try_get("posting_attempt_count")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        insert_compensation_posting_audit_event(
            &mut tx,
            row_id,
            "compensation_posting_manual_retry",
            &context.actor_user_id,
            &context.actor_role,
            &subject_user_id,
            &ticket_id,
            &correlation_id,
            json!({
                "proposer_kind": proposer_kind,
                "ledger_idempotency_key": ledger_idempotency_key,
                "attempt": posting_attempt_count,
            }),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(CompensationPostingDraft {
            proposal_id: proposal_id.to_owned(),
            subject_user_id,
            ticket_id,
            amount_minor_units,
            correlation_id,
            ledger_idempotency_key,
            posting_attempt_count,
        })
    }

    async fn start_compensation_retry_attempt(
        &self,
        proposal_id: &str,
    ) -> Result<CompensationPostingDraft, AdminOpsStoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let row = sqlx::query(
            r"
            UPDATE admin_compensation_proposals
            SET status = 'posting',
                posting_attempt_count = posting_attempt_count + 1,
                last_posting_error = NULL
            WHERE proposal_id = $1
              AND status IN ('posting', 'posting_failed', 'posting_unknown')
              AND ledger_idempotency_key IS NOT NULL
            RETURNING id, proposal_id, subject_user_id, ticket_id, amount_minor_units,
                      correlation_id, ledger_idempotency_key, posting_attempt_count,
                      proposer_kind, actor_user_id, actor_role
            ",
        )
        .bind(proposal_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
        .ok_or_else(|| AdminOpsStoreError::new("compensation retry not available"))?;
        let row_id: i64 = row
            .try_get("id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let subject_user_id: String = row
            .try_get("subject_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let ticket_id: String = row
            .try_get("ticket_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let amount_minor_units: i64 = row
            .try_get("amount_minor_units")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let correlation_id: String = row
            .try_get("correlation_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let ledger_idempotency_key: String = row
            .try_get("ledger_idempotency_key")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let posting_attempt_count: i32 = row
            .try_get("posting_attempt_count")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let actor_user_id: String = row
            .try_get("actor_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let actor_role: String = row
            .try_get("actor_role")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let proposer_kind: String = row
            .try_get("proposer_kind")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        insert_compensation_posting_audit_event(
            &mut tx,
            row_id,
            "compensation_posting_attempt_started",
            &actor_user_id,
            &actor_role,
            &subject_user_id,
            &ticket_id,
            &correlation_id,
            json!({
                "proposer_kind": proposer_kind,
                "ledger_idempotency_key": ledger_idempotency_key,
                "attempt": posting_attempt_count,
            }),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(CompensationPostingDraft {
            proposal_id: proposal_id.to_owned(),
            subject_user_id,
            ticket_id,
            amount_minor_units,
            correlation_id,
            ledger_idempotency_key,
            posting_attempt_count,
        })
    }

    async fn mark_compensation_reconcile_posted(
        &self,
        proposal_id: &str,
        txn_id: i64,
    ) -> Result<CompensationProposal, AdminOpsStoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let row = sqlx::query(
            r"
            UPDATE admin_compensation_proposals
            SET status = 'posted',
                posted_at = now(),
                ledger_transaction_posted = TRUE,
                ledger_transaction_id = $2,
                last_posting_error = NULL
            WHERE proposal_id = $1
            RETURNING id, proposal_id, subject_user_id, ticket_id, reason, amount_minor_units,
                      status, proposer_kind, ledger_transaction_posted, ledger_transaction_id,
                      ledger_idempotency_key, posting_attempt_count, last_posting_error,
                      correlation_id, actor_user_id, actor_role, approved_at::TEXT AS approved_at
            ",
        )
        .bind(proposal_id)
        .bind(txn_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let row_id: i64 = row
            .try_get("id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let subject_user_id: String = row
            .try_get("subject_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let ticket_id: String = row
            .try_get("ticket_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let correlation_id: String = row
            .try_get("correlation_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let actor_user_id: String = row
            .try_get("actor_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let actor_role: String = row
            .try_get("actor_role")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let proposer_kind: String = row
            .try_get("proposer_kind")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let ledger_idempotency_key: Option<String> = row
            .try_get("ledger_idempotency_key")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        insert_compensation_posting_audit_event(
            &mut tx,
            row_id,
            "compensation_reconcile_marked_posted",
            &actor_user_id,
            &actor_role,
            &subject_user_id,
            &ticket_id,
            &correlation_id,
            json!({
                "proposer_kind": proposer_kind,
                "ledger_idempotency_key": ledger_idempotency_key,
                "ledger_transaction_id": txn_id,
            }),
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(compensation_response_from_row(&self.pool, &row)
            .await?
            .proposal)
    }

    async fn find_compensation_by_idempotency_key(
        &self,
        idempotency_key: &str,
        request_hash: &str,
    ) -> Result<Option<CompensationProposalResponse>, AdminOpsStoreError> {
        let Some(row) = sqlx::query(
            r"
            SELECT id, proposal_id, subject_user_id, ticket_id, reason, amount_minor_units,
                   status, proposer_kind, ledger_transaction_posted, ledger_transaction_id,
                   ledger_idempotency_key, posting_attempt_count, last_posting_error, correlation_id,
                   request_hash, actor_user_id, approved_at::TEXT AS approved_at
            FROM admin_compensation_proposals
            WHERE idempotency_key = $1
            ",
        )
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
        else {
            return Ok(None);
        };
        let stored_hash: Option<String> = row
            .try_get("request_hash")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        if stored_hash.as_deref() != Some(request_hash) {
            return Err(AdminOpsStoreError::new("idempotency key payload mismatch"));
        }
        compensation_response_from_row(&self.pool, &row)
            .await
            .map(Some)
    }

    async fn find_member_action_by_idempotency_key(
        &self,
        idempotency_key: &str,
        request_hash: &str,
    ) -> Result<Option<MemberActionResponse>, AdminOpsStoreError> {
        let Some(row) = sqlx::query(
            r"
            SELECT action_id, target_user_id, action_type, ticket_id, status,
                   requires_second_approval, actor_user_id, actor_role, correlation_id,
                   ledger_transaction_posted, request_hash
            FROM admin_member_actions
            WHERE idempotency_key = $1
            ",
        )
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
        else {
            return Ok(None);
        };
        let stored_hash: Option<String> = row
            .try_get("request_hash")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        if stored_hash.as_deref() != Some(request_hash) {
            return Err(AdminOpsStoreError::new("idempotency key payload mismatch"));
        }
        member_action_response_from_row(&row).map(Some)
    }

    async fn create_compensation_proposal(
        &self,
        user_id: &str,
        request: CompensationProposalRequest,
        context: AdminOpsRequestContext,
    ) -> Result<CompensationProposalResponse, AdminOpsStoreError> {
        let request_hash = request_hash(&request)?;
        if let Some(idempotency_key) = context.idempotency_key.as_deref() {
            if let Some(existing) = self
                .find_compensation_by_idempotency_key(idempotency_key, &request_hash)
                .await?
            {
                return Ok(existing);
            }
        }
        let ticket_id = request
            .ticket_id
            .ok_or_else(|| AdminOpsStoreError::new("validated ticket id is missing"))?;
        let reason = request
            .reason
            .ok_or_else(|| AdminOpsStoreError::new("validated reason is missing"))?;
        let amount_minor_units = request
            .amount_minor_units
            .ok_or_else(|| AdminOpsStoreError::new("validated amount is missing"))?;
        let proposer_kind = proposer_kind_from_role(&context.actor_role).to_owned();
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let correlation_seed: i64 =
            sqlx::query_scalar("SELECT nextval('admin_compensation_proposals_id_seq')")
                .fetch_one(&mut *tx)
                .await
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let correlation_id = format!("admin-compensation-{ticket_id}-{correlation_seed}");
        let row = sqlx::query(
            r"
            INSERT INTO admin_compensation_proposals (
                id, subject_user_id, ticket_id, reason, amount_minor_units, status,
                proposer_kind, ledger_transaction_posted, correlation_id, actor_user_id, actor_role,
                idempotency_key, request_hash
            )
            VALUES ($1, $2, $3, $4, $5, 'proposed', $6, FALSE, $7, $8, $9, $10, $11)
            RETURNING id, proposal_id
            ",
        )
        .bind(correlation_seed)
        .bind(user_id)
        .bind(&ticket_id)
        .bind(&reason)
        .bind(amount_minor_units)
        .bind(&proposer_kind)
        .bind(&correlation_id)
        .bind(&context.actor_user_id)
        .bind(&context.actor_role)
        .bind(&context.idempotency_key)
        .bind(&request_hash)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let row_id: i64 = row
            .try_get("id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let proposal_id: String = row
            .try_get("proposal_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;

        insert_compensation_audit_event(
            &mut tx,
            row_id,
            user_id,
            &ticket_id,
            &correlation_id,
            amount_minor_units,
            &proposer_kind,
            &context,
        )
        .await?;
        tx.commit()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;

        Ok(compensation_proposal_response(
            CompensationProposalResponseParts {
                proposal_id,
                user_id: user_id.to_owned(),
                ticket_id,
                reason,
                amount_minor_units,
                approvals: Vec::new(),
                correlation_id,
                approval_roles: Vec::new(),
                proposer_kind,
                proposer_user_id: context.actor_user_id,
            },
        ))
    }

    async fn create_member_action(
        &self,
        request: MemberActionRequest,
        context: AdminOpsRequestContext,
    ) -> Result<MemberActionResponse, AdminOpsStoreError> {
        let request_hash = request_hash(&request)?;
        if let Some(idempotency_key) = context.idempotency_key.as_deref() {
            if let Some(existing) = self
                .find_member_action_by_idempotency_key(idempotency_key, &request_hash)
                .await?
            {
                return Ok(existing);
            }
        }
        let requires_second_approval = request.action_type != "add_note";
        let status = if requires_second_approval {
            "pending_second_approval"
        } else {
            "submitted"
        };
        let correlation_id = request
            .correlation_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map_or_else(
                || {
                    let suffix = context
                        .request_id
                        .as_deref()
                        .unwrap_or(request.ticket_id.as_str());
                    format!("admin-member-action-{suffix}")
                },
                str::to_owned,
            );
        let row = sqlx::query(
            r"
            INSERT INTO admin_member_actions (
                target_user_id, action_type, ticket_id, reason, status,
                requires_second_approval, actor_user_id, actor_role, correlation_id,
                idempotency_key, request_hash
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING action_id
            ",
        )
        .bind(&request.target_user_id)
        .bind(&request.action_type)
        .bind(&request.ticket_id)
        .bind(&request.reason)
        .bind(status)
        .bind(requires_second_approval)
        .bind(&context.actor_user_id)
        .bind(&context.actor_role)
        .bind(&correlation_id)
        .bind(&context.idempotency_key)
        .bind(&request_hash)
        .fetch_one(&self.pool)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let action_id = row
            .try_get("action_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(member_action_response(
            action_id,
            request,
            status,
            requires_second_approval,
            context,
            correlation_id,
        ))
    }

    async fn list_member_actions(
        &self,
        query: MemberActionListQuery,
    ) -> Result<MemberActionListResponse, AdminOpsStoreError> {
        let rows = sqlx::query(
            r"
            SELECT action_id, target_user_id, action_type, ticket_id, reason, status,
                   requires_second_approval, actor_user_id, actor_role, correlation_id,
                   reviewer_user_id, reviewer_role, review_decision, review_reason,
                   reviewed_at::text AS reviewed_at, side_effects, created_at::text AS created_at
            FROM admin_member_actions
            WHERE ($1::TEXT IS NULL OR status = $1)
              AND ($2::TEXT IS NULL OR target_user_id = $2)
            ORDER BY created_at DESC
            LIMIT 200
            ",
        )
        .bind(query.status.as_deref())
        .bind(query.target_user_id.as_deref())
        .fetch_all(&self.pool)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        Ok(MemberActionListResponse {
            actions: rows
                .iter()
                .map(member_action_record_from_row)
                .collect::<Result<Vec<_>, _>>()?,
            source: "api-gateway:admin-ops".to_owned(),
        })
    }

    async fn review_member_action(
        &self,
        action_id: &str,
        request: MemberActionReviewRequest,
        context: AdminOpsRequestContext,
    ) -> Result<MemberActionRecord, AdminOpsStoreError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let Some(existing) = sqlx::query(
            r"
            SELECT action_id, action_type, status, actor_user_id
            FROM admin_member_actions
            WHERE action_id = $1
            FOR UPDATE
            ",
        )
        .bind(action_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
        else {
            return Err(AdminOpsStoreError::new("member action not found"));
        };
        let actor_user_id: String = existing
            .try_get("actor_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        if actor_user_id == context.actor_user_id {
            return Err(AdminOpsStoreError::new(
                "member action self approval is not allowed",
            ));
        }
        let current_status: String = existing
            .try_get("status")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        if current_status != "pending_second_approval" {
            return Err(AdminOpsStoreError::new(
                "member action cannot be reviewed in its current status",
            ));
        }
        let action_type: String = existing
            .try_get("action_type")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let next_status = if request.decision == "approve" {
            "approved"
        } else {
            "rejected"
        };
        let side_effects = if request.decision == "approve" {
            action_side_effects(&action_type)
        } else {
            Vec::new()
        };
        let row = sqlx::query(
            r"
            UPDATE admin_member_actions
            SET status = $2,
                reviewer_user_id = $3,
                reviewer_role = $4,
                review_decision = $5,
                review_reason = $6,
                reviewed_at = now(),
                side_effects = $7
            WHERE action_id = $1
            RETURNING action_id, target_user_id, action_type, ticket_id, reason, status,
                      requires_second_approval, actor_user_id, actor_role, correlation_id,
                      reviewer_user_id, reviewer_role, review_decision, review_reason,
                      reviewed_at::text AS reviewed_at, side_effects, created_at::text AS created_at
            ",
        )
        .bind(action_id)
        .bind(next_status)
        .bind(&context.actor_user_id)
        .bind(&context.actor_role)
        .bind(&request.decision)
        .bind(&request.reason)
        .bind(serde_json::json!(side_effects))
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        member_action_record_from_row(&row)
    }
}

async fn insert_compensation_audit_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    proposal_row_id: i64,
    user_id: &str,
    ticket_id: &str,
    correlation_id: &str,
    amount_minor_units: i64,
    proposer_kind: &str,
    context: &AdminOpsRequestContext,
) -> Result<(), AdminOpsStoreError> {
    let mut metadata = json!({
        "proposer_kind": proposer_kind,
        "amount_minor_units": amount_minor_units,
        "ticket_id": ticket_id,
    });
    if let Some(relationship) = context.compensation_referral_relationship.as_deref() {
        metadata["relationship"] = json!(relationship);
    }
    sqlx::query(
        r"
        INSERT INTO admin_compensation_audit_events (
            proposal_id, action, actor_user_id, actor_role, subject_user_id,
            ticket_id, correlation_id, metadata
        )
        VALUES (
            $1, 'compensation_proposal_created', $2, $3, $4,
            $5, $6, $7
        )
        ",
    )
    .bind(proposal_row_id)
    .bind(&context.actor_user_id)
    .bind(&context.actor_role)
    .bind(user_id)
    .bind(ticket_id)
    .bind(correlation_id)
    .bind(metadata)
    .execute(&mut **tx)
    .await
    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    Ok(())
}

async fn insert_compensation_review_audit_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    proposal_row_id: i64,
    proposal_row: &sqlx::postgres::PgRow,
    request: &CompensationReviewRequest,
    context: &AdminOpsRequestContext,
) -> Result<(), AdminOpsStoreError> {
    let action = if request.decision == "approved" {
        "compensation_proposal_approved"
    } else {
        "compensation_proposal_rejected"
    };
    let subject_user_id: String = proposal_row
        .try_get("subject_user_id")
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    let ticket_id: String = proposal_row
        .try_get("ticket_id")
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    let correlation_id: String = proposal_row
        .try_get("correlation_id")
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    let proposer_kind: String = proposal_row
        .try_get("proposer_kind")
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    sqlx::query(
        r"
        INSERT INTO admin_compensation_audit_events (
            proposal_id, action, actor_user_id, actor_role, subject_user_id,
            ticket_id, correlation_id, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ",
    )
    .bind(proposal_row_id)
    .bind(action)
    .bind(&context.actor_user_id)
    .bind(&context.actor_role)
    .bind(&subject_user_id)
    .bind(&ticket_id)
    .bind(&correlation_id)
    .bind(json!({
        "proposer_kind": proposer_kind,
        "decision": request.decision,
        "note": request.note,
    }))
    .execute(&mut **tx)
    .await
    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    Ok(())
}

async fn insert_compensation_posting_audit_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    proposal_row_id: i64,
    action: &str,
    actor_user_id: &str,
    actor_role: &str,
    subject_user_id: &str,
    ticket_id: &str,
    correlation_id: &str,
    metadata: Value,
) -> Result<(), AdminOpsStoreError> {
    sqlx::query(
        r"
        INSERT INTO admin_compensation_audit_events (
            proposal_id, action, actor_user_id, actor_role, subject_user_id,
            ticket_id, correlation_id, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ",
    )
    .bind(proposal_row_id)
    .bind(action)
    .bind(actor_user_id)
    .bind(actor_role)
    .bind(subject_user_id)
    .bind(ticket_id)
    .bind(correlation_id)
    .bind(metadata)
    .execute(&mut **tx)
    .await
    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    Ok(())
}

struct CompensationProposalResponseParts {
    proposal_id: String,
    user_id: String,
    ticket_id: String,
    reason: String,
    amount_minor_units: i64,
    approvals: Vec<CompensationApprovalRequest>,
    correlation_id: String,
    approval_roles: Vec<String>,
    proposer_kind: String,
    proposer_user_id: String,
}

fn compensation_proposal_response(
    parts: CompensationProposalResponseParts,
) -> CompensationProposalResponse {
    CompensationProposalResponse {
        proposal: CompensationProposal {
            proposal_id: parts.proposal_id,
            user_id: parts.user_id.clone(),
            ticket_id: parts.ticket_id.clone(),
            reason: parts.reason,
            amount_minor_units: parts.amount_minor_units,
            status: CompensationProposalStatus::Proposed,
            proposer_kind: parts.proposer_kind.clone(),
            proposer_user_id: parts.proposer_user_id,
            approvals: parts.approvals,
            ledger_transaction_posted: false,
            ledger_idempotency_key: None,
            ledger_transaction_id: None,
            posting_attempt_count: 0,
            last_posting_error: None,
            approved_at: None,
        },
        audit: CompensationAuditMetadata {
            action: "compensation_proposal_created".to_owned(),
            subject_user_id: parts.user_id,
            ticket_id: parts.ticket_id,
            correlation_id: parts.correlation_id,
            approval_roles: parts.approval_roles,
            proposer_kind: parts.proposer_kind,
            ledger_transaction_posted: false,
        },
    }
}

async fn compensation_response_from_row(
    pool: &PgPool,
    row: &sqlx::postgres::PgRow,
) -> Result<CompensationProposalResponse, AdminOpsStoreError> {
    let row_id: i64 = row
        .try_get("id")
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    let approvals = sqlx::query(
        r"
        SELECT approver_user_id, role
        FROM admin_compensation_proposal_approvals
        WHERE proposal_id = $1
        ORDER BY ordinal NULLS LAST, reviewed_at
        ",
    )
    .bind(row_id)
    .fetch_all(pool)
    .await
    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
    .iter()
    .map(|row| {
        Ok(CompensationApprovalRequest {
            approver_user_id: row
                .try_get("approver_user_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            role: row
                .try_get("role")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        })
    })
    .collect::<Result<Vec<_>, AdminOpsStoreError>>()?;
    let approval_roles = approvals
        .iter()
        .map(|approval| approval.role.clone())
        .collect::<Vec<_>>();
    let status: String = row
        .try_get("status")
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    Ok(CompensationProposalResponse {
        proposal: CompensationProposal {
            proposal_id: row
                .try_get("proposal_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            user_id: row
                .try_get("subject_user_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            ticket_id: row
                .try_get("ticket_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            reason: row
                .try_get("reason")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            amount_minor_units: row
                .try_get("amount_minor_units")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            status: compensation_status_from_str(&status),
            proposer_kind: row
                .try_get("proposer_kind")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            proposer_user_id: row
                .try_get("actor_user_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            approvals,
            ledger_transaction_posted: row
                .try_get("ledger_transaction_posted")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            ledger_idempotency_key: row
                .try_get("ledger_idempotency_key")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            ledger_transaction_id: row
                .try_get("ledger_transaction_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            posting_attempt_count: row
                .try_get("posting_attempt_count")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            last_posting_error: row
                .try_get("last_posting_error")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            approved_at: row
                .try_get("approved_at")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        },
        audit: CompensationAuditMetadata {
            action: "compensation_proposal_created".to_owned(),
            subject_user_id: row
                .try_get("subject_user_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            ticket_id: row
                .try_get("ticket_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            correlation_id: row
                .try_get("correlation_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            approval_roles,
            proposer_kind: row
                .try_get("proposer_kind")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            ledger_transaction_posted: row
                .try_get("ledger_transaction_posted")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        },
    })
}

async fn compensation_detail_from_row(
    pool: &PgPool,
    row: &sqlx::postgres::PgRow,
) -> Result<CompensationProposalDetailResponse, AdminOpsStoreError> {
    let response = compensation_response_from_row(pool, row).await?;
    let row_id: i64 = row
        .try_get("id")
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    let audit_rows = sqlx::query(
        r"
        SELECT action, actor_user_id, actor_role, correlation_id, metadata, created_at::TEXT AS created_at
        FROM admin_compensation_audit_events
        WHERE proposal_id = $1
        ORDER BY created_at DESC, id DESC
        LIMIT 10
        ",
    )
    .bind(row_id)
    .fetch_all(pool)
    .await
    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    let recent_audit_events = audit_rows
        .iter()
        .map(|row| {
            Ok(CompensationAuditEventSummary {
                action: row
                    .try_get("action")
                    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
                actor_user_id: row
                    .try_get("actor_user_id")
                    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
                actor_role: row
                    .try_get("actor_role")
                    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
                correlation_id: row
                    .try_get("correlation_id")
                    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
                metadata: row
                    .try_get("metadata")
                    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
                created_at: row
                    .try_get("created_at")
                    .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            })
        })
        .collect::<Result<Vec<_>, AdminOpsStoreError>>()?;
    Ok(CompensationProposalDetailResponse {
        approvals: response.proposal.approvals.clone(),
        posting_attempt_count: response.proposal.posting_attempt_count,
        last_posting_error: response.proposal.last_posting_error.clone(),
        proposal: response.proposal,
        recent_audit_events,
    })
}

fn compensation_status_from_str(status: &str) -> CompensationProposalStatus {
    match status {
        "proposed" => CompensationProposalStatus::Proposed,
        "approved" => CompensationProposalStatus::Approved,
        "posting" => CompensationProposalStatus::Posting,
        "posted" => CompensationProposalStatus::Posted,
        "posting_failed" => CompensationProposalStatus::PostingFailed,
        "posting_unknown" => CompensationProposalStatus::PostingUnknown,
        "rejected" => CompensationProposalStatus::Rejected,
        "cancelled" => CompensationProposalStatus::Cancelled,
        "pending_approval" => CompensationProposalStatus::PendingApproval,
        _ => CompensationProposalStatus::ApprovedReadyForLedger,
    }
}

fn compensation_status_matches(status: CompensationProposalStatus, filter: Option<&str>) -> bool {
    match filter.map(str::trim).filter(|filter| !filter.is_empty()) {
        None => true,
        Some("all_active") => matches!(
            status,
            CompensationProposalStatus::Proposed
                | CompensationProposalStatus::Approved
                | CompensationProposalStatus::Posting
                | CompensationProposalStatus::PostingFailed
                | CompensationProposalStatus::PostingUnknown
        ),
        Some(filter) => compensation_status_name(status) == filter,
    }
}

fn compensation_status_name(status: CompensationProposalStatus) -> &'static str {
    match status {
        CompensationProposalStatus::Proposed => "proposed",
        CompensationProposalStatus::Approved => "approved",
        CompensationProposalStatus::Posting => "posting",
        CompensationProposalStatus::Posted => "posted",
        CompensationProposalStatus::PostingFailed => "posting_failed",
        CompensationProposalStatus::PostingUnknown => "posting_unknown",
        CompensationProposalStatus::Rejected => "rejected",
        CompensationProposalStatus::Cancelled => "cancelled",
        CompensationProposalStatus::PendingApproval => "pending_approval",
        CompensationProposalStatus::ApprovedReadyForLedger => "approved_ready_for_ledger",
    }
}

fn member_action_response_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<MemberActionResponse, AdminOpsStoreError> {
    Ok(MemberActionResponse {
        action_id: row
            .try_get("action_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        target_user_id: row
            .try_get("target_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        action_type: row
            .try_get("action_type")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        status: row
            .try_get("status")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        requires_second_approval: row
            .try_get("requires_second_approval")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        audit: MemberActionAuditMetadata {
            actor_user_id: row
                .try_get("actor_user_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            actor_role: row
                .try_get("actor_role")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            target_user_id: row
                .try_get("target_user_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            ticket_id: row
                .try_get("ticket_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            correlation_id: row
                .try_get("correlation_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            ledger_transaction_posted: row
                .try_get("ledger_transaction_posted")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        },
    })
}

fn member_action_record_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<MemberActionRecord, AdminOpsStoreError> {
    let side_effects_value: Value = row
        .try_get("side_effects")
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    let side_effects = serde_json::from_value::<Vec<String>>(side_effects_value)
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    let review_decision: Option<String> = row
        .try_get("review_decision")
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    let review = if let Some(decision) = review_decision {
        Some(MemberActionReviewSummary {
            reviewer_user_id: row
                .try_get("reviewer_user_id")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            reviewer_role: row
                .try_get("reviewer_role")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            decision,
            reason: row
                .try_get("review_reason")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
            reviewed_at: row
                .try_get("reviewed_at")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        })
    } else {
        None
    };
    Ok(MemberActionRecord {
        action_id: row
            .try_get("action_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        target_user_id: row
            .try_get("target_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        action_type: row
            .try_get("action_type")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        ticket_id: row
            .try_get("ticket_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        reason: row
            .try_get("reason")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        status: row
            .try_get("status")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        requires_second_approval: row
            .try_get("requires_second_approval")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        actor_user_id: row
            .try_get("actor_user_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        actor_role: row
            .try_get("actor_role")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        correlation_id: row
            .try_get("correlation_id")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        review,
        side_effects,
        created_at: row
            .try_get("created_at")
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
    })
}

#[async_trait]
impl AdminOpsStore for HttpAdminOpsStore {
    async fn pending_member_cases(
        &self,
        query: PendingMemberCasesQuery,
    ) -> Result<PendingMemberCasesResponse, AdminOpsStoreError> {
        let mut response = self
            .client
            .get(self.pending_cases_url(&query)?)
            .send()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .error_for_status()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .json::<PendingMemberCasesResponse>()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;

        self.enrich_pending_contacts(&mut response).await?;
        self.enqueue_pending_case_reminders(&response).await?;
        Ok(response)
    }

    async fn list_members(&self, query: MemberListQuery) -> Result<Value, AdminOpsStoreError> {
        self.user_service_json(self.client.get(self.list_members_url(&query)?))
            .await
    }

    async fn list_role_accounts(
        &self,
        query: RoleAccountListQuery,
        context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError> {
        let headers = Self::user_service_actor_headers(&context)?;
        self.user_service_json(
            self.client
                .get(self.list_role_accounts_url(&query)?)
                .headers(headers),
        )
        .await
    }

    async fn member_summary(&self, user_id: &str) -> Result<Value, AdminOpsStoreError> {
        let path = format!(
            "/v1/internal/admin/members/{}/summary",
            url_encode_path_segment(user_id)
        );
        self.user_service_json(self.client.get(self.user_members_url(&path)?))
            .await
    }

    async fn create_member(
        &self,
        request: Value,
        context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError> {
        let headers = Self::user_service_actor_headers(&context)?;
        self.user_service_json(
            self.client
                .post(self.user_members_url("/v1/internal/admin/members")?)
                .headers(headers)
                .json(&request),
        )
        .await
    }

    async fn update_member(
        &self,
        user_id: &str,
        request: Value,
        context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError> {
        let headers = Self::user_service_actor_headers(&context)?;
        let path = format!(
            "/v1/internal/admin/members/{}",
            url_encode_path_segment(user_id)
        );
        self.user_service_json(
            self.client
                .patch(self.user_members_url(&path)?)
                .headers(headers)
                .json(&request),
        )
        .await
    }

    async fn delete_member(
        &self,
        user_id: &str,
        context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError> {
        if self
            .role_account_is_active_manager(user_id, &context)
            .await?
            && self
                .manager_has_active_affiliates(user_id, &context)
                .await?
        {
            return Err(AdminOpsStoreError::from_upstream(
                StatusCode::CONFLICT.as_u16(),
                Some("ERROR_CODE_MANAGER_HAS_AFFILIATES".to_owned()),
                "manager has active affiliates; reassign them before disabling",
            ));
        }
        let headers = Self::user_service_actor_headers(&context)?;
        let path = format!(
            "/v1/internal/admin/members/{}",
            url_encode_path_segment(user_id)
        );
        self.user_service_json(
            self.client
                .delete(self.user_members_url(&path)?)
                .headers(headers),
        )
        .await
    }

    async fn list_member_actions(
        &self,
        _query: MemberActionListQuery,
    ) -> Result<MemberActionListResponse, AdminOpsStoreError> {
        Err(AdminOpsStoreError::new(
            "member action production source is not connected",
        ))
    }

    async fn list_compensation_proposals(
        &self,
        _query: CompensationProposalListQuery,
    ) -> Result<CompensationProposalListResponse, AdminOpsStoreError> {
        Err(AdminOpsStoreError::new(
            "compensation proposal production source is not connected",
        ))
    }

    async fn compensation_proposal_detail(
        &self,
        _proposal_id: &str,
    ) -> Result<CompensationProposalDetailResponse, AdminOpsStoreError> {
        Err(AdminOpsStoreError::new(
            "compensation proposal production source is not connected",
        ))
    }

    async fn review_compensation_proposal(
        &self,
        _proposal_id: &str,
        _request: CompensationReviewRequest,
        _context: AdminOpsRequestContext,
    ) -> Result<CompensationProposalReviewResponse, AdminOpsStoreError> {
        Err(AdminOpsStoreError::new(
            "compensation proposal production source is not connected",
        ))
    }

    async fn retry_compensation_posting(
        &self,
        _proposal_id: &str,
        _context: AdminOpsRequestContext,
    ) -> Result<CompensationPostingRetryResponse, AdminOpsStoreError> {
        Err(AdminOpsStoreError::new(
            "compensation proposal production source is not connected",
        ))
    }

    async fn create_compensation_proposal(
        &self,
        _user_id: &str,
        _request: CompensationProposalRequest,
        _context: AdminOpsRequestContext,
    ) -> Result<CompensationProposalResponse, AdminOpsStoreError> {
        Err(AdminOpsStoreError::new(
            "compensation proposal production source is not connected",
        ))
    }

    async fn create_member_action(
        &self,
        _request: MemberActionRequest,
        _context: AdminOpsRequestContext,
    ) -> Result<MemberActionResponse, AdminOpsStoreError> {
        Err(AdminOpsStoreError::new(
            "member action production source is not connected",
        ))
    }

    async fn review_member_action(
        &self,
        _action_id: &str,
        _request: MemberActionReviewRequest,
        _context: AdminOpsRequestContext,
    ) -> Result<MemberActionRecord, AdminOpsStoreError> {
        Err(AdminOpsStoreError::new(
            "member action production source is not connected",
        ))
    }

    fn kyc_service_base_url(&self) -> String {
        self.kyc_service_url.as_str().to_owned()
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAdminOpsStore {
    state: Arc<Mutex<AdminOpsState>>,
    /// Test-only: override for `ADMIN_INTERNAL_CALL_SECRET`.
    kyc_secret_override: Option<String>,
    /// Test-only: override for `KYC_SERVICE_URL`.
    kyc_url_override: Option<String>,
    compensation_known_members: Vec<String>,
    compensation_affiliate_scope_subjects: Vec<String>,
    compensation_referral_scope_unavailable: bool,
    compensation_ledger_outcome: Option<CompensationLedgerPostOutcome>,
    compensation_ledger_lookup_txn_id: Option<i64>,
}

#[derive(Debug, Default)]
struct AdminOpsState {
    next_proposal_number: u64,
    next_action_number: u64,
    compensation_proposals: Vec<CompensationProposal>,
    compensation_proposal_actors: Vec<(String, String)>,
    audit_records: Vec<CompensationAuditMetadata>,
    member_actions: Vec<MemberActionRecord>,
}

impl InMemoryAdminOpsStore {
    #[must_use]
    pub fn audit_records(&self) -> Vec<CompensationAuditMetadata> {
        self.state.lock().map_or_else(
            |poisoned| poisoned.into_inner().audit_records.clone(),
            |state| state.audit_records.clone(),
        )
    }

    /// Configures test-only KYC proxy overrides so tests can inject the secret
    /// and service URL without touching environment variables.
    #[must_use]
    pub fn with_kyc_proxy_config(
        mut self,
        secret: impl Into<String>,
        kyc_service_url: impl Into<String>,
    ) -> Self {
        self.kyc_secret_override = Some(secret.into());
        self.kyc_url_override = Some(kyc_service_url.into());
        self
    }

    #[must_use]
    pub fn with_compensation_known_members(mut self, members: Vec<String>) -> Self {
        self.compensation_known_members = members;
        self
    }

    #[must_use]
    pub fn with_compensation_affiliate_scope_subjects(mut self, subjects: Vec<String>) -> Self {
        self.compensation_affiliate_scope_subjects = subjects;
        self
    }

    #[must_use]
    pub const fn with_compensation_referral_scope_unavailable(mut self) -> Self {
        self.compensation_referral_scope_unavailable = true;
        self
    }

    #[must_use]
    pub fn with_compensation_ledger_success(mut self, txn_id: i64) -> Self {
        self.compensation_ledger_outcome = Some(CompensationLedgerPostOutcome::Posted { txn_id });
        self
    }

    #[must_use]
    pub fn with_compensation_ledger_4xx(mut self) -> Self {
        self.compensation_ledger_outcome = Some(CompensationLedgerPostOutcome::Failed {
            error_class: "ledger_4xx".to_owned(),
            message: "ledger rejected compensation".to_owned(),
        });
        self
    }

    #[must_use]
    pub fn with_compensation_ledger_5xx(mut self) -> Self {
        self.compensation_ledger_outcome = Some(CompensationLedgerPostOutcome::Unknown {
            error_class: "ledger_5xx".to_owned(),
            message: "ledger unavailable".to_owned(),
        });
        self
    }

    #[must_use]
    pub fn with_compensation_ledger_timeout(mut self) -> Self {
        self.compensation_ledger_outcome = Some(CompensationLedgerPostOutcome::Unknown {
            error_class: "ledger_timeout".to_owned(),
            message: "ledger request timed out".to_owned(),
        });
        self
    }

    #[must_use]
    pub const fn with_compensation_ledger_lookup_txn(mut self, txn_id: i64) -> Self {
        self.compensation_ledger_lookup_txn_id = Some(txn_id);
        self
    }
}

#[async_trait]
impl AdminOpsStore for InMemoryAdminOpsStore {
    async fn pending_member_cases(
        &self,
        query: PendingMemberCasesQuery,
    ) -> Result<PendingMemberCasesResponse, AdminOpsStoreError> {
        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(10).clamp(1, 50);
        let q = query.q.as_deref().map_or("", str::trim).to_lowercase();
        let status = query.status.as_deref().map_or("", str::trim);
        let restriction = query.restriction.as_deref().map_or("", str::trim);
        let mut rows = vec![
            PendingMemberCase {
                member_id: "MEM-1042".to_owned(),
                display_name: "陳柏安".to_owned(),
                restriction_reason: "withdrawals_locked".to_owned(),
                contact: PendingMemberContact {
                    email_masked: "c***@example.net".to_owned(),
                    phone_masked: "0987***112".to_owned(),
                },
                status: "restricted".to_owned(),
                kyc_status: "review_required".to_owned(),
                affiliate_id: Some("AFF-014".to_owned()),
            },
            PendingMemberCase {
                member_id: "user_123".to_owned(),
                display_name: "Member user_123".to_owned(),
                restriction_reason: "kyc_pending".to_owned(),
                contact: PendingMemberContact {
                    email_masked: "m***@example.com".to_owned(),
                    phone_masked: "0900***123".to_owned(),
                },
                status: "inactive".to_owned(),
                kyc_status: "pending".to_owned(),
                affiliate_id: Some("AFF-001".to_owned()),
            },
        ];

        rows.retain(|item| {
            (status.is_empty() || item.status == status)
                && (restriction.is_empty() || item.restriction_reason == restriction)
                && (q.is_empty()
                    || item.member_id.to_lowercase().contains(&q)
                    || item.display_name.to_lowercase().contains(&q)
                    || item.contact.email_masked.to_lowercase().contains(&q)
                    || item.contact.phone_masked.to_lowercase().contains(&q))
        });
        let total = rows.len();
        let start = page.saturating_sub(1) * page_size;
        let items = rows.into_iter().skip(start).take(page_size).collect();

        Ok(PendingMemberCasesResponse {
            items,
            page,
            page_size,
            total,
            source: "api-gateway:admin-ops".to_owned(),
        })
    }

    async fn member_summary(&self, user_id: &str) -> Result<Value, AdminOpsStoreError> {
        let summary = MemberSummaryResponse {
            member: MemberIdentitySummary {
                user_id: user_id.to_owned(),
                display_name: format!("Member {user_id}"),
                email_masked: "not_connected".to_owned(),
                status: "not_connected".to_owned(),
            },
            wallet: WalletOpsSummary {
                balance: WalletBalanceSummary {
                    available_minor_units: 0,
                    locked_minor_units: 0,
                    frozen_minor_units: 0,
                    currency: "minor_units".to_owned(),
                },
                entries_summary: WalletEntriesSummary {
                    recent_entries_count: 0,
                    last_entry_at_epoch_seconds: None,
                    pending_adjustments_count: 0,
                },
            },
            kyc_risk: KycRiskSummary {
                kyc_status: "not_connected".to_owned(),
                risk_level: "unknown".to_owned(),
                risk_score: None,
                last_reviewed_at_epoch_seconds: None,
                restrictions: Vec::new(),
            },
            affiliate: AffiliateOpsSummary {
                referral_code: None,
                successful_referrals_count: 0,
                total_rewards_minor_units: 0,
                pending_rewards_minor_units: 0,
            },
            open_cases: Vec::new(),
            audit: AuditPostureSummary {
                correlation_id: format!("admin-summary-{user_id}"),
                source: "api-gateway:admin-ops".to_owned(),
                audit_required: true,
                latest_admin_action_at_epoch_seconds: None,
            },
        };
        serde_json::to_value(summary).map_err(|error| AdminOpsStoreError::new(error.to_string()))
    }

    async fn list_members(&self, _query: MemberListQuery) -> Result<Value, AdminOpsStoreError> {
        Ok(json!({
            "members": [],
            "source": "api-gateway:admin-ops"
        }))
    }

    async fn list_role_accounts(
        &self,
        query: RoleAccountListQuery,
        _context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError> {
        Ok(json!({
            "accounts": [{
                "user_id": "MAN-001",
                "email": "manager@example.com",
                "phone": "0912345001",
                "role": query.role,
                "affiliate_id": null,
                "disabled": false
            }],
            "source": "api-gateway:admin-ops"
        }))
    }

    async fn create_member(
        &self,
        request: Value,
        _context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError> {
        Ok(request)
    }

    async fn update_member(
        &self,
        _user_id: &str,
        request: Value,
        _context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError> {
        Ok(request)
    }

    async fn delete_member(
        &self,
        user_id: &str,
        _context: AdminOpsRequestContext,
    ) -> Result<Value, AdminOpsStoreError> {
        Ok(json!({ "user_id": user_id }))
    }

    async fn list_member_actions(
        &self,
        query: MemberActionListQuery,
    ) -> Result<MemberActionListResponse, AdminOpsStoreError> {
        let state = self
            .state
            .lock()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let actions = state
            .member_actions
            .iter()
            .filter(|action| {
                query
                    .status
                    .as_deref()
                    .is_none_or(|status| action.status == status)
                    && query
                        .target_user_id
                        .as_deref()
                        .is_none_or(|user_id| action.target_user_id == user_id)
            })
            .cloned()
            .collect();
        Ok(MemberActionListResponse {
            actions,
            source: "api-gateway:admin-ops".to_owned(),
        })
    }

    async fn list_compensation_proposals(
        &self,
        query: CompensationProposalListQuery,
    ) -> Result<CompensationProposalListResponse, AdminOpsStoreError> {
        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(25).clamp(1, 100);
        let state = self
            .state
            .lock()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let mut proposals = state
            .compensation_proposals
            .iter()
            .filter(|proposal| {
                compensation_status_matches(proposal.status, query.status.as_deref())
            })
            .filter(|proposal| {
                query
                    .proposer_kind
                    .as_deref()
                    .map(str::trim)
                    .filter(|proposer_kind| !proposer_kind.is_empty())
                    .is_none_or(|proposer_kind| proposal.proposer_kind == proposer_kind)
            })
            .cloned()
            .collect::<Vec<_>>();
        proposals.sort_by(|left, right| right.proposal_id.cmp(&left.proposal_id));
        let total = proposals.len();
        let start = page.saturating_sub(1) * page_size;
        let proposals = proposals.into_iter().skip(start).take(page_size).collect();
        Ok(CompensationProposalListResponse {
            proposals,
            page,
            page_size,
            total,
            source: "api-gateway:compensation".to_owned(),
        })
    }

    async fn compensation_proposal_detail(
        &self,
        proposal_id: &str,
    ) -> Result<CompensationProposalDetailResponse, AdminOpsStoreError> {
        let state = self
            .state
            .lock()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let proposal = state
            .compensation_proposals
            .iter()
            .find(|proposal| proposal.proposal_id == proposal_id)
            .cloned()
            .ok_or_else(|| AdminOpsStoreError::new("compensation proposal not found"))?;
        let recent_audit_events = state
            .audit_records
            .iter()
            .filter(|audit| {
                audit.subject_user_id == proposal.user_id && audit.ticket_id == proposal.ticket_id
            })
            .map(|audit| CompensationAuditEventSummary {
                action: audit.action.clone(),
                actor_user_id: "admin-ops-test".to_owned(),
                actor_role: audit.proposer_kind.clone(),
                correlation_id: audit.correlation_id.clone(),
                metadata: json!({
                    "proposer_kind": audit.proposer_kind,
                    "ticket_id": audit.ticket_id,
                }),
                created_at: "test-now".to_owned(),
            })
            .collect();
        Ok(CompensationProposalDetailResponse {
            approvals: proposal.approvals.clone(),
            posting_attempt_count: proposal.posting_attempt_count,
            last_posting_error: proposal.last_posting_error.clone(),
            proposal,
            recent_audit_events,
        })
    }

    async fn review_compensation_proposal(
        &self,
        proposal_id: &str,
        request: CompensationReviewRequest,
        context: AdminOpsRequestContext,
    ) -> Result<CompensationProposalReviewResponse, AdminOpsStoreError> {
        if !matches!(context.actor_role.as_str(), "manager" | "admin") {
            return Err(AdminOpsStoreError::new("compensation review forbidden"));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let proposer_user_id = state
            .compensation_proposal_actors
            .iter()
            .find_map(|(stored_proposal_id, actor_user_id)| {
                (stored_proposal_id == proposal_id).then(|| actor_user_id.clone())
            })
            .ok_or_else(|| AdminOpsStoreError::new("compensation proposal not found"))?;
        if proposer_user_id == context.actor_user_id {
            return Err(AdminOpsStoreError::new("compensation self review"));
        }
        let proposal = state
            .compensation_proposals
            .iter_mut()
            .find(|proposal| proposal.proposal_id == proposal_id)
            .ok_or_else(|| AdminOpsStoreError::new("compensation proposal not found"))?;
        if proposal.status != CompensationProposalStatus::Proposed {
            if proposal
                .approvals
                .iter()
                .any(|approval| approval.approver_user_id == context.actor_user_id)
            {
                return Ok(CompensationProposalReviewResponse {
                    proposal: proposal.clone(),
                    review: CompensationReviewSummary {
                        reviewer_user_id: context.actor_user_id,
                        reviewer_role: context.actor_role,
                        decision: request.decision,
                        note: request.note,
                    },
                    replayed: true,
                });
            }
            return Err(AdminOpsStoreError::new("compensation proposal not pending"));
        }
        proposal.status = if request.decision == "approved" {
            CompensationProposalStatus::Approved
        } else {
            CompensationProposalStatus::Rejected
        };
        if request.decision == "approved" {
            proposal.approved_at = Some("now".to_owned());
        }
        proposal.approvals.push(CompensationApprovalRequest {
            approver_user_id: context.actor_user_id.clone(),
            role: context.actor_role.clone(),
        });
        let proposal = proposal.clone();
        state.audit_records.push(CompensationAuditMetadata {
            action: if request.decision == "approved" {
                "compensation_proposal_approved".to_owned()
            } else {
                "compensation_proposal_rejected".to_owned()
            },
            subject_user_id: proposal.user_id.clone(),
            ticket_id: proposal.ticket_id.clone(),
            correlation_id: format!("{}:review", proposal.proposal_id),
            approval_roles: vec![context.actor_role.clone()],
            proposer_kind: proposal.proposer_kind.clone(),
            ledger_transaction_posted: false,
        });
        let mut proposal = proposal;
        if request.decision == "approved" {
            let ledger_idempotency_key = format!("cs-cmp-{proposal_id}");
            proposal.status = CompensationProposalStatus::Posting;
            proposal.ledger_idempotency_key = Some(ledger_idempotency_key.clone());
            proposal.posting_attempt_count += 1;
            state.audit_records.push(CompensationAuditMetadata {
                action: "compensation_posting_attempt_started".to_owned(),
                subject_user_id: proposal.user_id.clone(),
                ticket_id: proposal.ticket_id.clone(),
                correlation_id: format!("{}:posting", proposal.proposal_id),
                approval_roles: vec![context.actor_role.clone()],
                proposer_kind: proposal.proposer_kind.clone(),
                ledger_transaction_posted: false,
            });
            match self
                .compensation_ledger_outcome
                .clone()
                .unwrap_or(CompensationLedgerPostOutcome::Posted { txn_id: 9001 })
            {
                CompensationLedgerPostOutcome::Posted { txn_id } => {
                    proposal.status = CompensationProposalStatus::Posted;
                    proposal.ledger_transaction_posted = true;
                    proposal.ledger_transaction_id = Some(txn_id);
                    proposal.last_posting_error = None;
                    state.audit_records.push(CompensationAuditMetadata {
                        action: "compensation_ledger_transaction_posted".to_owned(),
                        subject_user_id: proposal.user_id.clone(),
                        ticket_id: proposal.ticket_id.clone(),
                        correlation_id: format!("{}:posted", proposal.proposal_id),
                        approval_roles: vec![context.actor_role.clone()],
                        proposer_kind: proposal.proposer_kind.clone(),
                        ledger_transaction_posted: true,
                    });
                }
                CompensationLedgerPostOutcome::Failed {
                    error_class,
                    message,
                } => {
                    proposal.status = CompensationProposalStatus::PostingFailed;
                    proposal.last_posting_error = Some(format!("{error_class}: {message}"));
                    state.audit_records.push(CompensationAuditMetadata {
                        action: "compensation_posting_attempt_failed".to_owned(),
                        subject_user_id: proposal.user_id.clone(),
                        ticket_id: proposal.ticket_id.clone(),
                        correlation_id: format!("{}:posting-failed", proposal.proposal_id),
                        approval_roles: vec![context.actor_role.clone()],
                        proposer_kind: proposal.proposer_kind.clone(),
                        ledger_transaction_posted: false,
                    });
                }
                CompensationLedgerPostOutcome::Unknown {
                    error_class,
                    message,
                } => {
                    proposal.status = CompensationProposalStatus::PostingUnknown;
                    proposal.last_posting_error = Some(format!("{error_class}: {message}"));
                    state.audit_records.push(CompensationAuditMetadata {
                        action: "compensation_posting_attempt_failed".to_owned(),
                        subject_user_id: proposal.user_id.clone(),
                        ticket_id: proposal.ticket_id.clone(),
                        correlation_id: format!("{}:posting-unknown", proposal.proposal_id),
                        approval_roles: vec![context.actor_role.clone()],
                        proposer_kind: proposal.proposer_kind.clone(),
                        ledger_transaction_posted: false,
                    });
                }
            }
            if let Some(stored) = state
                .compensation_proposals
                .iter_mut()
                .find(|stored| stored.proposal_id == proposal.proposal_id)
            {
                *stored = proposal.clone();
            }
        }
        Ok(CompensationProposalReviewResponse {
            proposal,
            review: CompensationReviewSummary {
                reviewer_user_id: context.actor_user_id,
                reviewer_role: context.actor_role,
                decision: request.decision,
                note: request.note,
            },
            replayed: false,
        })
    }

    async fn retry_compensation_posting(
        &self,
        proposal_id: &str,
        context: AdminOpsRequestContext,
    ) -> Result<CompensationPostingRetryResponse, AdminOpsStoreError> {
        if context.actor_role != "admin" {
            return Err(AdminOpsStoreError::new("compensation retry forbidden"));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let proposal_index = state
            .compensation_proposals
            .iter()
            .position(|proposal| proposal.proposal_id == proposal_id)
            .ok_or_else(|| AdminOpsStoreError::new("compensation proposal not found"))?;
        let proposal = state.compensation_proposals[proposal_index].clone();
        if !matches!(
            proposal.status,
            CompensationProposalStatus::Posting
                | CompensationProposalStatus::PostingFailed
                | CompensationProposalStatus::PostingUnknown
                | CompensationProposalStatus::Posted
        ) {
            return Err(AdminOpsStoreError::new("compensation retry not available"));
        }
        let Some(_ledger_idempotency_key) = proposal.ledger_idempotency_key.clone() else {
            return Err(AdminOpsStoreError::new(
                "compensation ledger idempotency key missing",
            ));
        };
        state.audit_records.push(CompensationAuditMetadata {
            action: "compensation_posting_manual_retry".to_owned(),
            subject_user_id: proposal.user_id.clone(),
            ticket_id: proposal.ticket_id.clone(),
            correlation_id: format!("{}:manual-retry", proposal.proposal_id),
            approval_roles: vec![context.actor_role.clone()],
            proposer_kind: proposal.proposer_kind.clone(),
            ledger_transaction_posted: proposal.ledger_transaction_posted,
        });
        if let Some(txn_id) = self
            .compensation_ledger_lookup_txn_id
            .or(proposal.ledger_transaction_id)
        {
            let proposal = {
                let proposal = &mut state.compensation_proposals[proposal_index];
                proposal.status = CompensationProposalStatus::Posted;
                proposal.ledger_transaction_posted = true;
                proposal.ledger_transaction_id = Some(txn_id);
                proposal.last_posting_error = None;
                proposal.clone()
            };
            state.audit_records.push(CompensationAuditMetadata {
                action: "compensation_reconcile_marked_posted".to_owned(),
                subject_user_id: proposal.user_id.clone(),
                ticket_id: proposal.ticket_id.clone(),
                correlation_id: format!("{}:reconcile", proposal.proposal_id),
                approval_roles: vec![context.actor_role],
                proposer_kind: proposal.proposer_kind.clone(),
                ledger_transaction_posted: true,
            });
            return Ok(CompensationPostingRetryResponse {
                proposal,
                replayed: true,
            });
        }
        state.audit_records.push(CompensationAuditMetadata {
            action: "compensation_posting_attempt_started".to_owned(),
            subject_user_id: proposal.user_id.clone(),
            ticket_id: proposal.ticket_id.clone(),
            correlation_id: format!("{}:retry-posting", proposal.proposal_id),
            approval_roles: vec![context.actor_role.clone()],
            proposer_kind: proposal.proposer_kind.clone(),
            ledger_transaction_posted: false,
        });
        let outcome = self
            .compensation_ledger_outcome
            .clone()
            .unwrap_or(CompensationLedgerPostOutcome::Posted { txn_id: 9001 });
        let proposal = {
            let proposal = &mut state.compensation_proposals[proposal_index];
            proposal.status = CompensationProposalStatus::Posting;
            proposal.posting_attempt_count += 1;
            match outcome {
                CompensationLedgerPostOutcome::Posted { txn_id } => {
                    proposal.status = CompensationProposalStatus::Posted;
                    proposal.ledger_transaction_posted = true;
                    proposal.ledger_transaction_id = Some(txn_id);
                    proposal.last_posting_error = None;
                }
                CompensationLedgerPostOutcome::Failed {
                    error_class,
                    message,
                } => {
                    proposal.status = CompensationProposalStatus::PostingFailed;
                    proposal.last_posting_error = Some(format!("{error_class}: {message}"));
                }
                CompensationLedgerPostOutcome::Unknown {
                    error_class,
                    message,
                } => {
                    proposal.status = CompensationProposalStatus::PostingUnknown;
                    proposal.last_posting_error = Some(format!("{error_class}: {message}"));
                }
            }
            proposal.clone()
        };
        Ok(CompensationPostingRetryResponse {
            proposal,
            replayed: false,
        })
    }

    async fn create_compensation_proposal(
        &self,
        user_id: &str,
        request: CompensationProposalRequest,
        mut context: AdminOpsRequestContext,
    ) -> Result<CompensationProposalResponse, AdminOpsStoreError> {
        let proposer_kind = proposer_kind_from_role(&context.actor_role).to_owned();
        if !self.compensation_known_members.is_empty()
            && !self
                .compensation_known_members
                .iter()
                .any(|known_user_id| known_user_id == user_id)
        {
            if proposer_kind == "affiliate" {
                return Err(AdminOpsStoreError::new("compensation subject out of scope"));
            }
            return Err(AdminOpsStoreError::from_upstream(
                StatusCode::NOT_FOUND.as_u16(),
                Some("ERROR_CODE_MEMBER_NOT_FOUND".to_owned()),
                "member not found",
            ));
        }
        if proposer_kind == "affiliate" {
            let amount_minor_units = request
                .amount_minor_units
                .ok_or_else(|| AdminOpsStoreError::new("validated amount is missing"))?;
            if amount_minor_units > affiliate_compensation_max_minor_units() {
                return Err(AdminOpsStoreError::new(
                    "compensation amount over affiliate limit",
                ));
            }
            if self.compensation_referral_scope_unavailable {
                return Err(AdminOpsStoreError::new("referral scope unavailable"));
            }
            if !self
                .compensation_affiliate_scope_subjects
                .iter()
                .any(|subject| subject == user_id)
            {
                return Err(AdminOpsStoreError::new("compensation subject out of scope"));
            }
            context.compensation_referral_relationship = Some("direct_referral".to_owned());
        }
        let mut state = self
            .state
            .lock()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        state.next_proposal_number += 1;
        let ticket_id = request
            .ticket_id
            .ok_or_else(|| AdminOpsStoreError::new("validated ticket id is missing"))?;
        let reason = request
            .reason
            .ok_or_else(|| AdminOpsStoreError::new("validated reason is missing"))?;
        let amount_minor_units = request
            .amount_minor_units
            .ok_or_else(|| AdminOpsStoreError::new("validated amount is missing"))?;
        let correlation_id = format!(
            "admin-compensation-{ticket_id}-{}",
            state.next_proposal_number
        );
        let audit = CompensationAuditMetadata {
            action: "compensation_proposal_created".to_owned(),
            subject_user_id: user_id.to_owned(),
            ticket_id: ticket_id.clone(),
            correlation_id,
            approval_roles: Vec::new(),
            proposer_kind: proposer_kind.clone(),
            ledger_transaction_posted: false,
        };
        state.audit_records.push(audit.clone());

        let proposal = CompensationProposal {
            proposal_id: format!("cmp-{number:06}", number = state.next_proposal_number),
            user_id: user_id.to_owned(),
            ticket_id,
            reason,
            amount_minor_units,
            status: CompensationProposalStatus::Proposed,
            proposer_kind,
            proposer_user_id: context.actor_user_id.clone(),
            approvals: Vec::new(),
            ledger_transaction_posted: false,
            ledger_idempotency_key: None,
            ledger_transaction_id: None,
            posting_attempt_count: 0,
            last_posting_error: None,
            approved_at: None,
        };
        state
            .compensation_proposal_actors
            .push((proposal.proposal_id.clone(), context.actor_user_id.clone()));
        state.compensation_proposals.push(proposal.clone());
        Ok(CompensationProposalResponse { proposal, audit })
    }

    async fn create_member_action(
        &self,
        request: MemberActionRequest,
        context: AdminOpsRequestContext,
    ) -> Result<MemberActionResponse, AdminOpsStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        state.next_action_number += 1;
        let requires_second_approval = request.action_type != "add_note";
        let status = if requires_second_approval {
            "pending_second_approval"
        } else {
            "submitted"
        };
        let correlation_id = request
            .correlation_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map_or_else(
                || format!("admin-member-action-{}", state.next_action_number),
                str::to_owned,
            );
        let side_effects = if requires_second_approval {
            Vec::new()
        } else {
            action_side_effects(&request.action_type)
        };
        let record = MemberActionRecord {
            action_id: format!("mem-act-{number:06}", number = state.next_action_number),
            target_user_id: request.target_user_id,
            action_type: request.action_type,
            ticket_id: request.ticket_id,
            reason: request.reason,
            status: status.to_owned(),
            requires_second_approval,
            actor_user_id: context.actor_user_id,
            actor_role: context.actor_role,
            correlation_id,
            review: None,
            side_effects,
            created_at: "now".to_owned(),
        };
        state.member_actions.push(record.clone());
        Ok(member_action_response_from_record(&record))
    }

    async fn review_member_action(
        &self,
        action_id: &str,
        request: MemberActionReviewRequest,
        context: AdminOpsRequestContext,
    ) -> Result<MemberActionRecord, AdminOpsStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        let action = state
            .member_actions
            .iter_mut()
            .find(|action| action.action_id == action_id)
            .ok_or_else(|| AdminOpsStoreError::new("member action not found"))?;
        if action.actor_user_id == context.actor_user_id {
            return Err(AdminOpsStoreError::new(
                "member action self approval is not allowed",
            ));
        }
        if action.status != "pending_second_approval" {
            return Err(AdminOpsStoreError::new(
                "member action cannot be reviewed in its current status",
            ));
        }
        action.status = if request.decision == "approve" {
            "approved".to_owned()
        } else {
            "rejected".to_owned()
        };
        action.side_effects = if request.decision == "approve" {
            action_side_effects(&action.action_type)
        } else {
            Vec::new()
        };
        action.review = Some(MemberActionReviewSummary {
            reviewer_user_id: context.actor_user_id,
            reviewer_role: context.actor_role,
            decision: request.decision,
            reason: request.reason,
            reviewed_at: "now".to_owned(),
        });
        Ok(action.clone())
    }

    fn kyc_proxy_secret(&self) -> Option<String> {
        self.kyc_secret_override
            .clone()
            .or_else(|| env::var("ADMIN_INTERNAL_CALL_SECRET").ok())
    }

    fn kyc_service_base_url(&self) -> String {
        self.kyc_url_override
            .clone()
            .unwrap_or_else(crate::kyc_service_base_url_from_env)
    }
}
