use std::{
    collections::HashSet,
    env,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
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
    /// Reads `KYC_SERVICE_URL` from the process environment by default.
    fn kyc_service_base_url(&self) -> String {
        env::var("KYC_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8085".to_owned())
    }
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

fn member_summary_upstream_error_code(code: &str) -> Option<&'static str> {
    match code {
        "ERROR_CODE_MEMBER_NOT_FOUND" => Some("ERROR_CODE_MEMBER_NOT_FOUND"),
        "ERROR_CODE_INVALID_MEMBER" => Some("ERROR_CODE_INVALID_MEMBER"),
        "ERROR_CODE_MEMBER_EXISTS" => Some("ERROR_CODE_MEMBER_EXISTS"),
        "ERROR_CODE_INTERNAL" => Some("ERROR_CODE_INTERNAL"),
        _ => None,
    }
}

fn member_summary_status_error_code(status: StatusCode) -> &'static str {
    match status {
        StatusCode::NOT_FOUND => "ERROR_CODE_MEMBER_NOT_FOUND",
        StatusCode::BAD_REQUEST => "ERROR_CODE_INVALID_MEMBER",
        StatusCode::CONFLICT => "ERROR_CODE_MEMBER_EXISTS",
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

#[derive(Debug, Clone)]
pub struct HttpAdminOpsStore {
    client: reqwest::Client,
    kyc_service_url: Url,
    user_service_url: Url,
    wallet_ledger_url: Option<Url>,
    risk_service_url: Option<Url>,
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
        let kyc_service_url =
            env::var("KYC_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8085".to_owned());
        let user_service_url =
            env::var("USER_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8084".to_owned());
        let store = Self::with_side_effect_services(
            kyc_service_url,
            user_service_url,
            env::var("WALLET_LEDGER_BASE_URL").ok(),
            env::var("RISK_SERVICE_URL").ok(),
        )?;
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
            .update_member(&record.target_user_id, request)
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

    async fn list_member_actions(
        &self,
        _query: MemberActionListQuery,
    ) -> Result<MemberActionListResponse, AdminOpsStoreError> {
        Err(AdminOpsStoreError::new(
            "member action production source is not connected",
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
}

#[derive(Debug, Default)]
struct AdminOpsState {
    next_proposal_number: u64,
    next_action_number: u64,
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
        self.kyc_url_override.clone().unwrap_or_else(|| {
            env::var("KYC_SERVICE_URL").unwrap_or_else(|_| "http://127.0.0.1:8085".to_owned())
        })
    }
}
