//! # Observability
//!
//! 可观测性模块：Tracing + Prometheus 指标。
//!
//! ## 功能
//!
//! - Tracing 初始化 (JSON/Pretty 格式)
//! - Prometheus 指标导出
//! - SyncMeta 指标收集与统计
//!
//! ## 使用示例
//!
//! ```ignore
//! use observability::{init, metrics};
//!
//! // 初始化
//! observability::init()?;
//!
//! // 记录同步指标
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

/// 初始化可观测性（Tracing + Prometheus）
///
/// - Tracing: JSON 格式，支持 RUST_LOG 环境变量
/// - Prometheus: 监听 0.0.0.0:9000
pub fn init() -> Result<()> {
    init_with_config(ObservabilityConfig::default())
}

/// 可观测性配置
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    /// 日志格式
    pub log_format: LogFormat,
    /// Prometheus 端口 (None = 禁用)
    pub metrics_port: Option<u16>,
    /// 默认日志级别
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

/// 日志格式
#[derive(Debug, Clone, Copy, Default)]
pub enum LogFormat {
    /// JSON 结构化日志
    #[default]
    Json,
    /// 人类可读格式
    Pretty,
    /// 紧凑单行格式
    Compact,
}

/// 使用自定义配置初始化
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

/// 仅初始化 Prometheus 指标（不初始化 Tracing）
///
/// 用于 Tracing 已由其他模块初始化的场景。
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
