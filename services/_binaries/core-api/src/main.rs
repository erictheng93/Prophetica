use sdk::{config::ServiceConfig, service_stub};
use std::env;
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
        build_user_service_router(&config).await?
    } else {
        service_stub::router(&config)
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(service_name))
        .await?;

    Ok(())
}

async fn build_user_service_router(config: &ServiceConfig) -> anyhow::Result<axum::Router> {
    if store_mode(env::var("USER_SERVICE_STORE").ok().as_deref()) == "postgres" {
        let database_url = env::var("USER_SERVICE_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .map_err(|_| {
                anyhow::anyhow!(
                    "USER_SERVICE_DATABASE_URL or DATABASE_URL is required when USER_SERVICE_STORE=postgres"
                )
            })?;
        let pool = sqlx::PgPool::connect(&database_url).await?;
        sqlx::migrate!("../../user-service/migrations")
            .run(&pool)
            .await?;
        Ok(service_stub::router(config)
            .merge(user_service::auth::auth_router())
            .merge(user_service::members::internal_members_router(
                user_service::members::PostgresMemberStore::new(pool),
            )))
    } else {
        Ok(service_stub::router(config)
            .merge(user_service::auth::auth_router())
            .merge(user_service::members::internal_members_router(
                user_service::members::InMemoryMemberStore::default(),
            )))
    }
}

fn store_mode(value: Option<&str>) -> &'static str {
    match value {
        Some("postgres") => "postgres",
        _ => "memory",
    }
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
