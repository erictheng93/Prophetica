use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

use prometheus::{GaugeVec, HistogramOpts, HistogramVec, Opts};

pub struct Timer {
    started_at: Instant,
}

impl Timer {
    #[must_use]
    pub fn start() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }

    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }
}

pub fn observe_http_request(method: &str, route: &str, status: u16, elapsed: Duration) {
    engine_http_request_duration_seconds()
        .with_label_values(&[method, route, &status.to_string()])
        .observe(elapsed.as_secs_f64());
}

pub fn observe_command(command: &str, market_id: &str, elapsed: Duration) {
    engine_command_duration_seconds()
        .with_label_values(&[command, market_id])
        .observe(elapsed.as_secs_f64());
}

pub fn set_command_queue_depth(market_id: &str, depth: usize) {
    engine_command_queue_depth()
        .with_label_values(&[market_id])
        .set(f64::from(u32::try_from(depth).unwrap_or(u32::MAX)));
}

fn engine_http_request_duration_seconds() -> &'static HistogramVec {
    static HISTOGRAM: OnceLock<HistogramVec> = OnceLock::new();
    HISTOGRAM.get_or_init(|| {
        let histogram = HistogramVec::new(
            HistogramOpts::new(
                "proph_engine_http_request_duration_seconds",
                "Matching engine HTTP request latency by route.",
            )
            .buckets(vec![
                0.001, 0.0025, 0.005, 0.010, 0.025, 0.050, 0.100, 0.250, 0.500, 1.0,
            ]),
            &["method", "route", "status"],
        )
        .unwrap_or_else(|error| panic!("invalid engine HTTP histogram definition: {error}"));
        let _ = prometheus::default_registry().register(Box::new(histogram.clone()));
        histogram
    })
}

fn engine_command_duration_seconds() -> &'static HistogramVec {
    static HISTOGRAM: OnceLock<HistogramVec> = OnceLock::new();
    HISTOGRAM.get_or_init(|| {
        let histogram = HistogramVec::new(
            HistogramOpts::new(
                "proph_engine_command_duration_seconds",
                "Matching engine internal command round-trip latency by command and market.",
            )
            .buckets(vec![
                0.0005, 0.001, 0.0025, 0.005, 0.010, 0.025, 0.050, 0.100, 0.250,
            ]),
            &["command", "market_id"],
        )
        .unwrap_or_else(|error| panic!("invalid engine command histogram definition: {error}"));
        let _ = prometheus::default_registry().register(Box::new(histogram.clone()));
        histogram
    })
}

fn engine_command_queue_depth() -> &'static GaugeVec {
    static GAUGE: OnceLock<GaugeVec> = OnceLock::new();
    GAUGE.get_or_init(|| {
        let gauge = GaugeVec::new(
            Opts::new(
                "proph_engine_command_queue_depth",
                "Matching engine per-market command queue depth.",
            ),
            &["market_id"],
        )
        .unwrap_or_else(|error| panic!("invalid engine queue depth gauge definition: {error}"));
        let _ = prometheus::default_registry().register(Box::new(gauge.clone()));
        gauge
    })
}
