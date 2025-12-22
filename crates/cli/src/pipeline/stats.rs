//! Pipeline statistics and metrics.

use std::time::Duration;

use observability::SyncMetricsAggregator;

/// Statistics from a pipeline run
#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    /// Total frames successfully synchronized
    pub frames_synced: u64,

    /// Total frames dropped due to backpressure or missing data
    pub frames_dropped: u64,

    /// Total packets received from sensors
    pub packets_received: u64,

    /// Total duration of the pipeline run
    pub duration: Duration,

    /// Number of sensors that were active
    pub active_sensors: usize,

    /// Number of sinks that received data
    pub active_sinks: usize,

    /// Sync engine metrics aggregator
    pub sync_metrics: SyncMetricsAggregator,
}

impl PipelineStats {
    /// Calculate frames per second throughput
    pub fn fps(&self) -> f64 {
        if self.duration.as_secs_f64() > 0.0 {
            self.frames_synced as f64 / self.duration.as_secs_f64()
        } else {
            0.0
        }
    }

    /// Calculate drop rate as percentage
    #[allow(dead_code)]
    pub fn drop_rate(&self) -> f64 {
        let total = self.frames_synced + self.frames_dropped;
        if total > 0 {
            (self.frames_dropped as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Print detailed summary
    pub fn print_summary(&self) {
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║                    Pipeline Statistics                       ║");
        println!("╚══════════════════════════════════════════════════════════════╝\n");

        println!("📊 Overview");
        println!("   ├─ Duration: {:.2}s", self.duration.as_secs_f64());
        println!("   ├─ Frames synced: {}", self.frames_synced);
        println!("   ├─ Packets received: {}", self.packets_received);
        println!("   ├─ FPS: {:.2}", self.fps());
        println!("   ├─ Active sensors: {}", self.active_sensors);
        println!("   └─ Active sinks: {}", self.active_sinks);

        let summary = self.sync_metrics.summary();

        println!("\n📈 Sync Engine Metrics");
        println!("   ├─ Total dropped packets: {}", summary.total_dropped);
        println!("   ├─ Out-of-order packets: {}", summary.total_out_of_order);
        println!(
            "   ├─ Frames with missing sensors: {} ({:.2}%)",
            summary.frames_with_missing, summary.missing_rate
        );
        println!("   ├─ Window size (ms): {}", summary.window_size_ms);
        println!("   └─ Motion intensity: {}", summary.motion_intensity);

        if !summary.sensor_missing_counts.is_empty() {
            println!("\n⚠️  Missing Sensor Counts");
            for (sensor, count) in &summary.sensor_missing_counts {
                println!("   ├─ {}: {}", sensor, count);
            }
        }

        println!();
    }
}
