use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    Resource,
    trace::{self, Tracer},
};
use prometheus::{Encoder, GaugeVec, IntCounterVec, Opts, Registry, TextEncoder, core::Collector};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_json_tracing(service_name: &'static str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = fmt::layer()
        .json()
        .with_target(true)
        .with_current_span(true);

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);

    if let Some(tracer) = otlp_tracer(service_name) {
        subscriber
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .init();
    } else {
        subscriber.init();
    }

    tracing::info!(service.name = service_name, "json tracing initialized");
}

#[must_use]
pub fn prometheus_registry(_service_name: &str) -> Registry {
    Registry::new()
}

#[must_use]
pub fn prometheus_bootstrap_metrics(service_name: &str) -> String {
    let registry = prometheus_registry(service_name);
    let service_up = service_up();
    service_up.with_label_values(&[service_name]).set(1.0);
    register(&registry, service_up);

    let http_requests = http_requests();
    http_requests
        .with_label_values(&[service_name, "GET", "/healthz/live", "200"])
        .inc_by(0);
    register(&registry, http_requests);

    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    if encoder.encode(&registry.gather(), &mut buffer).is_err() {
        return fallback_metrics(service_name);
    }
    if encoder.encode(&prometheus::gather(), &mut buffer).is_err() {
        return fallback_metrics(service_name);
    }

    String::from_utf8(buffer).unwrap_or_else(|_| fallback_metrics(service_name))
}

fn otlp_tracer(service_name: &'static str) -> Option<Tracer> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok()?;
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(endpoint);
    let resource = Resource::new(vec![opentelemetry::KeyValue::new(
        "service.name",
        service_name,
    )]);

    match opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .with_trace_config(trace::config().with_resource(resource))
        .install_batch(opentelemetry_sdk::runtime::Tokio)
    {
        Ok(tracer) => {
            tracing::info!(
                service.name = service_name,
                "otlp tracing exporter initialized"
            );
            Some(tracer)
        }
        Err(error) => {
            tracing::warn!(service.name = service_name, %error, "failed to initialize otlp tracing exporter");
            None
        }
    }
}

fn service_up() -> GaugeVec {
    GaugeVec::new(
        Opts::new("proph_service_up", "Service process is up."),
        &["service"],
    )
    .unwrap_or_else(|error| panic!("invalid prometheus gauge definition: {error}"))
}

fn http_requests() -> IntCounterVec {
    IntCounterVec::new(
        Opts::new(
            "proph_http_requests_total",
            "HTTP requests observed by the service stub.",
        ),
        &["service", "method", "route", "status"],
    )
    .unwrap_or_else(|error| panic!("invalid prometheus counter definition: {error}"))
}

fn register(registry: &Registry, collector: impl Collector + 'static) {
    let _ = registry.register(Box::new(collector));
}

fn fallback_metrics(service_name: &str) -> String {
    format!(
        "# HELP proph_service_up Service process is up.\n# TYPE proph_service_up gauge\nproph_service_up{{service=\"{service_name}\"}} 1\n# HELP proph_http_requests_total HTTP requests observed by the service stub.\n# TYPE proph_http_requests_total counter\nproph_http_requests_total{{service=\"{service_name}\",method=\"GET\",route=\"/healthz/live\",status=\"200\"}} 0\n"
    )
}

pub fn shutdown_tracer_provider() {
    global::shutdown_tracer_provider();
}
