//! redemption-expiry-sweeper binary.
//!
//! SPEC-024 implementation plan §1 P5.2: scans `redemption_orders` for
//! orders past their entitlement TTL and runs
//! [`redemption_service::service::RedemptionService::run_expiry_sweep`] to
//! post the `entitlement_expiry` ledger txn and advance state to `Expired`.
//!
//! Env vars:
//!
//! | env var                                    | default      | effect                              |
//! |--------------------------------------------|--------------|-------------------------------------|
//! | `REDEMPTION_DATABASE_URL`                  | (required)   | redemption-service PG               |
//! | `WALLET_LEDGER_URL`                        | (required)   | wallet-ledger-service HTTP base     |
//! | `REDEMPTION_EXPIRY_INTERVAL_SECONDS`       | 300 (5 min)  | seconds between passes              |
//! | `REDEMPTION_EXPIRY_BATCH_SIZE`             | 500          | max orders processed per pass       |
//! | `REDEMPTION_EXPIRY_ONESHOT`                | (unset)      | when set to any value, run one pass and exit |
//!
//! Designed to be deployed either as a long-running tokio task or as a cron
//! job that calls the binary in oneshot mode. The orchestrator inside
//! `run_expiry_sweep` keeps per-order failures from blocking the rest of the
//! batch, so a single bad order doesn't stall the queue.

use std::{sync::Arc, time::Duration};

use redemption_service::{
    ledger::HttpLedgerClient,
    service::{RedemptionService, current_unix_secs},
    store_pg::{PgRedemptionStore, connect_pool},
};
use sdk::observability;
use tokio::time;

const DEFAULT_INTERVAL_SECONDS: u64 = 300;
const DEFAULT_BATCH_SIZE: usize = 500;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    observability::init_json_tracing("redemption-expiry-sweeper");

    let database_url = std::env::var("REDEMPTION_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("REDEMPTION_DATABASE_URL must be set"))?;
    let ledger_url = std::env::var("WALLET_LEDGER_URL")
        .map_err(|_| anyhow::anyhow!("WALLET_LEDGER_URL must be set"))?;
    let interval_secs = std::env::var("REDEMPTION_EXPIRY_INTERVAL_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map_or(DEFAULT_INTERVAL_SECONDS, |v| v.max(1));
    let batch_size = std::env::var("REDEMPTION_EXPIRY_BATCH_SIZE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .map_or(DEFAULT_BATCH_SIZE, |v| v.max(1));
    let oneshot = std::env::var("REDEMPTION_EXPIRY_ONESHOT").is_ok();

    let pool = connect_pool(&database_url).await?;
    sqlx::migrate!("../../redemption-service/migrations")
        .run(&pool)
        .await?;
    let store = Arc::new(PgRedemptionStore::new(pool));
    let ledger = Arc::new(HttpLedgerClient::new(&ledger_url)?);
    let service = RedemptionService::new(store, ledger);

    tracing::info!(
        interval_secs,
        batch_size,
        oneshot,
        "redemption-expiry-sweeper starting"
    );

    if oneshot {
        run_one_pass(&service, batch_size).await;
        return Ok(());
    }

    loop {
        run_one_pass(&service, batch_size).await;
        time::sleep(Duration::from_secs(interval_secs)).await;
    }
}

async fn run_one_pass(service: &RedemptionService, batch_size: usize) {
    let now = current_unix_secs();
    match service.run_expiry_sweep(now, batch_size).await {
        Ok(stats) => {
            tracing::info!(
                processed = stats.processed,
                ledger_failed = stats.ledger_failed,
                store_failed = stats.store_failed,
                now,
                "expiry sweep pass complete"
            );
        }
        Err(error) => {
            tracing::error!(%error, now, "expiry sweep pass aborted");
        }
    }
}
