use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_json_tracing(service_name: &'static str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = fmt::layer()
        .json()
        .with_target(true)
        .with_current_span(true);
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    tracing::info!(service.name = service_name, "json tracing initialized");
    if std::env::var_os("OTEL_EXPORTER_OTLP_ENDPOINT").is_some() {
        tracing::info!(
            service.name = service_name,
            "otlp endpoint configured; exporter wiring is scaffolded for SPEC-007"
        );
    }
}
