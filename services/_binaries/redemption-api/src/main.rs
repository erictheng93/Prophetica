//! redemption-api binary entrypoint.
//!
//! SPEC-024 implementation plan §1 P3.1 → P3.4. The binary picks its store
//! and ledger client based on environment variables:
//!
//! | env var                    | value           | effect                                |
//! |----------------------------|-----------------|---------------------------------------|
//! | `REDEMPTION_DATABASE_URL`  | postgres URL    | use `PgRedemptionStore` + run migrations |
//! | (unset)                    | —               | fall back to `InMemoryRedemptionStore` (dev only) |
//! | `WALLET_LEDGER_URL`        | http(s) base    | use `HttpLedgerClient`                |
//! | (unset)                    | —               | fall back to `InMemoryLedgerClient` (dev only)    |
//!
//! The in-memory fall-back is deliberately non-persistent — restarting the
//! process loses all orders. Intended for local QA / contract tests only;
//! `PROPHETICA_ENV=production` fails startup unless both production backing
//! services are configured.

use std::sync::Arc;

use redemption_service::{
    api,
    ledger::{HttpLedgerClient, InMemoryLedgerClient, LedgerClient},
    service::RedemptionService,
    store::{InMemoryRedemptionStore, RedemptionStore},
    store_pg::{PgRedemptionStore, connect_pool},
};
use sdk::{config::ServiceConfig, observability, service_stub};
use tokio::{net::TcpListener, signal};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    observability::init_json_tracing(redemption_service::SERVICE_NAME);
    let config = ServiceConfig::from_env(
        redemption_service::SERVICE_NAME,
        redemption_service::DEFAULT_PORT,
    )?;
    tracing::info!(?config, "redemption-api service config loaded");

    validate_runtime_config(
        &std::env::var("PROPHETICA_ENV").unwrap_or_else(|_| "dev".to_owned()),
        std::env::var("REDEMPTION_DATABASE_URL").is_ok(),
        std::env::var("WALLET_LEDGER_URL").is_ok(),
    )?;

    let store = build_store().await?;
    let ledger = build_ledger()?;
    let service = Arc::new(RedemptionService::new(store, ledger));

    let app = service_stub::router(&config).merge(api::router(service));
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "redemption-api listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn build_store() -> anyhow::Result<Arc<dyn RedemptionStore>> {
    if let Ok(database_url) = std::env::var("REDEMPTION_DATABASE_URL") {
        let pool = connect_pool(&database_url).await?;
        sqlx::migrate!("../../redemption-service/migrations")
            .run(&pool)
            .await?;
        tracing::info!("redemption-api using PgRedemptionStore");
        Ok(Arc::new(PgRedemptionStore::new(pool)))
    } else {
        tracing::warn!(
            "REDEMPTION_DATABASE_URL not set — falling back to InMemoryRedemptionStore (dev only, NOT production)"
        );
        Ok(Arc::new(InMemoryRedemptionStore::new()))
    }
}

fn build_ledger() -> anyhow::Result<Arc<dyn LedgerClient>> {
    if let Ok(ledger_url) = std::env::var("WALLET_LEDGER_URL") {
        let client = HttpLedgerClient::new(&ledger_url)?;
        tracing::info!(%ledger_url, "redemption-api using HttpLedgerClient");
        Ok(Arc::new(client))
    } else {
        tracing::warn!(
            "WALLET_LEDGER_URL not set — falling back to InMemoryLedgerClient (dev only, NOT production)"
        );
        Ok(Arc::new(InMemoryLedgerClient::new()))
    }
}

fn validate_runtime_config(
    prophetica_env: &str,
    has_redemption_database_url: bool,
    has_wallet_ledger_url: bool,
) -> anyhow::Result<()> {
    if prophetica_env.eq_ignore_ascii_case("production")
        && (!has_redemption_database_url || !has_wallet_ledger_url)
    {
        anyhow::bail!(
            "REDEMPTION_DATABASE_URL and WALLET_LEDGER_URL are required when PROPHETICA_ENV=production"
        );
    }
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        tracing::warn!(%error, "failed to install ctrl-c handler");
    }
}

#[cfg(test)]
mod tests {
    use super::validate_runtime_config;

    #[test]
    fn production_requires_persistent_store_and_http_ledger() {
        let result = validate_runtime_config("production", false, false);

        assert!(result.is_err());
        assert_eq!(
            result.err().map(|error| error.to_string()).as_deref(),
            Some(
                "REDEMPTION_DATABASE_URL and WALLET_LEDGER_URL are required when PROPHETICA_ENV=production"
            )
        );
    }

    #[test]
    fn dev_allows_in_memory_fallbacks() {
        assert!(validate_runtime_config("dev", false, false).is_ok());
    }
}
