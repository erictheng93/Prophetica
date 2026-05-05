use std::{env, net::SocketAddr};

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ServiceConfig {
    pub service_name: &'static str,
    pub bind_addr: SocketAddr,
    pub dependencies: ServiceDependencies,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceDependencies {
    pub database_url: ConfigPresence,
    pub redis_url: ConfigPresence,
    pub kafka_brokers: ConfigPresence,
    pub otlp_endpoint: ConfigPresence,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigPresence {
    Present,
    Missing,
}

impl ConfigPresence {
    fn from_env_var(name: &str) -> Self {
        if env::var_os(name).is_some() {
            Self::Present
        } else {
            Self::Missing
        }
    }
}

impl ServiceConfig {
    /// Loads service bootstrap configuration from process environment.
    ///
    /// # Errors
    ///
    /// Returns an error if `.env` loading fails unexpectedly.
    pub fn from_env(service_name: &'static str, default_port: u16) -> anyhow::Result<Self> {
        let _ = dotenvy::dotenv();
        let port_var = format!(
            "{}_PORT",
            service_name.replace('-', "_").to_ascii_uppercase()
        );
        let port = env::var(port_var)
            .ok()
            .and_then(|raw| raw.parse::<u16>().ok())
            .unwrap_or(default_port);
        let bind_addr = SocketAddr::from(([0, 0, 0, 0], port));

        Ok(Self {
            service_name,
            bind_addr,
            dependencies: ServiceDependencies {
                database_url: ConfigPresence::from_env_var("DATABASE_URL"),
                redis_url: ConfigPresence::from_env_var("REDIS_URL"),
                kafka_brokers: ConfigPresence::from_env_var("KAFKA_BROKERS"),
                otlp_endpoint: ConfigPresence::from_env_var("OTEL_EXPORTER_OTLP_ENDPOINT"),
            },
        })
    }
}
