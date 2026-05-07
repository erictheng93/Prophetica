use axum::{Json, Router, response::IntoResponse, routing::get};
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
    check: &'static str,
}

pub fn router(config: &ServiceConfig) -> Router {
    let live = config.service_name;
    let ready = config.service_name;
    let startup = config.service_name;
    let metrics_service = config.service_name;

    Router::new()
        .route(
            "/healthz/live",
            get(move || async move { Json(health_response("live", live)) }),
        )
        .route(
            "/healthz/ready",
            get(move || async move { Json(health_response("ready", ready)) }),
        )
        .route(
            "/healthz/startup",
            get(move || async move { Json(health_response("startup", startup)) }),
        )
        .route(
            "/metrics",
            get(move || async move { metrics_response(metrics_service) }),
        )
        .layer(TraceLayer::new_for_http())
}

/// Runs a minimal HTTP service with health, readiness, metrics, and tracing.
///
/// # Errors
///
/// Returns an error when configuration loading, socket binding, or serving fails.
#[tracing::instrument(fields(service.name = descriptor.name), skip_all)]
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

#[tracing::instrument(fields(service.name = service, health.check = check))]
fn health_response(check: &'static str, service: &'static str) -> HealthResponse {
    HealthResponse {
        status: "ok",
        service,
        check,
    }
}

#[tracing::instrument(fields(service.name = service_name))]
fn metrics_response(service_name: &'static str) -> impl IntoResponse {
    (
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        metrics_body(service_name),
    )
}

fn metrics_body(service_name: &str) -> String {
    observability::prometheus_bootstrap_metrics(service_name)
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

#[cfg(test)]
mod tests {
    use super::{health_response, metrics_body};

    #[test]
    fn metrics_body_exposes_proph_service_metrics_with_service_label() {
        let body = metrics_body("api-gateway");

        assert!(body.contains("proph_service_up"));
        assert!(body.contains("proph_http_requests_total"));
        assert!(body.contains("service=\"api-gateway\""));
    }

    #[test]
    fn startup_health_response_uses_ok_status_and_service_name() {
        let response = health_response("startup", "matching-engine");

        assert_eq!(response.status, "ok");
        assert_eq!(response.service, "matching-engine");
        assert_eq!(response.check, "startup");
    }
}
