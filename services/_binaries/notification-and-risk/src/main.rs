use std::env;

use axum::Router;
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

    let app = if service_name == notification_service::SERVICE_NAME {
        notification_app(&config).await?
    } else if service_name == risk_antifraud_service::SERVICE_NAME {
        risk_app(&config)
    } else {
        service_stub::router(&config)
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(service_name))
        .await?;

    Ok(())
}

async fn notification_app(config: &ServiceConfig) -> anyhow::Result<Router> {
    let base = service_stub::router(config);
    let database_url =
        env::var("NOTIFICATION_SERVICE_DATABASE_URL").or_else(|_| env::var("DATABASE_URL"));

    match database_url {
        Ok(database_url) => {
            let pool = sqlx::PgPool::connect(&database_url).await?;
            sqlx::migrate!("../../notification-service/migrations")
                .run(&pool)
                .await?;
            Ok(base.merge(notification_service::router(
                notification_service::PostgresNotificationStore::new(pool),
            )))
        }
        Err(_) if env::var("PROPHETICA_ENV").is_ok_and(|value| value == "production") => {
            anyhow::bail!(
                "NOTIFICATION_SERVICE_DATABASE_URL or DATABASE_URL is required in production"
            )
        }
        Err(_) => {
            tracing::warn!(
                "notification-service database is not configured; notification API is unavailable"
            );
            Ok(base)
        }
    }
}

fn risk_app(config: &ServiceConfig) -> Router {
    service_stub::router(config)
        .merge(risk_antifraud_service::internal_reports::internal_reports_router())
        .merge(risk_antifraud_service::member_actions::router(
            risk_antifraud_service::member_actions::InMemoryRiskEventStore::default(),
        ))
}

async fn shutdown_signal(service_name: &'static str) {
    if let Err(error) = signal::ctrl_c().await {
        tracing::warn!(service.name = service_name, %error, "failed to install ctrl-c handler");
    }
}
