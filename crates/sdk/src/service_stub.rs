use axum::{Json, Router, routing::get};
use serde::Serialize;
use tokio::{net::TcpListener, signal};
use tower_http::trace::TraceLayer;

use crate::{config::ServiceConfig, observability};

#[derive(Debug, Clone, Copy)]
pub struct ServiceDescriptor {
    pub name: &'static str,
    pub default_port: u16,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

pub fn router(config: &ServiceConfig) -> Router {
    let live = config.service_name;
    let ready = config.service_name;

    Router::new()
        .route(
            "/healthz/live",
            get(move || async move {
                Json(HealthResponse {
                    status: "ok",
                    service: live,
                })
            }),
        )
        .route(
            "/healthz/ready",
            get(move || async move {
                Json(HealthResponse {
                    status: "ok",
                    service: ready,
                })
            }),
        )
        .route("/metrics", get(metrics))
        .layer(TraceLayer::new_for_http())
}

/// Runs a minimal HTTP service with health, readiness, metrics, and tracing.
///
/// # Errors
///
/// Returns an error when configuration loading, socket binding, or serving fails.
pub async fn run_http(descriptor: ServiceDescriptor) -> anyhow::Result<()> {
    observability::init_json_tracing(descriptor.name);
    let config = ServiceConfig::from_env(descriptor.name, descriptor.default_port)?;
    tracing::info!(?config, "service bootstrap config loaded");

    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(service.name = descriptor.name, addr = %config.bind_addr, "listening");

    axum::serve(listener, router(&config))
        .with_graceful_shutdown(shutdown_signal(descriptor.name))
        .await?;

    Ok(())
}

async fn metrics() -> &'static str {
    "# HELP proph_service_bootstrap_info Service bootstrap marker\n# TYPE proph_service_bootstrap_info gauge\nproph_service_bootstrap_info 1\n"
}

async fn shutdown_signal(service_name: &'static str) {
    let ctrl_c = async {
        if let Err(error) = signal::ctrl_c().await {
            tracing::warn!(%error, "failed to install ctrl-c handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(error) => tracing::warn!(%error, "failed to install sigterm handler"),
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!(service.name = service_name, "shutdown signal received");
}
