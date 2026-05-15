pub const SERVICE_NAME: &str = "referral-service";
pub const DEFAULT_PORT: u16 = 8092;

use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use sdk::{
    ledger::{LedgerAmount, LedgerEntryDraft, LedgerTransactionDraft},
    referral::code::ReferralCodeError,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use thiserror::Error;

const QUALIFICATION_DEPOSIT_MINOR_UNITS: i64 = 50_000;
const QUALIFICATION_MIN_KYC_TIER: u8 = 1;
const QUALIFICATION_MIN_TRADE_COUNT: u32 = 5;
const QUALIFICATION_WINDOW_DAYS: i64 = 30;
const QUALIFICATION_BONUS_MINOR_UNITS: i64 = 10_000;
const REFERRED_SIGNUP_BONUS_MINOR_UNITS: i64 = 5_000;
const TRADE_COMMISSION_RATE_NUMERATOR: i64 = 10;
const TRADE_COMMISSION_RATE_DENOMINATOR: i64 = 100;
const TRADE_COMMISSION_WINDOW_DAYS: i64 = 365;
const MONTHLY_SUCCESSFUL_REFERRAL_CAP: u32 = 20;
const LIFETIME_SUCCESSFUL_REFERRAL_CAP: u32 = 200;
const MONTHLY_TRADE_COMMISSION_CAP_MINOR_UNITS: i64 = 5_000_000;
const DAYS_PER_CAP_MONTH: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferralStatus {
    Pending,
    Qualified,
    Disqualified,
    PendingReview,
    PendingEddReview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferralRecord {
    pub referrer_id: String,
    pub referred_id: String,
    pub referral_code: String,
    pub referred_at_day: i64,
    pub observation_window_ends_at_day: i64,
    pub qualification_evaluated_at_day: Option<i64>,
    pub qualified_at_day: Option<i64>,
    pub signup_bonus_txn_id: Option<i64>,
    pub kinship_flagged: bool,
    pub kinship_score: Option<u16>,
    pub disqualify_reason: Option<String>,
    pub eddreview_reason: Option<String>,
    pub status: ReferralStatus,
}

impl ReferralRecord {
    /// Creates a pending first-touch referral record.
    ///
    /// # Errors
    ///
    /// Returns [`ReferralError::SelfReferral`] when both user ids are equal, or
    /// [`ReferralError::InvalidReferralCode`] when the captured code is empty.
    #[allow(clippy::similar_names)]
    pub fn pending(
        sponsor_user_id: impl Into<String>,
        new_user_id: impl Into<String>,
        referral_code: impl Into<String>,
        referred_at_day: i64,
    ) -> Result<Self, ReferralError> {
        let referrer_id = sponsor_user_id.into();
        let referred_id = new_user_id.into();
        let referral_code = referral_code.into();

        if referrer_id == referred_id {
            return Err(ReferralError::SelfReferral);
        }
        if referral_code.trim().is_empty() {
            return Err(ReferralError::InvalidReferralCode);
        }

        Ok(Self {
            referrer_id,
            referred_id,
            referral_code,
            referred_at_day,
            observation_window_ends_at_day: referred_at_day + QUALIFICATION_WINDOW_DAYS,
            qualification_evaluated_at_day: None,
            qualified_at_day: None,
            signup_bonus_txn_id: None,
            kinship_flagged: false,
            kinship_score: None,
            disqualify_reason: None,
            eddreview_reason: None,
            status: ReferralStatus::Pending,
        })
    }

    fn mark_pending_review(&mut self, score: u16) {
        self.kinship_flagged = true;
        self.kinship_score = Some(score);
        self.status = ReferralStatus::PendingReview;
    }

    fn mark_pending_edd_review(&mut self, reason: impl Into<String>) {
        self.eddreview_reason = Some(reason.into());
        self.status = ReferralStatus::PendingEddReview;
    }

    fn disqualify(&mut self, evaluated_at_day: i64, reason: impl Into<String>) {
        self.status = ReferralStatus::Disqualified;
        self.qualification_evaluated_at_day = Some(evaluated_at_day);
        self.disqualify_reason = Some(reason.into());
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct AntiAbuseFlags {
    pub same_ip: bool,
    pub same_device_fingerprint: bool,
    pub same_kyc_identity: bool,
    pub referred_self_trade: bool,
    pub chargeback_or_void: bool,
    pub referrer_account_abnormal: bool,
    pub referred_permanently_banned: bool,
}

impl AntiAbuseFlags {
    #[must_use]
    pub const fn is_blocked(self) -> bool {
        self.same_ip
            || self.same_device_fingerprint
            || self.same_kyc_identity
            || self.referred_self_trade
            || self.chargeback_or_void
            || self.referrer_account_abnormal
            || self.referred_permanently_banned
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct AffiliateKinshipSignals {
    pub same_admin_division: bool,
    pub same_last_name: bool,
    pub near_full_address: bool,
    pub ip_overlap_30d: bool,
    pub shared_device_fingerprint: bool,
    pub same_bank_account_holder_name: bool,
}

impl AffiliateKinshipSignals {
    #[must_use]
    pub const fn score(self) -> u16 {
        let mut score = 0_u16;
        if self.same_admin_division {
            score += 30;
        }
        if self.same_last_name {
            score += 20;
        }
        if self.near_full_address {
            score += 25;
        }
        if self.ip_overlap_30d {
            score += 20;
        }
        if self.shared_device_fingerprint {
            score += 30;
        }
        if self.same_bank_account_holder_name {
            score += 40;
        }
        score
    }

    #[must_use]
    pub const fn requires_review(self) -> bool {
        self.score() >= 50
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualificationEvent {
    pub referred_id: String,
    pub observed_at_day: i64,
    pub deposit_minor_units: i64,
    pub kyc_tier: u8,
    pub completed_trade_count: u32,
    pub anti_abuse: AntiAbuseFlags,
    pub kinship_review_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionEvent {
    pub referred_id: String,
    pub trade_id: String,
    pub traded_at_day: i64,
    pub maker_fee_minor_units: i64,
    pub taker_fee_minor_units: i64,
    pub anti_abuse: AntiAbuseFlags,
    pub referrer_group_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionAccrual {
    pub referrer_id: String,
    pub referred_id: String,
    pub trade_id: String,
    pub fee_amount_minor_units: i64,
    pub commission_amount_minor_units: i64,
    pub accrued_at_day: i64,
    pub paid_out_txn_id: Option<i64>,
    pub paid_out_at_day: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffiliateReferralListItem {
    pub referred_id: String,
    pub masked_username: String,
    pub referred_at_day: i64,
    pub status: String,
    pub cumulative_commission_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffiliateCommissionSummary {
    pub successful_referrals_count: u32,
    pub signup_bonus_minor_units: i64,
    pub trade_commission_minor_units: i64,
    pub total_rewards_minor_units: i64,
    pub last_30d_commission_minor_units: i64,
    pub last_12m_commission_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffiliateDashboardResponse {
    pub user_id: String,
    pub referral_code: String,
    pub share_url: String,
    pub summary: AffiliateCommissionSummary,
    pub referrals: Vec<AffiliateReferralListItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffiliateSummary {
    pub id: String,
    pub user_id: Option<String>,
    pub display_name: String,
    pub email_masked: String,
    pub status: String,
    pub owner_sponsor_id: Option<String>,
    pub commission_rate_bps: i32,
    pub tier: i16,
    pub sub_tree_size: i32,
    pub active_sub_count: i32,
    pub pending_payout_minor: i64,
    pub pending_payout_currency: String,
    pub last_activity_at: String,
    pub risk_flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAffiliateMutationRequest {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub display_name: Option<String>,
    pub email_masked: Option<String>,
    pub status: Option<String>,
    pub owner_sponsor_id: Option<String>,
    pub commission_rate_bps: Option<i32>,
    pub tier: Option<i16>,
    pub credit_rating: Option<String>,
    pub credit_status: Option<String>,
    pub max_active_credit_accounts: Option<i32>,
    pub total_credit_limit_minor_units: Option<i64>,
    pub per_account_limit_minor_units: Option<i64>,
    pub credit_review_expires_at: Option<String>,
    pub reason: String,
    pub authorization_basis: String,
    pub effective_at: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAffiliateMutationResponse {
    pub affiliate: AffiliateDetail,
    pub audit_correlation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminAffiliateActor {
    pub actor_id: String,
    pub role: String,
    pub request_id: Option<String>,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffiliatePayoutRow {
    pub id: String,
    pub amount_minor: i64,
    pub currency: String,
    pub settled_at: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffiliateCommissionAdjustmentReview {
    pub id: String,
    pub affiliate_id: String,
    pub adjustment_type: String,
    pub amount_minor: i64,
    pub currency: String,
    pub reason: String,
    pub status: String,
    pub requested_by: String,
    pub requested_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffiliatePayoutReview {
    pub id: String,
    pub affiliate_id: String,
    pub amount_minor: i64,
    pub currency: String,
    pub requested_at: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffiliateRateChange {
    pub effective_at: String,
    pub previous_bps: i32,
    pub next_bps: i32,
    pub changed_by: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffiliateCreditAccountSummary {
    pub id: String,
    pub status: String,
    pub credit_limit_minor_units: i64,
    pub used_minor_units: i64,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffiliateCreditProfile {
    pub rating: String,
    pub status: String,
    pub max_active_credit_accounts: i32,
    pub active_credit_accounts: i64,
    pub total_credit_limit_minor_units: i64,
    pub per_account_limit_minor_units: i64,
    pub review_expires_at: Option<String>,
    pub last_changed_by: String,
    pub last_changed_at: String,
    pub audit_correlation_ids: Vec<String>,
    pub accounts: Vec<AffiliateCreditAccountSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffiliateSubTreeBreakdown {
    pub tier: i16,
    pub count: i32,
    pub active_count: i32,
    pub mtd_contribution_minor: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffiliateDetail {
    #[serde(flatten)]
    pub summary: AffiliateSummary,
    pub sub_tree_breakdown: Vec<AffiliateSubTreeBreakdown>,
    pub recent_payouts: Vec<AffiliatePayoutRow>,
    pub rate_history: Vec<AffiliateRateChange>,
    pub audit_correlation_ids: Vec<String>,
    pub credit_profile: AffiliateCreditProfile,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AffiliateQuery {
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub error: &'static str,
    pub message: String,
}

#[async_trait]
pub trait AffiliateDashboardStore {
    async fn referral_code_for_user(&self, user_id: &str) -> Result<String, ReferralStoreError>;

    async fn list_referrals_for_referrer(
        &self,
        referrer_id: &str,
        limit: usize,
    ) -> Result<Vec<AffiliateReferralListItem>, ReferralStoreError>;

    async fn commission_summary_for_referrer(
        &self,
        referrer_id: &str,
    ) -> Result<AffiliateCommissionSummary, ReferralStoreError>;

    async fn list_admin_affiliates(&self) -> Result<Vec<AffiliateSummary>, ReferralStoreError>;

    async fn admin_affiliate_detail(
        &self,
        affiliate_id: &str,
    ) -> Result<AffiliateDetail, ReferralStoreError>;

    async fn create_admin_affiliate(
        &self,
        request: AdminAffiliateMutationRequest,
        actor: AdminAffiliateActor,
    ) -> Result<AdminAffiliateMutationResponse, ReferralStoreError>;

    async fn update_admin_affiliate(
        &self,
        affiliate_id: &str,
        request: AdminAffiliateMutationRequest,
        actor: AdminAffiliateActor,
    ) -> Result<AdminAffiliateMutationResponse, ReferralStoreError>;

    async fn archive_admin_affiliate(
        &self,
        affiliate_id: &str,
        request: AdminAffiliateMutationRequest,
        actor: AdminAffiliateActor,
    ) -> Result<AdminAffiliateMutationResponse, ReferralStoreError>;

    async fn list_commission_adjustment_reviews(
        &self,
    ) -> Result<Vec<AffiliateCommissionAdjustmentReview>, ReferralStoreError>;

    async fn list_payout_reviews(&self) -> Result<Vec<AffiliatePayoutReview>, ReferralStoreError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReferralStoreError {
    #[error("affiliate data not found")]
    NotFound,
    #[error("invalid affiliate request: {0}")]
    Invalid(String),
    #[error("affiliate request is forbidden: {0}")]
    Forbidden(String),
    #[error("affiliate store unavailable: {0}")]
    Unavailable(String),
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAffiliateDashboardStore {
    referral_codes_by_user: HashMap<String, String>,
    referrals_by_referrer: HashMap<String, Vec<AffiliateReferralListItem>>,
    summaries_by_referrer: HashMap<String, AffiliateCommissionSummary>,
    commission_adjustment_reviews: Vec<AffiliateCommissionAdjustmentReview>,
    payout_reviews: Vec<AffiliatePayoutReview>,
    admin_affiliates: Arc<tokio::sync::Mutex<HashMap<String, AffiliateDetail>>>,
}

impl InMemoryAffiliateDashboardStore {
    #[must_use]
    pub fn new(
        referral_codes_by_user: HashMap<String, String>,
        referrals_by_referrer: HashMap<String, Vec<AffiliateReferralListItem>>,
        summaries_by_referrer: HashMap<String, AffiliateCommissionSummary>,
    ) -> Self {
        Self {
            referral_codes_by_user,
            referrals_by_referrer,
            summaries_by_referrer,
            commission_adjustment_reviews: Vec::new(),
            payout_reviews: Vec::new(),
            admin_affiliates: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    #[must_use]
    pub fn with_admin_review_queues(
        mut self,
        commission_adjustment_reviews: Vec<AffiliateCommissionAdjustmentReview>,
        payout_reviews: Vec<AffiliatePayoutReview>,
    ) -> Self {
        self.commission_adjustment_reviews = commission_adjustment_reviews;
        self.payout_reviews = payout_reviews;
        self
    }

    #[must_use]
    pub fn with_admin_affiliates(mut self, affiliates: Vec<AffiliateDetail>) -> Self {
        self.admin_affiliates = Arc::new(tokio::sync::Mutex::new(
            affiliates
                .into_iter()
                .map(|affiliate| (affiliate.summary.id.clone(), affiliate))
                .collect(),
        ));
        self
    }
}

#[async_trait]
impl AffiliateDashboardStore for InMemoryAffiliateDashboardStore {
    async fn referral_code_for_user(&self, user_id: &str) -> Result<String, ReferralStoreError> {
        self.referral_codes_by_user
            .get(user_id)
            .cloned()
            .ok_or(ReferralStoreError::NotFound)
    }

    async fn list_referrals_for_referrer(
        &self,
        referrer_id: &str,
        limit: usize,
    ) -> Result<Vec<AffiliateReferralListItem>, ReferralStoreError> {
        Ok(self
            .referrals_by_referrer
            .get(referrer_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .take(limit)
            .collect())
    }

    async fn commission_summary_for_referrer(
        &self,
        referrer_id: &str,
    ) -> Result<AffiliateCommissionSummary, ReferralStoreError> {
        Ok(self
            .summaries_by_referrer
            .get(referrer_id)
            .cloned()
            .unwrap_or_else(|| AffiliateCommissionSummary {
                successful_referrals_count: 0,
                signup_bonus_minor_units: 0,
                trade_commission_minor_units: 0,
                total_rewards_minor_units: 0,
                last_30d_commission_minor_units: 0,
                last_12m_commission_minor_units: 0,
            }))
    }

    async fn list_admin_affiliates(&self) -> Result<Vec<AffiliateSummary>, ReferralStoreError> {
        Ok(self
            .admin_affiliates
            .lock()
            .await
            .values()
            .filter(|affiliate| affiliate.summary.status != "archived")
            .map(|affiliate| affiliate.summary.clone())
            .collect())
    }

    async fn admin_affiliate_detail(
        &self,
        affiliate_id: &str,
    ) -> Result<AffiliateDetail, ReferralStoreError> {
        self.admin_affiliates
            .lock()
            .await
            .get(affiliate_id)
            .cloned()
            .ok_or(ReferralStoreError::NotFound)
    }

    async fn create_admin_affiliate(
        &self,
        request: AdminAffiliateMutationRequest,
        actor: AdminAffiliateActor,
    ) -> Result<AdminAffiliateMutationResponse, ReferralStoreError> {
        validate_admin_affiliate_mutation(&request, true, &actor)?;
        let affiliate_id = request
            .id
            .clone()
            .ok_or_else(|| ReferralStoreError::Invalid("id is required".to_owned()))?;
        let now = now_string();
        let audit_correlation_id = audit_correlation_id("affiliate-create", &affiliate_id, &actor);
        let detail = AffiliateDetail {
            summary: AffiliateSummary {
                id: affiliate_id.clone(),
                user_id: request.user_id,
                display_name: request.display_name.unwrap_or_default(),
                email_masked: request.email_masked.unwrap_or_default(),
                status: request.status.unwrap_or_else(|| "onboarding".to_owned()),
                owner_sponsor_id: request.owner_sponsor_id,
                commission_rate_bps: request.commission_rate_bps.unwrap_or(0),
                tier: request.tier.unwrap_or(1),
                sub_tree_size: 0,
                active_sub_count: 0,
                pending_payout_minor: 0,
                pending_payout_currency: "USDT".to_owned(),
                last_activity_at: now.clone(),
                risk_flags: Vec::new(),
            },
            sub_tree_breakdown: Vec::new(),
            recent_payouts: Vec::new(),
            rate_history: Vec::new(),
            audit_correlation_ids: vec![audit_correlation_id.clone()],
            credit_profile: AffiliateCreditProfile {
                rating: request
                    .credit_rating
                    .unwrap_or_else(|| "restricted".to_owned()),
                status: request
                    .credit_status
                    .unwrap_or_else(|| "pending_review".to_owned()),
                max_active_credit_accounts: request.max_active_credit_accounts.unwrap_or(0),
                active_credit_accounts: 0,
                total_credit_limit_minor_units: request.total_credit_limit_minor_units.unwrap_or(0),
                per_account_limit_minor_units: request.per_account_limit_minor_units.unwrap_or(0),
                review_expires_at: request.credit_review_expires_at,
                last_changed_by: actor.actor_id,
                last_changed_at: now,
                audit_correlation_ids: vec![audit_correlation_id.clone()],
                accounts: Vec::new(),
            },
        };
        let mut affiliates = self.admin_affiliates.lock().await;
        if affiliates.contains_key(&affiliate_id) {
            return Err(ReferralStoreError::Invalid(
                "affiliate already exists".to_owned(),
            ));
        }
        affiliates.insert(affiliate_id, detail.clone());
        Ok(AdminAffiliateMutationResponse {
            affiliate: detail,
            audit_correlation_id,
        })
    }

    async fn update_admin_affiliate(
        &self,
        affiliate_id: &str,
        request: AdminAffiliateMutationRequest,
        actor: AdminAffiliateActor,
    ) -> Result<AdminAffiliateMutationResponse, ReferralStoreError> {
        validate_admin_affiliate_mutation(&request, false, &actor)?;
        let mut affiliates = self.admin_affiliates.lock().await;
        let detail = affiliates
            .get_mut(affiliate_id)
            .ok_or(ReferralStoreError::NotFound)?;
        let audit_correlation_id = audit_correlation_id("affiliate-update", affiliate_id, &actor);
        apply_affiliate_mutation(detail, request, &actor, &audit_correlation_id);
        Ok(AdminAffiliateMutationResponse {
            affiliate: detail.clone(),
            audit_correlation_id,
        })
    }

    async fn archive_admin_affiliate(
        &self,
        affiliate_id: &str,
        request: AdminAffiliateMutationRequest,
        actor: AdminAffiliateActor,
    ) -> Result<AdminAffiliateMutationResponse, ReferralStoreError> {
        validate_admin_affiliate_mutation(&request, false, &actor)?;
        let mut affiliates = self.admin_affiliates.lock().await;
        let detail = affiliates
            .get_mut(affiliate_id)
            .ok_or(ReferralStoreError::NotFound)?;
        let audit_correlation_id = audit_correlation_id("affiliate-archive", affiliate_id, &actor);
        "archived".clone_into(&mut detail.summary.status);
        "closed".clone_into(&mut detail.credit_profile.status);
        detail.credit_profile.max_active_credit_accounts = 0;
        detail.credit_profile.last_changed_by = actor.actor_id;
        detail.credit_profile.last_changed_at = now_string();
        detail
            .audit_correlation_ids
            .push(audit_correlation_id.clone());
        detail
            .credit_profile
            .audit_correlation_ids
            .push(audit_correlation_id.clone());
        Ok(AdminAffiliateMutationResponse {
            affiliate: detail.clone(),
            audit_correlation_id,
        })
    }

    async fn list_commission_adjustment_reviews(
        &self,
    ) -> Result<Vec<AffiliateCommissionAdjustmentReview>, ReferralStoreError> {
        Ok(self.commission_adjustment_reviews.clone())
    }

    async fn list_payout_reviews(&self) -> Result<Vec<AffiliatePayoutReview>, ReferralStoreError> {
        Ok(self.payout_reviews.clone())
    }
}

#[derive(Debug, Clone)]
pub struct PostgresAffiliateStore {
    pool: PgPool,
}

impl PostgresAffiliateStore {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

pub fn router<S>(store: S) -> Router
where
    S: AffiliateDashboardStore + Clone + Send + Sync + 'static,
{
    let state = Arc::new(store);
    Router::new()
        .route(
            "/internal/referrals/users/:user_id/affiliate-dashboard",
            get(get_affiliate_dashboard::<S>),
        )
        .route(
            "/internal/referrals/users/:user_id/referrals",
            get(list_affiliate_referrals::<S>),
        )
        .route(
            "/internal/referrals/users/:user_id/commission-summary",
            get(get_affiliate_commission_summary::<S>),
        )
        .route(
            "/internal/admin/affiliates",
            get(list_admin_affiliates::<S>).post(create_admin_affiliate::<S>),
        )
        .route(
            "/internal/admin/affiliates/:affiliate_id",
            get(get_admin_affiliate_detail::<S>)
                .patch(update_admin_affiliate::<S>)
                .delete(archive_admin_affiliate::<S>),
        )
        .route(
            "/internal/admin/affiliate-commission-adjustments",
            get(list_commission_adjustment_reviews::<S>),
        )
        .route(
            "/internal/admin/affiliate-payouts",
            get(list_payout_reviews::<S>),
        )
        .with_state(state)
}

async fn get_affiliate_dashboard<S>(
    State(store): State<Arc<S>>,
    Path(user_id): Path<String>,
    Query(query): Query<AffiliateQuery>,
) -> Result<Json<AffiliateDashboardResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let referral_code = store
        .referral_code_for_user(&user_id)
        .await
        .map_err(|error| api_error(&error))?;
    let summary = store
        .commission_summary_for_referrer(&user_id)
        .await
        .map_err(|error| api_error(&error))?;
    let referrals = store
        .list_referrals_for_referrer(&user_id, limit)
        .await
        .map_err(|error| api_error(&error))?;

    Ok(Json(AffiliateDashboardResponse {
        user_id,
        share_url: format!("https://proph.net/?ref={referral_code}"),
        referral_code,
        summary,
        referrals,
    }))
}

async fn list_affiliate_referrals<S>(
    State(store): State<Arc<S>>,
    Path(user_id): Path<String>,
    Query(query): Query<AffiliateQuery>,
) -> Result<Json<Vec<AffiliateReferralListItem>>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let referrals = store
        .list_referrals_for_referrer(&user_id, limit)
        .await
        .map_err(|error| api_error(&error))?;
    Ok(Json(referrals))
}

async fn get_affiliate_commission_summary<S>(
    State(store): State<Arc<S>>,
    Path(user_id): Path<String>,
) -> Result<Json<AffiliateCommissionSummary>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    let summary = store
        .commission_summary_for_referrer(&user_id)
        .await
        .map_err(|error| api_error(&error))?;
    Ok(Json(summary))
}

async fn list_admin_affiliates<S>(
    State(store): State<Arc<S>>,
) -> Result<Json<Vec<AffiliateSummary>>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    let affiliates = store
        .list_admin_affiliates()
        .await
        .map_err(|error| api_error(&error))?;
    Ok(Json(affiliates))
}

async fn get_admin_affiliate_detail<S>(
    State(store): State<Arc<S>>,
    Path(affiliate_id): Path<String>,
) -> Result<Json<AffiliateDetail>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    let affiliate = store
        .admin_affiliate_detail(&affiliate_id)
        .await
        .map_err(|error| api_error(&error))?;
    Ok(Json(affiliate))
}

async fn create_admin_affiliate<S>(
    State(store): State<Arc<S>>,
    headers: HeaderMap,
    Json(request): Json<AdminAffiliateMutationRequest>,
) -> Result<(StatusCode, Json<AdminAffiliateMutationResponse>), (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    let actor = admin_actor_from_headers(&headers).map_err(|error| api_error(&error))?;
    let response = store
        .create_admin_affiliate(request, actor)
        .await
        .map_err(|error| api_error(&error))?;
    Ok((StatusCode::CREATED, Json(response)))
}

async fn update_admin_affiliate<S>(
    State(store): State<Arc<S>>,
    headers: HeaderMap,
    Path(affiliate_id): Path<String>,
    Json(request): Json<AdminAffiliateMutationRequest>,
) -> Result<Json<AdminAffiliateMutationResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    let actor = admin_actor_from_headers(&headers).map_err(|error| api_error(&error))?;
    let response = store
        .update_admin_affiliate(&affiliate_id, request, actor)
        .await
        .map_err(|error| api_error(&error))?;
    Ok(Json(response))
}

async fn archive_admin_affiliate<S>(
    State(store): State<Arc<S>>,
    headers: HeaderMap,
    Path(affiliate_id): Path<String>,
    Json(request): Json<AdminAffiliateMutationRequest>,
) -> Result<Json<AdminAffiliateMutationResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    let actor = admin_actor_from_headers(&headers).map_err(|error| api_error(&error))?;
    let response = store
        .archive_admin_affiliate(&affiliate_id, request, actor)
        .await
        .map_err(|error| api_error(&error))?;
    Ok(Json(response))
}

async fn list_commission_adjustment_reviews<S>(
    State(store): State<Arc<S>>,
) -> Result<Json<Vec<AffiliateCommissionAdjustmentReview>>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    let reviews = store
        .list_commission_adjustment_reviews()
        .await
        .map_err(|error| api_error(&error))?;
    Ok(Json(reviews))
}

async fn list_payout_reviews<S>(
    State(store): State<Arc<S>>,
) -> Result<Json<Vec<AffiliatePayoutReview>>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    let reviews = store
        .list_payout_reviews()
        .await
        .map_err(|error| api_error(&error))?;
    Ok(Json(reviews))
}

fn api_error(error: &ReferralStoreError) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        ReferralStoreError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(ApiErrorResponse {
                error: "affiliate_not_found",
                message: error.to_string(),
            }),
        ),
        ReferralStoreError::Invalid(_) => (
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: "invalid_affiliate_request",
                message: error.to_string(),
            }),
        ),
        ReferralStoreError::Forbidden(_) => (
            StatusCode::FORBIDDEN,
            Json(ApiErrorResponse {
                error: "affiliate_forbidden",
                message: error.to_string(),
            }),
        ),
        ReferralStoreError::Unavailable(_) => (
            StatusCode::BAD_GATEWAY,
            Json(ApiErrorResponse {
                error: "affiliate_store_unavailable",
                message: error.to_string(),
            }),
        ),
    }
}

fn admin_actor_from_headers(
    headers: &HeaderMap,
) -> Result<AdminAffiliateActor, ReferralStoreError> {
    let actor_id = header_value(headers, "x-actor-id")
        .ok_or_else(|| ReferralStoreError::Forbidden("missing actor id".to_owned()))?;
    let role = header_value(headers, "x-operator-role")
        .ok_or_else(|| ReferralStoreError::Forbidden("missing operator role".to_owned()))?;
    Ok(AdminAffiliateActor {
        actor_id,
        role,
        request_id: header_value(headers, "x-request-id"),
        user_agent: header_value(headers, "user-agent"),
        ip: header_value(headers, "x-forwarded-for"),
    })
}

fn header_value(headers: &HeaderMap, name: &'static str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn validate_admin_affiliate_mutation(
    request: &AdminAffiliateMutationRequest,
    create: bool,
    actor: &AdminAffiliateActor,
) -> Result<(), ReferralStoreError> {
    if actor.role != "admin" && actor.role != "manager" {
        return Err(ReferralStoreError::Forbidden(
            "only admin or manager can mutate affiliates".to_owned(),
        ));
    }
    if request.reason.trim().is_empty() || request.authorization_basis.trim().is_empty() {
        return Err(ReferralStoreError::Invalid(
            "reason and authorizationBasis are required".to_owned(),
        ));
    }
    if create
        && (request.id.as_deref().is_none_or(str::is_empty)
            || request.user_id.as_deref().is_none_or(str::is_empty)
            || request.display_name.as_deref().is_none_or(str::is_empty)
            || request.email_masked.as_deref().is_none_or(str::is_empty))
    {
        return Err(ReferralStoreError::Invalid(
            "id, userId, displayName and emailMasked are required".to_owned(),
        ));
    }
    if request.status.as_deref().is_some_and(|status| {
        !matches!(
            status,
            "active" | "review" | "frozen" | "onboarding" | "archived"
        )
    }) {
        return Err(ReferralStoreError::Invalid(
            "invalid affiliate status".to_owned(),
        ));
    }
    if request
        .credit_rating
        .as_deref()
        .is_some_and(|rating| !matches!(rating, "A" | "B" | "C" | "restricted" | "suspended"))
    {
        return Err(ReferralStoreError::Invalid(
            "invalid credit rating".to_owned(),
        ));
    }
    if request.credit_status.as_deref().is_some_and(|status| {
        !matches!(
            status,
            "pending_review" | "active" | "limited" | "frozen" | "closed"
        )
    }) {
        return Err(ReferralStoreError::Invalid(
            "invalid credit status".to_owned(),
        ));
    }
    if request.commission_rate_bps.is_some_and(|value| value < 0)
        || request
            .max_active_credit_accounts
            .is_some_and(|value| value < 0)
        || request
            .total_credit_limit_minor_units
            .is_some_and(|value| value < 0)
        || request
            .per_account_limit_minor_units
            .is_some_and(|value| value < 0)
        || request.tier.is_some_and(|value| !matches!(value, 1..=3))
    {
        return Err(ReferralStoreError::Invalid(
            "numeric fields must be within allowed ranges".to_owned(),
        ));
    }
    if actor.role == "manager"
        && (request.credit_rating.is_some()
            || request.credit_status.is_some()
            || request.max_active_credit_accounts.is_some()
            || request.total_credit_limit_minor_units.is_some()
            || request.per_account_limit_minor_units.is_some())
    {
        return Err(ReferralStoreError::Forbidden(
            "manager can update profile fields but cannot change credit controls".to_owned(),
        ));
    }
    Ok(())
}

fn apply_affiliate_mutation(
    detail: &mut AffiliateDetail,
    request: AdminAffiliateMutationRequest,
    actor: &AdminAffiliateActor,
    audit_correlation_id: &str,
) {
    if let Some(display_name) = request.display_name {
        detail.summary.display_name = display_name;
    }
    if let Some(email_masked) = request.email_masked {
        detail.summary.email_masked = email_masked;
    }
    if let Some(status) = request.status {
        detail.summary.status = status;
    }
    if let Some(owner_sponsor_id) = request.owner_sponsor_id {
        detail.summary.owner_sponsor_id = Some(owner_sponsor_id);
    }
    if let Some(commission_rate_bps) = request.commission_rate_bps {
        let previous_bps = detail.summary.commission_rate_bps;
        detail.summary.commission_rate_bps = commission_rate_bps;
        if previous_bps != commission_rate_bps {
            detail.rate_history.insert(
                0,
                AffiliateRateChange {
                    effective_at: request
                        .effective_at
                        .clone()
                        .unwrap_or_else(|| now_string().chars().take(10).collect()),
                    previous_bps,
                    next_bps: commission_rate_bps,
                    changed_by: actor.actor_id.clone(),
                    reason: request.reason.clone(),
                },
            );
        }
    }
    if let Some(tier) = request.tier {
        detail.summary.tier = tier;
    }
    if let Some(rating) = request.credit_rating {
        detail.credit_profile.rating = rating;
    }
    if let Some(status) = request.credit_status {
        detail.credit_profile.status = status;
    }
    if let Some(value) = request.max_active_credit_accounts {
        detail.credit_profile.max_active_credit_accounts = value;
    }
    if let Some(value) = request.total_credit_limit_minor_units {
        detail.credit_profile.total_credit_limit_minor_units = value;
    }
    if let Some(value) = request.per_account_limit_minor_units {
        detail.credit_profile.per_account_limit_minor_units = value;
    }
    if let Some(value) = request.credit_review_expires_at {
        detail.credit_profile.review_expires_at = Some(value);
    }
    detail
        .credit_profile
        .last_changed_by
        .clone_from(&actor.actor_id);
    detail.credit_profile.last_changed_at = now_string();
    detail
        .audit_correlation_ids
        .push(audit_correlation_id.to_owned());
    detail
        .credit_profile
        .audit_correlation_ids
        .push(audit_correlation_id.to_owned());
}

fn audit_correlation_id(action: &str, affiliate_id: &str, actor: &AdminAffiliateActor) -> String {
    format!(
        "audit:{action}:{affiliate_id}:{}",
        actor.request_id.as_deref().unwrap_or(&actor.actor_id)
    )
}

fn now_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    format!("{seconds}")
}

async fn insert_affiliate_audit(
    pool: &PgPool,
    action: &str,
    affiliate_id: &str,
    before: Option<&AffiliateDetail>,
    request: &AdminAffiliateMutationRequest,
    actor: &AdminAffiliateActor,
    audit_correlation_id: &str,
) -> Result<(), ReferralStoreError> {
    let before_summary = before.map_or_else(String::new, |detail| {
        format!(
            "status={},rate={},rating={},credit_status={},max_accounts={},total_limit={},per_account_limit={}",
            detail.summary.status,
            detail.summary.commission_rate_bps,
            detail.credit_profile.rating,
            detail.credit_profile.status,
            detail.credit_profile.max_active_credit_accounts,
            detail.credit_profile.total_credit_limit_minor_units,
            detail.credit_profile.per_account_limit_minor_units
        )
    });
    let after_patch = format!(
        "status={:?},rate={:?},rating={:?},credit_status={:?},max_accounts={:?},total_limit={:?},per_account_limit={:?},effective_at={:?},expires_at={:?}",
        request.status,
        request.commission_rate_bps,
        request.credit_rating,
        request.credit_status,
        request.max_active_credit_accounts,
        request.total_credit_limit_minor_units,
        request.per_account_limit_minor_units,
        request.effective_at,
        request.expires_at
    );
    sqlx::query(
        r"
        INSERT INTO affiliate_operation_audit_logs (
            correlation_id, affiliate_id, action, actor_id, actor_role, reason,
            authorization_basis, before_summary, after_patch, request_id,
            ip, user_agent
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ",
    )
    .bind(audit_correlation_id)
    .bind(affiliate_id)
    .bind(action)
    .bind(&actor.actor_id)
    .bind(&actor.role)
    .bind(&request.reason)
    .bind(&request.authorization_basis)
    .bind(before_summary)
    .bind(after_patch)
    .bind(actor.request_id.as_deref())
    .bind(actor.ip.as_deref())
    .bind(actor.user_agent.as_deref())
    .execute(pool)
    .await
    .map_err(sqlx_error)?;
    Ok(())
}

#[async_trait]
impl AffiliateDashboardStore for PostgresAffiliateStore {
    async fn referral_code_for_user(&self, user_id: &str) -> Result<String, ReferralStoreError> {
        let row = sqlx::query("SELECT affiliate_id FROM affiliate_profiles WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlx_error)?;
        row.map(|row| row.try_get("affiliate_id"))
            .transpose()
            .map_err(sqlx_error)?
            .ok_or(ReferralStoreError::NotFound)
    }

    async fn list_referrals_for_referrer(
        &self,
        referrer_id: &str,
        limit: usize,
    ) -> Result<Vec<AffiliateReferralListItem>, ReferralStoreError> {
        let rows = sqlx::query(
            r"
            SELECT referred_id,
                   referred_id AS masked_username,
                   FLOOR(EXTRACT(EPOCH FROM referred_at) / 86400)::BIGINT AS referred_at_day,
                   status,
                   COALESCE((
                       SELECT SUM(commission_amount)
                       FROM commission_accruals
                       WHERE commission_accruals.referred_id = referrals.referred_id
                   ), 0)::BIGINT AS cumulative_commission_minor_units
            FROM referrals
            WHERE referrer_id = $1
            ORDER BY referred_at DESC
            LIMIT $2
            ",
        )
        .bind(referrer_id)
        .bind(
            i64::try_from(limit)
                .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_error)?;

        rows.iter().map(referral_item_from_row).collect()
    }

    async fn commission_summary_for_referrer(
        &self,
        referrer_id: &str,
    ) -> Result<AffiliateCommissionSummary, ReferralStoreError> {
        let row = sqlx::query(
            r"
            SELECT
                COALESCE(COUNT(*) FILTER (WHERE referrals.status = 'qualified'), 0)::INT AS successful_referrals_count,
                COALESCE(SUM(commission_amount), 0)::BIGINT AS trade_commission_minor_units,
                COALESCE(SUM(commission_amount) FILTER (WHERE accrued_at >= now() - INTERVAL '30 days'), 0)::BIGINT AS last_30d_commission_minor_units,
                COALESCE(SUM(commission_amount) FILTER (WHERE accrued_at >= now() - INTERVAL '365 days'), 0)::BIGINT AS last_12m_commission_minor_units
            FROM referrals
            LEFT JOIN commission_accruals USING (referred_id)
            WHERE referrals.referrer_id = $1
            ",
        )
        .bind(referrer_id)
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_error)?;
        let successful_referrals_count: i32 = row
            .try_get("successful_referrals_count")
            .map_err(sqlx_error)?;
        let trade_commission_minor_units = row
            .try_get("trade_commission_minor_units")
            .map_err(sqlx_error)?;
        let signup_bonus_minor_units =
            i64::from(successful_referrals_count) * QUALIFICATION_BONUS_MINOR_UNITS;
        Ok(AffiliateCommissionSummary {
            successful_referrals_count: u32::try_from(successful_referrals_count)
                .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?,
            signup_bonus_minor_units,
            trade_commission_minor_units,
            total_rewards_minor_units: signup_bonus_minor_units + trade_commission_minor_units,
            last_30d_commission_minor_units: row
                .try_get("last_30d_commission_minor_units")
                .map_err(sqlx_error)?,
            last_12m_commission_minor_units: row
                .try_get("last_12m_commission_minor_units")
                .map_err(sqlx_error)?,
        })
    }

    async fn list_admin_affiliates(&self) -> Result<Vec<AffiliateSummary>, ReferralStoreError> {
        let rows = sqlx::query(&admin_affiliate_summary_sql(
            "",
            "ORDER BY p.last_activity_at DESC",
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_error)?;
        rows.iter().map(affiliate_summary_from_row).collect()
    }

    async fn admin_affiliate_detail(
        &self,
        affiliate_id: &str,
    ) -> Result<AffiliateDetail, ReferralStoreError> {
        let row = sqlx::query(&admin_affiliate_summary_sql(
            "WHERE p.affiliate_id = $1",
            "",
        ))
        .bind(affiliate_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_error)?
        .ok_or(ReferralStoreError::NotFound)?;
        let summary = affiliate_summary_from_row(&row)?;
        let sub_tree_breakdown = fetch_sub_tree_breakdown(&self.pool, affiliate_id).await?;
        let recent_payouts = fetch_affiliate_payouts(&self.pool, affiliate_id).await?;
        let rate_history = fetch_rate_history(&self.pool, affiliate_id).await?;
        let credit_profile = fetch_credit_profile(&self.pool, affiliate_id).await?;
        let audit_correlation_ids = credit_profile.audit_correlation_ids.clone();
        Ok(AffiliateDetail {
            summary,
            sub_tree_breakdown,
            recent_payouts,
            rate_history,
            audit_correlation_ids,
            credit_profile,
        })
    }

    async fn create_admin_affiliate(
        &self,
        request: AdminAffiliateMutationRequest,
        actor: AdminAffiliateActor,
    ) -> Result<AdminAffiliateMutationResponse, ReferralStoreError> {
        validate_admin_affiliate_mutation(&request, true, &actor)?;
        let affiliate_id = request
            .id
            .clone()
            .ok_or_else(|| ReferralStoreError::Invalid("id is required".to_owned()))?;
        let audit_correlation_id = audit_correlation_id("affiliate-create", &affiliate_id, &actor);
        sqlx::query(
            r"
            INSERT INTO affiliate_profiles (
                affiliate_id, user_id, display_name, email_masked, status, owner_sponsor_id,
                commission_rate_bps, tier, credit_rating, credit_status,
                max_active_credit_accounts, total_credit_limit_minor_units,
                per_account_limit_minor_units, credit_review_expires_at,
                credit_last_changed_by, credit_audit_correlation_ids
            )
            VALUES ($1, $2, $3, $4, COALESCE($5, 'onboarding'), $6,
                    COALESCE($7, 0), COALESCE($8, 1), COALESCE($9, 'restricted'),
                    COALESCE($10, 'pending_review'), COALESCE($11, 0),
                    COALESCE($12, 0), COALESCE($13, 0), $14::DATE, $15, ARRAY[$16])
            ",
        )
        .bind(&affiliate_id)
        .bind(request.user_id.as_deref())
        .bind(request.display_name.as_deref())
        .bind(request.email_masked.as_deref())
        .bind(request.status.as_deref())
        .bind(request.owner_sponsor_id.as_deref())
        .bind(request.commission_rate_bps)
        .bind(request.tier)
        .bind(request.credit_rating.as_deref())
        .bind(request.credit_status.as_deref())
        .bind(request.max_active_credit_accounts)
        .bind(request.total_credit_limit_minor_units)
        .bind(request.per_account_limit_minor_units)
        .bind(request.credit_review_expires_at.as_deref())
        .bind(&actor.actor_id)
        .bind(&audit_correlation_id)
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;
        insert_affiliate_audit(
            &self.pool,
            "create",
            &affiliate_id,
            None,
            &request,
            &actor,
            &audit_correlation_id,
        )
        .await?;
        Ok(AdminAffiliateMutationResponse {
            affiliate: self.admin_affiliate_detail(&affiliate_id).await?,
            audit_correlation_id,
        })
    }

    async fn update_admin_affiliate(
        &self,
        affiliate_id: &str,
        request: AdminAffiliateMutationRequest,
        actor: AdminAffiliateActor,
    ) -> Result<AdminAffiliateMutationResponse, ReferralStoreError> {
        validate_admin_affiliate_mutation(&request, false, &actor)?;
        let before = self.admin_affiliate_detail(affiliate_id).await?;
        let audit_correlation_id = audit_correlation_id("affiliate-update", affiliate_id, &actor);
        sqlx::query(
            r"
            UPDATE affiliate_profiles
            SET display_name = COALESCE($2, display_name),
                email_masked = COALESCE($3, email_masked),
                status = COALESCE($4, status),
                owner_sponsor_id = COALESCE($5, owner_sponsor_id),
                commission_rate_bps = COALESCE($6, commission_rate_bps),
                tier = COALESCE($7, tier),
                credit_rating = COALESCE($8, credit_rating),
                credit_status = COALESCE($9, credit_status),
                max_active_credit_accounts = COALESCE($10, max_active_credit_accounts),
                total_credit_limit_minor_units = COALESCE($11, total_credit_limit_minor_units),
                per_account_limit_minor_units = COALESCE($12, per_account_limit_minor_units),
                credit_review_expires_at = COALESCE($13::DATE, credit_review_expires_at),
                credit_last_changed_by = $14,
                credit_last_changed_at = now(),
                credit_audit_correlation_ids = array_append(credit_audit_correlation_ids, $15),
                updated_at = now()
            WHERE affiliate_id = $1
            ",
        )
        .bind(affiliate_id)
        .bind(request.display_name.as_deref())
        .bind(request.email_masked.as_deref())
        .bind(request.status.as_deref())
        .bind(request.owner_sponsor_id.as_deref())
        .bind(request.commission_rate_bps)
        .bind(request.tier)
        .bind(request.credit_rating.as_deref())
        .bind(request.credit_status.as_deref())
        .bind(request.max_active_credit_accounts)
        .bind(request.total_credit_limit_minor_units)
        .bind(request.per_account_limit_minor_units)
        .bind(request.credit_review_expires_at.as_deref())
        .bind(&actor.actor_id)
        .bind(&audit_correlation_id)
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;
        if request
            .commission_rate_bps
            .is_some_and(|next_bps| next_bps != before.summary.commission_rate_bps)
        {
            let next_bps = request.commission_rate_bps.ok_or_else(|| {
                ReferralStoreError::Unavailable("missing commission rate".to_owned())
            })?;
            sqlx::query(
                r"
                INSERT INTO affiliate_rate_changes
                    (affiliate_id, effective_at, previous_bps, next_bps, changed_by, reason)
                VALUES ($1, COALESCE($2::DATE, CURRENT_DATE), $3, $4, $5, $6)
                ",
            )
            .bind(affiliate_id)
            .bind(request.effective_at.as_deref())
            .bind(before.summary.commission_rate_bps)
            .bind(next_bps)
            .bind(&actor.actor_id)
            .bind(&request.reason)
            .execute(&self.pool)
            .await
            .map_err(sqlx_error)?;
        }
        insert_affiliate_audit(
            &self.pool,
            "update",
            affiliate_id,
            Some(&before),
            &request,
            &actor,
            &audit_correlation_id,
        )
        .await?;
        Ok(AdminAffiliateMutationResponse {
            affiliate: self.admin_affiliate_detail(affiliate_id).await?,
            audit_correlation_id,
        })
    }

    async fn archive_admin_affiliate(
        &self,
        affiliate_id: &str,
        request: AdminAffiliateMutationRequest,
        actor: AdminAffiliateActor,
    ) -> Result<AdminAffiliateMutationResponse, ReferralStoreError> {
        validate_admin_affiliate_mutation(&request, false, &actor)?;
        let before = self.admin_affiliate_detail(affiliate_id).await?;
        let audit_correlation_id = audit_correlation_id("affiliate-archive", affiliate_id, &actor);
        sqlx::query(
            r"
            UPDATE affiliate_profiles
            SET status = 'archived',
                credit_status = 'closed',
                max_active_credit_accounts = 0,
                credit_last_changed_by = $2,
                credit_last_changed_at = now(),
                credit_audit_correlation_ids = array_append(credit_audit_correlation_ids, $3),
                updated_at = now()
            WHERE affiliate_id = $1
            ",
        )
        .bind(affiliate_id)
        .bind(&actor.actor_id)
        .bind(&audit_correlation_id)
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;
        insert_affiliate_audit(
            &self.pool,
            "archive",
            affiliate_id,
            Some(&before),
            &request,
            &actor,
            &audit_correlation_id,
        )
        .await?;
        Ok(AdminAffiliateMutationResponse {
            affiliate: self.admin_affiliate_detail(affiliate_id).await?,
            audit_correlation_id,
        })
    }

    async fn list_commission_adjustment_reviews(
        &self,
    ) -> Result<Vec<AffiliateCommissionAdjustmentReview>, ReferralStoreError> {
        let rows = sqlx::query(
            r"
            SELECT adjustment_id, affiliate_id, adjustment_type, amount_minor, currency,
                   reason, status, requested_by,
                   to_char(requested_at, 'YYYY-MM-DD HH24:MI') AS requested_at
            FROM affiliate_commission_adjustment_reviews
            WHERE status IN ('pending', 'reviewing')
            ORDER BY requested_at ASC
            LIMIT 200
            ",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_error)?;

        rows.iter().map(commission_adjustment_from_row).collect()
    }

    async fn list_payout_reviews(&self) -> Result<Vec<AffiliatePayoutReview>, ReferralStoreError> {
        let rows = sqlx::query(
            r"
            SELECT payout_id, affiliate_id, amount_minor, currency,
                   to_char(created_at, 'YYYY-MM-DD HH24:MI') AS requested_at,
                   status
            FROM affiliate_payouts
            WHERE status IN ('pending', 'blocked')
            ORDER BY created_at ASC
            LIMIT 200
            ",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_error)?;

        rows.iter().map(payout_review_from_row).collect()
    }
}

#[allow(clippy::needless_pass_by_value)]
fn sqlx_error(error: sqlx::Error) -> ReferralStoreError {
    ReferralStoreError::Unavailable(error.to_string())
}

fn admin_affiliate_summary_sql(pre_group: &str, post_group: &str) -> String {
    format!(
        r#"
        SELECT p.affiliate_id, p.user_id, p.display_name, p.email_masked, p.status, p.owner_sponsor_id,
               p.commission_rate_bps, p.tier, to_char(p.last_activity_at, 'YYYY-MM-DD"T"HH24:MI:SSOF') AS last_activity_at,
               p.risk_flags,
               COALESCE(SUM(s.count), 0)::INT AS sub_tree_size,
               COALESCE(SUM(s.active_count), 0)::INT AS active_sub_count,
               COALESCE((
                   SELECT SUM(amount_minor)
                   FROM affiliate_payouts
                   WHERE affiliate_payouts.affiliate_id = p.affiliate_id
                     AND affiliate_payouts.status = 'pending'
               ), 0)::BIGINT AS pending_payout_minor,
               COALESCE((
                   SELECT currency
                   FROM affiliate_payouts
                   WHERE affiliate_payouts.affiliate_id = p.affiliate_id
                   ORDER BY created_at DESC
                   LIMIT 1
               ), 'USDT') AS pending_payout_currency
        FROM affiliate_profiles p
        LEFT JOIN affiliate_sub_tree_stats s ON s.affiliate_id = p.affiliate_id
        {pre_group}
        GROUP BY p.affiliate_id
        {post_group}
        "#
    )
}

fn affiliate_summary_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<AffiliateSummary, ReferralStoreError> {
    Ok(AffiliateSummary {
        id: row.try_get("affiliate_id").map_err(sqlx_error)?,
        user_id: row.try_get("user_id").map_err(sqlx_error)?,
        display_name: row.try_get("display_name").map_err(sqlx_error)?,
        email_masked: row.try_get("email_masked").map_err(sqlx_error)?,
        status: row.try_get("status").map_err(sqlx_error)?,
        owner_sponsor_id: row.try_get("owner_sponsor_id").map_err(sqlx_error)?,
        commission_rate_bps: row.try_get("commission_rate_bps").map_err(sqlx_error)?,
        tier: row.try_get("tier").map_err(sqlx_error)?,
        sub_tree_size: row.try_get("sub_tree_size").map_err(sqlx_error)?,
        active_sub_count: row.try_get("active_sub_count").map_err(sqlx_error)?,
        pending_payout_minor: row.try_get("pending_payout_minor").map_err(sqlx_error)?,
        pending_payout_currency: row.try_get("pending_payout_currency").map_err(sqlx_error)?,
        last_activity_at: row.try_get("last_activity_at").map_err(sqlx_error)?,
        risk_flags: row.try_get("risk_flags").map_err(sqlx_error)?,
    })
}

fn commission_adjustment_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<AffiliateCommissionAdjustmentReview, ReferralStoreError> {
    Ok(AffiliateCommissionAdjustmentReview {
        id: row.try_get("adjustment_id").map_err(sqlx_error)?,
        affiliate_id: row.try_get("affiliate_id").map_err(sqlx_error)?,
        adjustment_type: row.try_get("adjustment_type").map_err(sqlx_error)?,
        amount_minor: row.try_get("amount_minor").map_err(sqlx_error)?,
        currency: row.try_get("currency").map_err(sqlx_error)?,
        reason: row.try_get("reason").map_err(sqlx_error)?,
        status: row.try_get("status").map_err(sqlx_error)?,
        requested_by: row.try_get("requested_by").map_err(sqlx_error)?,
        requested_at: row.try_get("requested_at").map_err(sqlx_error)?,
    })
}

fn payout_review_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<AffiliatePayoutReview, ReferralStoreError> {
    Ok(AffiliatePayoutReview {
        id: row.try_get("payout_id").map_err(sqlx_error)?,
        affiliate_id: row.try_get("affiliate_id").map_err(sqlx_error)?,
        amount_minor: row.try_get("amount_minor").map_err(sqlx_error)?,
        currency: row.try_get("currency").map_err(sqlx_error)?,
        requested_at: row.try_get("requested_at").map_err(sqlx_error)?,
        status: row.try_get("status").map_err(sqlx_error)?,
    })
}

fn referral_item_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<AffiliateReferralListItem, ReferralStoreError> {
    Ok(AffiliateReferralListItem {
        referred_id: row.try_get("referred_id").map_err(sqlx_error)?,
        masked_username: row.try_get("masked_username").map_err(sqlx_error)?,
        referred_at_day: row.try_get("referred_at_day").map_err(sqlx_error)?,
        status: row.try_get("status").map_err(sqlx_error)?,
        cumulative_commission_minor_units: row
            .try_get("cumulative_commission_minor_units")
            .map_err(sqlx_error)?,
    })
}

async fn fetch_sub_tree_breakdown(
    pool: &PgPool,
    affiliate_id: &str,
) -> Result<Vec<AffiliateSubTreeBreakdown>, ReferralStoreError> {
    let rows = sqlx::query(
        r"
        SELECT tier, count, active_count, mtd_contribution_minor
        FROM affiliate_sub_tree_stats
        WHERE affiliate_id = $1
        ORDER BY tier
        ",
    )
    .bind(affiliate_id)
    .fetch_all(pool)
    .await
    .map_err(sqlx_error)?;
    rows.into_iter()
        .map(|row| {
            Ok(AffiliateSubTreeBreakdown {
                tier: row.try_get("tier").map_err(sqlx_error)?,
                count: row.try_get("count").map_err(sqlx_error)?,
                active_count: row.try_get("active_count").map_err(sqlx_error)?,
                mtd_contribution_minor: row
                    .try_get("mtd_contribution_minor")
                    .map_err(sqlx_error)?,
            })
        })
        .collect()
}

async fn fetch_affiliate_payouts(
    pool: &PgPool,
    affiliate_id: &str,
) -> Result<Vec<AffiliatePayoutRow>, ReferralStoreError> {
    let rows = sqlx::query(
        r"
        SELECT payout_id, amount_minor, currency, to_char(settled_at, 'YYYY-MM-DD') AS settled_at, status
        FROM affiliate_payouts
        WHERE affiliate_id = $1
        ORDER BY settled_at DESC
        LIMIT 20
        ",
    )
    .bind(affiliate_id)
    .fetch_all(pool)
    .await
    .map_err(sqlx_error)?;
    rows.into_iter()
        .map(|row| {
            Ok(AffiliatePayoutRow {
                id: row.try_get("payout_id").map_err(sqlx_error)?,
                amount_minor: row.try_get("amount_minor").map_err(sqlx_error)?,
                currency: row.try_get("currency").map_err(sqlx_error)?,
                settled_at: row.try_get("settled_at").map_err(sqlx_error)?,
                status: row.try_get("status").map_err(sqlx_error)?,
            })
        })
        .collect()
}

async fn fetch_rate_history(
    pool: &PgPool,
    affiliate_id: &str,
) -> Result<Vec<AffiliateRateChange>, ReferralStoreError> {
    let rows = sqlx::query(
        r"
        SELECT to_char(effective_at, 'YYYY-MM-DD') AS effective_at, previous_bps, next_bps, changed_by, reason
        FROM affiliate_rate_changes
        WHERE affiliate_id = $1
        ORDER BY effective_at DESC
        LIMIT 20
        ",
    )
    .bind(affiliate_id)
    .fetch_all(pool)
    .await
    .map_err(sqlx_error)?;
    rows.into_iter()
        .map(|row| {
            Ok(AffiliateRateChange {
                effective_at: row.try_get("effective_at").map_err(sqlx_error)?,
                previous_bps: row.try_get("previous_bps").map_err(sqlx_error)?,
                next_bps: row.try_get("next_bps").map_err(sqlx_error)?,
                changed_by: row.try_get("changed_by").map_err(sqlx_error)?,
                reason: row.try_get("reason").map_err(sqlx_error)?,
            })
        })
        .collect()
}

async fn fetch_credit_profile(
    pool: &PgPool,
    affiliate_id: &str,
) -> Result<AffiliateCreditProfile, ReferralStoreError> {
    let row = sqlx::query(
        r#"
        SELECT credit_rating, credit_status, max_active_credit_accounts,
               (SELECT COUNT(*) FROM affiliate_credit_accounts WHERE affiliate_id = affiliate_profiles.affiliate_id AND status IN ('active', 'limited'))::BIGINT AS active_credit_accounts,
               total_credit_limit_minor_units, per_account_limit_minor_units,
               to_char(credit_review_expires_at, 'YYYY-MM-DD') AS review_expires_at,
               credit_last_changed_by,
               to_char(credit_last_changed_at, 'YYYY-MM-DD"T"HH24:MI:SSOF') AS last_changed_at,
               credit_audit_correlation_ids
        FROM affiliate_profiles
        WHERE affiliate_id = $1
        "#,
    )
    .bind(affiliate_id)
    .fetch_one(pool)
    .await
    .map_err(sqlx_error)?;
    let accounts = fetch_credit_accounts(pool, affiliate_id).await?;
    Ok(AffiliateCreditProfile {
        rating: row.try_get("credit_rating").map_err(sqlx_error)?,
        status: row.try_get("credit_status").map_err(sqlx_error)?,
        max_active_credit_accounts: row
            .try_get("max_active_credit_accounts")
            .map_err(sqlx_error)?,
        active_credit_accounts: row.try_get("active_credit_accounts").map_err(sqlx_error)?,
        total_credit_limit_minor_units: row
            .try_get("total_credit_limit_minor_units")
            .map_err(sqlx_error)?,
        per_account_limit_minor_units: row
            .try_get("per_account_limit_minor_units")
            .map_err(sqlx_error)?,
        review_expires_at: row.try_get("review_expires_at").map_err(sqlx_error)?,
        last_changed_by: row.try_get("credit_last_changed_by").map_err(sqlx_error)?,
        last_changed_at: row.try_get("last_changed_at").map_err(sqlx_error)?,
        audit_correlation_ids: row
            .try_get("credit_audit_correlation_ids")
            .map_err(sqlx_error)?,
        accounts,
    })
}

async fn fetch_credit_accounts(
    pool: &PgPool,
    affiliate_id: &str,
) -> Result<Vec<AffiliateCreditAccountSummary>, ReferralStoreError> {
    let rows = sqlx::query(
        r"
        SELECT account_id, status, credit_limit_minor_units, used_minor_units,
               to_char(expires_at, 'YYYY-MM-DD') AS expires_at
        FROM affiliate_credit_accounts
        WHERE affiliate_id = $1
        ORDER BY account_id
        ",
    )
    .bind(affiliate_id)
    .fetch_all(pool)
    .await
    .map_err(sqlx_error)?;
    rows.into_iter()
        .map(|row| {
            Ok(AffiliateCreditAccountSummary {
                id: row.try_get("account_id").map_err(sqlx_error)?,
                status: row.try_get("status").map_err(sqlx_error)?,
                credit_limit_minor_units: row
                    .try_get("credit_limit_minor_units")
                    .map_err(sqlx_error)?,
                used_minor_units: row.try_get("used_minor_units").map_err(sqlx_error)?,
                expires_at: row.try_get("expires_at").map_err(sqlx_error)?,
            })
        })
        .collect()
}

#[derive(Debug, Default)]
pub struct AffiliateCommissionEngine {
    referrals_by_referred_id: HashMap<String, ReferralRecord>,
    successful_referrals_by_referrer_month: HashMap<(String, i64), u32>,
    lifetime_successful_referrals_by_referrer: HashMap<String, u32>,
    trade_commission_by_referrer_month: HashMap<(String, i64), i64>,
    trade_commission_by_group_month: HashMap<(String, i64), i64>,
    accrued_trade_ids: HashMap<String, CommissionAccrual>,
}

impl AffiliateCommissionEngine {
    /// Records first-touch referral attribution for a newly referred user.
    ///
    /// # Errors
    ///
    /// Returns an error when the referred user was already attributed or when
    /// the record itself is invalid.
    pub fn record_referral(
        &mut self,
        sponsor_user_id: impl Into<String>,
        new_user_id: impl Into<String>,
        referral_code: impl Into<String>,
        referred_at_day: i64,
    ) -> Result<&ReferralRecord, ReferralError> {
        let record =
            ReferralRecord::pending(sponsor_user_id, new_user_id, referral_code, referred_at_day)?;

        if self
            .referrals_by_referred_id
            .contains_key(&record.referred_id)
        {
            return Err(ReferralError::ReferredUserAlreadyAttributed {
                referred_id: record.referred_id,
            });
        }

        let key = record.referred_id.clone();
        self.referrals_by_referred_id.insert(key.clone(), record);
        self.referrals_by_referred_id
            .get(&key)
            .ok_or(ReferralError::InternalState)
    }

    /// Applies the SPEC-010 `affiliate_kinship_link` score to a referral.
    ///
    /// # Errors
    ///
    /// Returns [`ReferralError::ReferralNotFound`] when the referred user has no
    /// first-touch referral record.
    pub fn flag_affiliate_kinship_link(
        &mut self,
        referred_id: &str,
        signals: AffiliateKinshipSignals,
    ) -> Result<Option<u16>, ReferralError> {
        let score = signals.score();
        if !signals.requires_review() {
            return Ok(None);
        }

        let record = self
            .referrals_by_referred_id
            .get_mut(referred_id)
            .ok_or_else(|| ReferralError::ReferralNotFound {
                referred_id: referred_id.to_owned(),
            })?;
        record.mark_pending_review(score);
        Ok(Some(score))
    }

    /// Marks a referral as qualified and returns the one-time bonus ledger draft.
    ///
    /// # Errors
    ///
    /// Returns an error when no referral exists, eligibility checks fail, or
    /// anti-abuse flags block payout.
    pub fn qualify_referral(
        &mut self,
        event: &QualificationEvent,
    ) -> Result<LedgerTransactionDraft, ReferralError> {
        if event.anti_abuse.is_blocked() {
            return Err(ReferralError::AntiAbuseBlocked);
        }

        let referrer_id = {
            let record = self
                .referrals_by_referred_id
                .get(&event.referred_id)
                .ok_or_else(|| ReferralError::ReferralNotFound {
                    referred_id: event.referred_id.clone(),
                })?;

            if record.status == ReferralStatus::Qualified {
                return Err(ReferralError::AlreadyQualified);
            }
            if record.status == ReferralStatus::PendingReview && !event.kinship_review_passed {
                return Err(ReferralError::KinshipReviewPending);
            }

            let elapsed_days = event.observed_at_day - record.referred_at_day;
            if elapsed_days < QUALIFICATION_WINDOW_DAYS {
                return Err(ReferralError::ObservationWindowOpen);
            }

            let within_window = elapsed_days <= QUALIFICATION_WINDOW_DAYS;
            let criteria_met = event.deposit_minor_units >= QUALIFICATION_DEPOSIT_MINOR_UNITS
                && event.kyc_tier >= QUALIFICATION_MIN_KYC_TIER
                && event.completed_trade_count >= QUALIFICATION_MIN_TRADE_COUNT
                && within_window;

            if !criteria_met {
                return Err(ReferralError::QualificationCriteriaNotMet);
            }

            record.referrer_id.clone()
        };

        if self
            .ensure_referrer_signup_cap_allows_payout(&referrer_id, event.observed_at_day)
            .is_err()
        {
            let record = self
                .referrals_by_referred_id
                .get_mut(&event.referred_id)
                .ok_or(ReferralError::InternalState)?;
            record.mark_pending_edd_review("signup_referral_cap_exceeded");
            return Err(ReferralError::ManualReviewRequired { referrer_id });
        }
        let record = self
            .referrals_by_referred_id
            .get_mut(&event.referred_id)
            .ok_or(ReferralError::InternalState)?;
        record.status = ReferralStatus::Qualified;
        record.qualification_evaluated_at_day = Some(event.observed_at_day);
        record.qualified_at_day = Some(event.observed_at_day);
        self.record_successful_referral(&referrer_id, event.observed_at_day);

        Ok(Self::signup_bonus_draft(&referrer_id, &event.referred_id))
    }

    /// Marks a referral as disqualified without emitting any ledger draft.
    ///
    /// # Errors
    ///
    /// Returns [`ReferralError::ReferralNotFound`] when the referred user has no
    /// referral record.
    pub fn disqualify_referral(
        &mut self,
        referred_id: &str,
        evaluated_at_day: i64,
        reason: impl Into<String>,
    ) -> Result<(), ReferralError> {
        let record = self
            .referrals_by_referred_id
            .get_mut(referred_id)
            .ok_or_else(|| ReferralError::ReferralNotFound {
                referred_id: referred_id.to_owned(),
            })?;
        record.disqualify(evaluated_at_day, reason);
        Ok(())
    }

    /// Calculates the 10% maker+taker fee commission for a qualified referral.
    ///
    /// # Errors
    ///
    /// Returns an error when the referral is missing, not qualified, blocked by
    /// anti-abuse rules, outside the 12-month window, or produces no payable amount.
    pub fn calculate_trade_commission(
        &self,
        event: &CommissionEvent,
    ) -> Result<LedgerTransactionDraft, ReferralError> {
        let (referrer_id, commission) = self.trade_commission_parts(event)?;
        Self::player_commission_draft(&referrer_id, commission)
    }

    /// Aggregates trade-derived commissions into one daily payout transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when any event is ineligible or the batch would mix
    /// multiple referrers into one ledger transaction.
    pub fn calculate_daily_trade_commission_batch(
        &self,
        events: &[CommissionEvent],
    ) -> Result<LedgerTransactionDraft, ReferralError> {
        let mut batch_referrer_id: Option<String> = None;
        let mut total_commission = 0_i64;

        for event in events {
            let (referrer_id, commission) = self.trade_commission_parts(event)?;
            if let Some(existing_referrer_id) = &batch_referrer_id {
                if existing_referrer_id != &referrer_id {
                    return Err(ReferralError::MixedReferrerBatch);
                }
            } else {
                batch_referrer_id = Some(referrer_id);
            }
            total_commission += commission;
        }

        let referrer_id = batch_referrer_id.ok_or(ReferralError::NonPositiveAmount)?;
        Self::player_commission_draft(&referrer_id, total_commission)
    }

    /// Creates an application-level per-trade commission accrual.
    ///
    /// # Errors
    ///
    /// Returns an error when the trade was already accrued, the referral is not
    /// eligible, or the referrer/group monthly cap requires EDD review.
    pub fn accrue_trade_commission(
        &mut self,
        event: &CommissionEvent,
    ) -> Result<CommissionAccrual, ReferralError> {
        if self.accrued_trade_ids.contains_key(&event.trade_id) {
            return Err(ReferralError::DuplicateTradeAccrual {
                trade_id: event.trade_id.clone(),
            });
        }

        let (referrer_id, commission) = self.trade_commission_parts(event)?;
        self.ensure_referrer_trade_cap_allows_payout(
            &referrer_id,
            event.referrer_group_id.as_deref(),
            event.traded_at_day,
            commission,
        )?;
        self.record_trade_commission_cap_usage(
            &referrer_id,
            event.referrer_group_id.as_deref(),
            event.traded_at_day,
            commission,
        );

        let accrual = CommissionAccrual {
            referrer_id,
            referred_id: event.referred_id.clone(),
            trade_id: event.trade_id.clone(),
            fee_amount_minor_units: event.maker_fee_minor_units + event.taker_fee_minor_units,
            commission_amount_minor_units: commission,
            accrued_at_day: event.traded_at_day,
            paid_out_txn_id: None,
            paid_out_at_day: None,
        };
        self.accrued_trade_ids
            .insert(event.trade_id.clone(), accrual.clone());
        Ok(accrual)
    }

    /// Builds the daily ledger draft for unpaid commission accruals.
    ///
    /// # Errors
    ///
    /// Returns an error when the batch is empty, has no unpaid rows, or mixes
    /// multiple referrers.
    pub fn payout_commission_accruals(
        &self,
        accruals: &[CommissionAccrual],
    ) -> Result<LedgerTransactionDraft, ReferralError> {
        let mut batch_referrer_id: Option<&str> = None;
        let mut total_commission = 0_i64;

        for accrual in accruals
            .iter()
            .filter(|accrual| accrual.paid_out_txn_id.is_none())
        {
            if let Some(existing_referrer_id) = batch_referrer_id {
                if existing_referrer_id != accrual.referrer_id {
                    return Err(ReferralError::MixedReferrerBatch);
                }
            } else {
                batch_referrer_id = Some(&accrual.referrer_id);
            }
            total_commission += accrual.commission_amount_minor_units;
        }

        let referrer_id = batch_referrer_id.ok_or(ReferralError::NonPositiveAmount)?;
        Self::player_commission_draft(referrer_id, total_commission)
    }

    fn trade_commission_parts(
        &self,
        event: &CommissionEvent,
    ) -> Result<(String, i64), ReferralError> {
        if event.anti_abuse.is_blocked() {
            return Err(ReferralError::AntiAbuseBlocked);
        }

        let record = self
            .referrals_by_referred_id
            .get(&event.referred_id)
            .ok_or_else(|| ReferralError::ReferralNotFound {
                referred_id: event.referred_id.clone(),
            })?;

        if record.status != ReferralStatus::Qualified {
            return Err(ReferralError::ReferralNotQualified);
        }

        let qualified_at_day = record
            .qualified_at_day
            .ok_or(ReferralError::ReferralNotQualified)?;

        if event.traded_at_day - qualified_at_day > TRADE_COMMISSION_WINDOW_DAYS {
            return Err(ReferralError::CommissionWindowExpired);
        }

        let total_fee = event.maker_fee_minor_units + event.taker_fee_minor_units;
        let commission =
            total_fee * TRADE_COMMISSION_RATE_NUMERATOR / TRADE_COMMISSION_RATE_DENOMINATOR;

        if commission <= 0 {
            return Err(ReferralError::NonPositiveAmount);
        }

        Ok((record.referrer_id.clone(), commission))
    }

    /// Builds the ledger draft for a B2B partner invoice / accounts payable fee.
    ///
    /// # Errors
    ///
    /// Returns [`ReferralError::NonPositiveAmount`] for zero or negative fees.
    pub fn partner_fee_draft(
        partner_id: impl AsRef<str>,
        amount_minor_units: i64,
    ) -> Result<LedgerTransactionDraft, ReferralError> {
        if amount_minor_units <= 0 {
            return Err(ReferralError::NonPositiveAmount);
        }

        Ok(LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Expense.AffiliatePartnerFee",
                LedgerAmount::from_minor_units(amount_minor_units),
            ),
            LedgerEntryDraft::new(
                format!("Liability.AccountsPayable.{}", partner_id.as_ref()),
                LedgerAmount::from_minor_units(-amount_minor_units),
            ),
        ]))
    }

    fn player_commission_draft(
        referrer_id: &str,
        amount_minor_units: i64,
    ) -> Result<LedgerTransactionDraft, ReferralError> {
        if amount_minor_units <= 0 {
            return Err(ReferralError::NonPositiveAmount);
        }

        Ok(LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Expense.AffiliateCommission",
                LedgerAmount::from_minor_units(amount_minor_units),
            ),
            LedgerEntryDraft::new(
                format!("Player.Wallet.{referrer_id}"),
                LedgerAmount::from_minor_units(-amount_minor_units),
            ),
        ]))
    }

    #[allow(clippy::similar_names)]
    fn signup_bonus_draft(referrer_id: &str, referred_id: &str) -> LedgerTransactionDraft {
        LedgerTransactionDraft::new([
            LedgerEntryDraft::new(
                "Expense.AffiliateCommission",
                LedgerAmount::from_minor_units(QUALIFICATION_BONUS_MINOR_UNITS),
            ),
            LedgerEntryDraft::new(
                format!("Player.Wallet.{referrer_id}"),
                LedgerAmount::from_minor_units(-QUALIFICATION_BONUS_MINOR_UNITS),
            ),
            LedgerEntryDraft::new(
                "Expense.AffiliateCommission",
                LedgerAmount::from_minor_units(REFERRED_SIGNUP_BONUS_MINOR_UNITS),
            ),
            LedgerEntryDraft::new(
                format!("Player.Wallet.{referred_id}"),
                LedgerAmount::from_minor_units(-REFERRED_SIGNUP_BONUS_MINOR_UNITS),
            ),
        ])
    }

    fn ensure_referrer_signup_cap_allows_payout(
        &self,
        referrer_id: &str,
        observed_at_day: i64,
    ) -> Result<(), ReferralError> {
        let month_key = (referrer_id.to_owned(), observed_at_day / DAYS_PER_CAP_MONTH);
        let monthly_count = self
            .successful_referrals_by_referrer_month
            .get(&month_key)
            .copied()
            .unwrap_or_default();
        let lifetime_count = self
            .lifetime_successful_referrals_by_referrer
            .get(referrer_id)
            .copied()
            .unwrap_or_default();

        if monthly_count >= MONTHLY_SUCCESSFUL_REFERRAL_CAP
            || lifetime_count >= LIFETIME_SUCCESSFUL_REFERRAL_CAP
        {
            return Err(ReferralError::ManualReviewRequired {
                referrer_id: referrer_id.to_owned(),
            });
        }

        Ok(())
    }

    fn ensure_referrer_trade_cap_allows_payout(
        &self,
        referrer_id: &str,
        group_id: Option<&str>,
        observed_at_day: i64,
        next_commission_minor_units: i64,
    ) -> Result<(), ReferralError> {
        let month = observed_at_day / DAYS_PER_CAP_MONTH;
        let referrer_key = (referrer_id.to_owned(), month);
        let referrer_total = self
            .trade_commission_by_referrer_month
            .get(&referrer_key)
            .copied()
            .unwrap_or_default()
            + next_commission_minor_units;

        if referrer_total > MONTHLY_TRADE_COMMISSION_CAP_MINOR_UNITS {
            return Err(ReferralError::ManualReviewRequired {
                referrer_id: referrer_id.to_owned(),
            });
        }

        if let Some(group_id) = group_id {
            let group_key = (group_id.to_owned(), month);
            let group_total = self
                .trade_commission_by_group_month
                .get(&group_key)
                .copied()
                .unwrap_or_default()
                + next_commission_minor_units;
            if group_total > MONTHLY_TRADE_COMMISSION_CAP_MINOR_UNITS {
                return Err(ReferralError::ManualReviewRequired {
                    referrer_id: referrer_id.to_owned(),
                });
            }
        }

        Ok(())
    }

    fn record_successful_referral(&mut self, referrer_id: &str, observed_at_day: i64) {
        let month_key = (referrer_id.to_owned(), observed_at_day / DAYS_PER_CAP_MONTH);
        *self
            .successful_referrals_by_referrer_month
            .entry(month_key)
            .or_insert(0) += 1;
        *self
            .lifetime_successful_referrals_by_referrer
            .entry(referrer_id.to_owned())
            .or_insert(0) += 1;
    }

    fn record_trade_commission_cap_usage(
        &mut self,
        referrer_id: &str,
        group_id: Option<&str>,
        observed_at_day: i64,
        commission_minor_units: i64,
    ) {
        let month = observed_at_day / DAYS_PER_CAP_MONTH;
        *self
            .trade_commission_by_referrer_month
            .entry((referrer_id.to_owned(), month))
            .or_insert(0) += commission_minor_units;
        if let Some(group_id) = group_id {
            *self
                .trade_commission_by_group_month
                .entry((group_id.to_owned(), month))
                .or_insert(0) += commission_minor_units;
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReferralError {
    #[error("referrer cannot refer themselves")]
    SelfReferral,
    #[error("referral code cannot be empty")]
    InvalidReferralCode,
    #[error("ERROR_CODE_REFERRAL_CODE_COLLISION")]
    ReferralCodeCollision,
    #[error("ERROR_CODE_REFERRAL_CAP_EXCEEDED")]
    ReferralCapExceeded,
    #[error("referred user {referred_id} is already attributed")]
    ReferredUserAlreadyAttributed { referred_id: String },
    #[error("referral for referred user {referred_id} was not found")]
    ReferralNotFound { referred_id: String },
    #[error("referral is already qualified")]
    AlreadyQualified,
    #[error("referral qualification criteria are not met")]
    QualificationCriteriaNotMet,
    #[error("referral is still in the 30-day observation window")]
    ObservationWindowOpen,
    #[error("referrer {referrer_id} requires EDD manual review before more referral payouts")]
    ManualReviewRequired { referrer_id: String },
    #[error("kinship review is still pending")]
    KinshipReviewPending,
    #[error("referral is blocked by anti-abuse rules")]
    AntiAbuseBlocked,
    #[error("referral is not qualified")]
    ReferralNotQualified,
    #[error("commission window has expired")]
    CommissionWindowExpired,
    #[error("commission amount must be positive")]
    NonPositiveAmount,
    #[error("daily affiliate commission batch cannot mix referrers")]
    MixedReferrerBatch,
    #[error("trade {trade_id} already has a commission accrual")]
    DuplicateTradeAccrual { trade_id: String },
    #[error("referral engine internal state is inconsistent")]
    InternalState,
}

impl From<ReferralCodeError> for ReferralError {
    fn from(error: ReferralCodeError) -> Self {
        match error {
            ReferralCodeError::CollisionLimitExceeded => Self::ReferralCodeCollision,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::collections::HashMap;

    use super::{
        AffiliateCommissionAdjustmentReview, AffiliateCommissionEngine, AffiliateCommissionSummary,
        AffiliateKinshipSignals, AffiliatePayoutReview, AffiliateReferralListItem, AntiAbuseFlags,
        CommissionEvent, InMemoryAffiliateDashboardStore, QualificationEvent, ReferralError,
        ReferralRecord, ReferralStatus, router,
    };
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[test]
    fn first_touch_referral_rejects_self_and_duplicate_referred_user() {
        let mut engine = AffiliateCommissionEngine::default();

        let record = engine
            .record_referral("user_referrer", "user_referred", "abc123_X9pK", 10)
            .expect("first referral should be accepted");

        assert_eq!(record.status, ReferralStatus::Pending);
        assert_eq!(record.referrer_id, "user_referrer");
        assert_eq!(record.referred_id, "user_referred");

        let duplicate = engine.record_referral("other_referrer", "user_referred", "zzz9_Q1wE", 11);
        assert_eq!(
            duplicate,
            Err(ReferralError::ReferredUserAlreadyAttributed {
                referred_id: "user_referred".to_owned(),
            })
        );

        let self_referral = engine.record_referral("user_same", "user_same", "same_A1b2", 12);
        assert_eq!(self_referral, Err(ReferralError::SelfReferral));
    }

    #[tokio::test]
    async fn affiliate_dashboard_handler_returns_code_summary_and_flat_referrals() {
        let app = router(sample_dashboard_store());
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/internal/referrals/users/user_referrer/affiliate-dashboard?limit=1")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["referral_code"], json!("a3f9c12be801_X9PK"));
        assert_eq!(
            body["share_url"],
            json!("https://proph.net/?ref=a3f9c12be801_X9PK")
        );
        assert_eq!(body["summary"]["successful_referrals_count"], json!(2));
        assert_eq!(body["summary"]["total_rewards_minor_units"], json!(42_000));
        assert_eq!(body["referrals"].as_array().map(Vec::len), Some(1));
        assert_eq!(body["referrals"][0]["masked_username"], json!("ali*e"));
    }

    #[tokio::test]
    async fn affiliate_referrals_and_commission_summary_are_queryable_separately() {
        let app = router(sample_dashboard_store());

        let referrals = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/internal/referrals/users/user_referrer/referrals?limit=10")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");
        assert_eq!(referrals.status(), StatusCode::OK);
        assert_eq!(
            response_json(referrals).await.as_array().map(Vec::len),
            Some(2)
        );

        let summary = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/internal/referrals/users/user_referrer/commission-summary")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");
        assert_eq!(summary.status(), StatusCode::OK);
        assert_eq!(
            response_json(summary).await["trade_commission_minor_units"],
            json!(22_000)
        );
    }

    #[tokio::test]
    async fn admin_review_queues_return_commission_adjustments_and_payouts() {
        let app = router(sample_dashboard_store());

        let commission_reviews = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/internal/admin/affiliate-commission-adjustments")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");
        assert_eq!(commission_reviews.status(), StatusCode::OK);
        let body = response_json(commission_reviews).await;
        assert_eq!(body.as_array().map(Vec::len), Some(1));
        assert_eq!(body[0]["id"], json!("COM-2026-0514-001"));
        assert_eq!(body[0]["affiliateId"], json!("AFF-001"));
        assert_eq!(body[0]["adjustmentType"], json!("credit"));

        let payout_reviews = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/internal/admin/affiliate-payouts")
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("request should build: {error}")),
            )
            .await
            .expect("request should be handled");
        assert_eq!(payout_reviews.status(), StatusCode::OK);
        let body = response_json(payout_reviews).await;
        assert_eq!(body.as_array().map(Vec::len), Some(1));
        assert_eq!(body[0]["id"], json!("PAY-2026-0514-001"));
        assert_eq!(body[0]["status"], json!("pending"));
    }

    #[test]
    fn qualification_bonus_requires_deposit_kyc_and_five_trades_within_thirty_days() {
        let mut engine = AffiliateCommissionEngine::default();
        engine
            .record_referral("user_referrer", "user_referred", "abc123_X9pK", 10)
            .expect("referral should be accepted");

        let still_observing = engine.qualify_referral(&qualification_event("user_referred", 39, 5));
        assert_eq!(still_observing, Err(ReferralError::ObservationWindowOpen));

        let too_few_trades = engine.qualify_referral(&qualification_event("user_referred", 40, 4));
        assert_eq!(
            too_few_trades,
            Err(ReferralError::QualificationCriteriaNotMet)
        );

        let draft = engine
            .qualify_referral(&qualification_event("user_referred", 40, 5))
            .expect("qualification should issue bonus ledger draft");

        assert_eq!(
            draft.entries()[0].account_code(),
            "Expense.AffiliateCommission"
        );
        assert_eq!(draft.entries()[0].amount().minor_units(), 10_000);
        assert_eq!(
            draft.entries()[1].account_code(),
            "Player.Wallet.user_referrer"
        );
        assert_eq!(draft.entries()[1].amount().minor_units(), -10_000);
        assert_eq!(
            draft.entries()[2].account_code(),
            "Expense.AffiliateCommission"
        );
        assert_eq!(draft.entries()[2].amount().minor_units(), 5_000);
        assert_eq!(
            draft.entries()[3].account_code(),
            "Player.Wallet.user_referred"
        );
        assert_eq!(draft.entries()[3].amount().minor_units(), -5_000);
        draft
            .validate()
            .expect("affiliate commission draft must satisfy ledger invariants");
    }

    #[test]
    fn kinship_link_moves_referral_to_pending_review_until_compliance_passes() {
        let mut engine = AffiliateCommissionEngine::default();
        engine
            .record_referral("user_referrer", "user_referred", "abc123_X9pK", 10)
            .expect("referral should be accepted");

        let score = engine
            .flag_affiliate_kinship_link(
                "user_referred",
                AffiliateKinshipSignals {
                    same_admin_division: true,
                    same_last_name: true,
                    ..AffiliateKinshipSignals::default()
                },
            )
            .expect("kinship flag should be recorded");
        assert_eq!(score, Some(50));

        let pending = engine.qualify_referral(&qualification_event("user_referred", 40, 5));
        assert_eq!(pending, Err(ReferralError::KinshipReviewPending));

        let mut reviewed = qualification_event("user_referred", 40, 5);
        reviewed.kinship_review_passed = true;
        engine
            .qualify_referral(&reviewed)
            .expect("compliance-approved referral should qualify");
    }

    #[test]
    fn qualification_caps_send_referrer_to_manual_review() {
        let mut engine = AffiliateCommissionEngine::default();

        for index in 0..20 {
            let referred_id = format!("user_referred_{index}");
            engine
                .record_referral("user_referrer", referred_id.clone(), "abc123_X9pK", 10)
                .expect("referral should be accepted");
            engine
                .qualify_referral(&qualification_event(&referred_id, 40, 5))
                .expect("monthly quota should still allow payout");
        }

        engine
            .record_referral("user_referrer", "user_referred_21", "abc123_X9pK", 10)
            .expect("referral should be accepted");
        let capped = engine.qualify_referral(&qualification_event("user_referred_21", 40, 5));

        assert_eq!(
            capped,
            Err(ReferralError::ManualReviewRequired {
                referrer_id: "user_referrer".to_owned(),
            })
        );
    }

    #[test]
    fn anti_abuse_flags_block_qualification_and_trade_commission() {
        let mut engine = AffiliateCommissionEngine::default();
        engine
            .record_referral("user_referrer", "user_referred", "abc123_X9pK", 10)
            .expect("referral should be accepted");

        let blocked = engine.qualify_referral(&QualificationEvent {
            referred_id: "user_referred".to_owned(),
            observed_at_day: 20,
            deposit_minor_units: 50_000,
            kyc_tier: 1,
            completed_trade_count: 5,
            anti_abuse: AntiAbuseFlags {
                same_ip: true,
                ..AntiAbuseFlags::default()
            },
            kinship_review_passed: false,
        });
        assert_eq!(blocked, Err(ReferralError::AntiAbuseBlocked));

        engine
            .qualify_referral(&qualification_event("user_referred", 40, 5))
            .expect("clean referral should qualify");

        let commission = engine.calculate_trade_commission(&CommissionEvent {
            referred_id: "user_referred".to_owned(),
            trade_id: "trade_blocked".to_owned(),
            traded_at_day: 22,
            maker_fee_minor_units: 1_000,
            taker_fee_minor_units: 2_000,
            anti_abuse: AntiAbuseFlags {
                referred_self_trade: true,
                ..AntiAbuseFlags::default()
            },
            referrer_group_id: None,
        });
        assert_eq!(commission, Err(ReferralError::AntiAbuseBlocked));
    }

    #[test]
    fn trade_commission_is_ten_percent_within_twelve_month_window() {
        let engine = qualified_engine();

        let draft = engine
            .calculate_trade_commission(&commission_event("trade_1", 100, 1_005, 2_000))
            .expect("trade commission should be paid inside window");

        assert_eq!(
            draft.entries()[0].account_code(),
            "Expense.AffiliateCommission"
        );
        assert_eq!(draft.entries()[0].amount().minor_units(), 300);
        assert_eq!(
            draft.entries()[1].account_code(),
            "Player.Wallet.user_referrer"
        );
        assert_eq!(draft.entries()[1].amount().minor_units(), -300);
        draft
            .validate()
            .expect("trade commission draft must satisfy ledger invariants");

        let expired = engine.calculate_trade_commission(&commission_event(
            "trade_expired",
            500,
            1_000,
            2_000,
        ));
        assert_eq!(expired, Err(ReferralError::CommissionWindowExpired));
    }

    #[test]
    fn daily_trade_commission_batch_aggregates_multiple_trade_fees() {
        let engine = qualified_engine();

        let draft = engine
            .calculate_daily_trade_commission_batch(&[
                commission_event("trade_1", 100, 1_000, 2_000),
                commission_event("trade_2", 100, 3_000, 0),
            ])
            .expect("daily batch should aggregate payable commissions");

        assert_eq!(
            draft.entries()[0].account_code(),
            "Expense.AffiliateCommission"
        );
        assert_eq!(draft.entries()[0].amount().minor_units(), 600);
        assert_eq!(
            draft.entries()[1].account_code(),
            "Player.Wallet.user_referrer"
        );
        assert_eq!(draft.entries()[1].amount().minor_units(), -600);
        draft
            .validate()
            .expect("daily commission batch draft must satisfy ledger invariants");
    }

    #[test]
    fn commission_accruals_are_unique_by_trade_and_pay_out_unpaid_rows() {
        let mut engine = qualified_engine();
        let accrual = engine
            .accrue_trade_commission(&commission_event("trade_unique", 100, 1_000, 2_000))
            .expect("trade should accrue commission");
        assert_eq!(accrual.trade_id, "trade_unique");
        assert_eq!(accrual.commission_amount_minor_units, 300);

        let duplicate =
            engine.accrue_trade_commission(&commission_event("trade_unique", 100, 1_000, 2_000));
        assert_eq!(
            duplicate,
            Err(ReferralError::DuplicateTradeAccrual {
                trade_id: "trade_unique".to_owned(),
            })
        );

        let payout = engine
            .payout_commission_accruals(&[accrual])
            .expect("unpaid accrual should pay out");
        assert_eq!(payout.entries()[0].amount().minor_units(), 300);
        payout
            .validate()
            .expect("accrual payout draft must satisfy ledger invariants");
    }

    #[test]
    fn b2b_partner_fee_uses_accounts_payable_not_player_wallet() {
        let draft = AffiliateCommissionEngine::partner_fee_draft("partner_007", 25_000)
            .expect("partner fee draft should be created");

        assert_eq!(
            draft.entries()[0].account_code(),
            "Expense.AffiliatePartnerFee"
        );
        assert_eq!(
            draft.entries()[1].account_code(),
            "Liability.AccountsPayable.partner_007"
        );
        assert_eq!(draft.entries()[1].amount().minor_units(), -25_000);
        draft
            .validate()
            .expect("partner fee must not violate player wallet INV-6");
    }

    fn qualified_engine() -> AffiliateCommissionEngine {
        let mut engine = AffiliateCommissionEngine::default();
        engine
            .record_referral("user_referrer", "user_referred", "abc123_X9pK", 10)
            .expect("referral should be accepted");
        engine
            .qualify_referral(&qualification_event("user_referred", 40, 5))
            .expect("referral should qualify");
        engine
    }

    fn qualification_event(
        referred_id: &str,
        observed_at_day: i64,
        trades: u32,
    ) -> QualificationEvent {
        QualificationEvent {
            referred_id: referred_id.to_owned(),
            observed_at_day,
            deposit_minor_units: 50_000,
            kyc_tier: 1,
            completed_trade_count: trades,
            anti_abuse: AntiAbuseFlags::default(),
            kinship_review_passed: false,
        }
    }

    fn commission_event(
        trade_id: &str,
        traded_at_day: i64,
        maker_fee_minor_units: i64,
        taker_fee_minor_units: i64,
    ) -> CommissionEvent {
        CommissionEvent {
            referred_id: "user_referred".to_owned(),
            trade_id: trade_id.to_owned(),
            traded_at_day,
            maker_fee_minor_units,
            taker_fee_minor_units,
            anti_abuse: AntiAbuseFlags::default(),
            referrer_group_id: None,
        }
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let collected = response
            .into_body()
            .collect()
            .await
            .unwrap_or_else(|error| panic!("response body should collect: {error}"));
        serde_json::from_slice(&collected.to_bytes())
            .unwrap_or_else(|error| panic!("response should be json: {error}"))
    }

    fn sample_dashboard_store() -> InMemoryAffiliateDashboardStore {
        InMemoryAffiliateDashboardStore::new(
            HashMap::from([("user_referrer".to_owned(), "a3f9c12be801_X9PK".to_owned())]),
            HashMap::from([(
                "user_referrer".to_owned(),
                vec![
                    AffiliateReferralListItem {
                        referred_id: "user_referred_1".to_owned(),
                        masked_username: "ali*e".to_owned(),
                        referred_at_day: 10,
                        status: "qualified".to_owned(),
                        cumulative_commission_minor_units: 30_000,
                    },
                    AffiliateReferralListItem {
                        referred_id: "user_referred_2".to_owned(),
                        masked_username: "bo*".to_owned(),
                        referred_at_day: 20,
                        status: "pending".to_owned(),
                        cumulative_commission_minor_units: 0,
                    },
                ],
            )]),
            HashMap::from([(
                "user_referrer".to_owned(),
                AffiliateCommissionSummary {
                    successful_referrals_count: 2,
                    signup_bonus_minor_units: 20_000,
                    trade_commission_minor_units: 22_000,
                    total_rewards_minor_units: 42_000,
                    last_30d_commission_minor_units: 12_000,
                    last_12m_commission_minor_units: 42_000,
                },
            )]),
        )
        .with_admin_review_queues(
            vec![AffiliateCommissionAdjustmentReview {
                id: "COM-2026-0514-001".to_owned(),
                affiliate_id: "AFF-001".to_owned(),
                adjustment_type: "credit".to_owned(),
                amount_minor: 42_000,
                currency: "USDT".to_owned(),
                reason: "活動轉換補發".to_owned(),
                status: "pending".to_owned(),
                requested_by: "admin_ops".to_owned(),
                requested_at: "2026-05-14 09:20".to_owned(),
            }],
            vec![AffiliatePayoutReview {
                id: "PAY-2026-0514-001".to_owned(),
                affiliate_id: "AFF-001".to_owned(),
                amount_minor: 128_400,
                currency: "USDT".to_owned(),
                requested_at: "2026-05-14 10:05".to_owned(),
                status: "pending".to_owned(),
            }],
        )
    }

    #[test]
    fn referral_record_defaults_to_pending() {
        let record = ReferralRecord::pending("a", "b", "code_A1b2", 1)
            .expect("valid referral record should build");
        assert_eq!(record.status, ReferralStatus::Pending);
        assert_eq!(record.qualified_at_day, None);
    }
}
