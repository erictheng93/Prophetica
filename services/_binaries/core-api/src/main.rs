use sdk::{
    config::ServiceConfig,
    postgres_migrations::{
        REFERRAL_SERVICE_MIGRATION_TABLE, USER_SERVICE_MIGRATION_TABLE, run_postgres_migrations,
    },
    service_stub,
};
use std::{env, net::SocketAddr};
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

    if service_name == user_service::SERVICE_NAME {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal(service_name))
        .await?;
    } else {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal(service_name))
            .await?;
    }

    Ok(())
}

async fn build_user_service_router(config: &ServiceConfig) -> anyhow::Result<axum::Router> {
    let prophetica_env = env::var("PROPHETICA_ENV").unwrap_or_else(|_| "dev".to_owned());
    let mode = store_mode(env::var("USER_SERVICE_STORE").ok().as_deref());
    validate_user_service_store_mode(&prophetica_env, mode)?;

    if mode == "postgres" {
        let database_url = env::var("USER_SERVICE_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .map_err(|_| {
                anyhow::anyhow!(
                    "USER_SERVICE_DATABASE_URL or DATABASE_URL is required when USER_SERVICE_STORE=postgres"
                )
            })?;
        let pool = sqlx::PgPool::connect(&database_url).await?;
        run_postgres_migrations(
            sqlx::migrate!("../../user-service/migrations"),
            &pool,
            USER_SERVICE_MIGRATION_TABLE,
        )
        .await?;
        Ok(service_stub::router(config)
            .merge(user_service::auth::auth_router_with_config(
                user_service::auth::AuthConfig::from_env()
                    .with_credential_store(std::sync::Arc::new(
                        user_service::auth::PostgresPasswordCredentialStore::new(pool.clone()),
                    ))
                    .with_login_event_store(std::sync::Arc::new(
                        user_service::login_events::PostgresLoginEventStore::new(pool.clone()),
                    ))
                    .with_login_event_outbox(std::sync::Arc::new(
                        user_service::login_events::PostgresLoginEventOutbox::new(pool.clone()),
                    ))
                    .with_liff_linked_account_store(std::sync::Arc::new(
                        user_service::auth::PostgresLiffLinkedAccountStore::new(pool.clone()),
                    ))
                    .with_webauthn_store(std::sync::Arc::new(
                        user_service::webauthn_store::PostgresWebauthnStore::new(pool.clone()),
                    )),
            ))
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

fn validate_user_service_store_mode(prophetica_env: &str, mode: &str) -> anyhow::Result<()> {
    if prophetica_env.eq_ignore_ascii_case("production") && mode != "postgres" {
        anyhow::bail!("USER_SERVICE_STORE=postgres is required when PROPHETICA_ENV=production");
    }
    Ok(())
}

async fn run_referral_http() -> anyhow::Result<()> {
    let config = ServiceConfig::from_env(
        referral_service::SERVICE_NAME,
        referral_service::DEFAULT_PORT,
    )?;
    let prophetica_env = env::var("PROPHETICA_ENV").unwrap_or_else(|_| "dev".to_owned());
    let mode = store_mode(env::var("REFERRAL_SERVICE_STORE").ok().as_deref());
    validate_referral_service_store_mode(&prophetica_env, mode)?;
    let app = if mode == "postgres" {
        let database_url = env::var("REFERRAL_SERVICE_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .map_err(|_| {
                anyhow::anyhow!(
                    "REFERRAL_SERVICE_DATABASE_URL or DATABASE_URL is required when REFERRAL_SERVICE_STORE=postgres"
                )
            })?;
        let pool = sqlx::PgPool::connect(&database_url).await?;
        run_postgres_migrations(
            sqlx::migrate!("../../referral-service/migrations"),
            &pool,
            REFERRAL_SERVICE_MIGRATION_TABLE,
        )
        .await?;
        service_stub::router(&config).merge(referral_service::router(
            referral_service::PostgresAffiliateStore::new(pool),
        ))
    } else {
        service_stub::router(&config).merge(referral_service::router(
            referral_service::InMemoryAffiliateDashboardStore::default(),
        ))
    };
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

fn validate_referral_service_store_mode(prophetica_env: &str, mode: &str) -> anyhow::Result<()> {
    if prophetica_env.eq_ignore_ascii_case("production") && mode != "postgres" {
        anyhow::bail!("REFERRAL_SERVICE_STORE=postgres is required when PROPHETICA_ENV=production");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        store_mode, validate_referral_service_store_mode, validate_user_service_store_mode,
    };

    #[test]
    fn store_mode_defaults_to_memory() {
        assert_eq!(store_mode(None), "memory");
    }

    #[test]
    fn store_mode_accepts_postgres() {
        assert_eq!(store_mode(Some("postgres")), "postgres");
    }

    #[test]
    fn store_mode_rejects_non_postgres_values() {
        assert_eq!(store_mode(Some("memory")), "memory");
        assert_eq!(store_mode(Some("postgres ")), "memory");
    }

    #[test]
    fn production_requires_postgres_store() {
        assert!(validate_user_service_store_mode("production", "postgres").is_ok());
        assert!(validate_user_service_store_mode("dev", "memory").is_ok());

        let result = validate_user_service_store_mode("production", "memory");
        assert!(result.is_err());
        let error = result.err().map(|error| error.to_string());
        assert_eq!(
            error.as_deref(),
            Some("USER_SERVICE_STORE=postgres is required when PROPHETICA_ENV=production")
        );
    }

    #[test]
    fn production_requires_referral_postgres_store() {
        assert!(validate_referral_service_store_mode("production", "postgres").is_ok());
        assert!(validate_referral_service_store_mode("dev", "memory").is_ok());

        let result = validate_referral_service_store_mode("production", "memory");
        assert!(result.is_err());
        let error = result.err().map(|error| error.to_string());
        assert_eq!(
            error.as_deref(),
            Some("REFERRAL_SERVICE_STORE=postgres is required when PROPHETICA_ENV=production")
        );
    }
}
