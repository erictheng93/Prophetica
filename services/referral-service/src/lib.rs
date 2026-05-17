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
    routing::{get, post},
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualificationEvent {
    pub referred_id: String,
    pub observed_at_day: i64,
    pub deposit_minor_units: i64,
    pub kyc_tier: u8,
    pub completed_trade_count: u32,
    pub anti_abuse: AntiAbuseFlags,
    pub kinship_review_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommissionEvent {
    pub referred_id: String,
    pub trade_id: String,
    pub traded_at_day: i64,
    pub maker_fee_minor_units: i64,
    pub taker_fee_minor_units: i64,
    pub anti_abuse: AntiAbuseFlags,
    pub referrer_group_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommissionAccrual {
    pub referrer_id: String,
    pub referrer_group_id: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CaptureReferralRequest {
    pub referral_code: String,
    pub referred_user_id: String,
    pub referred_at_day: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CaptureReferralResponse {
    pub referrer_id: String,
    pub referred_id: String,
    pub referral_code: String,
    pub referred_at_day: i64,
    pub observation_window_ends_at_day: i64,
    pub status: String,
    pub captured: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QualificationEvaluationResponse {
    pub referred_id: String,
    pub referrer_id: String,
    pub status: String,
    pub qualification_evaluated_at_day: Option<i64>,
    pub qualified_at_day: Option<i64>,
    pub signup_bonus_minor_units: i64,
    pub referred_signup_bonus_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SignupBonusLedgerRequest {
    pub referred_id: String,
    pub signup_bonus_txn_id: i64,
    pub posted_at_day: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignupBonusLedgerResponse {
    pub referred_id: String,
    pub referrer_id: String,
    pub status: String,
    pub signup_bonus_txn_id: i64,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignupBonusLedgerDraftResponse {
    pub idempotency_key: String,
    pub txn_type: String,
    pub correlation_id: String,
    pub description: String,
    pub metadata: serde_json::Value,
    pub entries: Vec<SignupBonusLedgerEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignupBonusLedgerEntry {
    pub account_code: String,
    pub direction: String,
    pub amount_minor_units: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DueQualificationReferral {
    pub referrer_id: String,
    pub referred_id: String,
    pub referral_code: String,
    pub referred_at_day: i64,
    pub observation_window_ends_at_day: i64,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DueQualificationQuery {
    pub observed_at_day: i64,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CommissionPayoutRequest {
    pub referrer_id: String,
    pub paid_out_txn_id: i64,
    pub paid_out_at_day: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommissionPayoutResponse {
    pub referrer_id: String,
    pub paid_out_txn_id: i64,
    pub paid_out_at_day: i64,
    pub accrual_count: usize,
    pub total_commission_minor_units: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommissionPayoutLedgerDraftQuery {
    pub paid_out_at_day: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommissionPayoutLedgerDraftResponse {
    pub idempotency_key: String,
    pub txn_type: String,
    pub correlation_id: String,
    pub description: String,
    pub metadata: serde_json::Value,
    pub entries: Vec<SignupBonusLedgerEntry>,
    pub accrual_count: usize,
    pub total_commission_minor_units: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DueCommissionPayoutQuery {
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DueCommissionPayoutReferrer {
    pub referrer_id: String,
    pub accrual_count: usize,
    pub total_commission_minor_units: i64,
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
    async fn capture_referral(
        &self,
        request: CaptureReferralRequest,
    ) -> Result<(ReferralRecord, bool), ReferralStoreError>;

    async fn evaluate_referral_qualification(
        &self,
        event: QualificationEvent,
    ) -> Result<QualificationEvaluationResponse, ReferralStoreError>;

    async fn list_due_qualification_referrals(
        &self,
        observed_at_day: i64,
        limit: usize,
    ) -> Result<Vec<DueQualificationReferral>, ReferralStoreError>;

    async fn finalize_signup_bonus_ledger(
        &self,
        request: SignupBonusLedgerRequest,
    ) -> Result<SignupBonusLedgerResponse, ReferralStoreError>;

    async fn signup_bonus_ledger_draft(
        &self,
        referred_id: &str,
    ) -> Result<SignupBonusLedgerDraftResponse, ReferralStoreError>;

    async fn accrue_trade_commission(
        &self,
        event: CommissionEvent,
    ) -> Result<CommissionAccrual, ReferralStoreError>;

    async fn mark_commission_accruals_paid(
        &self,
        request: CommissionPayoutRequest,
    ) -> Result<CommissionPayoutResponse, ReferralStoreError>;

    async fn commission_payout_ledger_draft(
        &self,
        referrer_id: &str,
        paid_out_at_day: i64,
    ) -> Result<CommissionPayoutLedgerDraftResponse, ReferralStoreError>;

    async fn list_due_commission_payout_referrers(
        &self,
        limit: usize,
    ) -> Result<Vec<DueCommissionPayoutReferrer>, ReferralStoreError>;

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

#[async_trait]
pub trait QualificationInputProvider {
    async fn qualification_event_for(
        &self,
        due: &DueQualificationReferral,
        observed_at_day: i64,
    ) -> Result<QualificationEvent, ReferralStoreError>;
}

#[async_trait]
pub trait ReferralLedgerPoster {
    async fn post_signup_bonus(
        &self,
        draft: SignupBonusLedgerDraftResponse,
    ) -> Result<i64, ReferralStoreError>;

    async fn post_commission_payout(
        &self,
        draft: CommissionPayoutLedgerDraftResponse,
    ) -> Result<i64, ReferralStoreError>;
}

#[derive(Clone)]
pub struct HttpReferralLedgerPoster {
    client: reqwest::Client,
    base_url: reqwest::Url,
}

#[derive(Debug, Deserialize)]
struct LedgerPostResponse {
    txn_id: i64,
}

impl HttpReferralLedgerPoster {
    /// Builds an HTTP ledger poster for the wallet-ledger service.
    ///
    /// # Errors
    ///
    /// Returns [`ReferralStoreError`] when `base_url` is not a valid URL.
    pub fn new(base_url: &str) -> Result<Self, ReferralStoreError> {
        let base_url = reqwest::Url::parse(base_url)
            .map_err(|error| ReferralStoreError::Invalid(error.to_string()))?;
        Ok(Self {
            client: reqwest::Client::new(),
            base_url,
        })
    }

    async fn post_transaction(
        &self,
        idempotency_key: String,
        body: serde_json::Value,
    ) -> Result<i64, ReferralStoreError> {
        let url = self
            .base_url
            .join("/internal/ledger/transactions")
            .map_err(|error| ReferralStoreError::Invalid(error.to_string()))?;
        let response = self
            .client
            .post(url)
            .header("idempotency-key", idempotency_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ReferralStoreError::Unavailable(format!(
                "wallet-ledger-service returned {status}: {body}"
            )));
        }
        response
            .json::<LedgerPostResponse>()
            .await
            .map(|posted| posted.txn_id)
            .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))
    }
}

#[async_trait]
impl ReferralLedgerPoster for HttpReferralLedgerPoster {
    async fn post_signup_bonus(
        &self,
        draft: SignupBonusLedgerDraftResponse,
    ) -> Result<i64, ReferralStoreError> {
        let idempotency_key = draft.idempotency_key.clone();
        let body = serde_json::to_value(draft)
            .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?;
        self.post_transaction(idempotency_key, body).await
    }

    async fn post_commission_payout(
        &self,
        draft: CommissionPayoutLedgerDraftResponse,
    ) -> Result<i64, ReferralStoreError> {
        let idempotency_key = draft.idempotency_key.clone();
        let body = serde_json::to_value(draft)
            .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?;
        self.post_transaction(idempotency_key, body).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualificationWorkerReport {
    pub observed_at_day: i64,
    pub scanned_count: usize,
    pub qualified_count: usize,
    pub signup_bonus_posted_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionPayoutWorkerReport {
    pub referrer_id: String,
    pub paid_out_at_day: i64,
    pub paid_out_txn_id: i64,
    pub accrual_count: usize,
    pub total_commission_minor_units: i64,
}

pub struct ReferralDailyWorker<S, Q, L> {
    store: S,
    qualification_inputs: Q,
    ledger: L,
}

impl<S, Q, L> ReferralDailyWorker<S, Q, L> {
    #[must_use]
    pub const fn new(store: S, qualification_inputs: Q, ledger: L) -> Self {
        Self {
            store,
            qualification_inputs,
            ledger,
        }
    }
}

impl<S, Q, L> ReferralDailyWorker<S, Q, L>
where
    S: AffiliateDashboardStore + Send + Sync,
    Q: QualificationInputProvider + Send + Sync,
    L: ReferralLedgerPoster + Send + Sync,
{
    /// Evaluates due referrals and posts signup bonuses for newly qualified members.
    ///
    /// # Errors
    ///
    /// Returns [`ReferralStoreError`] when due referrals cannot be loaded, qualification inputs
    /// cannot be produced, qualification persistence fails, or signup bonus ledger posting/finalize
    /// operations fail.
    pub async fn run_qualification_batch(
        &self,
        observed_at_day: i64,
        limit: usize,
    ) -> Result<QualificationWorkerReport, ReferralStoreError> {
        let due = self
            .store
            .list_due_qualification_referrals(observed_at_day, limit)
            .await?;
        let mut qualified_count = 0_usize;
        let mut signup_bonus_posted_count = 0_usize;
        for due_referral in &due {
            let event = self
                .qualification_inputs
                .qualification_event_for(due_referral, observed_at_day)
                .await?;
            let evaluation = self.store.evaluate_referral_qualification(event).await?;
            if evaluation.status == "qualified" && evaluation.signup_bonus_minor_units > 0 {
                qualified_count += 1;
                let draft = self
                    .store
                    .signup_bonus_ledger_draft(&evaluation.referred_id)
                    .await?;
                let txn_id = self.ledger.post_signup_bonus(draft).await?;
                self.store
                    .finalize_signup_bonus_ledger(SignupBonusLedgerRequest {
                        referred_id: evaluation.referred_id,
                        signup_bonus_txn_id: txn_id,
                        posted_at_day: observed_at_day,
                    })
                    .await?;
                signup_bonus_posted_count += 1;
            }
        }
        Ok(QualificationWorkerReport {
            observed_at_day,
            scanned_count: due.len(),
            qualified_count,
            signup_bonus_posted_count,
        })
    }

    /// Posts a commission payout for one referrer and marks its accruals as paid.
    ///
    /// # Errors
    ///
    /// Returns [`ReferralStoreError`] when the payout draft cannot be loaded, the ledger post
    /// fails, or paid accrual persistence fails.
    pub async fn run_commission_payout(
        &self,
        referrer_id: &str,
        paid_out_at_day: i64,
    ) -> Result<CommissionPayoutWorkerReport, ReferralStoreError> {
        let draft = self
            .store
            .commission_payout_ledger_draft(referrer_id, paid_out_at_day)
            .await?;
        let accrual_count = draft.accrual_count;
        let total_commission_minor_units = draft.total_commission_minor_units;
        let txn_id = self.ledger.post_commission_payout(draft).await?;
        let payout = self
            .store
            .mark_commission_accruals_paid(CommissionPayoutRequest {
                referrer_id: referrer_id.to_owned(),
                paid_out_txn_id: txn_id,
                paid_out_at_day,
            })
            .await?;
        Ok(CommissionPayoutWorkerReport {
            referrer_id: payout.referrer_id,
            paid_out_at_day: payout.paid_out_at_day,
            paid_out_txn_id: payout.paid_out_txn_id,
            accrual_count,
            total_commission_minor_units,
        })
    }

    /// Posts commission payouts for referrers currently due for payout.
    ///
    /// # Errors
    ///
    /// Returns [`ReferralStoreError`] when due referrers cannot be listed or any individual payout
    /// fails.
    pub async fn run_due_commission_payouts(
        &self,
        paid_out_at_day: i64,
        limit: usize,
    ) -> Result<Vec<CommissionPayoutWorkerReport>, ReferralStoreError> {
        let due_referrers = self
            .store
            .list_due_commission_payout_referrers(limit)
            .await?;
        let mut reports = Vec::with_capacity(due_referrers.len());
        for due_referrer in due_referrers {
            reports.push(
                self.run_commission_payout(&due_referrer.referrer_id, paid_out_at_day)
                    .await?,
            );
        }
        Ok(reports)
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAffiliateDashboardStore {
    referral_codes_by_user: HashMap<String, String>,
    referrals_by_referrer: HashMap<String, Vec<AffiliateReferralListItem>>,
    captured_referrals_by_referred: Arc<tokio::sync::Mutex<HashMap<String, ReferralRecord>>>,
    commission_accruals_by_trade: Arc<tokio::sync::Mutex<HashMap<String, CommissionAccrual>>>,
    commission_payouts_by_txn: Arc<tokio::sync::Mutex<HashMap<i64, CommissionPayoutResponse>>>,
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
            captured_referrals_by_referred: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            commission_accruals_by_trade: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            commission_payouts_by_txn: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
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
    async fn capture_referral(
        &self,
        request: CaptureReferralRequest,
    ) -> Result<(ReferralRecord, bool), ReferralStoreError> {
        let referral_code = normalize_required(&request.referral_code, "referralCode")?;
        let referred_id = normalize_required(&request.referred_user_id, "referredUserId")?;
        let affiliate_user_id = self
            .referral_codes_by_user
            .iter()
            .find_map(|(user_id, code)| (code == &referral_code).then(|| user_id.clone()))
            .ok_or(ReferralStoreError::NotFound)?;
        let referred_at_day = request.referred_at_day.unwrap_or_else(current_epoch_day);
        let record = ReferralRecord::pending(
            affiliate_user_id,
            referred_id.clone(),
            referral_code,
            referred_at_day,
        )
        .map_err(|error| referral_error(&error))?;
        let mut referrals = self.captured_referrals_by_referred.lock().await;
        if let Some(existing) = referrals.get(&referred_id) {
            return Ok((existing.clone(), false));
        }
        referrals.insert(referred_id, record.clone());
        Ok((record, true))
    }

    async fn evaluate_referral_qualification(
        &self,
        event: QualificationEvent,
    ) -> Result<QualificationEvaluationResponse, ReferralStoreError> {
        let mut referrals = self.captured_referrals_by_referred.lock().await;
        let referrer_id = referrals
            .get(&event.referred_id)
            .ok_or(ReferralStoreError::NotFound)?
            .referrer_id
            .clone();
        let observed_month = event.observed_at_day / DAYS_PER_CAP_MONTH;
        let monthly_qualified_count = referrals
            .values()
            .filter(|record| {
                record.referrer_id == referrer_id
                    && record.status == ReferralStatus::Qualified
                    && record
                        .qualified_at_day
                        .is_some_and(|day| day / DAYS_PER_CAP_MONTH == observed_month)
            })
            .count();
        let lifetime_qualified_count = referrals
            .values()
            .filter(|record| {
                record.referrer_id == referrer_id && record.status == ReferralStatus::Qualified
            })
            .count();
        let record = referrals
            .get_mut(&event.referred_id)
            .ok_or(ReferralStoreError::NotFound)?;
        evaluate_referral_record(
            record,
            &event,
            monthly_qualified_count,
            lifetime_qualified_count,
        )
    }

    async fn list_due_qualification_referrals(
        &self,
        observed_at_day: i64,
        limit: usize,
    ) -> Result<Vec<DueQualificationReferral>, ReferralStoreError> {
        let referrals = self.captured_referrals_by_referred.lock().await;
        let mut due = referrals
            .values()
            .filter(|record| {
                record.status == ReferralStatus::Pending
                    && record.observation_window_ends_at_day <= observed_at_day
            })
            .map(due_qualification_referral)
            .collect::<Vec<_>>();
        due.sort_by(|left, right| left.referred_id.cmp(&right.referred_id));
        due.truncate(limit);
        Ok(due)
    }

    async fn finalize_signup_bonus_ledger(
        &self,
        request: SignupBonusLedgerRequest,
    ) -> Result<SignupBonusLedgerResponse, ReferralStoreError> {
        if request.signup_bonus_txn_id <= 0 {
            return Err(ReferralStoreError::Invalid(
                "signup_bonus_txn_id must be greater than zero".to_owned(),
            ));
        }
        let mut referrals = self.captured_referrals_by_referred.lock().await;
        let record = referrals
            .get_mut(&request.referred_id)
            .ok_or(ReferralStoreError::NotFound)?;
        if record.status != ReferralStatus::Qualified {
            return Err(ReferralStoreError::Invalid(
                "signup bonus ledger can only be finalized for qualified referrals".to_owned(),
            ));
        }
        if let Some(existing_txn_id) = record.signup_bonus_txn_id {
            if existing_txn_id != request.signup_bonus_txn_id {
                return Err(ReferralStoreError::Invalid(
                    "signup bonus ledger transaction already finalized".to_owned(),
                ));
            }
            return Ok(signup_bonus_ledger_response(record, true));
        }
        record.signup_bonus_txn_id = Some(request.signup_bonus_txn_id);
        Ok(signup_bonus_ledger_response(record, false))
    }

    async fn signup_bonus_ledger_draft(
        &self,
        referred_id: &str,
    ) -> Result<SignupBonusLedgerDraftResponse, ReferralStoreError> {
        let referrals = self.captured_referrals_by_referred.lock().await;
        let record = referrals
            .get(referred_id)
            .ok_or(ReferralStoreError::NotFound)?;
        signup_bonus_ledger_draft_response(record)
    }

    async fn accrue_trade_commission(
        &self,
        event: CommissionEvent,
    ) -> Result<CommissionAccrual, ReferralStoreError> {
        let mut accruals = self.commission_accruals_by_trade.lock().await;
        if let Some(existing) = accruals.get(&event.trade_id) {
            return Ok(existing.clone());
        }
        let referrals = self.captured_referrals_by_referred.lock().await;
        let record = referrals
            .get(&event.referred_id)
            .ok_or(ReferralStoreError::NotFound)?;
        let accrual = commission_accrual_from_event(record, &event)?;
        let month = event.traded_at_day / DAYS_PER_CAP_MONTH;
        let current_month_total = accruals
            .values()
            .filter(|existing| {
                existing.referrer_id == accrual.referrer_id
                    && existing.accrued_at_day / DAYS_PER_CAP_MONTH == month
            })
            .map(|existing| existing.commission_amount_minor_units)
            .sum::<i64>();
        ensure_referrer_monthly_trade_cap(
            current_month_total,
            accrual.commission_amount_minor_units,
        )?;
        if let Some(group_id) = &accrual.referrer_group_id {
            let current_group_month_total = accruals
                .values()
                .filter(|existing| {
                    existing.referrer_group_id.as_ref() == Some(group_id)
                        && existing.accrued_at_day / DAYS_PER_CAP_MONTH == month
                })
                .map(|existing| existing.commission_amount_minor_units)
                .sum::<i64>();
            ensure_monthly_trade_cap(
                current_group_month_total,
                accrual.commission_amount_minor_units,
                "group monthly trade commission cap requires EDD review",
            )?;
        }
        accruals.insert(event.trade_id.clone(), accrual.clone());
        Ok(accrual)
    }

    async fn mark_commission_accruals_paid(
        &self,
        request: CommissionPayoutRequest,
    ) -> Result<CommissionPayoutResponse, ReferralStoreError> {
        if request.paid_out_txn_id <= 0 {
            return Err(ReferralStoreError::Invalid(
                "paidOutTxnId must be positive".to_owned(),
            ));
        }
        let referrer_id = normalize_required(&request.referrer_id, "referrerId")?;
        let mut payouts = self.commission_payouts_by_txn.lock().await;
        if let Some(existing) = payouts.get(&request.paid_out_txn_id) {
            return Ok(existing.clone());
        }
        let mut accruals = self.commission_accruals_by_trade.lock().await;
        let mut accrual_count = 0_usize;
        let mut total_commission_minor_units = 0_i64;
        for accrual in accruals.values_mut().filter(|accrual| {
            accrual.referrer_id == referrer_id && accrual.paid_out_txn_id.is_none()
        }) {
            accrual_count += 1;
            total_commission_minor_units += accrual.commission_amount_minor_units;
            accrual.paid_out_txn_id = Some(request.paid_out_txn_id);
            accrual.paid_out_at_day = Some(request.paid_out_at_day);
        }
        if accrual_count == 0 {
            return Err(ReferralStoreError::NotFound);
        }
        let response = CommissionPayoutResponse {
            referrer_id,
            paid_out_txn_id: request.paid_out_txn_id,
            paid_out_at_day: request.paid_out_at_day,
            accrual_count,
            total_commission_minor_units,
        };
        payouts.insert(request.paid_out_txn_id, response.clone());
        Ok(response)
    }

    async fn commission_payout_ledger_draft(
        &self,
        referrer_id: &str,
        paid_out_at_day: i64,
    ) -> Result<CommissionPayoutLedgerDraftResponse, ReferralStoreError> {
        let referrer_id = normalize_required(referrer_id, "referrerId")?;
        let accruals = self.commission_accruals_by_trade.lock().await;
        let unpaid = accruals
            .values()
            .filter(|accrual| {
                accrual.referrer_id == referrer_id && accrual.paid_out_txn_id.is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        commission_payout_ledger_draft_response(&referrer_id, paid_out_at_day, &unpaid)
    }

    async fn list_due_commission_payout_referrers(
        &self,
        limit: usize,
    ) -> Result<Vec<DueCommissionPayoutReferrer>, ReferralStoreError> {
        let accruals = self.commission_accruals_by_trade.lock().await;
        let mut by_referrer = HashMap::<String, (usize, i64)>::new();
        for accrual in accruals
            .values()
            .filter(|accrual| accrual.paid_out_txn_id.is_none())
        {
            let entry = by_referrer
                .entry(accrual.referrer_id.clone())
                .or_insert((0, 0));
            entry.0 += 1;
            entry.1 += accrual.commission_amount_minor_units;
        }
        let mut due = by_referrer
            .into_iter()
            .map(
                |(referrer_id, (accrual_count, total_commission_minor_units))| {
                    DueCommissionPayoutReferrer {
                        referrer_id,
                        accrual_count,
                        total_commission_minor_units,
                    }
                },
            )
            .collect::<Vec<_>>();
        due.sort_by(|left, right| left.referrer_id.cmp(&right.referrer_id));
        due.truncate(limit);
        Ok(due)
    }

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

    async fn ensure_trade_commission_caps(
        &self,
        accrual: &CommissionAccrual,
        traded_at_day: i64,
    ) -> Result<(), ReferralStoreError> {
        let month_start_day = traded_at_day - (traded_at_day % DAYS_PER_CAP_MONTH);
        let cap_row = sqlx::query(
            r"
            SELECT COALESCE(SUM(commission_amount), 0)::BIGINT AS current_month_total
            FROM commission_accruals
            WHERE referrer_id = $1
              AND accrued_at >= to_timestamp($2 * 86400)
              AND accrued_at < to_timestamp(($2 + $3) * 86400)
            ",
        )
        .bind(&accrual.referrer_id)
        .bind(month_start_day)
        .bind(DAYS_PER_CAP_MONTH)
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_error)?;
        let current_month_total: i64 =
            cap_row.try_get("current_month_total").map_err(sqlx_error)?;
        ensure_referrer_monthly_trade_cap(
            current_month_total,
            accrual.commission_amount_minor_units,
        )?;
        if let Some(group_id) = &accrual.referrer_group_id {
            self.ensure_group_trade_commission_cap(
                group_id,
                month_start_day,
                accrual.commission_amount_minor_units,
            )
            .await?;
        }
        Ok(())
    }

    async fn ensure_group_trade_commission_cap(
        &self,
        group_id: &str,
        month_start_day: i64,
        next_commission_minor_units: i64,
    ) -> Result<(), ReferralStoreError> {
        let group_cap_row = sqlx::query(
            r"
            SELECT COALESCE(SUM(commission_amount), 0)::BIGINT AS current_month_total
            FROM commission_accruals
            WHERE referrer_group_id = $1
              AND accrued_at >= to_timestamp($2 * 86400)
              AND accrued_at < to_timestamp(($2 + $3) * 86400)
            ",
        )
        .bind(group_id)
        .bind(month_start_day)
        .bind(DAYS_PER_CAP_MONTH)
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_error)?;
        let current_group_month_total: i64 = group_cap_row
            .try_get("current_month_total")
            .map_err(sqlx_error)?;
        ensure_monthly_trade_cap(
            current_group_month_total,
            next_commission_minor_units,
            "group monthly trade commission cap requires EDD review",
        )
    }
}

pub fn router<S>(store: S) -> Router
where
    S: AffiliateDashboardStore + Clone + Send + Sync + 'static,
{
    let state = Arc::new(store);
    Router::new()
        .route("/internal/referrals/capture", post(capture_referral::<S>))
        .route(
            "/internal/referrals/qualification-evaluate",
            post(evaluate_referral_qualification::<S>),
        )
        .route(
            "/internal/referrals/qualification-due",
            get(list_due_qualification_referrals::<S>),
        )
        .route(
            "/internal/referrals/signup-bonus-finalize",
            post(finalize_signup_bonus_ledger::<S>),
        )
        .route(
            "/internal/referrals/signup-bonus-ledger-draft/:referred_id",
            get(get_signup_bonus_ledger_draft::<S>),
        )
        .route(
            "/internal/referrals/commission-accruals",
            post(accrue_trade_commission::<S>),
        )
        .route(
            "/internal/referrals/commission-accruals-payout",
            post(mark_commission_accruals_paid::<S>),
        )
        .route(
            "/internal/referrals/commission-accruals-payout-ledger-draft/:referrer_id",
            get(get_commission_payout_ledger_draft::<S>),
        )
        .route(
            "/internal/referrals/commission-accruals-payout-due",
            get(list_due_commission_payout_referrers::<S>),
        )
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

async fn capture_referral<S>(
    State(store): State<Arc<S>>,
    Json(request): Json<CaptureReferralRequest>,
) -> Result<(StatusCode, Json<CaptureReferralResponse>), (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    let (record, captured) = store
        .capture_referral(request)
        .await
        .map_err(|error| api_error(&error))?;
    let status = if captured {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((
        status,
        Json(CaptureReferralResponse::from_record(record, captured)),
    ))
}

async fn evaluate_referral_qualification<S>(
    State(store): State<Arc<S>>,
    Json(event): Json<QualificationEvent>,
) -> Result<Json<QualificationEvaluationResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    store
        .evaluate_referral_qualification(event)
        .await
        .map(Json)
        .map_err(|error| api_error(&error))
}

async fn list_due_qualification_referrals<S>(
    State(store): State<Arc<S>>,
    Query(query): Query<DueQualificationQuery>,
) -> Result<Json<Vec<DueQualificationReferral>>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    store
        .list_due_qualification_referrals(query.observed_at_day, limit)
        .await
        .map(Json)
        .map_err(|error| api_error(&error))
}

async fn finalize_signup_bonus_ledger<S>(
    State(store): State<Arc<S>>,
    Json(request): Json<SignupBonusLedgerRequest>,
) -> Result<Json<SignupBonusLedgerResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    store
        .finalize_signup_bonus_ledger(request)
        .await
        .map(Json)
        .map_err(|error| api_error(&error))
}

async fn get_signup_bonus_ledger_draft<S>(
    State(store): State<Arc<S>>,
    Path(referred_id): Path<String>,
) -> Result<Json<SignupBonusLedgerDraftResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    store
        .signup_bonus_ledger_draft(&referred_id)
        .await
        .map(Json)
        .map_err(|error| api_error(&error))
}

async fn accrue_trade_commission<S>(
    State(store): State<Arc<S>>,
    Json(event): Json<CommissionEvent>,
) -> Result<Json<CommissionAccrual>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    store
        .accrue_trade_commission(event)
        .await
        .map(Json)
        .map_err(|error| api_error(&error))
}

async fn mark_commission_accruals_paid<S>(
    State(store): State<Arc<S>>,
    Json(request): Json<CommissionPayoutRequest>,
) -> Result<Json<CommissionPayoutResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    store
        .mark_commission_accruals_paid(request)
        .await
        .map(Json)
        .map_err(|error| api_error(&error))
}

async fn get_commission_payout_ledger_draft<S>(
    State(store): State<Arc<S>>,
    Path(referrer_id): Path<String>,
    Query(query): Query<CommissionPayoutLedgerDraftQuery>,
) -> Result<Json<CommissionPayoutLedgerDraftResponse>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    store
        .commission_payout_ledger_draft(&referrer_id, query.paid_out_at_day)
        .await
        .map(Json)
        .map_err(|error| api_error(&error))
}

async fn list_due_commission_payout_referrers<S>(
    State(store): State<Arc<S>>,
    Query(query): Query<DueCommissionPayoutQuery>,
) -> Result<Json<Vec<DueCommissionPayoutReferrer>>, (StatusCode, Json<ApiErrorResponse>)>
where
    S: AffiliateDashboardStore + Send + Sync,
{
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    store
        .list_due_commission_payout_referrers(limit)
        .await
        .map(Json)
        .map_err(|error| api_error(&error))
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

impl CaptureReferralResponse {
    fn from_record(record: ReferralRecord, captured: bool) -> Self {
        Self {
            referrer_id: record.referrer_id,
            referred_id: record.referred_id,
            referral_code: record.referral_code,
            referred_at_day: record.referred_at_day,
            observation_window_ends_at_day: record.observation_window_ends_at_day,
            status: referral_status_as_str(record.status).to_owned(),
            captured,
        }
    }
}

fn referral_status_as_str(status: ReferralStatus) -> &'static str {
    match status {
        ReferralStatus::Pending => "pending",
        ReferralStatus::Qualified => "qualified",
        ReferralStatus::Disqualified => "disqualified",
        ReferralStatus::PendingReview => "pending_review",
        ReferralStatus::PendingEddReview => "pending_eddreview",
    }
}

fn normalize_required(value: &str, field: &str) -> Result<String, ReferralStoreError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ReferralStoreError::Invalid(format!("{field} is required")));
    }
    Ok(trimmed.to_owned())
}

fn current_epoch_day() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs() / 86_400).unwrap_or(0)
        })
}

fn evaluate_referral_record(
    record: &mut ReferralRecord,
    event: &QualificationEvent,
    monthly_qualified_count: usize,
    lifetime_qualified_count: usize,
) -> Result<QualificationEvaluationResponse, ReferralStoreError> {
    if record.status == ReferralStatus::Qualified {
        return Ok(qualification_response(record));
    }
    if record.status == ReferralStatus::PendingReview && !event.kinship_review_passed {
        return Ok(qualification_response(record));
    }
    if event.observed_at_day < record.observation_window_ends_at_day {
        return Err(ReferralStoreError::Invalid(
            "observation window is still open".to_owned(),
        ));
    }
    record.qualification_evaluated_at_day = Some(event.observed_at_day);
    if event.anti_abuse.is_blocked() {
        record.disqualify(event.observed_at_day, "anti_abuse_blocked");
        return Ok(qualification_response(record));
    }
    let criteria_met = event.deposit_minor_units >= QUALIFICATION_DEPOSIT_MINOR_UNITS
        && event.kyc_tier >= QUALIFICATION_MIN_KYC_TIER
        && event.completed_trade_count >= QUALIFICATION_MIN_TRADE_COUNT;
    if !criteria_met {
        record.disqualify(event.observed_at_day, "qualification_criteria_not_met");
        return Ok(qualification_response(record));
    }
    if monthly_qualified_count >= MONTHLY_SUCCESSFUL_REFERRAL_CAP as usize
        || lifetime_qualified_count >= LIFETIME_SUCCESSFUL_REFERRAL_CAP as usize
    {
        record.mark_pending_edd_review("signup_referral_cap_exceeded");
        return Ok(qualification_response(record));
    }
    record.status = ReferralStatus::Qualified;
    record.qualified_at_day = Some(event.observed_at_day);
    Ok(qualification_response(record))
}

fn qualification_response(record: &ReferralRecord) -> QualificationEvaluationResponse {
    let qualified = record.status == ReferralStatus::Qualified;
    QualificationEvaluationResponse {
        referred_id: record.referred_id.clone(),
        referrer_id: record.referrer_id.clone(),
        status: referral_status_as_str(record.status).to_owned(),
        qualification_evaluated_at_day: record.qualification_evaluated_at_day,
        qualified_at_day: record.qualified_at_day,
        signup_bonus_minor_units: if qualified {
            QUALIFICATION_BONUS_MINOR_UNITS
        } else {
            0
        },
        referred_signup_bonus_minor_units: if qualified {
            REFERRED_SIGNUP_BONUS_MINOR_UNITS
        } else {
            0
        },
    }
}

fn due_qualification_referral(record: &ReferralRecord) -> DueQualificationReferral {
    DueQualificationReferral {
        referrer_id: record.referrer_id.clone(),
        referred_id: record.referred_id.clone(),
        referral_code: record.referral_code.clone(),
        referred_at_day: record.referred_at_day,
        observation_window_ends_at_day: record.observation_window_ends_at_day,
        status: referral_status_as_str(record.status).to_owned(),
    }
}

fn signup_bonus_ledger_response(
    record: &ReferralRecord,
    replayed: bool,
) -> SignupBonusLedgerResponse {
    SignupBonusLedgerResponse {
        referred_id: record.referred_id.clone(),
        referrer_id: record.referrer_id.clone(),
        status: referral_status_as_str(record.status).to_owned(),
        signup_bonus_txn_id: record.signup_bonus_txn_id.unwrap_or_default(),
        replayed,
    }
}

fn signup_bonus_ledger_draft_response(
    record: &ReferralRecord,
) -> Result<SignupBonusLedgerDraftResponse, ReferralStoreError> {
    if record.status != ReferralStatus::Qualified {
        return Err(ReferralStoreError::Invalid(
            "signup bonus ledger draft requires a qualified referral".to_owned(),
        ));
    }
    if record.signup_bonus_txn_id.is_some() {
        return Err(ReferralStoreError::Invalid(
            "signup bonus ledger transaction already finalized".to_owned(),
        ));
    }
    let idempotency_key = format!(
        "referral-signup-bonus:{}:{}",
        record.referrer_id, record.referred_id
    );
    Ok(SignupBonusLedgerDraftResponse {
        correlation_id: idempotency_key.clone(),
        idempotency_key,
        txn_type: "referral_signup_bonus".to_owned(),
        description: format!(
            "Referral signup bonus for referrer {} and referred {}",
            record.referrer_id, record.referred_id
        ),
        metadata: serde_json::json!({
            "referrer_id": record.referrer_id,
            "referred_id": record.referred_id,
            "referral_code": record.referral_code,
            "source": "referral-service"
        }),
        entries: vec![
            SignupBonusLedgerEntry {
                account_code: "Expense.AffiliateCommission".to_owned(),
                direction: "debit".to_owned(),
                amount_minor_units: QUALIFICATION_BONUS_MINOR_UNITS,
            },
            SignupBonusLedgerEntry {
                account_code: format!("Player.Wallet.{}", record.referrer_id),
                direction: "credit".to_owned(),
                amount_minor_units: QUALIFICATION_BONUS_MINOR_UNITS,
            },
            SignupBonusLedgerEntry {
                account_code: "Expense.AffiliateCommission".to_owned(),
                direction: "debit".to_owned(),
                amount_minor_units: REFERRED_SIGNUP_BONUS_MINOR_UNITS,
            },
            SignupBonusLedgerEntry {
                account_code: format!("Player.Wallet.{}", record.referred_id),
                direction: "credit".to_owned(),
                amount_minor_units: REFERRED_SIGNUP_BONUS_MINOR_UNITS,
            },
        ],
    })
}

fn commission_payout_ledger_draft_response(
    referrer_id: &str,
    paid_out_at_day: i64,
    accruals: &[CommissionAccrual],
) -> Result<CommissionPayoutLedgerDraftResponse, ReferralStoreError> {
    if accruals.is_empty() {
        return Err(ReferralStoreError::NotFound);
    }
    let total_commission_minor_units = accruals
        .iter()
        .map(|accrual| accrual.commission_amount_minor_units)
        .sum::<i64>();
    if total_commission_minor_units <= 0 {
        return Err(ReferralStoreError::Invalid(
            "commission payout ledger draft requires positive commission".to_owned(),
        ));
    }
    let mut trade_ids = accruals
        .iter()
        .map(|accrual| accrual.trade_id.clone())
        .collect::<Vec<_>>();
    trade_ids.sort();
    let idempotency_key = format!("referral-commission-payout:{referrer_id}:{paid_out_at_day}");
    Ok(CommissionPayoutLedgerDraftResponse {
        correlation_id: idempotency_key.clone(),
        idempotency_key,
        txn_type: "referral_commission_payout".to_owned(),
        description: format!(
            "Referral commission payout for referrer {referrer_id} on day {paid_out_at_day}"
        ),
        metadata: serde_json::json!({
            "referrer_id": referrer_id,
            "paid_out_at_day": paid_out_at_day,
            "trade_ids": trade_ids,
            "source": "referral-service"
        }),
        entries: vec![
            SignupBonusLedgerEntry {
                account_code: "Expense.AffiliateCommission".to_owned(),
                direction: "debit".to_owned(),
                amount_minor_units: total_commission_minor_units,
            },
            SignupBonusLedgerEntry {
                account_code: format!("Player.Wallet.{referrer_id}"),
                direction: "credit".to_owned(),
                amount_minor_units: total_commission_minor_units,
            },
        ],
        accrual_count: accruals.len(),
        total_commission_minor_units,
    })
}

fn commission_accrual_from_event(
    record: &ReferralRecord,
    event: &CommissionEvent,
) -> Result<CommissionAccrual, ReferralStoreError> {
    if event.anti_abuse.is_blocked() {
        return Err(ReferralStoreError::Invalid("anti-abuse blocked".to_owned()));
    }
    if record.status != ReferralStatus::Qualified {
        return Err(ReferralStoreError::Invalid(
            "referral is not qualified".to_owned(),
        ));
    }
    let qualified_at_day = record.qualified_at_day.ok_or_else(|| {
        ReferralStoreError::Invalid("qualified referral is missing qualified_at".to_owned())
    })?;
    if event.traded_at_day - qualified_at_day > TRADE_COMMISSION_WINDOW_DAYS {
        return Err(ReferralStoreError::Invalid(
            "commission window expired".to_owned(),
        ));
    }
    let fee_amount_minor_units = event.maker_fee_minor_units + event.taker_fee_minor_units;
    let commission_amount_minor_units = fee_amount_minor_units * TRADE_COMMISSION_RATE_NUMERATOR
        / TRADE_COMMISSION_RATE_DENOMINATOR;
    if commission_amount_minor_units <= 0 {
        return Err(ReferralStoreError::Invalid(
            "commission amount must be positive".to_owned(),
        ));
    }
    Ok(CommissionAccrual {
        referrer_id: record.referrer_id.clone(),
        referrer_group_id: event.referrer_group_id.clone(),
        referred_id: record.referred_id.clone(),
        trade_id: event.trade_id.clone(),
        fee_amount_minor_units,
        commission_amount_minor_units,
        accrued_at_day: event.traded_at_day,
        paid_out_txn_id: None,
        paid_out_at_day: None,
    })
}

fn ensure_referrer_monthly_trade_cap(
    current_month_total: i64,
    next_commission_minor_units: i64,
) -> Result<(), ReferralStoreError> {
    ensure_monthly_trade_cap(
        current_month_total,
        next_commission_minor_units,
        "monthly trade commission cap requires EDD review",
    )
}

fn ensure_monthly_trade_cap(
    current_month_total: i64,
    next_commission_minor_units: i64,
    message: &'static str,
) -> Result<(), ReferralStoreError> {
    if current_month_total + next_commission_minor_units > MONTHLY_TRADE_COMMISSION_CAP_MINOR_UNITS
    {
        return Err(ReferralStoreError::Invalid(message.to_owned()));
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
    async fn capture_referral(
        &self,
        request: CaptureReferralRequest,
    ) -> Result<(ReferralRecord, bool), ReferralStoreError> {
        let referral_code = normalize_required(&request.referral_code, "referralCode")?;
        let referred_id = normalize_required(&request.referred_user_id, "referredUserId")?;
        let referred_at_day = request.referred_at_day.unwrap_or_else(current_epoch_day);
        let referrer_row = sqlx::query(
            "SELECT user_id FROM affiliate_profiles WHERE affiliate_id = $1 AND status <> 'archived'",
        )
        .bind(&referral_code)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_error)?
        .ok_or(ReferralStoreError::NotFound)?;
        let affiliate_user_id: String = referrer_row.try_get("user_id").map_err(sqlx_error)?;
        let pending = ReferralRecord::pending(
            affiliate_user_id,
            referred_id.clone(),
            referral_code.clone(),
            referred_at_day,
        )
        .map_err(|error| referral_error(&error))?;

        let inserted = sqlx::query(
            r"
            INSERT INTO referrals (
                referrer_id, referred_id, referral_code, referred_at,
                observation_window_ends_at, status, metadata
            )
            VALUES (
                $1, $2, $3, to_timestamp($4 * 86400),
                to_timestamp(($4 + $5) * 86400), 'pending', '{}'::jsonb
            )
            ON CONFLICT (referred_id) DO NOTHING
            RETURNING referrer_id, referred_id, referral_code,
                      FLOOR(EXTRACT(EPOCH FROM referred_at) / 86400)::BIGINT AS referred_at_day,
                      FLOOR(EXTRACT(EPOCH FROM observation_window_ends_at) / 86400)::BIGINT AS observation_window_ends_at_day,
                      FLOOR(EXTRACT(EPOCH FROM qualification_evaluated_at) / 86400)::BIGINT AS qualification_evaluated_at_day,
                      FLOOR(EXTRACT(EPOCH FROM qualified_at) / 86400)::BIGINT AS qualified_at_day,
                      signup_bonus_txn_id, status, metadata
            ",
        )
        .bind(&pending.referrer_id)
        .bind(&pending.referred_id)
        .bind(&pending.referral_code)
        .bind(pending.referred_at_day)
        .bind(QUALIFICATION_WINDOW_DAYS)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_error)?;

        if let Some(row) = inserted {
            return Ok((referral_record_from_row(&row)?, true));
        }

        let row = sqlx::query(
            r"
            SELECT referrer_id, referred_id, referral_code,
                   FLOOR(EXTRACT(EPOCH FROM referred_at) / 86400)::BIGINT AS referred_at_day,
                   FLOOR(EXTRACT(EPOCH FROM observation_window_ends_at) / 86400)::BIGINT AS observation_window_ends_at_day,
                   FLOOR(EXTRACT(EPOCH FROM qualification_evaluated_at) / 86400)::BIGINT AS qualification_evaluated_at_day,
                   FLOOR(EXTRACT(EPOCH FROM qualified_at) / 86400)::BIGINT AS qualified_at_day,
                   signup_bonus_txn_id, status, metadata
            FROM referrals
            WHERE referred_id = $1
            ",
        )
        .bind(&referred_id)
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_error)?;
        Ok((referral_record_from_row(&row)?, false))
    }

    async fn evaluate_referral_qualification(
        &self,
        event: QualificationEvent,
    ) -> Result<QualificationEvaluationResponse, ReferralStoreError> {
        let row = sqlx::query(
            r"
            SELECT referrer_id, referred_id, referral_code,
                   FLOOR(EXTRACT(EPOCH FROM referred_at) / 86400)::BIGINT AS referred_at_day,
                   FLOOR(EXTRACT(EPOCH FROM observation_window_ends_at) / 86400)::BIGINT AS observation_window_ends_at_day,
                   FLOOR(EXTRACT(EPOCH FROM qualification_evaluated_at) / 86400)::BIGINT AS qualification_evaluated_at_day,
                   FLOOR(EXTRACT(EPOCH FROM qualified_at) / 86400)::BIGINT AS qualified_at_day,
                   signup_bonus_txn_id, status, metadata
            FROM referrals
            WHERE referred_id = $1
            FOR UPDATE
            ",
        )
        .bind(&event.referred_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_error)?
        .ok_or(ReferralStoreError::NotFound)?;
        let mut record = referral_record_from_row(&row)?;
        let observed_month_start_day =
            event.observed_at_day - (event.observed_at_day % DAYS_PER_CAP_MONTH);
        let cap_row = sqlx::query(
            r"
            SELECT
                COUNT(*) FILTER (
                    WHERE status = 'qualified'
                      AND qualified_at >= to_timestamp($2 * 86400)
                      AND qualified_at < to_timestamp(($2 + $3) * 86400)
                )::BIGINT AS monthly_qualified_count,
                COUNT(*) FILTER (WHERE status = 'qualified')::BIGINT AS lifetime_qualified_count
            FROM referrals
            WHERE referrer_id = $1
            ",
        )
        .bind(&record.referrer_id)
        .bind(observed_month_start_day)
        .bind(DAYS_PER_CAP_MONTH)
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_error)?;
        let monthly_qualified_count: i64 = cap_row
            .try_get("monthly_qualified_count")
            .map_err(sqlx_error)?;
        let lifetime_qualified_count: i64 = cap_row
            .try_get("lifetime_qualified_count")
            .map_err(sqlx_error)?;
        let response = evaluate_referral_record(
            &mut record,
            &event,
            usize::try_from(monthly_qualified_count)
                .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?,
            usize::try_from(lifetime_qualified_count)
                .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?,
        )?;
        sqlx::query(
            r"
            UPDATE referrals
            SET status = $2,
                qualification_evaluated_at = CASE
                    WHEN $3::BIGINT IS NULL THEN qualification_evaluated_at
                    ELSE to_timestamp($3 * 86400)
                END,
                qualified_at = CASE
                    WHEN $4::BIGINT IS NULL THEN qualified_at
                    ELSE to_timestamp($4 * 86400)
                END,
                metadata = metadata || $5::jsonb
            WHERE referred_id = $1
            ",
        )
        .bind(&record.referred_id)
        .bind(referral_status_as_str(record.status))
        .bind(record.qualification_evaluated_at_day)
        .bind(record.qualified_at_day)
        .bind(serde_json::json!({
            "disqualify_reason": record.disqualify_reason,
            "eddreview_reason": record.eddreview_reason,
            "qualification_evaluator": "referral-service"
        }))
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;
        Ok(response)
    }

    async fn list_due_qualification_referrals(
        &self,
        observed_at_day: i64,
        limit: usize,
    ) -> Result<Vec<DueQualificationReferral>, ReferralStoreError> {
        let rows = sqlx::query(
            r"
            SELECT referrer_id, referred_id, referral_code,
                   FLOOR(EXTRACT(EPOCH FROM referred_at) / 86400)::BIGINT AS referred_at_day,
                   FLOOR(EXTRACT(EPOCH FROM observation_window_ends_at) / 86400)::BIGINT AS observation_window_ends_at_day,
                   status
            FROM referrals
            WHERE status = 'pending'
              AND observation_window_ends_at <= to_timestamp($1 * 86400)
            ORDER BY observation_window_ends_at ASC, referred_id ASC
            LIMIT $2
            ",
        )
        .bind(observed_at_day)
        .bind(
            i64::try_from(limit)
                .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_error)?;
        rows.into_iter()
            .map(|row| {
                Ok(DueQualificationReferral {
                    referrer_id: row.try_get("referrer_id").map_err(sqlx_error)?,
                    referred_id: row.try_get("referred_id").map_err(sqlx_error)?,
                    referral_code: row.try_get("referral_code").map_err(sqlx_error)?,
                    referred_at_day: row.try_get("referred_at_day").map_err(sqlx_error)?,
                    observation_window_ends_at_day: row
                        .try_get("observation_window_ends_at_day")
                        .map_err(sqlx_error)?,
                    status: row.try_get("status").map_err(sqlx_error)?,
                })
            })
            .collect()
    }

    async fn finalize_signup_bonus_ledger(
        &self,
        request: SignupBonusLedgerRequest,
    ) -> Result<SignupBonusLedgerResponse, ReferralStoreError> {
        if request.signup_bonus_txn_id <= 0 {
            return Err(ReferralStoreError::Invalid(
                "signup_bonus_txn_id must be greater than zero".to_owned(),
            ));
        }
        let row = sqlx::query(
            r"
            SELECT referrer_id, referred_id, referral_code,
                   FLOOR(EXTRACT(EPOCH FROM referred_at) / 86400)::BIGINT AS referred_at_day,
                   FLOOR(EXTRACT(EPOCH FROM observation_window_ends_at) / 86400)::BIGINT AS observation_window_ends_at_day,
                   FLOOR(EXTRACT(EPOCH FROM qualification_evaluated_at) / 86400)::BIGINT AS qualification_evaluated_at_day,
                   FLOOR(EXTRACT(EPOCH FROM qualified_at) / 86400)::BIGINT AS qualified_at_day,
                   signup_bonus_txn_id, status, metadata
            FROM referrals
            WHERE referred_id = $1
            FOR UPDATE
            ",
        )
        .bind(&request.referred_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_error)?
        .ok_or(ReferralStoreError::NotFound)?;
        let mut record = referral_record_from_row(&row)?;
        if record.status != ReferralStatus::Qualified {
            return Err(ReferralStoreError::Invalid(
                "signup bonus ledger can only be finalized for qualified referrals".to_owned(),
            ));
        }
        if let Some(existing_txn_id) = record.signup_bonus_txn_id {
            if existing_txn_id != request.signup_bonus_txn_id {
                return Err(ReferralStoreError::Invalid(
                    "signup bonus ledger transaction already finalized".to_owned(),
                ));
            }
            return Ok(signup_bonus_ledger_response(&record, true));
        }
        sqlx::query(
            r"
            UPDATE referrals
            SET signup_bonus_txn_id = $2,
                metadata = metadata || $3::jsonb
            WHERE referred_id = $1
              AND signup_bonus_txn_id IS NULL
            ",
        )
        .bind(&record.referred_id)
        .bind(request.signup_bonus_txn_id)
        .bind(serde_json::json!({
            "signup_bonus_posted_at_day": request.posted_at_day,
            "signup_bonus_ledger_finalizer": "referral-service"
        }))
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;
        record.signup_bonus_txn_id = Some(request.signup_bonus_txn_id);
        Ok(signup_bonus_ledger_response(&record, false))
    }

    async fn signup_bonus_ledger_draft(
        &self,
        referred_id: &str,
    ) -> Result<SignupBonusLedgerDraftResponse, ReferralStoreError> {
        let row = sqlx::query(
            r"
            SELECT referrer_id, referred_id, referral_code,
                   FLOOR(EXTRACT(EPOCH FROM referred_at) / 86400)::BIGINT AS referred_at_day,
                   FLOOR(EXTRACT(EPOCH FROM observation_window_ends_at) / 86400)::BIGINT AS observation_window_ends_at_day,
                   FLOOR(EXTRACT(EPOCH FROM qualification_evaluated_at) / 86400)::BIGINT AS qualification_evaluated_at_day,
                   FLOOR(EXTRACT(EPOCH FROM qualified_at) / 86400)::BIGINT AS qualified_at_day,
                   signup_bonus_txn_id, status, metadata
            FROM referrals
            WHERE referred_id = $1
            ",
        )
        .bind(referred_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_error)?
        .ok_or(ReferralStoreError::NotFound)?;
        let record = referral_record_from_row(&row)?;
        signup_bonus_ledger_draft_response(&record)
    }

    async fn accrue_trade_commission(
        &self,
        event: CommissionEvent,
    ) -> Result<CommissionAccrual, ReferralStoreError> {
        if let Some(row) = sqlx::query(
            r"
            SELECT referrer_id, referrer_group_id, referred_id, trade_id, fee_amount, commission_amount,
                   FLOOR(EXTRACT(EPOCH FROM accrued_at) / 86400)::BIGINT AS accrued_at_day,
                   paid_out_txn_id,
                   FLOOR(EXTRACT(EPOCH FROM paid_out_at) / 86400)::BIGINT AS paid_out_at_day
            FROM commission_accruals
            WHERE trade_id = $1
            ",
        )
        .bind(&event.trade_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_error)?
        {
            return commission_accrual_from_row(&row);
        }

        let row = sqlx::query(
            r"
            SELECT referrer_id, referred_id, referral_code,
                   FLOOR(EXTRACT(EPOCH FROM referred_at) / 86400)::BIGINT AS referred_at_day,
                   FLOOR(EXTRACT(EPOCH FROM observation_window_ends_at) / 86400)::BIGINT AS observation_window_ends_at_day,
                   FLOOR(EXTRACT(EPOCH FROM qualification_evaluated_at) / 86400)::BIGINT AS qualification_evaluated_at_day,
                   FLOOR(EXTRACT(EPOCH FROM qualified_at) / 86400)::BIGINT AS qualified_at_day,
                   signup_bonus_txn_id, status, metadata
            FROM referrals
            WHERE referred_id = $1
            ",
        )
        .bind(&event.referred_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_error)?
        .ok_or(ReferralStoreError::NotFound)?;
        let record = referral_record_from_row(&row)?;
        let accrual = commission_accrual_from_event(&record, &event)?;
        self.ensure_trade_commission_caps(&accrual, event.traded_at_day)
            .await?;
        sqlx::query(
            r"
            INSERT INTO commission_accruals (
                referrer_id, referrer_group_id, referred_id, trade_id, fee_amount, commission_amount, accrued_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, to_timestamp($7 * 86400))
            ",
        )
        .bind(&accrual.referrer_id)
        .bind(accrual.referrer_group_id.as_deref())
        .bind(&accrual.referred_id)
        .bind(&accrual.trade_id)
        .bind(accrual.fee_amount_minor_units)
        .bind(accrual.commission_amount_minor_units)
        .bind(accrual.accrued_at_day)
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;
        Ok(accrual)
    }

    async fn mark_commission_accruals_paid(
        &self,
        request: CommissionPayoutRequest,
    ) -> Result<CommissionPayoutResponse, ReferralStoreError> {
        if request.paid_out_txn_id <= 0 {
            return Err(ReferralStoreError::Invalid(
                "paidOutTxnId must be positive".to_owned(),
            ));
        }
        let referrer_id = normalize_required(&request.referrer_id, "referrerId")?;
        let already_paid = sqlx::query(
            r"
            SELECT COUNT(*)::BIGINT AS accrual_count,
                   COALESCE(SUM(commission_amount), 0)::BIGINT AS total_commission_minor_units
            FROM commission_accruals
            WHERE referrer_id = $1 AND paid_out_txn_id = $2
            ",
        )
        .bind(&referrer_id)
        .bind(request.paid_out_txn_id)
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_error)?;
        let already_count: i64 = already_paid.try_get("accrual_count").map_err(sqlx_error)?;
        if already_count > 0 {
            return Ok(CommissionPayoutResponse {
                referrer_id,
                paid_out_txn_id: request.paid_out_txn_id,
                paid_out_at_day: request.paid_out_at_day,
                accrual_count: usize::try_from(already_count)
                    .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?,
                total_commission_minor_units: already_paid
                    .try_get("total_commission_minor_units")
                    .map_err(sqlx_error)?,
            });
        }

        let pending = sqlx::query(
            r"
            SELECT COUNT(*)::BIGINT AS accrual_count,
                   COALESCE(SUM(commission_amount), 0)::BIGINT AS total_commission_minor_units
            FROM commission_accruals
            WHERE referrer_id = $1 AND paid_out_txn_id IS NULL
            ",
        )
        .bind(&referrer_id)
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_error)?;
        let pending_count: i64 = pending.try_get("accrual_count").map_err(sqlx_error)?;
        if pending_count == 0 {
            return Err(ReferralStoreError::NotFound);
        }
        sqlx::query(
            r"
            UPDATE commission_accruals
            SET paid_out_txn_id = $2,
                paid_out_at = to_timestamp($3 * 86400)
            WHERE referrer_id = $1 AND paid_out_txn_id IS NULL
            ",
        )
        .bind(&referrer_id)
        .bind(request.paid_out_txn_id)
        .bind(request.paid_out_at_day)
        .execute(&self.pool)
        .await
        .map_err(sqlx_error)?;
        Ok(CommissionPayoutResponse {
            referrer_id,
            paid_out_txn_id: request.paid_out_txn_id,
            paid_out_at_day: request.paid_out_at_day,
            accrual_count: usize::try_from(pending_count)
                .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?,
            total_commission_minor_units: pending
                .try_get("total_commission_minor_units")
                .map_err(sqlx_error)?,
        })
    }

    async fn commission_payout_ledger_draft(
        &self,
        referrer_id: &str,
        paid_out_at_day: i64,
    ) -> Result<CommissionPayoutLedgerDraftResponse, ReferralStoreError> {
        let referrer_id = normalize_required(referrer_id, "referrerId")?;
        let rows = sqlx::query(
            r"
            SELECT referrer_id, referrer_group_id, referred_id, trade_id, fee_amount, commission_amount,
                   FLOOR(EXTRACT(EPOCH FROM accrued_at) / 86400)::BIGINT AS accrued_at_day,
                   paid_out_txn_id,
                   FLOOR(EXTRACT(EPOCH FROM paid_out_at) / 86400)::BIGINT AS paid_out_at_day
            FROM commission_accruals
            WHERE referrer_id = $1
              AND paid_out_txn_id IS NULL
            ORDER BY accrued_at ASC, trade_id ASC
            ",
        )
        .bind(&referrer_id)
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_error)?;
        let accruals = rows
            .iter()
            .map(commission_accrual_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        commission_payout_ledger_draft_response(&referrer_id, paid_out_at_day, &accruals)
    }

    async fn list_due_commission_payout_referrers(
        &self,
        limit: usize,
    ) -> Result<Vec<DueCommissionPayoutReferrer>, ReferralStoreError> {
        let rows = sqlx::query(
            r"
            SELECT referrer_id,
                   COUNT(*)::BIGINT AS accrual_count,
                   COALESCE(SUM(commission_amount), 0)::BIGINT AS total_commission_minor_units
            FROM commission_accruals
            WHERE paid_out_txn_id IS NULL
            GROUP BY referrer_id
            ORDER BY referrer_id ASC
            LIMIT $1
            ",
        )
        .bind(
            i64::try_from(limit)
                .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_error)?;
        rows.into_iter()
            .map(|row| {
                let accrual_count: i64 = row.try_get("accrual_count").map_err(sqlx_error)?;
                Ok(DueCommissionPayoutReferrer {
                    referrer_id: row.try_get("referrer_id").map_err(sqlx_error)?,
                    accrual_count: usize::try_from(accrual_count)
                        .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?,
                    total_commission_minor_units: row
                        .try_get("total_commission_minor_units")
                        .map_err(sqlx_error)?,
                })
            })
            .collect()
    }

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

fn commission_accrual_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<CommissionAccrual, ReferralStoreError> {
    Ok(CommissionAccrual {
        referrer_id: row.try_get("referrer_id").map_err(sqlx_error)?,
        referrer_group_id: row.try_get("referrer_group_id").map_err(sqlx_error)?,
        referred_id: row.try_get("referred_id").map_err(sqlx_error)?,
        trade_id: row.try_get("trade_id").map_err(sqlx_error)?,
        fee_amount_minor_units: row.try_get("fee_amount").map_err(sqlx_error)?,
        commission_amount_minor_units: row.try_get("commission_amount").map_err(sqlx_error)?,
        accrued_at_day: row.try_get("accrued_at_day").map_err(sqlx_error)?,
        paid_out_txn_id: row.try_get("paid_out_txn_id").map_err(sqlx_error)?,
        paid_out_at_day: row.try_get("paid_out_at_day").map_err(sqlx_error)?,
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

fn referral_record_from_row(
    row: &sqlx::postgres::PgRow,
) -> Result<ReferralRecord, ReferralStoreError> {
    let metadata: serde_json::Value = row.try_get("metadata").map_err(sqlx_error)?;
    let status: String = row.try_get("status").map_err(sqlx_error)?;
    Ok(ReferralRecord {
        referrer_id: row.try_get("referrer_id").map_err(sqlx_error)?,
        referred_id: row.try_get("referred_id").map_err(sqlx_error)?,
        referral_code: row.try_get("referral_code").map_err(sqlx_error)?,
        referred_at_day: row.try_get("referred_at_day").map_err(sqlx_error)?,
        observation_window_ends_at_day: row
            .try_get("observation_window_ends_at_day")
            .map_err(sqlx_error)?,
        qualification_evaluated_at_day: row
            .try_get("qualification_evaluated_at_day")
            .map_err(sqlx_error)?,
        qualified_at_day: row.try_get("qualified_at_day").map_err(sqlx_error)?,
        signup_bonus_txn_id: row.try_get("signup_bonus_txn_id").map_err(sqlx_error)?,
        kinship_flagged: metadata
            .get("kinship_flagged")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        kinship_score: metadata
            .get("kinship_score")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u16::try_from(value).ok()),
        disqualify_reason: metadata
            .get("disqualify_reason")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        eddreview_reason: metadata
            .get("eddreview_reason")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        status: referral_status_from_str(&status)?,
    })
}

fn referral_status_from_str(status: &str) -> Result<ReferralStatus, ReferralStoreError> {
    match status {
        "pending" => Ok(ReferralStatus::Pending),
        "qualified" => Ok(ReferralStatus::Qualified),
        "disqualified" => Ok(ReferralStatus::Disqualified),
        "pending_review" => Ok(ReferralStatus::PendingReview),
        "pending_eddreview" => Ok(ReferralStatus::PendingEddReview),
        _ => Err(ReferralStoreError::Unavailable(format!(
            "unknown referral status {status}"
        ))),
    }
}

fn referral_error(error: &ReferralError) -> ReferralStoreError {
    ReferralStoreError::Invalid(error.to_string())
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
            referrer_group_id: event.referrer_group_id.clone(),
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
