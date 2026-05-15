use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, patch, post},
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};

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

    /// Builds the internal admin member summary facade.
    ///
    /// # Errors
    ///
    /// Returns an error when the backing store cannot be read.
    async fn member_summary(&self, user_id: &str) -> Result<Value, AdminOpsStoreError>;

    async fn create_member(&self, request: Value) -> Result<Value, AdminOpsStoreError>;

    async fn update_member(
        &self,
        user_id: &str,
        request: Value,
    ) -> Result<Value, AdminOpsStoreError>;

    async fn delete_member(&self, user_id: &str) -> Result<Value, AdminOpsStoreError>;

    async fn create_member_action(
        &self,
        request: MemberActionRequest,
        context: AdminOpsRequestContext,
    ) -> Result<MemberActionResponse, AdminOpsStoreError>;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminOpsRequestContext {
    pub actor_user_id: String,
    pub actor_role: String,
    pub request_id: Option<String>,
    pub idempotency_key: Option<String>,
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
pub struct MemberListQuery {
    pub q: Option<String>,
    pub affiliate_id: Option<String>,
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
    pub approvals: Option<Vec<CompensationApprovalRequest>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct MemberActionRequest {
    pub target_user_id: String,
    pub action_type: String,
    pub ticket_id: String,
    pub reason: String,
    pub correlation_id: Option<String>,
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
pub struct CompensationProposalResponse {
    pub proposal: CompensationProposal,
    pub audit: CompensationAuditMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompensationProposal {
    pub proposal_id: String,
    pub user_id: String,
    pub ticket_id: String,
    pub reason: String,
    pub amount_minor_units: i64,
    pub status: CompensationProposalStatus,
    pub approvals: Vec<CompensationApprovalRequest>,
    pub ledger_transaction_posted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompensationProposalStatus {
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
    pub ledger_transaction_posted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminOpsStoreError {
    message: String,
}

impl AdminOpsStoreError {
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

pub fn internal_admin_router<S>(store: S) -> Router
where
    S: AdminOpsStore + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/internal/admin/members", get(list_members::<S>))
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
            post(create_member_action::<S>),
        )
        .route(
            "/api/internal/admin/members/:user_id/compensation-proposals",
            post(create_compensation_proposal::<S>),
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
        .map_err(|_| internal_error())
}

async fn create_member<S>(
    State(store): State<S>,
    Json(request): Json<Value>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    store
        .create_member(request)
        .await
        .map(|body| (StatusCode::CREATED, Json(body)))
        .map_err(|_| internal_error())
}

async fn update_member<S>(
    State(store): State<S>,
    Path(user_id): Path<String>,
    Json(request): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    store
        .update_member(&user_id, request)
        .await
        .map(Json)
        .map_err(|_| internal_error())
}

async fn delete_member<S>(
    State(store): State<S>,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    store
        .delete_member(&user_id)
        .await
        .map(Json)
        .map_err(|_| internal_error())
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
        .map_err(|_| internal_error())
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
        || request
            .approvals
            .as_ref()
            .is_none_or(|approvals| approvals.len() != 2)
    {
        return Err(invalid_proposal());
    }

    let mut approver_ids = HashSet::new();
    let approvals = request.approvals.as_ref().ok_or_else(invalid_proposal)?;
    for approval in approvals {
        if approval.approver_user_id.trim().is_empty()
            || approval.role.trim().is_empty()
            || !approver_ids.insert(approval.approver_user_id.clone())
        {
            return Err(invalid_proposal());
        }
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

#[derive(Debug, Clone)]
pub struct HttpAdminOpsStore {
    client: reqwest::Client,
    kyc_service_url: Url,
    user_service_url: Url,
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
        })
    }

    /// Loads upstream service URLs from the process environment.
    ///
    /// # Errors
    ///
    /// Returns an error when the configured KYC service URL is invalid.
    pub fn from_env() -> Result<Self, AdminOpsStoreError> {
        let kyc_service_url =
            std::env::var("KYC_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8085".to_owned());
        let user_service_url = std::env::var("USER_SERVICE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8084".to_owned());
        Self::new(kyc_service_url, user_service_url)
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

    async fn user_service_json(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<Value, AdminOpsStoreError> {
        request
            .send()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .error_for_status()
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))?
            .json::<Value>()
            .await
            .map_err(|error| AdminOpsStoreError::new(error.to_string()))
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

    async fn member_summary(&self, user_id: &str) -> Result<Value, AdminOpsStoreError> {
        self.upstream.member_summary(user_id).await
    }

    async fn create_member(&self, request: Value) -> Result<Value, AdminOpsStoreError> {
        self.upstream.create_member(request).await
    }

    async fn update_member(
        &self,
        user_id: &str,
        request: Value,
    ) -> Result<Value, AdminOpsStoreError> {
        self.upstream.update_member(user_id, request).await
    }

    async fn delete_member(&self, user_id: &str) -> Result<Value, AdminOpsStoreError> {
        self.upstream.delete_member(user_id).await
    }

    async fn create_member_action(
        &self,
        request: MemberActionRequest,
        context: AdminOpsRequestContext,
    ) -> Result<MemberActionResponse, AdminOpsStoreError> {
        self.compensation
            .create_member_action(request, context)
            .await
    }

    async fn create_compensation_proposal(
        &self,
        user_id: &str,
        request: CompensationProposalRequest,
        context: AdminOpsRequestContext,
    ) -> Result<CompensationProposalResponse, AdminOpsStoreError> {
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

    async fn find_compensation_by_idempotency_key(
        &self,
        idempotency_key: &str,
        request_hash: &str,
    ) -> Result<Option<CompensationProposalResponse>, AdminOpsStoreError> {
        let Some(row) = sqlx::query(
            r"
            SELECT id, proposal_id, subject_user_id, ticket_id, reason, amount_minor_units,
                   status, ledger_transaction_posted, correlation_id, request_hash
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
        let approvals = request
            .approvals
            .ok_or_else(|| AdminOpsStoreError::new("validated approvals are missing"))?;
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
                ledger_transaction_posted, correlation_id, actor_user_id, actor_role,
                idempotency_key, request_hash
            )
            VALUES ($1, $2, $3, $4, $5, 'approved_ready_for_ledger', FALSE, $6, $7, $8, $9, $10)
            RETURNING id, proposal_id
            ",
        )
        .bind(correlation_seed)
        .bind(user_id)
        .bind(&ticket_id)
        .bind(&reason)
        .bind(amount_minor_units)
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

        let approval_roles = approvals
            .iter()
            .map(|approval| approval.role.clone())
            .collect::<Vec<_>>();
        insert_compensation_approvals(&mut tx, row_id, &approvals).await?;
        insert_compensation_audit_event(
            &mut tx,
            row_id,
            user_id,
            &ticket_id,
            &correlation_id,
            &approval_roles,
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
                approvals,
                correlation_id,
                approval_roles,
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
}

async fn insert_compensation_approvals(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    proposal_row_id: i64,
    approvals: &[CompensationApprovalRequest],
) -> Result<(), AdminOpsStoreError> {
    for (index, approval) in approvals.iter().enumerate() {
        let ordinal =
            i16::try_from(index + 1).map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
        sqlx::query(
            r"
            INSERT INTO admin_compensation_proposal_approvals (
                proposal_id, approver_user_id, role, ordinal
            )
            VALUES ($1, $2, $3, $4)
            ",
        )
        .bind(proposal_row_id)
        .bind(&approval.approver_user_id)
        .bind(&approval.role)
        .bind(ordinal)
        .execute(&mut **tx)
        .await
        .map_err(|error| AdminOpsStoreError::new(error.to_string()))?;
    }
    Ok(())
}

async fn insert_compensation_audit_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    proposal_row_id: i64,
    user_id: &str,
    ticket_id: &str,
    correlation_id: &str,
    approval_roles: &[String],
    context: &AdminOpsRequestContext,
) -> Result<(), AdminOpsStoreError> {
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
    .bind(json!({ "approval_roles": approval_roles }))
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
            status: CompensationProposalStatus::ApprovedReadyForLedger,
            approvals: parts.approvals,
            ledger_transaction_posted: false,
        },
        audit: CompensationAuditMetadata {
            action: "compensation_proposal_created".to_owned(),
            subject_user_id: parts.user_id,
            ticket_id: parts.ticket_id,
            correlation_id: parts.correlation_id,
            approval_roles: parts.approval_roles,
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
        ORDER BY ordinal
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
            status: if status == "pending_approval" {
                CompensationProposalStatus::PendingApproval
            } else {
                CompensationProposalStatus::ApprovedReadyForLedger
            },
            approvals,
            ledger_transaction_posted: row
                .try_get("ledger_transaction_posted")
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
            ledger_transaction_posted: row
                .try_get("ledger_transaction_posted")
                .map_err(|error| AdminOpsStoreError::new(error.to_string()))?,
        },
    })
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
        Ok(response)
    }

    async fn list_members(&self, query: MemberListQuery) -> Result<Value, AdminOpsStoreError> {
        self.user_service_json(self.client.get(self.list_members_url(&query)?))
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

    async fn create_member(&self, request: Value) -> Result<Value, AdminOpsStoreError> {
        self.user_service_json(
            self.client
                .post(self.user_members_url("/v1/internal/admin/members")?)
                .json(&request),
        )
        .await
    }

    async fn update_member(
        &self,
        user_id: &str,
        request: Value,
    ) -> Result<Value, AdminOpsStoreError> {
        let path = format!(
            "/v1/internal/admin/members/{}",
            url_encode_path_segment(user_id)
        );
        self.user_service_json(
            self.client
                .patch(self.user_members_url(&path)?)
                .json(&request),
        )
        .await
    }

    async fn delete_member(&self, user_id: &str) -> Result<Value, AdminOpsStoreError> {
        let path = format!(
            "/v1/internal/admin/members/{}",
            url_encode_path_segment(user_id)
        );
        self.user_service_json(self.client.delete(self.user_members_url(&path)?))
            .await
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
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAdminOpsStore {
    state: Arc<Mutex<AdminOpsState>>,
}

#[derive(Debug, Default)]
struct AdminOpsState {
    next_proposal_number: u64,
    next_action_number: u64,
    audit_records: Vec<CompensationAuditMetadata>,
}

impl InMemoryAdminOpsStore {
    #[must_use]
    pub fn audit_records(&self) -> Vec<CompensationAuditMetadata> {
        self.state.lock().map_or_else(
            |poisoned| poisoned.into_inner().audit_records.clone(),
            |state| state.audit_records.clone(),
        )
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
                status: "placeholder".to_owned(),
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

    async fn create_member(&self, request: Value) -> Result<Value, AdminOpsStoreError> {
        Ok(request)
    }

    async fn update_member(
        &self,
        _user_id: &str,
        request: Value,
    ) -> Result<Value, AdminOpsStoreError> {
        Ok(request)
    }

    async fn delete_member(&self, user_id: &str) -> Result<Value, AdminOpsStoreError> {
        Ok(json!({ "user_id": user_id }))
    }

    async fn create_compensation_proposal(
        &self,
        user_id: &str,
        request: CompensationProposalRequest,
        _context: AdminOpsRequestContext,
    ) -> Result<CompensationProposalResponse, AdminOpsStoreError> {
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
        let approvals = request
            .approvals
            .ok_or_else(|| AdminOpsStoreError::new("validated approvals are missing"))?;
        let correlation_id = format!(
            "admin-compensation-{ticket_id}-{}",
            state.next_proposal_number
        );
        let audit = CompensationAuditMetadata {
            action: "compensation_proposal_created".to_owned(),
            subject_user_id: user_id.to_owned(),
            ticket_id: ticket_id.clone(),
            correlation_id,
            approval_roles: approvals
                .iter()
                .map(|approval| approval.role.clone())
                .collect(),
            ledger_transaction_posted: false,
        };
        state.audit_records.push(audit.clone());

        Ok(CompensationProposalResponse {
            proposal: CompensationProposal {
                proposal_id: format!("cmp-{number:06}", number = state.next_proposal_number),
                user_id: user_id.to_owned(),
                ticket_id,
                reason,
                amount_minor_units,
                status: CompensationProposalStatus::ApprovedReadyForLedger,
                approvals,
                ledger_transaction_posted: false,
            },
            audit,
        })
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
        Ok(member_action_response(
            format!("mem-act-{number:06}", number = state.next_action_number),
            request,
            status,
            requires_second_approval,
            context,
            correlation_id,
        ))
    }
}
