use std::env;

use sdk::{config::ServiceConfig, observability, service_stub};
use sqlx::postgres::PgPoolOptions;
use tokio::{net::TcpListener, signal};
use wallet_ledger_service::ledger::PostgresLedgerStore;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    observability::init_json_tracing(wallet_ledger_service::SERVICE_NAME);
    let config = ServiceConfig::from_env(
        wallet_ledger_service::SERVICE_NAME,
        wallet_ledger_service::DEFAULT_PORT,
    )?;
    tracing::info!(?config, "wallet ledger service config loaded");

    let database_url = env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let app = service_stub::router(&config).merge(wallet_ledger_service::api::router(
        PostgresLedgerStore::new(pool),
    ));
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "wallet ledger service listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        tracing::warn!(%error, "failed to install ctrl-c handler");
    }
}
