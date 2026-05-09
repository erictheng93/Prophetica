use sdk::{config::ServiceConfig, observability, service_stub};
use tokio::{net::TcpListener, signal};

const SERVICE_NAME: &str = "notification-and-risk";

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    observability::init_json_tracing(SERVICE_NAME);

    tokio::try_join!(
        run_stub_http(
            notification_service::SERVICE_NAME,
            notification_service::DEFAULT_PORT,
        ),
        run_stub_http(
            risk_antifraud_service::SERVICE_NAME,
            risk_antifraud_service::DEFAULT_PORT,
        ),
    )?;

    Ok(())
}

async fn run_stub_http(service_name: &'static str, default_port: u16) -> anyhow::Result<()> {
    let config = ServiceConfig::from_env(service_name, default_port)?;
    tracing::info!(
        ?config,
        "notification-and-risk member service config loaded"
    );

    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(service.name = service_name, addr = %config.bind_addr, "notification-and-risk member service listening");

    axum::serve(listener, service_stub::router(&config))
        .with_graceful_shutdown(shutdown_signal(service_name))
        .await?;

    Ok(())
}

async fn shutdown_signal(service_name: &'static str) {
    if let Err(error) = signal::ctrl_c().await {
        tracing::warn!(service.name = service_name, %error, "failed to install ctrl-c handler");
    }
}
