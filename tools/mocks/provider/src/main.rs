use axum::Router;
use sdk::{config::ServiceConfig, observability, service_stub};
use std::{env, net::IpAddr};
use tokio::{net::TcpListener, signal};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    observability::init_json_tracing("mock-provider");
    let config = ServiceConfig::from_env("mock-provider", 8081)?;
    let prophetica_env = env::var("PROPHETICA_ENV").unwrap_or_else(|_| "dev".to_owned());
    let webhook_secret = read_webhook_secret(&prophetica_env)?;
    ensure_local_bind(&prophetica_env, config.bind_addr.ip())?;
    tracing::info!(?config, "mock-provider config loaded");

    let app = Router::new()
        .merge(service_stub::router(&config))
        .merge(mock_provider::router(webhook_secret));
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "mock-provider listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn read_webhook_secret(prophetica_env: &str) -> anyhow::Result<String> {
    match env::var("KYC_PROVIDER_WEBHOOK_SECRET") {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        _ if prophetica_env == "dev" || prophetica_env == "test" => {
            tracing::warn!(
                secret = "KYC_PROVIDER_WEBHOOK_SECRET",
                "using dev fallback secret; DO NOT use in production"
            );
            Ok("dev-provider-webhook-secret".to_owned())
        }
        _ => anyhow::bail!(
            "KYC_PROVIDER_WEBHOOK_SECRET is required when PROPHETICA_ENV={prophetica_env}; refusing to start mock-provider"
        ),
    }
}

fn ensure_local_bind(prophetica_env: &str, ip: IpAddr) -> anyhow::Result<()> {
    if prophetica_env != "dev" && !ip.is_loopback() {
        anyhow::bail!(
            "mock-provider refuses to bind to {ip} when PROPHETICA_ENV={prophetica_env}; use localhost only"
        );
    }
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        tracing::warn!(%error, "failed to install ctrl-c handler");
    }
}
