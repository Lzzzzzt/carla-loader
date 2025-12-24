//! Sync Engine metrics collection module
//!
//! Collects and aggregates sync engine runtime metrics based on SyncMeta.

use contracts::SyncMeta;
use metrics::{counter, gauge, histogram};

/// Record metrics from SyncMeta
///
/// Call this function each time a SyncedFrame is produced to record metrics.
///
/// # Example
///
/// ```ignore
/// use observability::metrics::record_sync_metrics;
///
/// if let Some(frame) = sync_engine.push(packet) {
///     record_sync_metrics(&frame.sync_meta, frame.frame_id);
///     // ...
/// }
/// ```
pub fn record_sync_metrics(meta: &SyncMeta, frame_id: u64) {
    // Frame counter
    counter!("carla_syncer_frames_total").increment(1);

    // Frame ID (for detecting frame skips)
    gauge!("carla_syncer_last_frame_id").set(frame_id as f64);

    // Window size (seconds -> milliseconds)
    histogram!("carla_syncer_window_size_ms").record(meta.window_size * 1000.0);

    // Motion intensity
    if let Some(motion) = meta.motion_intensity {
        gauge!("carla_syncer_motion_intensity").set(motion);
        histogram!("carla_syncer_motion_intensity_hist").record(motion);
    }

    // Dropped packet count
    if meta.dropped_count > 0 {
        counter!("carla_syncer_packets_dropped_total").increment(meta.dropped_count as u64);
    }
    gauge!("carla_syncer_packets_dropped_current").set(meta.dropped_count as f64);

    // Out-of-order packet count
    if meta.out_of_order_count > 0 {
        counter!("carla_syncer_packets_out_of_order_total")
            .increment(meta.out_of_order_count as u64);
    }
    gauge!("carla_syncer_packets_out_of_order_current").set(meta.out_of_order_count as f64);

    // Missing sensors
    let missing_count = meta.missing_sensors.len();
    gauge!("carla_syncer_sensors_missing").set(missing_count as f64);
    if missing_count > 0 {
        counter!("carla_syncer_frames_with_missing_sensors_total").increment(1);
        for sensor_id in &meta.missing_sensors {
            counter!("carla_syncer_sensor_missing_total", "sensor_id" => sensor_id.to_string())
                .increment(1);
        }
    }

    // Time offset statistics
    for (sensor_id, offset) in &meta.time_offsets {
        gauge!(
            "carla_syncer_time_offset_ms",
            "sensor_id" => sensor_id.to_string()
        )
        .set(offset * 1000.0);

        histogram!(
            "carla_syncer_time_offset_ms_hist",
            "sensor_id" => sensor_id.to_string()
        )
        .record(offset.abs() * 1000.0);
    }

    // Kalman filter residuals
    for (sensor_id, residual) in &meta.kf_residuals {
        gauge!(
            "carla_syncer_kf_residual",
            "sensor_id" => sensor_id.to_string()
        )
        .set(*residual);

        histogram!(
            "carla_syncer_kf_residual_hist",
            "sensor_id" => sensor_id.to_string()
        )
        .record(residual.abs());
    }
}

/// Record sensor packet reception
pub fn record_packet_received(sensor_id: &str, sensor_type: &str) {
    counter!(
        "carla_syncer_packets_received_total",
        "sensor_id" => sensor_id.to_string(),
        "sensor_type" => sensor_type.to_string()
    )
    .increment(1);
}

/// Record synchronized frame dispatch
pub fn record_frame_dispatched(sink_name: &str, success: bool) {
    let status = if success { "success" } else { "failure" };
    counter!(
        "carla_syncer_frames_dispatched_total",
        "sink" => sink_name.to_string(),
        "status" => status.to_string()
    )
    .increment(1);
}

/// Record pipeline latency (from data generation to sync completion)
pub fn record_sync_latency_ms(latency_ms: f64) {
    histogram!("carla_syncer_sync_latency_ms").record(latency_ms);
}

/// Record buffer depth
pub fn record_buffer_depth(sensor_id: &str, depth: usize) {
    gauge!(
        "carla_syncer_buffer_depth",
        "sensor_id" => sensor_id.to_string()
    )
    .set(depth as f64);
}

/// Sync metrics aggregator
///
/// Aggregates metrics in memory for statistics and summary output.
#[derive(Debug, Clone, Default)]
pub struct SyncMetricsAggregator {
    /// Total frames
    pub total_frames: u64,

    /// Total dropped packets
    pub total_dropped: u64,

    /// Total out-of-order packets
    pub total_out_of_order: u64,

    /// Frames with missing sensors
    pub frames_with_missing: u64,

    /// Window size statistics
    pub window_stats: RunningStats,

    /// Motion intensity statistics
    pub motion_stats: RunningStats,

    /// Time offset statistics per sensor
    pub offset_stats: std::collections::HashMap<String, RunningStats>,

    /// Missing count per sensor
    pub missing_counts: std::collections::HashMap<String, u64>,
}

impl SyncMetricsAggregator {
    /// Create new aggregator
    pub fn new() -> Self {
        Self::default()
    }

    /// Update aggregate statistics
    pub fn update(&mut self, meta: &SyncMeta) {
        self.total_frames += 1;
        self.total_dropped += meta.dropped_count as u64;
        self.total_out_of_order += meta.out_of_order_count as u64;

        if !meta.missing_sensors.is_empty() {
            self.frames_with_missing += 1;
            for sensor_id in &meta.missing_sensors {
                *self
                    .missing_counts
                    .entry(sensor_id.to_string())
                    .or_insert(0) += 1;
            }
        }

        // Window size (milliseconds)
        self.window_stats.push(meta.window_size * 1000.0);

        // Motion intensity
        if let Some(motion) = meta.motion_intensity {
            self.motion_stats.push(motion);
        }

        // Time offsets
        for (sensor_id, offset) in &meta.time_offsets {
            self.offset_stats
                .entry(sensor_id.to_string())
                .or_default()
                .push(offset.abs() * 1000.0);
        }
    }

    /// Generate summary report
    pub fn summary(&self) -> MetricsSummary {
        MetricsSummary {
            total_frames: self.total_frames,
            total_dropped: self.total_dropped,
            total_out_of_order: self.total_out_of_order,
            frames_with_missing: self.frames_with_missing,
            drop_rate: if self.total_frames > 0 {
                self.total_dropped as f64 / self.total_frames as f64 * 100.0
            } else {
                0.0
            },
            missing_rate: if self.total_frames > 0 {
                self.frames_with_missing as f64 / self.total_frames as f64 * 100.0
            } else {
                0.0
            },
            window_size_ms: StatsSummary::from(&self.window_stats),
            motion_intensity: StatsSummary::from(&self.motion_stats),
            sensor_missing_counts: self.missing_counts.clone(),
        }
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Metrics summary
#[derive(Debug, Clone, Default)]
pub struct MetricsSummary {
    pub total_frames: u64,
    pub total_dropped: u64,
    pub total_out_of_order: u64,
    pub frames_with_missing: u64,
    pub drop_rate: f64,
    pub missing_rate: f64,
    pub window_size_ms: StatsSummary,
    pub motion_intensity: StatsSummary,
    pub sensor_missing_counts: std::collections::HashMap<String, u64>,
}

impl std::fmt::Display for MetricsSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Sync Metrics Summary ===")?;
        writeln!(f, "Total frames: {}", self.total_frames)?;
        writeln!(
            f,
            "Dropped packets: {} ({:.2}%)",
            self.total_dropped, self.drop_rate
        )?;
        writeln!(f, "Out-of-order packets: {}", self.total_out_of_order)?;
        writeln!(
            f,
            "Frames with missing sensors: {} ({:.2}%)",
            self.frames_with_missing, self.missing_rate
        )?;
        writeln!(f, "Window size (ms): {}", self.window_size_ms)?;
        writeln!(f, "Motion intensity: {}", self.motion_intensity)?;

        if !self.sensor_missing_counts.is_empty() {
            writeln!(f, "Missing sensor counts:")?;
            for (sensor, count) in &self.sensor_missing_counts {
                writeln!(f, "  {}: {}", sensor, count)?;
            }
        }

        Ok(())
    }
}

/// Statistics summary
#[derive(Debug, Clone, Default)]
pub struct StatsSummary {
    pub count: u64,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub std_dev: f64,
}

impl From<&RunningStats> for StatsSummary {
    fn from(stats: &RunningStats) -> Self {
        Self {
            count: stats.count,
            min: stats.min,
            max: stats.max,
            mean: stats.mean(),
            std_dev: stats.std_dev(),
        }
    }
}

impl std::fmt::Display for StatsSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.count == 0 {
            write!(f, "N/A")
        } else {
            write!(
                f,
                "min={:.3}, max={:.3}, mean={:.3}, std={:.3} (n={})",
                self.min, self.max, self.mean, self.std_dev, self.count
            )
        }
    }
}

/// Online statistics calculator (Welford's algorithm)
#[derive(Debug, Clone, Default)]
pub struct RunningStats {
    count: u64,
    mean: f64,
    m2: f64,
    min: f64,
    max: f64,
}

impl RunningStats {
    /// Add new value
    pub fn push(&mut self, value: f64) {
        self.count += 1;

        if self.count == 1 {
            self.min = value;
            self.max = value;
            self.mean = value;
            self.m2 = 0.0;
        } else {
            self.min = self.min.min(value);
            self.max = self.max.max(value);

            let delta = value - self.mean;
            self.mean += delta / self.count as f64;
            let delta2 = value - self.mean;
            self.m2 += delta * delta2;
        }
    }

    /// Sample count
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Mean
    pub fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.mean
        }
    }

    /// Variance
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }

    /// Standard deviation
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Minimum value
    pub fn min(&self) -> f64 {
        self.min
    }

    /// Maximum value
    pub fn max(&self) -> f64 {
        self.max
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_running_stats() {
        let mut stats = RunningStats::default();

        stats.push(1.0);
        stats.push(2.0);
        stats.push(3.0);
        stats.push(4.0);
        stats.push(5.0);

        assert_eq!(stats.count(), 5);
        assert!((stats.mean() - 3.0).abs() < 1e-10);
        assert!((stats.min() - 1.0).abs() < 1e-10);
        assert!((stats.max() - 5.0).abs() < 1e-10);
        assert!((stats.variance() - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_aggregator_update() {
        let mut aggregator = SyncMetricsAggregator::new();

        let meta = SyncMeta {
            reference_sensor_id: "cam".into(),
            window_size: 0.05,
            motion_intensity: Some(0.3),
            time_offsets: HashMap::from([("lidar".into(), 0.002)]),
            kf_residuals: HashMap::new(),
            missing_sensors: vec!["radar".into()],
            dropped_count: 2,
            out_of_order_count: 1,
        };

        aggregator.update(&meta);

        assert_eq!(aggregator.total_frames, 1);
        assert_eq!(aggregator.total_dropped, 2);
        assert_eq!(aggregator.total_out_of_order, 1);
        assert_eq!(aggregator.frames_with_missing, 1);
        assert_eq!(aggregator.missing_counts.get("radar"), Some(&1));
    }

    #[test]
    fn test_summary_display() {
        let summary = MetricsSummary {
            total_frames: 100,
            total_dropped: 5,
            total_out_of_order: 2,
            frames_with_missing: 3,
            drop_rate: 5.0,
            missing_rate: 3.0,
            window_size_ms: StatsSummary {
                count: 100,
                min: 20.0,
                max: 80.0,
                mean: 50.0,
                std_dev: 15.0,
            },
            motion_intensity: StatsSummary::default(),
            sensor_missing_counts: HashMap::new(),
        };

        let output = format!("{}", summary);
        assert!(output.contains("Total frames: 100"));
        assert!(output.contains("5.00%"));
    }
}
