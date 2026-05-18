use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use referral_service::{
    AntiAbuseFlags, DueQualificationReferral, HttpReferralLedgerPoster, PostgresAffiliateStore,
    QualificationEvent, QualificationInputProvider, ReferralDailyWorker, ReferralStoreError,
};
use sdk::{
    observability,
    postgres_migrations::{REFERRAL_SERVICE_MIGRATION_TABLE, run_postgres_migrations},
};
use sqlx::postgres::PgPoolOptions;

const SERVICE_NAME: &str = "referral-worker";
const DEFAULT_BATCH_LIMIT: usize = 500;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    observability::init_json_tracing(SERVICE_NAME);
    let mode = env::var("REFERRAL_WORKER_MODE").unwrap_or_else(|_| "daily-payout".to_owned());
    match mode.as_str() {
        "daily-payout" => run_daily_payout().await,
        "daily-qualification" => run_daily_qualification().await,
        "trade-accrual" => run_trade_accrual().await,
        other => anyhow::bail!("unsupported REFERRAL_WORKER_MODE={other}"),
    }
}

async fn run_trade_accrual() -> anyhow::Result<()> {
    let referral_database_url =
        env::var("REFERRAL_SERVICE_DATABASE_URL").or_else(|_| env::var("DATABASE_URL"))?;
    let ledger_database_url = env::var("WALLET_LEDGER_DATABASE_URL")?;
    let limit = env::var("REFERRAL_TRADE_ACCRUAL_BATCH_LIMIT")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_else(|| i64::try_from(DEFAULT_BATCH_LIMIT).unwrap_or(i64::MAX));

    let referral_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&referral_database_url)
        .await?;
    run_postgres_migrations(
        sqlx::migrate!("../../referral-service/migrations"),
        &referral_pool,
        REFERRAL_SERVICE_MIGRATION_TABLE,
    )
    .await?;
    let ledger_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&ledger_database_url)
        .await?;
    let store = PostgresAffiliateStore::new(referral_pool.clone());
    let candidates = trade_commission_candidates(&referral_pool, &ledger_pool, limit).await?;
    let mut accrued_count = 0_usize;
    for event in candidates {
        referral_service::AffiliateDashboardStore::accrue_trade_commission(&store, event)
            .await
            .map_err(|error| anyhow::anyhow!("trade commission accrual failed: {error}"))?;
        accrued_count += 1;
    }
    tracing::info!(accrued_count, "referral trade accrual worker completed");
    Ok(())
}

async fn run_daily_qualification() -> anyhow::Result<()> {
    let database_url =
        env::var("REFERRAL_SERVICE_DATABASE_URL").or_else(|_| env::var("DATABASE_URL"))?;
    let ledger_url = env::var("WALLET_LEDGER_BASE_URL")?;
    let observed_at_day = env::var("REFERRAL_QUALIFICATION_EPOCH_DAY")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_else(current_epoch_day);
    let limit = env::var("REFERRAL_QUALIFICATION_BATCH_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_BATCH_LIMIT);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    run_postgres_migrations(
        sqlx::migrate!("../../referral-service/migrations"),
        &pool,
        REFERRAL_SERVICE_MIGRATION_TABLE,
    )
    .await?;
    let store = PostgresAffiliateStore::new(pool);
    let ledger = HttpReferralLedgerPoster::new(&ledger_url)
        .map_err(|error| anyhow::anyhow!("invalid WALLET_LEDGER_BASE_URL: {error}"))?;
    let inputs = qualification_inputs_from_env().await?;
    let worker = ReferralDailyWorker::new(store, inputs, ledger);
    let report = worker
        .run_qualification_batch(observed_at_day, limit)
        .await
        .map_err(|error| anyhow::anyhow!("daily qualification worker failed: {error}"))?;

    tracing::info!(
        observed_at_day = report.observed_at_day,
        scanned_count = report.scanned_count,
        qualified_count = report.qualified_count,
        signup_bonus_posted_count = report.signup_bonus_posted_count,
        "referral daily qualification worker completed"
    );
    Ok(())
}

async fn trade_commission_candidates(
    referral_pool: &sqlx::PgPool,
    ledger_pool: &sqlx::PgPool,
    limit: i64,
) -> anyhow::Result<Vec<referral_service::CommissionEvent>> {
    let rows = sqlx::query(
        r"
        SELECT referred_id,
               FLOOR(EXTRACT(EPOCH FROM qualified_at) / 86400)::BIGINT AS qualified_at_day
        FROM referrals
        WHERE status = 'qualified'
        ORDER BY qualified_at ASC, referred_id ASC
        LIMIT $1
        ",
    )
    .bind(limit)
    .fetch_all(referral_pool)
    .await?;
    let mut events = Vec::new();
    for row in rows {
        let referred_id: String = sqlx::Row::try_get(&row, "referred_id")?;
        let qualified_at_day: i64 = sqlx::Row::try_get(&row, "qualified_at_day")?;
        let account_code = format!("Player.Wallet.{referred_id}");
        let ledger_rows = sqlx::query(
            r"
            SELECT t.id AS txn_id,
                   FLOOR(EXTRACT(EPOCH FROM t.posted_at) / 86400)::BIGINT AS traded_at_day,
                   COALESCE(SUM(CASE
                       WHEN fee_account.code IN ('Revenue.TradingFee', 'Revenue.SettlementFee')
                       THEN fee_entry.amount
                       ELSE 0
                   END), 0)::BIGINT AS fee_amount
            FROM ledger_transactions t
            JOIN ledger_entries user_entry ON user_entry.txn_id = t.id
            JOIN ledger_accounts user_account ON user_account.id = user_entry.account_id
            LEFT JOIN ledger_entries fee_entry ON fee_entry.txn_id = t.id
            LEFT JOIN ledger_accounts fee_account ON fee_account.id = fee_entry.account_id
            WHERE t.txn_type IN ('trade', 'settle')
              AND user_account.code = $1
              AND FLOOR(EXTRACT(EPOCH FROM t.posted_at) / 86400)::BIGINT >= $2
            GROUP BY t.id, t.posted_at
            HAVING COALESCE(SUM(CASE
                       WHEN fee_account.code IN ('Revenue.TradingFee', 'Revenue.SettlementFee')
                       THEN fee_entry.amount
                       ELSE 0
                   END), 0) > 0
            ORDER BY t.posted_at ASC, t.id ASC
            LIMIT $3
            ",
        )
        .bind(&account_code)
        .bind(qualified_at_day)
        .bind(limit)
        .fetch_all(ledger_pool)
        .await?;
        for ledger_row in ledger_rows {
            let txn_id: i64 = sqlx::Row::try_get(&ledger_row, "txn_id")?;
            let traded_at_day: i64 = sqlx::Row::try_get(&ledger_row, "traded_at_day")?;
            let fee_amount: i64 = sqlx::Row::try_get(&ledger_row, "fee_amount")?;
            events.push(referral_service::CommissionEvent {
                referred_id: referred_id.clone(),
                trade_id: format!("ledger:{txn_id}"),
                traded_at_day,
                maker_fee_minor_units: fee_amount,
                taker_fee_minor_units: 0,
                anti_abuse: clean_anti_abuse(),
                referrer_group_id: None,
            });
        }
    }
    Ok(events)
}

fn clean_anti_abuse() -> AntiAbuseFlags {
    AntiAbuseFlags {
        same_ip: false,
        same_device_fingerprint: false,
        same_kyc_identity: false,
        referred_self_trade: false,
        chargeback_or_void: false,
        referrer_account_abnormal: false,
        referred_permanently_banned: false,
    }
}

async fn qualification_inputs_from_env() -> anyhow::Result<QualificationInputs> {
    if let Ok(input_base_url) = env::var("REFERRAL_QUALIFICATION_INPUT_BASE_URL") {
        return Ok(QualificationInputs::Http(HttpQualificationInputs::new(
            &input_base_url,
        )?));
    }
    let kyc_database_url = env::var("KYC_SERVICE_DATABASE_URL")?;
    let ledger_database_url = env::var("WALLET_LEDGER_DATABASE_URL")?;
    let kyc_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&kyc_database_url)
        .await?;
    let ledger_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&ledger_database_url)
        .await?;
    Ok(QualificationInputs::Postgres(PostgresQualificationInputs {
        kyc_pool,
        ledger_pool,
    }))
}

async fn run_daily_payout() -> anyhow::Result<()> {
    let database_url =
        env::var("REFERRAL_SERVICE_DATABASE_URL").or_else(|_| env::var("DATABASE_URL"))?;
    let ledger_url = env::var("WALLET_LEDGER_BASE_URL")?;
    let paid_out_at_day = env::var("REFERRAL_PAYOUT_EPOCH_DAY")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_else(current_epoch_day);
    let limit = env::var("REFERRAL_PAYOUT_BATCH_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_BATCH_LIMIT);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    run_postgres_migrations(
        sqlx::migrate!("../../referral-service/migrations"),
        &pool,
        REFERRAL_SERVICE_MIGRATION_TABLE,
    )
    .await?;
    let store = PostgresAffiliateStore::new(pool);
    let ledger = HttpReferralLedgerPoster::new(&ledger_url)
        .map_err(|error| anyhow::anyhow!("invalid WALLET_LEDGER_BASE_URL: {error}"))?;
    let worker = ReferralDailyWorker::new(store, NoopQualificationInputs, ledger);
    let reports = worker
        .run_due_commission_payouts(paid_out_at_day, limit)
        .await
        .map_err(|error| anyhow::anyhow!("daily payout worker failed: {error}"))?;

    tracing::info!(
        paid_out_at_day,
        referrer_count = reports.len(),
        total_commission_minor_units = reports
            .iter()
            .map(|report| report.total_commission_minor_units)
            .sum::<i64>(),
        "referral daily payout worker completed"
    );
    Ok(())
}

fn current_epoch_day() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_secs() / 86_400).unwrap_or(i64::MAX)
        })
}

struct HttpQualificationInputs {
    client: reqwest::Client,
    base_url: reqwest::Url,
}

impl HttpQualificationInputs {
    fn new(base_url: &str) -> anyhow::Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            base_url: reqwest::Url::parse(base_url)?,
        })
    }
}

struct PostgresQualificationInputs {
    kyc_pool: sqlx::PgPool,
    ledger_pool: sqlx::PgPool,
}

enum QualificationInputs {
    Http(HttpQualificationInputs),
    Postgres(PostgresQualificationInputs),
}

#[async_trait]
impl QualificationInputProvider for QualificationInputs {
    async fn qualification_event_for(
        &self,
        due: &DueQualificationReferral,
        observed_at_day: i64,
    ) -> Result<QualificationEvent, ReferralStoreError> {
        match self {
            Self::Http(provider) => provider.qualification_event_for(due, observed_at_day).await,
            Self::Postgres(provider) => {
                provider.qualification_event_for(due, observed_at_day).await
            }
        }
    }
}

#[async_trait]
impl QualificationInputProvider for HttpQualificationInputs {
    async fn qualification_event_for(
        &self,
        due: &DueQualificationReferral,
        observed_at_day: i64,
    ) -> Result<QualificationEvent, ReferralStoreError> {
        let mut url = self
            .base_url
            .join(&format!(
                "/internal/referrals/qualification-inputs/{}",
                due.referred_id
            ))
            .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("observed_at_day", &observed_at_day.to_string())
            .append_pair("referrer_id", &due.referrer_id);
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ReferralStoreError::Unavailable(format!(
                "qualification input service returned {status}: {body}"
            )));
        }
        response
            .json::<QualificationEvent>()
            .await
            .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))
    }
}

#[async_trait]
impl QualificationInputProvider for PostgresQualificationInputs {
    #[allow(clippy::too_many_lines)]
    async fn qualification_event_for(
        &self,
        due: &DueQualificationReferral,
        observed_at_day: i64,
    ) -> Result<QualificationEvent, ReferralStoreError> {
        let kyc_tier = sqlx::query_scalar::<_, i32>(
            r"
            SELECT COALESCE(MAX(tier), 0)::INT
            FROM user_kyc_records
            WHERE user_id = $1
              AND status = 'passed'
            ",
        )
        .bind(&due.referred_id)
        .fetch_one(&self.kyc_pool)
        .await
        .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?;
        let wallet_account_code = format!("Player.Wallet.{}", due.referred_id);
        let deposit_minor_units = sqlx::query_scalar::<_, i64>(
            r"
            SELECT COALESCE(SUM(e.amount), 0)::BIGINT
            FROM ledger_entries e
            JOIN ledger_accounts a ON a.id = e.account_id
            JOIN ledger_transactions t ON t.id = e.txn_id
            WHERE a.code = $1
              AND e.direction = -1
              AND t.txn_type = 'deposit'
            ",
        )
        .bind(&wallet_account_code)
        .fetch_one(&self.ledger_pool)
        .await
        .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?;
        let completed_trade_count = sqlx::query_scalar::<_, i64>(
            r"
            SELECT COUNT(DISTINCT t.id)::BIGINT
            FROM ledger_entries e
            JOIN ledger_accounts a ON a.id = e.account_id
            JOIN ledger_transactions t ON t.id = e.txn_id
            WHERE a.code = $1
              AND t.txn_type IN ('trade', 'settle')
            ",
        )
        .bind(&wallet_account_code)
        .fetch_one(&self.ledger_pool)
        .await
        .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?;
        let same_kyc_identity = sqlx::query_scalar::<_, bool>(
            r"
            SELECT EXISTS (
                SELECT 1
                FROM user_kyc_records referred
                JOIN user_kyc_records referrer
                  ON referrer.document_id_hash = referred.document_id_hash
                WHERE referred.user_id = $1
                  AND referrer.user_id = $2
                  AND referred.document_id_hash IS NOT NULL
            )
            ",
        )
        .bind(&due.referred_id)
        .bind(&due.referrer_id)
        .fetch_one(&self.kyc_pool)
        .await
        .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?;
        let chargeback_or_void = sqlx::query_scalar::<_, bool>(
            r"
            SELECT EXISTS (
                SELECT 1
                FROM aml_events
                WHERE user_id = $1
                  AND (
                    event_type ILIKE '%chargeback%'
                    OR event_type ILIKE '%void%'
                    OR rule_flags::TEXT ILIKE '%chargeback%'
                    OR rule_flags::TEXT ILIKE '%void%'
                  )
            )
            ",
        )
        .bind(&due.referred_id)
        .fetch_one(&self.kyc_pool)
        .await
        .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?;
        let referrer_account_abnormal = sqlx::query_scalar::<_, bool>(
            r"
            SELECT EXISTS (
                SELECT 1
                FROM aml_events
                WHERE user_id = $1
                  AND (
                    ml_score >= 80
                    OR rule_flags::TEXT ILIKE '%abnormal%'
                    OR rule_flags::TEXT ILIKE '%edd%'
                    OR rule_flags::TEXT ILIKE '%suspicious%'
                  )
            )
            ",
        )
        .bind(&due.referrer_id)
        .fetch_one(&self.kyc_pool)
        .await
        .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))?;
        let same_ip = aml_rule_exists(&self.kyc_pool, &due.referred_id, "same_ip").await?;
        let same_device_fingerprint =
            aml_rule_exists(&self.kyc_pool, &due.referred_id, "same_device").await?;
        Ok(QualificationEvent {
            referred_id: due.referred_id.clone(),
            observed_at_day,
            deposit_minor_units,
            kyc_tier: u8::try_from(kyc_tier).unwrap_or_default(),
            completed_trade_count: u32::try_from(completed_trade_count).unwrap_or(u32::MAX),
            anti_abuse: AntiAbuseFlags {
                same_ip,
                same_device_fingerprint,
                same_kyc_identity,
                referred_self_trade: false,
                chargeback_or_void,
                referrer_account_abnormal,
                referred_permanently_banned: false,
            },
            kinship_review_passed: false,
        })
    }
}

async fn aml_rule_exists(
    pool: &sqlx::PgPool,
    user_id: &str,
    pattern: &str,
) -> Result<bool, ReferralStoreError> {
    sqlx::query_scalar::<_, bool>(
        r"
        SELECT EXISTS (
            SELECT 1
            FROM aml_events
            WHERE user_id = $1
              AND rule_flags::TEXT ILIKE $2
        )
        ",
    )
    .bind(user_id)
    .bind(format!("%{pattern}%"))
    .fetch_one(pool)
    .await
    .map_err(|error| ReferralStoreError::Unavailable(error.to_string()))
}

struct NoopQualificationInputs;

#[async_trait]
impl QualificationInputProvider for NoopQualificationInputs {
    async fn qualification_event_for(
        &self,
        _due: &DueQualificationReferral,
        _observed_at_day: i64,
    ) -> Result<QualificationEvent, ReferralStoreError> {
        Err(ReferralStoreError::Unavailable(
            "referral-worker daily-payout mode does not evaluate qualifications".to_owned(),
        ))
    }
}
