use axum::Router;
use sdk::{config::ServiceConfig, observability, service_stub};
use tokio::{net::TcpListener, signal};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    observability::init_json_tracing("mock-sumsub");
    let config = ServiceConfig::from_env("mock-sumsub", 8081)?;
    tracing::info!(?config, "mock-sumsub config loaded");

    let app = Router::new()
        .merge(service_stub::router(&config))
        .merge(mock_sumsub::router());
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "mock-sumsub listening");

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
