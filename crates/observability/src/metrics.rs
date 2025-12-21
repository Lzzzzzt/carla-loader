//! Sync Engine 指标收集模块
//!
//! 基于 SyncMeta 收集和统计同步引擎的运行指标。

use contracts::SyncMeta;
use metrics::{counter, gauge, histogram};

/// 从 SyncMeta 记录指标
///
/// 每次产生 SyncedFrame 时调用此函数来记录指标。
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
    // 帧计数器
    counter!("carla_syncer_frames_total").increment(1);

    // 帧 ID (用于检测跳帧)
    gauge!("carla_syncer_last_frame_id").set(frame_id as f64);

    // 窗口大小 (秒 -> 毫秒)
    histogram!("carla_syncer_window_size_ms").record(meta.window_size * 1000.0);

    // 运动强度
    if let Some(motion) = meta.motion_intensity {
        gauge!("carla_syncer_motion_intensity").set(motion);
        histogram!("carla_syncer_motion_intensity_hist").record(motion);
    }

    // 丢包计数
    if meta.dropped_count > 0 {
        counter!("carla_syncer_packets_dropped_total").increment(meta.dropped_count as u64);
    }
    gauge!("carla_syncer_packets_dropped_current").set(meta.dropped_count as f64);

    // 乱序包计数
    if meta.out_of_order_count > 0 {
        counter!("carla_syncer_packets_out_of_order_total")
            .increment(meta.out_of_order_count as u64);
    }
    gauge!("carla_syncer_packets_out_of_order_current").set(meta.out_of_order_count as f64);

    // 缺失传感器
    let missing_count = meta.missing_sensors.len();
    gauge!("carla_syncer_sensors_missing").set(missing_count as f64);
    if missing_count > 0 {
        counter!("carla_syncer_frames_with_missing_sensors_total").increment(1);
        for sensor_id in &meta.missing_sensors {
            counter!("carla_syncer_sensor_missing_total", "sensor_id" => sensor_id.clone())
                .increment(1);
        }
    }

    // 时间偏移统计
    for (sensor_id, offset) in &meta.time_offsets {
        gauge!(
            "carla_syncer_time_offset_ms",
            "sensor_id" => sensor_id.clone()
        )
        .set(offset * 1000.0);

        histogram!(
            "carla_syncer_time_offset_ms_hist",
            "sensor_id" => sensor_id.clone()
        )
        .record(offset.abs() * 1000.0);
    }

    // 卡尔曼滤波残差
    for (sensor_id, residual) in &meta.kf_residuals {
        gauge!(
            "carla_syncer_kf_residual",
            "sensor_id" => sensor_id.clone()
        )
        .set(*residual);

        histogram!(
            "carla_syncer_kf_residual_hist",
            "sensor_id" => sensor_id.clone()
        )
        .record(residual.abs());
    }
}

/// 记录传感器数据包接收
pub fn record_packet_received(sensor_id: &str, sensor_type: &str) {
    counter!(
        "carla_syncer_packets_received_total",
        "sensor_id" => sensor_id.to_string(),
        "sensor_type" => sensor_type.to_string()
    )
    .increment(1);
}

/// 记录同步帧分发
pub fn record_frame_dispatched(sink_name: &str, success: bool) {
    let status = if success { "success" } else { "failure" };
    counter!(
        "carla_syncer_frames_dispatched_total",
        "sink" => sink_name.to_string(),
        "status" => status.to_string()
    )
    .increment(1);
}

/// 记录管道延迟 (从数据产生到同步完成)
pub fn record_sync_latency_ms(latency_ms: f64) {
    histogram!("carla_syncer_sync_latency_ms").record(latency_ms);
}

/// 记录缓冲区深度
pub fn record_buffer_depth(sensor_id: &str, depth: usize) {
    gauge!(
        "carla_syncer_buffer_depth",
        "sensor_id" => sensor_id.to_string()
    )
    .set(depth as f64);
}

/// 同步指标聚合器
///
/// 在内存中聚合指标，便于统计和输出摘要。
#[derive(Debug, Clone, Default)]
pub struct SyncMetricsAggregator {
    /// 总帧数
    pub total_frames: u64,

    /// 丢包总数
    pub total_dropped: u64,

    /// 乱序包总数
    pub total_out_of_order: u64,

    /// 有缺失传感器的帧数
    pub frames_with_missing: u64,

    /// 窗口大小统计
    pub window_stats: RunningStats,

    /// 运动强度统计
    pub motion_stats: RunningStats,

    /// 各传感器时间偏移统计
    pub offset_stats: std::collections::HashMap<String, RunningStats>,

    /// 各传感器缺失次数
    pub missing_counts: std::collections::HashMap<String, u64>,
}

impl SyncMetricsAggregator {
    /// 创建新的聚合器
    pub fn new() -> Self {
        Self::default()
    }

    /// 更新聚合统计
    pub fn update(&mut self, meta: &SyncMeta) {
        self.total_frames += 1;
        self.total_dropped += meta.dropped_count as u64;
        self.total_out_of_order += meta.out_of_order_count as u64;

        if !meta.missing_sensors.is_empty() {
            self.frames_with_missing += 1;
            for sensor_id in &meta.missing_sensors {
                *self.missing_counts.entry(sensor_id.clone()).or_insert(0) += 1;
            }
        }

        // 窗口大小 (毫秒)
        self.window_stats.push(meta.window_size * 1000.0);

        // 运动强度
        if let Some(motion) = meta.motion_intensity {
            self.motion_stats.push(motion);
        }

        // 时间偏移
        for (sensor_id, offset) in &meta.time_offsets {
            self.offset_stats
                .entry(sensor_id.clone())
                .or_default()
                .push(offset.abs() * 1000.0);
        }
    }

    /// 生成摘要报告
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

    /// 重置统计
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// 指标摘要
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

/// 统计摘要
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

/// 在线统计计算器 (Welford's algorithm)
#[derive(Debug, Clone, Default)]
pub struct RunningStats {
    count: u64,
    mean: f64,
    m2: f64,
    min: f64,
    max: f64,
}

impl RunningStats {
    /// 添加新值
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

    /// 样本数量
    pub fn count(&self) -> u64 {
        self.count
    }

    /// 均值
    pub fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.mean
        }
    }

    /// 方差
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }

    /// 标准差
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// 最小值
    pub fn min(&self) -> f64 {
        self.min
    }

    /// 最大值
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
            reference_sensor_id: "cam".to_string(),
            window_size: 0.05,
            motion_intensity: Some(0.3),
            time_offsets: HashMap::from([("lidar".to_string(), 0.002)]),
            kf_residuals: HashMap::new(),
            missing_sensors: vec!["radar".to_string()],
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
