use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, patch, post},
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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

    async fn list_members(
        &self,
        query: MemberListQuery,
    ) -> Result<Value, AdminOpsStoreError>;

    /// Builds the internal admin member summary facade.
    ///
    /// # Errors
    ///
    /// Returns an error when the backing store cannot be read.
    async fn member_summary(
        &self,
        user_id: &str,
    ) -> Result<Value, AdminOpsStoreError>;

    async fn create_member(&self, request: Value) -> Result<Value, AdminOpsStoreError>;

    async fn update_member(
        &self,
        user_id: &str,
        request: Value,
    ) -> Result<Value, AdminOpsStoreError>;

    async fn delete_member(&self, user_id: &str) -> Result<Value, AdminOpsStoreError>;

    /// Persists a compensation proposal without posting a ledger transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when the proposal or audit record cannot be persisted.
    async fn create_compensation_proposal(
        &self,
        user_id: &str,
        request: CompensationProposalRequest,
    ) -> Result<CompensationProposalResponse, AdminOpsStoreError>;
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CompensationProposalRequest {
    pub ticket_id: Option<String>,
    pub reason: Option<String>,
    pub amount_minor_units: Option<i64>,
    pub approvals: Option<Vec<CompensationApprovalRequest>>,
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

async fn create_compensation_proposal<S>(
    State(store): State<S>,
    Path(user_id): Path<String>,
    Json(request): Json<CompensationProposalRequest>,
) -> Result<Json<CompensationProposalResponse>, (StatusCode, Json<ErrorResponse>)>
where
    S: AdminOpsStore,
{
    validate_compensation_proposal(&request)?;
    store
        .create_compensation_proposal(&user_id, request)
        .await
        .map(Json)
        .map_err(|_| internal_error())
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

    async fn list_members(
        &self,
        query: MemberListQuery,
    ) -> Result<Value, AdminOpsStoreError> {
        self.user_service_json(self.client.get(self.list_members_url(&query)?))
            .await
    }

    async fn member_summary(
        &self,
        user_id: &str,
    ) -> Result<Value, AdminOpsStoreError> {
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
        self.user_service_json(self.client.patch(self.user_members_url(&path)?).json(&request))
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
    ) -> Result<CompensationProposalResponse, AdminOpsStoreError> {
        Err(AdminOpsStoreError::new(
            "compensation proposal production source is not connected",
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

    async fn member_summary(
        &self,
        user_id: &str,
    ) -> Result<Value, AdminOpsStoreError> {
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

    async fn list_members(
        &self,
        _query: MemberListQuery,
    ) -> Result<Value, AdminOpsStoreError> {
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
}
