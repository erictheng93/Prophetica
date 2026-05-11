pub const SERVICE_NAME: &str = "analytics-reporting-service";
pub const DEFAULT_PORT: u16 = 8094;

pub mod dashboard_metrics;
pub mod error;
pub mod exports;
pub mod job_lifecycle;
pub mod router;
pub mod sources;
pub mod store;
pub mod templates;
pub mod types;
