use analytics_reporting_service::{
    DEFAULT_PORT, SERVICE_NAME, router::router_with_sources, sources::SourceClients, store::PgStore,
};
use sdk::{config::ServiceConfig, observability, service_stub};
use sqlx::postgres::PgPoolOptions;
use tokio::{net::TcpListener, signal};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    observability::init_json_tracing(SERVICE_NAME);
    let config = ServiceConfig::from_env(SERVICE_NAME, DEFAULT_PORT)?;
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL is required for analytics-reporting-service"))?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let store = PgStore::new(pool);
    store.seed_templates().await?;
    let sources = SourceClients::from_env()?;
    let app = service_stub::router(&config).merge(router_with_sources(store, sources));
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "analytics reporting service listening");

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
