//! # Observability
//!
//! Observability module: Tracing + Prometheus metrics.
//!
//! ## Features
//!
//! - Tracing initialization (JSON/Pretty format)
//! - Prometheus metrics export
//! - SyncMeta metrics collection and statistics
//!
//! ## Usage Example
//!
//! ```ignore
//! use observability::{init, metrics};
//!
//! // Initialize
//! observability::init()?;
//!
//! // Record sync metrics
//! if let Some(frame) = sync_engine.push(packet) {
//!     metrics::record_sync_metrics(&frame.sync_meta, frame.frame_id);
//! }
//! ```

pub mod metrics;

use anyhow::{Context, Result};
use metrics_exporter_prometheus::PrometheusBuilder;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

// Re-exports
pub use crate::metrics::{
    record_buffer_depth, record_frame_dispatched, record_packet_received, record_sync_latency_ms,
    record_sync_metrics, MetricsSummary, RunningStats, StatsSummary, SyncMetricsAggregator,
};

/// Initialize observability (Tracing + Prometheus)
///
/// - Tracing: JSON format, supports RUST_LOG environment variable
/// - Prometheus: Listens on 0.0.0.0:9000
pub fn init() -> Result<()> {
    init_with_config(ObservabilityConfig::default())
}

/// Observability configuration
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    /// Log format
    pub log_format: LogFormat,
    /// Prometheus port (None = disabled)
    pub metrics_port: Option<u16>,
    /// Default log level
    pub default_log_level: String,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            log_format: LogFormat::Json,
            metrics_port: Some(9000),
            default_log_level: "info".to_string(),
        }
    }
}

/// Log format
#[derive(Debug, Clone, Copy, Default)]
pub enum LogFormat {
    /// JSON structured logging
    #[default]
    Json,
    /// Human-readable format
    Pretty,
    /// Compact single-line format
    Compact,
}

/// Initialize with custom configuration
pub fn init_with_config(config: ObservabilityConfig) -> Result<()> {
    // 1. Initialize Tracing
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.default_log_level));

    match config.log_format {
        LogFormat::Json => {
            let fmt_layer = fmt::layer()
                .json()
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true);

            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .try_init()
                .context("Failed to initialize tracing subscriber")?;
        }
        LogFormat::Pretty => {
            let fmt_layer = fmt::layer().pretty();

            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .try_init()
                .context("Failed to initialize tracing subscriber")?;
        }
        LogFormat::Compact => {
            let fmt_layer = fmt::layer().compact();

            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .try_init()
                .context("Failed to initialize tracing subscriber")?;
        }
    }

    // 2. Initialize Prometheus Exporter (if enabled)
    if let Some(port) = config.metrics_port {
        let builder = PrometheusBuilder::new();
        builder
            .with_http_listener(([0, 0, 0, 0], port))
            .install()
            .context("Failed to install Prometheus recorder")?;

        tracing::info!(port = port, "Prometheus metrics endpoint initialized");
    }

    tracing::info!(
        log_format = ?config.log_format,
        metrics_port = ?config.metrics_port,
        "Observability initialized"
    );

    Ok(())
}

/// Initialize only Prometheus metrics (without initializing Tracing)
///
/// Used when Tracing is already initialized by another module.
pub fn init_metrics_only(port: u16) -> Result<()> {
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(([0, 0, 0, 0], port))
        .install()
        .context("Failed to install Prometheus recorder")?;

    tracing::info!(port = port, "Prometheus metrics endpoint initialized");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ObservabilityConfig::default();
        assert_eq!(config.metrics_port, Some(9000));
        assert_eq!(config.default_log_level, "info");
    }
}
