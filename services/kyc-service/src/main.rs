use axum::Router;
use sdk::{config::ServiceConfig, observability, service_stub};
use std::env;
use tokio::{net::TcpListener, signal};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    observability::init_json_tracing(kyc_service::SERVICE_NAME);
    let config = ServiceConfig::from_env(kyc_service::SERVICE_NAME, kyc_service::DEFAULT_PORT)?;
    let prophetica_env = env::var("PROPHETICA_ENV").unwrap_or_else(|_| "dev".to_owned());
    let secrets = kyc_service::SecretConfig::from_env(&prophetica_env)?;
    tracing::info!(?config, "kyc service config loaded");

    let store = kyc_service::kyc::InMemoryKycStore::new(secrets.document_hash_salt.clone());
    let app = Router::new()
        .merge(service_stub::router(&config))
        .merge(kyc_service::api::router(store, secrets));
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "kyc service listening");

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
