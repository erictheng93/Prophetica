use sdk::{config::ServiceConfig, service_stub};
use tokio::{net::TcpListener, signal};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    tokio::try_join!(
        api_gateway::run_http(),
        run_stub_http(user_service::SERVICE_NAME, user_service::DEFAULT_PORT),
        market_service::api::run_http(),
        position_service::api::run_http(),
        run_referral_http(),
    )?;

    Ok(())
}

async fn run_stub_http(service_name: &'static str, default_port: u16) -> anyhow::Result<()> {
    let config = ServiceConfig::from_env(service_name, default_port)?;
    tracing::info!(?config, "core-api member service config loaded");

    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(service.name = service_name, addr = %config.bind_addr, "core-api member service listening");

    let app = if service_name == user_service::SERVICE_NAME {
        service_stub::router(&config)
            .merge(user_service::auth::auth_router())
            .merge(user_service::internal_contacts::internal_contacts_router())
    } else {
        service_stub::router(&config)
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(service_name))
        .await?;

    Ok(())
}

async fn run_referral_http() -> anyhow::Result<()> {
    let config = ServiceConfig::from_env(
        referral_service::SERVICE_NAME,
        referral_service::DEFAULT_PORT,
    )?;
    let app = service_stub::router(&config).merge(referral_service::router(
        referral_service::InMemoryAffiliateDashboardStore::default(),
    ));
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "referral service listening inside core-api");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(referral_service::SERVICE_NAME))
        .await?;

    Ok(())
}

async fn shutdown_signal(service_name: &'static str) {
    if let Err(error) = signal::ctrl_c().await {
        tracing::warn!(service.name = service_name, %error, "failed to install ctrl-c handler");
    }
}
