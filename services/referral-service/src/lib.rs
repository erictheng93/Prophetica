pub const SERVICE_NAME: &str = "referral-service";
pub const DEFAULT_PORT: u16 = 8092;

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use sdk::{
    ledger::{LedgerAmount, LedgerEntryDraft, LedgerTransactionDraft},
    referral::code::ReferralCodeError,
};
use serde::{Deserialize, Serialize};
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
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReferralStoreError {
    #[error("affiliate data not found")]
    NotFound,
    #[error("affiliate store unavailable: {0}")]
    Unavailable(String),
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryAffiliateDashboardStore {
    referral_codes_by_user: HashMap<String, String>,
    referrals_by_referrer: HashMap<String, Vec<AffiliateReferralListItem>>,
    summaries_by_referrer: HashMap<String, AffiliateCommissionSummary>,
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
        }
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

fn api_error(error: &ReferralStoreError) -> (StatusCode, Json<ApiErrorResponse>) {
    match error {
        ReferralStoreError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(ApiErrorResponse {
                error: "affiliate_not_found",
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
        AffiliateCommissionEngine, AffiliateCommissionSummary, AffiliateKinshipSignals,
        AffiliateReferralListItem, AntiAbuseFlags, CommissionEvent,
        InMemoryAffiliateDashboardStore, QualificationEvent, ReferralError, ReferralRecord,
        ReferralStatus, router,
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
    }

    #[test]
    fn referral_record_defaults_to_pending() {
        let record = ReferralRecord::pending("a", "b", "code_A1b2", 1)
            .expect("valid referral record should build");
        assert_eq!(record.status, ReferralStatus::Pending);
        assert_eq!(record.qualified_at_day, None);
    }
}
