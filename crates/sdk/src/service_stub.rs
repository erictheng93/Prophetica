use std::{sync::Arc, time::Duration};

use axum::{Json, Router, http::StatusCode, response::IntoResponse, routing::get};
use serde::Serialize;
use tokio::{
    net::{TcpListener, TcpStream},
    signal,
};
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<HealthDependencyResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct HealthDependencyResponse {
    name: String,
    status: &'static str,
    message: String,
}

#[derive(Debug, Clone)]
pub struct ReadinessState {
    dependencies: Arc<Vec<ReadinessDependency>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadinessDependency {
    Tcp {
        name: String,
        addr: String,
        timeout: Duration,
    },
}

impl ReadinessState {
    #[must_use]
    pub fn ready() -> Self {
        Self {
            dependencies: Arc::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn from_dependencies(dependencies: Vec<ReadinessDependency>) -> Self {
        Self {
            dependencies: Arc::new(dependencies),
        }
    }
}

pub fn router(config: &ServiceConfig) -> Router {
    router_with_readiness(config, ReadinessState::ready())
}

pub fn router_with_readiness(config: &ServiceConfig, readiness: ReadinessState) -> Router {
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
            get(move || {
                let readiness = readiness.clone();
                async move { readiness_response(ready, readiness).await }
            }),
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
        dependencies: Vec::new(),
    }
}

async fn readiness_response(service: &'static str, readiness: ReadinessState) -> impl IntoResponse {
    let (ready, dependencies) = readiness_report(&readiness).await;
    let status = if ready { "ok" } else { "unavailable" };
    let http_status = if ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        http_status,
        Json(HealthResponse {
            status,
            service,
            check: "ready",
            dependencies,
        }),
    )
}

async fn readiness_report(readiness: &ReadinessState) -> (bool, Vec<HealthDependencyResponse>) {
    let mut all_ready = true;
    let mut dependencies = Vec::with_capacity(readiness.dependencies.len());

    for dependency in readiness.dependencies.iter() {
        let dependency = check_dependency(dependency).await;
        all_ready &= dependency.status == "ok";
        dependencies.push(dependency);
    }

    (all_ready, dependencies)
}

async fn check_dependency(dependency: &ReadinessDependency) -> HealthDependencyResponse {
    match dependency {
        ReadinessDependency::Tcp {
            name,
            addr,
            timeout,
        } => match tokio::time::timeout(*timeout, TcpStream::connect(addr.as_str())).await {
            Ok(Ok(_stream)) => HealthDependencyResponse {
                name: name.clone(),
                status: "ok",
                message: format!("tcp dependency reachable at {addr}"),
            },
            Ok(Err(error)) => HealthDependencyResponse {
                name: name.clone(),
                status: "unavailable",
                message: format!("tcp dependency unreachable at {addr}: {error}"),
            },
            Err(_elapsed) => HealthDependencyResponse {
                name: name.clone(),
                status: "unavailable",
                message: format!("tcp dependency timed out at {addr}"),
            },
        },
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
    use std::time::Duration;

    use super::{
        ReadinessDependency, ReadinessState, health_response, metrics_body, readiness_report,
    };

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

    #[tokio::test]
    async fn readiness_report_marks_reachable_tcp_dependency_ok()
    -> Result<(), Box<dyn std::error::Error>> {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let addr = listener.local_addr()?;
        let state = readiness_state_for_addr(addr);

        let (ready, dependencies) = readiness_report(&state).await;

        assert!(ready);
        assert_eq!(dependencies[0].name, "kafka");
        assert_eq!(dependencies[0].status, "ok");
        Ok(())
    }

    #[tokio::test]
    async fn readiness_report_marks_unreachable_tcp_dependency_unavailable()
    -> Result<(), Box<dyn std::error::Error>> {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let addr = listener.local_addr()?;
        drop(listener);
        let state = readiness_state_for_addr(addr);

        let (ready, dependencies) = readiness_report(&state).await;

        assert!(!ready);
        assert_eq!(dependencies[0].name, "kafka");
        assert_eq!(dependencies[0].status, "unavailable");
        Ok(())
    }

    fn readiness_state_for_addr(addr: std::net::SocketAddr) -> ReadinessState {
        ReadinessState::from_dependencies(vec![ReadinessDependency::Tcp {
            name: "kafka".to_string(),
            addr: addr.to_string(),
            timeout: Duration::from_millis(50),
        }])
    }
}
