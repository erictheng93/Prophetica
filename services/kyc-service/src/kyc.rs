use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KycTier {
    #[serde(rename = "TIER_0")]
    Tier0,
    #[serde(rename = "TIER_1")]
    Tier1,
    #[serde(rename = "TIER_2")]
    Tier2,
    #[serde(rename = "TIER_3")]
    Tier3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KycStatus {
    Pending,
    Passed,
    Rejected,
    Expired,
    Frozen,
}

#[derive(Debug, Clone, Serialize)]
pub struct KycRecord {
    pub id: i64,
    pub user_id: String,
    pub tier: KycTier,
    pub status: KycStatus,
    pub provider: String,
    pub provider_applicant_id: Option<String>,
    pub document_id_hash: Option<String>,
    pub country: Option<String>,
    pub declared_monthly_income: Option<i64>,
    pub submitted_at: String,
    pub reviewed_at: Option<String>,
    pub expires_at: Option<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct StartUpgradeDraft {
    pub user_id: String,
    pub target_tier: KycTier,
    pub idempotency_key: String,
    pub declared_monthly_income: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct StartUpgradeOutcome {
    pub record: KycRecord,
    pub sdk_token: String,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct TierOneVerificationDraft {
    pub user_id: String,
    pub idempotency_key: String,
    pub legal_name: String,
    pub birthday: String,
    pub country: String,
    pub document_id: String,
}

#[derive(Debug, Clone)]
pub struct TierOneVerificationOutcome {
    pub record: KycRecord,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct SumsubWebhookDraft {
    pub applicant_id: String,
    pub external_user_id: String,
    pub review_answer: ReviewAnswer,
    pub kyc_reason: String,
    pub sanction_matches: Vec<String>,
    pub raw_payload_sha256: String,
}

#[derive(Debug, Clone)]
pub struct SumsubWebhookOutcome {
    pub record: KycRecord,
    pub replayed: bool,
}

#[derive(Debug, Clone)]
pub struct AmlEventDraft {
    pub user_id: String,
    pub event_type: String,
    pub amount_minor_units: i64,
    pub ml_score: i32,
    pub rule_flags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AmlEventOutcome {
    pub user_id: String,
    pub requires_compliance_review: bool,
    pub sar_candidate_id: Option<String>,
    pub reason: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct KycMonthlyReport {
    pub month: String,
    pub total_records: usize,
    pub by_tier: HashMap<String, usize>,
    pub by_status: HashMap<String, usize>,
    pub open_sar_candidates: i64,
}

#[derive(Debug, Clone)]
pub struct ScreeningMatchDraft {
    pub user_id: String,
    pub match_type: ScreeningMatchType,
    pub list_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreeningMatchType {
    Sanction,
    Pep,
}

#[derive(Debug, Clone)]
pub struct ScreeningRescanOutcome {
    pub scanned_matches: usize,
    pub frozen_users: Vec<String>,
    pub edd_users: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewAnswer {
    Green,
    Red,
    Yellow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum KycStoreError {
    #[error("kyc store is unavailable")]
    Unavailable,
    #[error("document hash is already associated with another user")]
    DuplicateDocument,
}

#[async_trait]
pub trait KycStore: Clone + Send + Sync + 'static {
    async fn current_record(&self, user_id: &str) -> Result<Option<KycRecord>, KycStoreError>;

    async fn verify_tier_one(
        &self,
        draft: TierOneVerificationDraft,
    ) -> Result<TierOneVerificationOutcome, KycStoreError>;

    async fn apply_sumsub_webhook(
        &self,
        draft: SumsubWebhookDraft,
    ) -> Result<Option<SumsubWebhookOutcome>, KycStoreError>;

    async fn record_aml_event(
        &self,
        draft: AmlEventDraft,
    ) -> Result<AmlEventOutcome, KycStoreError>;

    async fn monthly_report(&self, month: String) -> Result<KycMonthlyReport, KycStoreError>;

    async fn apply_screening_rescan(
        &self,
        matches: Vec<ScreeningMatchDraft>,
    ) -> Result<ScreeningRescanOutcome, KycStoreError>;

    async fn start_upgrade(
        &self,
        draft: StartUpgradeDraft,
    ) -> Result<StartUpgradeOutcome, KycStoreError>;
}

#[derive(Debug, Clone)]
pub struct InMemoryKycStore {
    records: Arc<Mutex<Vec<KycRecord>>>,
    idempotency: Arc<Mutex<HashMap<String, StartUpgradeOutcome>>>,
    tier_one_idempotency: Arc<Mutex<HashMap<String, TierOneVerificationOutcome>>>,
    webhook_idempotency: Arc<Mutex<HashMap<String, SumsubWebhookOutcome>>>,
    sar_counts: Arc<Mutex<HashMap<String, i64>>>,
    next_id: Arc<AtomicI64>,
    document_hash_salt: Arc<str>,
}

impl InMemoryKycStore {
    #[must_use]
    pub fn new(document_hash_salt: impl Into<String>) -> Self {
        Self {
            records: Arc::default(),
            idempotency: Arc::default(),
            tier_one_idempotency: Arc::default(),
            webhook_idempotency: Arc::default(),
            sar_counts: Arc::default(),
            next_id: Arc::default(),
            document_hash_salt: Arc::from(document_hash_salt.into()),
        }
    }
}

impl Default for InMemoryKycStore {
    fn default() -> Self {
        Self::new("dev-only-kyc-document-salt")
    }
}

#[async_trait]
impl KycStore for InMemoryKycStore {
    async fn current_record(&self, user_id: &str) -> Result<Option<KycRecord>, KycStoreError> {
        let records = self
            .records
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?;
        Ok(records
            .iter()
            .filter(|record| record.user_id == user_id)
            .max_by_key(|record| (record.tier.rank(), record.id))
            .cloned())
    }

    async fn verify_tier_one(
        &self,
        draft: TierOneVerificationDraft,
    ) -> Result<TierOneVerificationOutcome, KycStoreError> {
        if let Some(existing) = self
            .tier_one_idempotency
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?
            .get(&draft.idempotency_key)
            .cloned()
        {
            return Ok(TierOneVerificationOutcome {
                replayed: true,
                ..existing
            });
        }

        let document_id_hash =
            hash_document_id(&self.document_hash_salt, &draft.country, &draft.document_id);
        {
            let records = self
                .records
                .lock()
                .map_err(|_| KycStoreError::Unavailable)?;
            if records.iter().any(|record| {
                record.user_id != draft.user_id
                    && record
                        .document_id_hash
                        .as_ref()
                        .is_some_and(|stored| stored == &document_id_hash)
            }) {
                return Err(KycStoreError::DuplicateDocument);
            }
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        let record = KycRecord {
            id,
            user_id: draft.user_id,
            tier: KycTier::Tier1,
            status: KycStatus::Passed,
            provider: provider_for(KycTier::Tier1).to_owned(),
            provider_applicant_id: None,
            document_id_hash: Some(document_id_hash),
            country: Some(draft.country),
            declared_monthly_income: None,
            submitted_at: iso_like_now(),
            reviewed_at: Some(iso_like_now()),
            expires_at: None,
            idempotency_key: draft.idempotency_key.clone(),
        };
        let outcome = TierOneVerificationOutcome {
            record: record.clone(),
            replayed: false,
        };

        self.records
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?
            .push(record);
        self.tier_one_idempotency
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?
            .insert(draft.idempotency_key, outcome.clone());

        Ok(outcome)
    }

    async fn apply_sumsub_webhook(
        &self,
        draft: SumsubWebhookDraft,
    ) -> Result<Option<SumsubWebhookOutcome>, KycStoreError> {
        if let Some(existing) = self
            .webhook_idempotency
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?
            .get(&draft.raw_payload_sha256)
            .cloned()
        {
            return Ok(Some(SumsubWebhookOutcome {
                replayed: true,
                ..existing
            }));
        }

        let mut records = self
            .records
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?;
        let Some(record) = records.iter_mut().find(|record| {
            record.user_id == draft.external_user_id
                && record
                    .provider_applicant_id
                    .as_ref()
                    .is_some_and(|applicant_id| applicant_id == &draft.applicant_id)
        }) else {
            return Ok(None);
        };

        record.status = if draft.sanction_matches.is_empty() {
            match draft.review_answer {
                ReviewAnswer::Green => KycStatus::Passed,
                ReviewAnswer::Red => KycStatus::Frozen,
                ReviewAnswer::Yellow => KycStatus::Pending,
            }
        } else {
            KycStatus::Frozen
        };
        record.reviewed_at = Some(iso_like_now());
        let outcome = SumsubWebhookOutcome {
            record: record.clone(),
            replayed: false,
        };
        drop(records);

        self.webhook_idempotency
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?
            .insert(draft.raw_payload_sha256, outcome.clone());

        Ok(Some(outcome))
    }

    async fn record_aml_event(
        &self,
        draft: AmlEventDraft,
    ) -> Result<AmlEventOutcome, KycStoreError> {
        let reason = aml_review_reason(draft.ml_score, &draft.rule_flags);
        let sar_candidate_id = if reason.is_some() {
            let mut counts = self
                .sar_counts
                .lock()
                .map_err(|_| KycStoreError::Unavailable)?;
            let count = counts.entry(draft.user_id.clone()).or_insert(0);
            *count += 1;
            Some(format!("sar_{}_{}", draft.user_id, count))
        } else {
            None
        };

        Ok(AmlEventOutcome {
            user_id: draft.user_id,
            requires_compliance_review: reason.is_some(),
            sar_candidate_id,
            reason,
        })
    }

    async fn monthly_report(&self, month: String) -> Result<KycMonthlyReport, KycStoreError> {
        let records = self
            .records
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?;
        let mut by_tier = HashMap::new();
        let mut by_status = HashMap::new();
        for record in records.iter() {
            *by_tier.entry(record.tier.as_str().to_owned()).or_insert(0) += 1;
            *by_status
                .entry(record.status.as_str().to_owned())
                .or_insert(0) += 1;
        }
        let total_records = records.len();
        drop(records);

        let open_sar_candidates = self
            .sar_counts
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?
            .values()
            .sum();

        Ok(KycMonthlyReport {
            month,
            total_records,
            by_tier,
            by_status,
            open_sar_candidates,
        })
    }

    async fn apply_screening_rescan(
        &self,
        matches: Vec<ScreeningMatchDraft>,
    ) -> Result<ScreeningRescanOutcome, KycStoreError> {
        let mut frozen_users = Vec::new();
        let mut edd_users = Vec::new();
        let mut records = self
            .records
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?;

        for screening_match in &matches {
            match screening_match.match_type {
                ScreeningMatchType::Sanction => {
                    if let Some(record) = records
                        .iter_mut()
                        .filter(|record| record.user_id == screening_match.user_id)
                        .max_by_key(|record| (record.tier.rank(), record.id))
                    {
                        record.status = KycStatus::Frozen;
                        record.reviewed_at = Some(iso_like_now());
                        frozen_users.push(screening_match.user_id.clone());
                    }
                }
                ScreeningMatchType::Pep => {
                    let already_tier_three = records.iter().any(|record| {
                        record.user_id == screening_match.user_id && record.tier == KycTier::Tier3
                    });
                    if !already_tier_three {
                        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
                        records.push(KycRecord {
                            id,
                            user_id: screening_match.user_id.clone(),
                            tier: KycTier::Tier3,
                            status: KycStatus::Pending,
                            provider: "manual".to_owned(),
                            provider_applicant_id: None,
                            document_id_hash: None,
                            country: None,
                            declared_monthly_income: None,
                            submitted_at: iso_like_now(),
                            reviewed_at: None,
                            expires_at: None,
                            idempotency_key: format!(
                                "screening:pep:{}:{id}",
                                screening_match.user_id
                            ),
                        });
                    }
                    edd_users.push(screening_match.user_id.clone());
                }
            }
        }

        Ok(ScreeningRescanOutcome {
            scanned_matches: matches.len(),
            frozen_users,
            edd_users,
        })
    }

    async fn start_upgrade(
        &self,
        draft: StartUpgradeDraft,
    ) -> Result<StartUpgradeOutcome, KycStoreError> {
        if let Some(existing) = self
            .idempotency
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?
            .get(&draft.idempotency_key)
            .cloned()
        {
            return Ok(StartUpgradeOutcome {
                replayed: true,
                ..existing
            });
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        let now = iso_like_now();
        let record = KycRecord {
            id,
            user_id: draft.user_id,
            tier: draft.target_tier,
            status: KycStatus::Pending,
            provider: provider_for(draft.target_tier).to_owned(),
            provider_applicant_id: Some(format!("applicant_{id}")),
            document_id_hash: None,
            country: None,
            declared_monthly_income: draft.declared_monthly_income,
            submitted_at: now,
            reviewed_at: None,
            expires_at: None,
            idempotency_key: draft.idempotency_key.clone(),
        };
        let sdk_token = format!(
            "sumsub_sandbox_{}_{}_{id}",
            record.user_id,
            record.tier.as_str()
        );
        let outcome = StartUpgradeOutcome {
            record: record.clone(),
            sdk_token,
            replayed: false,
        };

        self.records
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?
            .push(record);
        self.idempotency
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?
            .insert(draft.idempotency_key, outcome.clone());

        Ok(outcome)
    }
}

impl KycTier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tier0 => "TIER_0",
            Self::Tier1 => "TIER_1",
            Self::Tier2 => "TIER_2",
            Self::Tier3 => "TIER_3",
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::Tier0 => 0,
            Self::Tier1 => 1,
            Self::Tier2 => 2,
            Self::Tier3 => 3,
        }
    }
}

impl KycStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Passed => "passed",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
            Self::Frozen => "frozen",
        }
    }
}

fn provider_for(tier: KycTier) -> &'static str {
    match tier {
        KycTier::Tier0 => "system",
        KycTier::Tier1 => "manual",
        KycTier::Tier2 | KycTier::Tier3 => "sumsub",
    }
}

fn aml_review_reason(ml_score: i32, rule_flags: &[String]) -> Option<&'static str> {
    if ml_score > 80 {
        Some("ml_score")
    } else if rule_flags.len() >= 2 {
        Some("rule_flags")
    } else {
        None
    }
}

fn hash_document_id(salt: &str, country: &str, document_id: &str) -> String {
    let normalized = normalize_document_id(country, document_id);
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(normalized.as_bytes());
    hex::encode(hasher.finalize())
}

#[must_use]
pub fn normalize_document_id(country: &str, document_id: &str) -> String {
    let compact = document_id
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '-')
        .collect::<String>();
    if country.eq_ignore_ascii_case("JP") {
        compact
    } else {
        compact.to_ascii_uppercase()
    }
}

#[must_use]
pub fn is_supported_tier_one_document(country: &str, document_id: &str) -> bool {
    let normalized = normalize_document_id(country, document_id);
    match country.to_ascii_uppercase().as_str() {
        "TW" => valid_tw_id(&normalized),
        "JP" => normalized.chars().all(|ch| ch.is_ascii_alphanumeric()) && normalized.len() == 12,
        "KR" => normalized.chars().all(|ch| ch.is_ascii_digit()) && normalized.len() == 13,
        "VN" => normalized.chars().all(|ch| ch.is_ascii_digit()) && normalized.len() == 12,
        "MY" => normalized.chars().all(|ch| ch.is_ascii_digit()) && normalized.len() == 12,
        _ => false,
    }
}

fn valid_tw_id(document_id: &str) -> bool {
    if document_id.len() != 10 {
        return false;
    }
    let mut chars = document_id.chars();
    let Some(letter) = chars.next() else {
        return false;
    };
    let Some(letter_code) = tw_letter_code(letter) else {
        return false;
    };
    let digits = chars.map(|ch| ch.to_digit(10)).collect::<Option<Vec<_>>>();
    let Some(digits) = digits else {
        return false;
    };
    if digits.len() != 9 {
        return false;
    }

    let mut sum = letter_code / 10 + (letter_code % 10) * 9;
    for (digit, weight) in digits.iter().take(8).zip((1_u32..=8).rev()) {
        sum += digit * weight;
    }
    sum += digits[8];
    sum % 10 == 0
}

fn tw_letter_code(letter: char) -> Option<u32> {
    match letter {
        'A' => Some(10),
        'B' => Some(11),
        'C' => Some(12),
        'D' => Some(13),
        'E' => Some(14),
        'F' => Some(15),
        'G' => Some(16),
        'H' => Some(17),
        'I' => Some(34),
        'J' => Some(18),
        'K' => Some(19),
        'L' => Some(20),
        'M' => Some(21),
        'N' => Some(22),
        'O' => Some(35),
        'P' => Some(23),
        'Q' => Some(24),
        'R' => Some(25),
        'S' => Some(26),
        'T' => Some(27),
        'U' => Some(28),
        'V' => Some(29),
        'W' => Some(32),
        'X' => Some(30),
        'Y' => Some(31),
        'Z' => Some(33),
        _ => None,
    }
}

fn iso_like_now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    format!("unix:{seconds}")
}
