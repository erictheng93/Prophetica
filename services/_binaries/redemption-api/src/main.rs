//! redemption-api binary entrypoint.
//!
//! SPEC-024 implementation plan §1 P2: thin composition layer over the
//! redemption-service library crate. Future phases (P3 / P4 / P5 / P6) plug
//! in fulfillment workers, refund jobs, and Postgres store wiring here.

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

    let app = service_stub::router(&config).merge(redemption_service::api::router());
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "redemption-api listening");

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
