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

    if store_mode(env::var("KYC_STORE").ok().as_deref()) == "postgres" {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL is required when KYC_STORE=postgres"))?;
        let pool = sqlx::PgPool::connect(&database_url).await?;
        sqlx::migrate!("../../kyc-service/migrations")
            .run(&pool)
            .await?;
        let store =
            kyc_service::kyc::PostgresKycStore::new(pool, secrets.document_hash_salt.clone());
        let app = Router::new()
            .merge(service_stub::router(&config))
            .merge(kyc_service::api::router(store, secrets))
            .merge(kyc_service::internal_reports::internal_reports_router());
        serve(config, app).await?;
    } else {
        let store = kyc_service::kyc::InMemoryKycStore::new(secrets.document_hash_salt.clone());
        let app = Router::new()
            .merge(service_stub::router(&config))
            .merge(kyc_service::api::router(store, secrets))
            .merge(kyc_service::internal_reports::internal_reports_router());
        serve(config, app).await?;
    }

    Ok(())
}

async fn serve(config: ServiceConfig, app: Router) -> anyhow::Result<()> {
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "kyc service listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn store_mode(value: Option<&str>) -> &'static str {
    match value {
        Some("postgres") => "postgres",
        _ => "memory",
    }
}

async fn shutdown_signal() {
    if let Err(error) = signal::ctrl_c().await {
        tracing::warn!(%error, "failed to install ctrl-c handler");
    }
}

#[cfg(test)]
mod tests {
    use super::store_mode;

    #[test]
    fn store_mode_defaults_to_memory() {
        assert_eq!(store_mode(None), "memory");
    }

    #[test]
    fn store_mode_accepts_postgres() {
        assert_eq!(store_mode(Some("postgres")), "postgres");
    }
}
