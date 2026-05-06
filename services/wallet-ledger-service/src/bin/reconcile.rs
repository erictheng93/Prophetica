use std::{env, time::Duration};

use sqlx::postgres::PgPoolOptions;
use tokio::time;
use wallet_ledger_service::reconciliation::{
    ReconciliationLayer, load_player_liability_snapshot, persist_reconciliation_finding,
    player_liability_finding, start_reconciliation_run,
};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let database_url = env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    let interval = env::var("WALLET_LEDGER_RECONCILE_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map_or(300, |value| value);
    let expected_player_liability = env::var("WALLET_LEDGER_EXPECTED_PLAYER_LIABILITY")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .map_or(0, |value| value);

    loop {
        run_layer2_once(&pool, expected_player_liability).await?;
        time::sleep(Duration::from_secs(interval)).await;
    }
}

async fn run_layer2_once(
    pool: &sqlx::PgPool,
    expected_player_liability: i64,
) -> anyhow::Result<()> {
    let run_id = start_reconciliation_run(pool, ReconciliationLayer::PlayerLiability).await?;
    let snapshot = load_player_liability_snapshot(pool).await?;

    if let Some(finding) = player_liability_finding(snapshot, expected_player_liability) {
        let finding_id = persist_reconciliation_finding(pool, run_id, &finding).await?;
        tracing::warn!(
            run_id,
            finding_id,
            finding_type = %finding.finding_type,
            difference_amount = finding.difference_amount,
            "reconciliation mismatch"
        );
    }

    sqlx::query(
        "UPDATE reconciliation_runs \
         SET status = 'completed', completed_at = NOW() \
         WHERE id = $1",
    )
    .bind(run_id)
    .execute(pool)
    .await?;

    Ok(())
}
