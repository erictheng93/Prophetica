use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

/// Audit entry written to `kyc_document_access_log` for every document list or view.
#[derive(Debug, Clone)]
pub struct DocumentAccessEntry {
    pub reviewer_id: String,
    pub reviewer_role: String,
    pub user_id: String,
    pub doc_id: Option<String>,
    /// "list" or "view"
    pub action: String,
    /// "success" or "denied"
    pub result: String,
    pub denial_reason: Option<String>,
    pub request_id: Option<String>,
    pub source_ip_hash: Option<String>,
}

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};

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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PendingMemberCasesQuery {
    pub q: Option<String>,
    pub status: Option<String>,
    pub restriction: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PendingMemberCasesResponse {
    pub items: Vec<PendingMemberCase>,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PendingMemberCase {
    pub member_id: String,
    pub display_name: String,
    pub restriction_reason: String,
    pub contact: PendingMemberContact,
    pub status: String,
    pub kyc_status: String,
    pub affiliate_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PendingMemberContact {
    pub email_masked: String,
    pub phone_masked: String,
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

    async fn tier_one_record(&self, user_id: &str) -> Result<Option<KycRecord>, KycStoreError>;

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

    async fn pending_member_cases(
        &self,
        query: PendingMemberCasesQuery,
    ) -> Result<PendingMemberCasesResponse, KycStoreError>;

    async fn start_upgrade(
        &self,
        draft: StartUpgradeDraft,
    ) -> Result<StartUpgradeOutcome, KycStoreError>;

    /// Resolves the SumSub applicant id for the user's current KYC record.
    async fn applicant_id_for_user(
        &self,
        user_id: &str,
    ) -> Result<Option<String>, KycStoreError>;

    /// Appends one KYC document access audit entry.
    async fn record_document_access(
        &self,
        entry: DocumentAccessEntry,
    ) -> Result<(), KycStoreError>;
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
    pub document_access_log: Arc<Mutex<Vec<DocumentAccessEntry>>>,
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
            document_access_log: Arc::default(),
        }
    }
}

impl Default for InMemoryKycStore {
    fn default() -> Self {
        Self::new("dev-only-kyc-document-salt")
    }
}

#[derive(Debug, Clone)]
pub struct PostgresKycStore {
    pool: PgPool,
    document_hash_salt: Arc<str>,
}

impl PostgresKycStore {
    #[must_use]
    pub fn new(pool: PgPool, document_hash_salt: impl Into<String>) -> Self {
        Self {
            pool,
            document_hash_salt: Arc::from(document_hash_salt.into()),
        }
    }

    #[cfg(test)]
    fn tier_one_record_from_draft(
        id: i64,
        draft: &TierOneVerificationDraft,
        document_hash_salt: &str,
        submitted_at: impl Into<String>,
    ) -> KycRecord {
        let country = draft.country.to_ascii_uppercase();
        KycRecord {
            id,
            user_id: draft.user_id.clone(),
            tier: KycTier::Tier1,
            status: KycStatus::Passed,
            provider: provider_for(KycTier::Tier1).to_owned(),
            provider_applicant_id: None,
            document_id_hash: Some(hash_document_id(
                document_hash_salt,
                &country,
                &draft.document_id,
            )),
            country: Some(country),
            declared_monthly_income: None,
            submitted_at: submitted_at.into(),
            reviewed_at: None,
            expires_at: None,
            idempotency_key: draft.idempotency_key.clone(),
        }
    }

    async fn find_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<KycRecord>, KycStoreError> {
        sqlx::query(RECORD_SELECT_SQL)
            .bind(idempotency_key)
            .fetch_optional(&self.pool)
            .await
            .map_err(sqlx_store_error)?
            .map(|row| row_to_kyc_record(&row))
            .transpose()
    }
}

#[async_trait]
impl KycStore for PostgresKycStore {
    async fn current_record(&self, user_id: &str) -> Result<Option<KycRecord>, KycStoreError> {
        sqlx::query(
            r"
            SELECT id, user_id, tier, status, provider, provider_applicant_id, document_id_hash,
                   country, declared_monthly_income, submitted_at::text AS submitted_at,
                   reviewed_at::text AS reviewed_at, expires_at::text AS expires_at,
                   idempotency_key
            FROM user_kyc_records
            WHERE user_id = $1
            ORDER BY tier DESC, id DESC
            LIMIT 1
            ",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_store_error)?
        .map(|row| row_to_kyc_record(&row))
        .transpose()
    }

    async fn tier_one_record(&self, user_id: &str) -> Result<Option<KycRecord>, KycStoreError> {
        sqlx::query(
            r"
            SELECT id, user_id, tier, status, provider, provider_applicant_id, document_id_hash,
                   country, declared_monthly_income, submitted_at::text AS submitted_at,
                   reviewed_at::text AS reviewed_at, expires_at::text AS expires_at,
                   idempotency_key
            FROM user_kyc_records
            WHERE user_id = $1 AND tier = 1
            ORDER BY id DESC
            LIMIT 1
            ",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_store_error)?
        .map(|row| row_to_kyc_record(&row))
        .transpose()
    }

    async fn verify_tier_one(
        &self,
        draft: TierOneVerificationDraft,
    ) -> Result<TierOneVerificationOutcome, KycStoreError> {
        if let Some(existing) = self.find_by_idempotency_key(&draft.idempotency_key).await? {
            return Ok(TierOneVerificationOutcome {
                record: existing,
                replayed: true,
            });
        }

        let document_hash =
            hash_document_id(&self.document_hash_salt, &draft.country, &draft.document_id);
        let inserted = sqlx::query(
            r"
            INSERT INTO user_kyc_records (
                user_id, tier, status, provider, document_id_hash, country,
                submitted_at, reviewed_at, idempotency_key
            )
            VALUES ($1, 1, 'passed', 'manual', $2, $3, now(), now(), $4)
            RETURNING id, user_id, tier, status, provider, provider_applicant_id, document_id_hash,
                      country, declared_monthly_income, submitted_at::text AS submitted_at,
                      reviewed_at::text AS reviewed_at, expires_at::text AS expires_at,
                      idempotency_key
            ",
        )
        .bind(&draft.user_id)
        .bind(document_hash)
        .bind(draft.country.to_ascii_uppercase())
        .bind(&draft.idempotency_key)
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_store_error)?;

        Ok(TierOneVerificationOutcome {
            record: row_to_kyc_record(&inserted)?,
            replayed: false,
        })
    }

    async fn apply_sumsub_webhook(
        &self,
        draft: SumsubWebhookDraft,
    ) -> Result<Option<SumsubWebhookOutcome>, KycStoreError> {
        if let Some(existing) = sqlx::query(
            r"
            SELECT r.id, r.user_id, r.tier, r.status, r.provider, r.provider_applicant_id,
                   r.document_id_hash, r.country, r.declared_monthly_income,
                   r.submitted_at::text AS submitted_at, r.reviewed_at::text AS reviewed_at,
                   r.expires_at::text AS expires_at, r.idempotency_key
            FROM kyc_webhook_events e
            JOIN user_kyc_records r ON r.id = e.record_id
            WHERE e.raw_payload_sha256 = $1
            LIMIT 1
            ",
        )
        .bind(&draft.raw_payload_sha256)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_store_error)?
        {
            return Ok(Some(SumsubWebhookOutcome {
                record: row_to_kyc_record(&existing)?,
                replayed: true,
            }));
        }

        let status = if draft.sanction_matches.is_empty() {
            match draft.review_answer {
                ReviewAnswer::Green => KycStatus::Passed,
                ReviewAnswer::Red => KycStatus::Frozen,
                ReviewAnswer::Yellow => KycStatus::Pending,
            }
        } else {
            KycStatus::Frozen
        };

        let Some(updated) = sqlx::query(
            r"
            UPDATE user_kyc_records
            SET status = $1, reviewed_at = now(), review_result = $2, sanction_match = $3
            WHERE user_id = $4 AND provider_applicant_id = $5
            RETURNING id, user_id, tier, status, provider, provider_applicant_id, document_id_hash,
                      country, declared_monthly_income, submitted_at::text AS submitted_at,
                      reviewed_at::text AS reviewed_at, expires_at::text AS expires_at,
                      idempotency_key
            ",
        )
        .bind(status.as_str())
        .bind(serde_json::json!({
            "review_answer": review_answer_str(draft.review_answer),
            "kyc_reason": draft.kyc_reason,
        }))
        .bind(serde_json::json!(draft.sanction_matches))
        .bind(&draft.external_user_id)
        .bind(&draft.applicant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_store_error)?
        else {
            return Ok(None);
        };

        let record = row_to_kyc_record(&updated)?;
        sqlx::query(
            r"
            INSERT INTO kyc_webhook_events (
                provider, provider_event_id, raw_payload_sha256, payload, record_id
            )
            VALUES ('sumsub', NULL, $1, $2, $3)
            ON CONFLICT (raw_payload_sha256) DO NOTHING
            ",
        )
        .bind(&draft.raw_payload_sha256)
        .bind(serde_json::json!({
            "applicant_id": draft.applicant_id,
            "external_user_id": draft.external_user_id,
            "review_answer": review_answer_str(draft.review_answer),
        }))
        .bind(record.id)
        .execute(&self.pool)
        .await
        .map_err(sqlx_store_error)?;

        Ok(Some(SumsubWebhookOutcome {
            record,
            replayed: false,
        }))
    }

    async fn record_aml_event(
        &self,
        draft: AmlEventDraft,
    ) -> Result<AmlEventOutcome, KycStoreError> {
        let reason = aml_review_reason(draft.ml_score, &draft.rule_flags);
        let event_id: i64 = sqlx::query(
            r"
            INSERT INTO aml_events (user_id, event_type, amount_minor_units, ml_score, rule_flags)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id
            ",
        )
        .bind(&draft.user_id)
        .bind(&draft.event_type)
        .bind(draft.amount_minor_units)
        .bind(draft.ml_score)
        .bind(serde_json::json!(draft.rule_flags))
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_store_error)?
        .try_get("id")
        .map_err(sqlx_store_error)?;

        let sar_candidate_id = if let Some(reason) = reason {
            let existing_count: i64 = sqlx::query(
                "SELECT COUNT(*)::BIGINT AS count FROM sar_candidates WHERE user_id = $1",
            )
            .bind(&draft.user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(sqlx_store_error)?
            .try_get("count")
            .map_err(sqlx_store_error)?;
            let candidate_key = format!("sar_{}_{}", draft.user_id, existing_count + 1);
            sqlx::query(
                r"
                INSERT INTO sar_candidates (candidate_key, user_id, reason, source_event_id)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (candidate_key) DO NOTHING
                ",
            )
            .bind(&candidate_key)
            .bind(&draft.user_id)
            .bind(reason)
            .bind(event_id)
            .execute(&self.pool)
            .await
            .map_err(sqlx_store_error)?;
            Some(candidate_key)
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
        let total_records: i64 =
            sqlx::query("SELECT COUNT(*)::BIGINT AS count FROM user_kyc_records")
                .fetch_one(&self.pool)
                .await
                .map_err(sqlx_store_error)?
                .try_get("count")
                .map_err(sqlx_store_error)?;
        let tier_rows = sqlx::query(
            "SELECT tier, COUNT(*)::BIGINT AS count FROM user_kyc_records GROUP BY tier",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_store_error)?;
        let status_rows = sqlx::query(
            "SELECT status, COUNT(*)::BIGINT AS count FROM user_kyc_records GROUP BY status",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_store_error)?;
        let open_sar_candidates: i64 = sqlx::query(
            "SELECT COUNT(*)::BIGINT AS count FROM sar_candidates WHERE status = 'needs_review'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_store_error)?
        .try_get("count")
        .map_err(sqlx_store_error)?;

        let mut by_tier = HashMap::new();
        for row in tier_rows {
            let tier: i16 = row.try_get("tier").map_err(sqlx_store_error)?;
            let count: i64 = row.try_get("count").map_err(sqlx_store_error)?;
            by_tier.insert(
                tier_from_db(tier)?.as_str().to_owned(),
                count_to_usize(count)?,
            );
        }
        let mut by_status = HashMap::new();
        for row in status_rows {
            let status: String = row.try_get("status").map_err(sqlx_store_error)?;
            let count: i64 = row.try_get("count").map_err(sqlx_store_error)?;
            by_status.insert(status, count_to_usize(count)?);
        }

        Ok(KycMonthlyReport {
            month,
            total_records: count_to_usize(total_records)?,
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

        for screening_match in &matches {
            match screening_match.match_type {
                ScreeningMatchType::Sanction => {
                    let updated = sqlx::query(
                        r"
                        UPDATE user_kyc_records
                        SET status = 'frozen', reviewed_at = now()
                        WHERE id = (
                            SELECT id FROM user_kyc_records
                            WHERE user_id = $1
                            ORDER BY tier DESC, id DESC
                            LIMIT 1
                        )
                        RETURNING id
                        ",
                    )
                    .bind(&screening_match.user_id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(sqlx_store_error)?;
                    if updated.is_some() {
                        frozen_users.push(screening_match.user_id.clone());
                    }
                    insert_screening_event(&self.pool, screening_match, "freeze").await?;
                }
                ScreeningMatchType::Pep => {
                    let exists: bool = sqlx::query(
                        "SELECT EXISTS(SELECT 1 FROM user_kyc_records WHERE user_id = $1 AND tier = 3) AS exists",
                    )
                    .bind(&screening_match.user_id)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(sqlx_store_error)?
                    .try_get("exists")
                    .map_err(sqlx_store_error)?;
                    if !exists {
                        sqlx::query(
                            r"
                            INSERT INTO user_kyc_records (
                                user_id, tier, status, provider, submitted_at, idempotency_key
                            )
                            VALUES ($1, 3, 'pending', 'manual', now(), $2)
                            ",
                        )
                        .bind(&screening_match.user_id)
                        .bind(format!("screening:pep:{}", screening_match.user_id))
                        .execute(&self.pool)
                        .await
                        .map_err(sqlx_store_error)?;
                    }
                    edd_users.push(screening_match.user_id.clone());
                    insert_screening_event(&self.pool, screening_match, "edd").await?;
                }
            }
        }

        Ok(ScreeningRescanOutcome {
            scanned_matches: matches.len(),
            frozen_users,
            edd_users,
        })
    }

    async fn pending_member_cases(
        &self,
        query: PendingMemberCasesQuery,
    ) -> Result<PendingMemberCasesResponse, KycStoreError> {
        let rows = sqlx::query(
            r"
            SELECT DISTINCT ON (user_id)
                   id, user_id, tier, status, provider, provider_applicant_id, document_id_hash,
                   country, declared_monthly_income, submitted_at::text AS submitted_at,
                   reviewed_at::text AS reviewed_at, expires_at::text AS expires_at,
                   idempotency_key
            FROM user_kyc_records
            WHERE status <> 'passed'
            ORDER BY user_id, tier DESC, id DESC
            ",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(sqlx_store_error)?;
        let records = rows
            .iter()
            .map(row_to_kyc_record)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(paginate_pending_member_cases(records, &query))
    }

    async fn start_upgrade(
        &self,
        draft: StartUpgradeDraft,
    ) -> Result<StartUpgradeOutcome, KycStoreError> {
        if let Some(existing) = self.find_by_idempotency_key(&draft.idempotency_key).await? {
            let sdk_token = format!(
                "sumsub_sandbox_{}_{}_{}",
                existing.user_id,
                existing.tier.as_str(),
                existing.id
            );
            return Ok(StartUpgradeOutcome {
                record: existing,
                sdk_token,
                replayed: true,
            });
        }

        let inserted = sqlx::query(
            r"
            INSERT INTO user_kyc_records (
                user_id, tier, status, provider, provider_applicant_id,
                declared_monthly_income, submitted_at, idempotency_key
            )
            VALUES ($1, $2, 'pending', $3, NULL, $4, now(), $5)
            RETURNING id, user_id, tier, status, provider, provider_applicant_id, document_id_hash,
                      country, declared_monthly_income, submitted_at::text AS submitted_at,
                      reviewed_at::text AS reviewed_at, expires_at::text AS expires_at,
                      idempotency_key
            ",
        )
        .bind(&draft.user_id)
        .bind(i16::from(draft.target_tier.rank()))
        .bind(provider_for(draft.target_tier))
        .bind(draft.declared_monthly_income)
        .bind(&draft.idempotency_key)
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_store_error)?;

        let mut record = row_to_kyc_record(&inserted)?;
        let applicant_id = format!("applicant_{}", record.id);
        let updated = sqlx::query(
            r"
            UPDATE user_kyc_records
            SET provider_applicant_id = $1
            WHERE id = $2
            RETURNING id, user_id, tier, status, provider, provider_applicant_id, document_id_hash,
                      country, declared_monthly_income, submitted_at::text AS submitted_at,
                      reviewed_at::text AS reviewed_at, expires_at::text AS expires_at,
                      idempotency_key
            ",
        )
        .bind(applicant_id)
        .bind(record.id)
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_store_error)?;
        record = row_to_kyc_record(&updated)?;
        let sdk_token = format!(
            "sumsub_sandbox_{}_{}_{}",
            record.user_id,
            record.tier.as_str(),
            record.id
        );

        Ok(StartUpgradeOutcome {
            record,
            sdk_token,
            replayed: false,
        })
    }

    async fn applicant_id_for_user(
        &self,
        user_id: &str,
    ) -> Result<Option<String>, KycStoreError> {
        Ok(self
            .current_record(user_id)
            .await?
            .and_then(|record| record.provider_applicant_id))
    }

    async fn record_document_access(
        &self,
        entry: DocumentAccessEntry,
    ) -> Result<(), KycStoreError> {
        sqlx::query(
            r"
            INSERT INTO kyc_document_access_log (
                reviewer_id, reviewer_role, user_id, doc_id, action, result,
                denial_reason, request_id, source_ip_hash
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ",
        )
        .bind(&entry.reviewer_id)
        .bind(&entry.reviewer_role)
        .bind(&entry.user_id)
        .bind(entry.doc_id.as_deref())
        .bind(&entry.action)
        .bind(&entry.result)
        .bind(entry.denial_reason.as_deref())
        .bind(entry.request_id.as_deref())
        .bind(entry.source_ip_hash.as_deref())
        .execute(&self.pool)
        .await
        .map_err(sqlx_store_error)?;
        Ok(())
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

    async fn tier_one_record(&self, user_id: &str) -> Result<Option<KycRecord>, KycStoreError> {
        let records = self
            .records
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?;
        Ok(records
            .iter()
            .filter(|record| record.user_id == user_id && record.tier == KycTier::Tier1)
            .max_by_key(|record| record.id)
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

    async fn pending_member_cases(
        &self,
        query: PendingMemberCasesQuery,
    ) -> Result<PendingMemberCasesResponse, KycStoreError> {
        let records = self
            .records
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?;
        let mut latest = HashMap::<String, KycRecord>::new();
        for record in records
            .iter()
            .filter(|record| record.status != KycStatus::Passed)
        {
            latest
                .entry(record.user_id.clone())
                .and_modify(|existing| {
                    if (record.tier.rank(), record.id) > (existing.tier.rank(), existing.id) {
                        *existing = record.clone();
                    }
                })
                .or_insert_with(|| record.clone());
        }

        Ok(paginate_pending_member_cases(
            latest.into_values().collect(),
            &query,
        ))
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

    async fn applicant_id_for_user(
        &self,
        user_id: &str,
    ) -> Result<Option<String>, KycStoreError> {
        Ok(self
            .current_record(user_id)
            .await?
            .and_then(|record| record.provider_applicant_id))
    }

    async fn record_document_access(
        &self,
        entry: DocumentAccessEntry,
    ) -> Result<(), KycStoreError> {
        self.document_access_log
            .lock()
            .map_err(|_| KycStoreError::Unavailable)?
            .push(entry);
        Ok(())
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

fn paginate_pending_member_cases(
    records: Vec<KycRecord>,
    query: &PendingMemberCasesQuery,
) -> PendingMemberCasesResponse {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(10).clamp(1, 50);
    let q = query.q.as_deref().map_or("", str::trim).to_lowercase();
    let status = query.status.as_deref().map_or("", str::trim);
    let restriction = query.restriction.as_deref().map_or("", str::trim);
    let mut rows = records
        .into_iter()
        .map(pending_case_from_record)
        .filter(|item| {
            (status.is_empty() || item.status == status)
                && (restriction.is_empty() || item.restriction_reason == restriction)
                && (q.is_empty()
                    || item.member_id.to_lowercase().contains(&q)
                    || item.display_name.to_lowercase().contains(&q)
                    || item.contact.email_masked.to_lowercase().contains(&q)
                    || item.contact.phone_masked.to_lowercase().contains(&q))
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.member_id.cmp(&right.member_id));

    let total = rows.len();
    let start = page.saturating_sub(1) * page_size;
    let items = rows.into_iter().skip(start).take(page_size).collect();

    PendingMemberCasesResponse {
        items,
        page,
        page_size,
        total,
        source: "kyc-service:kyc-store".to_owned(),
    }
}

fn pending_case_from_record(record: KycRecord) -> PendingMemberCase {
    let (restriction_reason, status) = match record.status {
        KycStatus::Pending => ("kyc_pending", "inactive"),
        KycStatus::Rejected => ("kyc_rejected", "restricted"),
        KycStatus::Expired => ("kyc_expired", "inactive"),
        KycStatus::Frozen => ("kyc_frozen", "restricted"),
        KycStatus::Passed => ("none", "active"),
    };

    PendingMemberCase {
        display_name: format!("Member {}", record.user_id),
        member_id: record.user_id,
        restriction_reason: restriction_reason.to_owned(),
        contact: PendingMemberContact {
            email_masked: "not_available".to_owned(),
            phone_masked: "not_available".to_owned(),
        },
        status: status.to_owned(),
        kyc_status: record.status.as_str().to_owned(),
        affiliate_id: None,
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

const RECORD_SELECT_SQL: &str = r"
    SELECT id, user_id, tier, status, provider, provider_applicant_id, document_id_hash,
           country, declared_monthly_income, submitted_at::text AS submitted_at,
           reviewed_at::text AS reviewed_at, expires_at::text AS expires_at,
           idempotency_key
    FROM user_kyc_records
    WHERE idempotency_key = $1
    LIMIT 1
";

fn row_to_kyc_record(row: &sqlx::postgres::PgRow) -> Result<KycRecord, KycStoreError> {
    let tier_value: i16 = row.try_get("tier").map_err(sqlx_store_error)?;
    let status_value: String = row.try_get("status").map_err(sqlx_store_error)?;
    Ok(KycRecord {
        id: row.try_get("id").map_err(sqlx_store_error)?,
        user_id: row.try_get("user_id").map_err(sqlx_store_error)?,
        tier: tier_from_db(tier_value)?,
        status: status_from_db(&status_value)?,
        provider: row.try_get("provider").map_err(sqlx_store_error)?,
        provider_applicant_id: row
            .try_get("provider_applicant_id")
            .map_err(sqlx_store_error)?,
        document_id_hash: row.try_get("document_id_hash").map_err(sqlx_store_error)?,
        country: row.try_get("country").map_err(sqlx_store_error)?,
        declared_monthly_income: row
            .try_get("declared_monthly_income")
            .map_err(sqlx_store_error)?,
        submitted_at: row.try_get("submitted_at").map_err(sqlx_store_error)?,
        reviewed_at: row.try_get("reviewed_at").map_err(sqlx_store_error)?,
        expires_at: row.try_get("expires_at").map_err(sqlx_store_error)?,
        idempotency_key: row.try_get("idempotency_key").map_err(sqlx_store_error)?,
    })
}

fn tier_from_db(value: i16) -> Result<KycTier, KycStoreError> {
    match value {
        0 => Ok(KycTier::Tier0),
        1 => Ok(KycTier::Tier1),
        2 => Ok(KycTier::Tier2),
        3 => Ok(KycTier::Tier3),
        _ => Err(KycStoreError::Unavailable),
    }
}

fn status_from_db(value: &str) -> Result<KycStatus, KycStoreError> {
    match value {
        "pending" => Ok(KycStatus::Pending),
        "passed" => Ok(KycStatus::Passed),
        "rejected" => Ok(KycStatus::Rejected),
        "expired" => Ok(KycStatus::Expired),
        "frozen" => Ok(KycStatus::Frozen),
        _ => Err(KycStoreError::Unavailable),
    }
}

fn count_to_usize(value: i64) -> Result<usize, KycStoreError> {
    usize::try_from(value).map_err(|_| KycStoreError::Unavailable)
}

fn sqlx_store_error(error: sqlx::Error) -> KycStoreError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error
            .constraint()
            .is_some_and(|constraint| constraint == "idx_kyc_document_hash_unique")
        {
            return KycStoreError::DuplicateDocument;
        }
    }
    drop(error);
    KycStoreError::Unavailable
}

async fn insert_screening_event(
    pool: &PgPool,
    screening_match: &ScreeningMatchDraft,
    action: &str,
) -> Result<(), KycStoreError> {
    sqlx::query(
        r"
        INSERT INTO screening_rescan_events (user_id, match_type, list_name, action)
        VALUES ($1, $2, $3, $4)
        ",
    )
    .bind(&screening_match.user_id)
    .bind(match screening_match.match_type {
        ScreeningMatchType::Sanction => "sanction",
        ScreeningMatchType::Pep => "pep",
    })
    .bind(&screening_match.list_name)
    .bind(action)
    .execute(pool)
    .await
    .map_err(sqlx_store_error)?;
    Ok(())
}

fn review_answer_str(review_answer: ReviewAnswer) -> &'static str {
    match review_answer {
        ReviewAnswer::Green => "GREEN",
        ReviewAnswer::Red => "RED",
        ReviewAnswer::Yellow => "YELLOW",
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

#[cfg(test)]
mod postgres_store_contract_tests {
    use super::{
        KycStatus, KycStore, KycTier, PostgresKycStore, TierOneVerificationDraft,
        normalize_document_id,
    };

    fn assert_kyc_store<S: KycStore>() {}

    #[test]
    fn postgres_store_implements_kyc_store_contract() {
        assert_kyc_store::<PostgresKycStore>();
    }

    #[test]
    fn postgres_tier_one_record_draft_matches_in_memory_normalization() {
        let draft = TierOneVerificationDraft {
            user_id: "user_pg_01".to_owned(),
            idempotency_key: "kyc:user_pg_01:tier1:001".to_owned(),
            legal_name: "Postgres User".to_owned(),
            birthday: "1990-01-02".to_owned(),
            country: "tw".to_owned(),
            document_id: " a123456789 ".to_owned(),
        };

        let record = PostgresKycStore::tier_one_record_from_draft(
            42,
            &draft,
            "salt",
            "2026-05-09T00:00:00Z",
        );

        assert_eq!(record.id, 42);
        assert_eq!(record.user_id, "user_pg_01");
        assert_eq!(record.tier, KycTier::Tier1);
        assert_eq!(record.status, KycStatus::Passed);
        assert_eq!(record.provider, "manual");
        assert_eq!(record.country.as_deref(), Some("TW"));
        assert_eq!(record.submitted_at, "2026-05-09T00:00:00Z");
        assert_eq!(record.idempotency_key, "kyc:user_pg_01:tier1:001");
        assert_eq!(normalize_document_id("tw", " a123456789 "), "A123456789");
        assert_ne!(record.document_id_hash.as_deref(), Some("A123456789"));
        assert_eq!(record.document_id_hash.as_deref().map(str::len), Some(64));
    }
}
