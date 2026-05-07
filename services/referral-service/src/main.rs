use sdk::{config::ServiceConfig, observability, service_stub};
use tokio::{net::TcpListener, signal};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    observability::init_json_tracing(referral_service::SERVICE_NAME);
    let config = ServiceConfig::from_env(
        referral_service::SERVICE_NAME,
        referral_service::DEFAULT_PORT,
    )?;
    let app = service_stub::router(&config).merge(referral_service::router(
        referral_service::InMemoryAffiliateDashboardStore::default(),
    ));
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "referral service listening");

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
