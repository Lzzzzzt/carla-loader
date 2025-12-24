//! Main sync engine implementation.
#![allow(clippy::type_complexity)]

use std::collections::HashMap;

use contracts::{
    ImuData, SensorId, SensorPacket, SensorPayload, SensorType, SyncMeta, SyncedFrame,
};
use tracing::instrument;

use crate::adakf::AdaKF;
use crate::buffer::SensorBuffer;
use crate::window::{compute_motion_intensity, compute_window_size, fuse_motion_pressure};
use crate::{MissingDataStrategy, SyncEngineConfig};

const DEFAULT_SENSOR_INTERVAL: f64 = 0.05;
const MIN_WINDOW_FLOOR_S: f64 = 0.005;

/// Sync engine state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SyncState {
    /// No packets in buffers
    Idle,
    /// Collecting packets, waiting for complete set
    Buffering,
    /// All required sensors have data, ready to sync
    Ready,
}

/// Selected sensor data for a single frame (aggregated for cache locality)
struct SelectedSensor {
    sensor_id: SensorId,
    packet: SensorPacket,
    time_offset: f64,
    kf_residual: f64,
    quality_score: f64,
}

/// Frame selection result using Vec for small N sensors
struct FrameSelection {
    /// Successfully selected sensors (Vec for cache locality)
    selected: Vec<SelectedSensor>,
    /// Sensors that couldn't be synced
    missing_sensors: Vec<SensorId>,
}

impl FrameSelection {
    fn with_capacity(cap: usize) -> Self {
        Self {
            selected: Vec::with_capacity(cap),
            missing_sensors: Vec::new(),
        }
    }

    /// Convert to HashMaps for SyncedFrame output
    fn into_hashmaps(
        self,
    ) -> (
        HashMap<SensorId, SensorPacket>,
        HashMap<SensorId, f64>,
        HashMap<SensorId, f64>,
        HashMap<SensorId, f64>,
        Vec<SensorId>,
    ) {
        let cap = self.selected.len();
        let mut frames = HashMap::with_capacity(cap);
        let mut time_offsets = HashMap::with_capacity(cap);
        let mut kf_residuals = HashMap::with_capacity(cap);
        let mut quality_scores = HashMap::with_capacity(cap);

        for s in self.selected {
            frames.insert(s.sensor_id.clone(), s.packet);
            time_offsets.insert(s.sensor_id.clone(), s.time_offset);
            kf_residuals.insert(s.sensor_id.clone(), s.kf_residual);
            quality_scores.insert(s.sensor_id, s.quality_score);
        }

        (
            frames,
            time_offsets,
            kf_residuals,
            quality_scores,
            self.missing_sensors,
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct SyncContext {
    reference_time: f64,
    window: f64,
    fused_intensity: f64,
    min_window_s: f64,
}

/// Per-sensor state aggregated in one place for cache locality
#[derive(Debug)]
struct SensorState {
    /// Sensor identifier
    id: SensorId,
    /// Packet buffer
    buffer: SensorBuffer,
    /// Kalman filter estimator
    estimator: AdaKF,
    /// Last estimator update time
    last_update_time: f64,
    /// Last emitted timestamp (for jitter tracking)
    last_emit_time: f64,
    /// Expected interval between packets
    expected_interval: f64,
}

impl SensorState {
    fn new(
        id: SensorId,
        buffer_size: usize,
        timeout_s: f64,
        adakf_config: &crate::AdaKFConfig,
        expected_interval: f64,
    ) -> Self {
        let mut kf_config = adakf_config.clone();
        kf_config.expected_interval = Some(expected_interval);
        Self {
            id,
            buffer: SensorBuffer::new(buffer_size, timeout_s),
            estimator: AdaKF::new(&kf_config),
            last_update_time: 0.0,
            last_emit_time: 0.0,
            expected_interval,
        }
    }
}

/// Multi-sensor synchronization engine
#[derive(Debug)]
pub struct SyncEngine {
    /// Configuration
    config: SyncEngineConfig,
    /// Per-sensor states (aggregated for cache locality)
    sensors: Vec<SensorState>,
    /// Index of reference sensor in sensors vec
    reference_idx: usize,
    /// Current state
    state: SyncState,
    /// Frame counter
    frame_counter: u64,
    /// Latest IMU data for window calculation
    latest_imu: Option<ImuData>,
    /// Current motion intensity
    motion_intensity: f64,
    /// Last synced timestamp for jitter calculation
    last_sync_time: Option<f64>,
    /// Adaptive quality threshold multiplier (1.0 = use base threshold)
    quality_multiplier: f64,
    /// Running accept rate for adaptive threshold
    accept_rate: f64,
}

impl SyncEngine {
    /// Create a new sync engine with the given configuration
    pub fn new(config: SyncEngineConfig) -> Self {
        let mut sensors = Vec::with_capacity(config.required_sensors.len() + 1);
        let mut reference_idx = 0;

        // Build sensor list from required sensors
        for (i, sensor_id) in config.required_sensors.iter().enumerate() {
            let expected_interval = config
                .sensor_intervals
                .get(sensor_id)
                .copied()
                .unwrap_or(DEFAULT_SENSOR_INTERVAL);

            sensors.push(SensorState::new(
                sensor_id.clone(),
                config.buffer.max_size,
                config.buffer.timeout_s,
                &config.adakf,
                expected_interval,
            ));

            if sensor_id == &config.reference_sensor_id {
                reference_idx = i;
            }
        }

        // Add reference sensor if not in required list
        if !config
            .required_sensors
            .contains(&config.reference_sensor_id)
        {
            reference_idx = sensors.len();
            sensors.push(SensorState::new(
                config.reference_sensor_id.clone(),
                config.buffer.max_size,
                config.buffer.timeout_s,
                &config.adakf,
                DEFAULT_SENSOR_INTERVAL,
            ));
        }

        Self {
            config,
            sensors,
            reference_idx,
            state: SyncState::Idle,
            frame_counter: 0,
            latest_imu: None,
            motion_intensity: 0.0,
            last_sync_time: None,
            quality_multiplier: 1.0,
            accept_rate: 1.0,
        }
    }

    /// Find sensor state by id (linear search, fast for small N)
    #[inline]
    fn find_sensor(&self, sensor_id: &str) -> Option<usize> {
        self.sensors.iter().position(|s| s.id == sensor_id)
    }

    /// Find or create sensor state
    #[inline]
    fn find_or_create_sensor(&mut self, sensor_id: &str) -> usize {
        if let Some(idx) = self.find_sensor(sensor_id) {
            return idx;
        }
        // Create new sensor state
        let expected_interval = self
            .config
            .sensor_intervals
            .get(sensor_id)
            .copied()
            .unwrap_or(DEFAULT_SENSOR_INTERVAL);
        self.sensors.push(SensorState::new(
            sensor_id.into(),
            self.config.buffer.max_size,
            self.config.buffer.timeout_s,
            &self.config.adakf,
            expected_interval,
        ));
        self.sensors.len() - 1
    }

    /// Push a packet into the sync engine
    ///
    /// Returns `Some(SyncedFrame)` if a synchronized frame can be produced.
    #[instrument(
        level = "trace",
        name = "sync_engine_push",
        skip(self, packet),
        fields(sensor_id = %packet.sensor_id, timestamp = packet.timestamp)
    )]
    pub fn push(&mut self, packet: SensorPacket) -> Option<SyncedFrame> {
        let sensor_id = packet.sensor_id.clone();

        self.update_motion_from_packet(&sensor_id, &packet);

        let idx = self.find_or_create_sensor(&sensor_id);
        self.sensors[idx].buffer.push(packet);

        self.update_state();

        self.try_sync()
    }

    /// Update internal state based on buffer contents
    fn update_state(&mut self) {
        if self.all_buffers_empty() {
            self.state = SyncState::Idle;
        } else if self.all_required_sensors_have_data() {
            self.state = SyncState::Ready;
        } else {
            self.state = SyncState::Buffering;
        }
    }

    /// Check if all buffers are empty
    fn all_buffers_empty(&self) -> bool {
        self.sensors.iter().all(|s| s.buffer.is_empty())
    }

    /// Check if all required sensors have at least one packet
    fn all_required_sensors_have_data(&self) -> bool {
        self.config.required_sensors.iter().all(|id| {
            self.find_sensor(id)
                .map(|idx| !self.sensors[idx].buffer.is_empty())
                .unwrap_or(false)
        })
    }

    fn average_buffer_pressure(&self) -> f64 {
        if self.sensors.is_empty() {
            return 0.0;
        }

        let total: f64 = self
            .sensors
            .iter()
            .map(|s| self.buffer_pressure(&s.buffer))
            .sum();

        (total / self.sensors.len() as f64).clamp(0.0, 1.0)
    }

    fn buffer_pressure(&self, buffer: &SensorBuffer) -> f64 {
        let capacity = self.config.buffer.max_size.max(1) as f64;
        let depth = buffer.len() as f64 / capacity;
        let drop = buffer.dropped_count() as f64 / capacity;
        let out_of_order = buffer.out_of_order_count() as f64 / capacity;
        let penalty = 0.25 * (drop + out_of_order);
        (depth + penalty).clamp(0.0, 1.0)
    }

    fn sensor_expected_interval(&self, sensor_id: &str) -> f64 {
        self.find_sensor(sensor_id)
            .map(|idx| self.sensors[idx].expected_interval)
            .unwrap_or(DEFAULT_SENSOR_INTERVAL)
            .max(1e-3)
    }

    fn derived_min_window_seconds(&self) -> f64 {
        let max_period = self
            .config
            .required_sensors
            .iter()
            .map(|id| self.sensor_expected_interval(id))
            .fold(0.0, f64::max);

        let base = if max_period > 0.0 {
            max_period / 2.0
        } else {
            DEFAULT_SENSOR_INTERVAL / 2.0
        };

        let capped = base.min(self.config.window.max_ms / 1000.0);
        capped.max(MIN_WINDOW_FLOOR_S)
    }

    fn estimator_dt(&mut self, idx: usize, t_ref: f64) -> f64 {
        let sensor = &mut self.sensors[idx];
        let dt = (t_ref - sensor.last_update_time).abs();
        sensor.last_update_time = t_ref;
        if dt > 0.0 {
            dt
        } else {
            sensor.expected_interval
        }
    }

    fn compute_quality_score(
        &self,
        packet: &SensorPacket,
        time_delta: f64,
        residual: f64,
        window: f64,
        min_window_s: f64,
        load_index: f64,
    ) -> f64 {
        let sigma_t = (window / 2.0).max(1e-3);
        let sigma_r = min_window_s.max(1e-3);
        let time_term = (-((time_delta.abs() / sigma_t).powi(2))).exp();
        let residual_term = (-((residual.abs() / sigma_r).powi(2))).exp();
        let load_term = 1.0 - 0.5 * load_index.clamp(0.0, 1.0);
        let sensor_bias = match packet.sensor_type {
            SensorType::Camera => 1.0,
            SensorType::Lidar => 0.9,
            SensorType::Imu => 0.8,
            _ => 0.95,
        };
        (time_term * residual_term * load_term * sensor_bias).clamp(0.0, 1.0)
    }

    /// Get quality threshold for a sensor type
    /// Uses base threshold with adaptive multiplier (targets 95% accept rate)
    fn quality_threshold(&self, sensor_type: SensorType) -> f64 {
        let base = match sensor_type {
            SensorType::Camera => 0.05,
            SensorType::Lidar => 0.04,
            SensorType::Imu => 0.02,
            _ => 0.03,
        };
        // Apply adaptive multiplier (lower multiplier = lower threshold = more accepting)
        (base * self.quality_multiplier).clamp(0.001, 1.0)
    }

    /// Update adaptive quality threshold based on accept/reject outcome
    /// Targets fixed 95% accept rate with EMA smoothing
    fn update_adaptive_threshold(&mut self, accepted: usize, total: usize) {
        if total == 0 {
            return;
        }

        const TARGET_ACCEPT_RATE: f64 = 0.95;
        const SMOOTHING: f64 = 0.98;

        let current_rate = accepted as f64 / total as f64;

        // Exponential moving average of accept rate
        self.accept_rate = SMOOTHING * self.accept_rate + (1.0 - SMOOTHING) * current_rate;

        // Adjust multiplier based on accept rate vs 95% target
        let adjustment = if self.accept_rate < TARGET_ACCEPT_RATE - 0.05 {
            0.995 // Lower threshold gradually
        } else if self.accept_rate > TARGET_ACCEPT_RATE + 0.02 {
            1.002 // Raise threshold gradually
        } else {
            1.0 // In acceptable range
        };

        self.quality_multiplier = (self.quality_multiplier * adjustment).clamp(0.1, 2.0);
    }

    fn check_sensor_jitter(&mut self, frames: &HashMap<SensorId, SensorPacket>) {
        for (sensor_id, packet) in frames {
            if let Some(idx) = self.find_sensor(sensor_id) {
                let sensor = &mut self.sensors[idx];
                let interval = (packet.timestamp - sensor.last_emit_time).abs();
                let budget = Self::sensor_jitter_budget(packet.sensor_type);
                if interval > budget && sensor.last_emit_time > 0.0 {
                    tracing::warn!(
                        sensor_id = %sensor_id,
                        jitter = interval,
                        budget,
                        "sensor jitter budget exceeded"
                    );
                    metrics::counter!(
                        "sync_sensor_jitter_exceeded",
                        "sensor_id" => sensor_id.to_string()
                    )
                    .increment(1);
                }
                sensor.last_emit_time = packet.timestamp;
            }
        }
    }

    fn sensor_jitter_budget(sensor_type: SensorType) -> f64 {
        match sensor_type {
            SensorType::Camera => 0.265,
            SensorType::Lidar => 0.4,
            SensorType::Imu => 0.12,
            SensorType::Gnss => 0.5,
            SensorType::Radar => 0.3,
        }
    }

    /// Try to produce a synchronized frame
    #[instrument(name = "sync_engine_try_sync", skip(self))]
    fn try_sync(&mut self) -> Option<SyncedFrame> {
        if self.state != SyncState::Ready {
            return None;
        }

        let context = self.prepare_sync_context()?;
        self.log_sync_attempt(context);
        self.perform_sync(context)
    }

    #[instrument(name = "sync_engine_prepare_context", level = "trace", skip(self))]
    fn prepare_sync_context(&self) -> Option<SyncContext> {
        let reference_time = self.reference_timestamp()?;
        let fused_intensity =
            fuse_motion_pressure(self.motion_intensity, self.average_buffer_pressure());
        let min_window_s = self.derived_min_window_seconds();
        let window = compute_window_size(fused_intensity, &self.config.window);
        Some(SyncContext {
            reference_time,
            window,
            fused_intensity,
            min_window_s,
        })
    }

    #[instrument(
        name = "sync_engine_attempt_metadata",
        level = "debug",
        skip(self),
        fields(
            t_ref = context.reference_time,
            window = context.window,
            motion_intensity = context.fused_intensity
        )
    )]
    fn log_sync_attempt(&self, context: SyncContext) {
        let _ = context;
    }

    #[instrument(name = "sync_engine_perform_sync", skip(self))]
    fn perform_sync(&mut self, context: SyncContext) -> Option<SyncedFrame> {
        let selection =
            self.collect_frames(context.reference_time, context.window, context.min_window_s);

        if self.should_drop_for_missing(&selection.missing_sensors) {
            self.evict_consumed(context.reference_time);
            return None;
        }

        let (dropped_count, out_of_order_count) = self.aggregate_buffer_counts();
        self.frame_counter += 1;

        // Convert to HashMaps for metrics and output
        let (frames, time_offsets, kf_residuals, quality_scores, missing_sensors) =
            selection.into_hashmaps();

        self.record_frame_metrics(
            context.reference_time,
            &frames,
            &time_offsets,
            &quality_scores,
        );

        self.check_sensor_jitter(&frames);

        let sync_meta = SyncMeta {
            reference_sensor_id: self.config.reference_sensor_id.clone(),
            window_size: context.window,
            motion_intensity: Some(context.fused_intensity),
            time_offsets,
            kf_residuals,
            missing_sensors,
            dropped_count,
            out_of_order_count,
        };

        self.evict_consumed(context.reference_time);

        Some(SyncedFrame {
            t_sync: context.reference_time,
            frame_id: self.frame_counter,
            frames,
            sync_meta,
        })
    }

    /// Evict frames that have been consumed
    #[instrument(name = "sync_engine_evict_consumed", skip(self))]
    fn evict_consumed(&mut self, up_to: f64) {
        for sensor in &mut self.sensors {
            sensor.buffer.remove_consumed(up_to);
        }
        self.update_state();
    }

    /// Get current buffer statistics
    #[instrument(name = "sync_engine_buffer_stats", skip(self))]
    pub fn buffer_stats(&self) -> crate::BufferStats {
        let mut depths = HashMap::new();
        let mut total = 0;
        let mut oldest: Option<f64> = None;
        let mut newest: Option<f64> = None;

        for sensor in &self.sensors {
            let len = sensor.buffer.len();
            total += len;

            // Get sensor type from first packet if available
            if let Some(packet) = sensor.buffer.peek() {
                depths.insert(packet.sensor_type, len);

                let ts = packet.timestamp;
                oldest = Some(oldest.map_or(ts, |o| o.min(ts)));
                newest = Some(newest.map_or(ts, |n| n.max(ts)));
            }
        }

        crate::BufferStats {
            buffer_depths: depths,
            total_packets: total,
            oldest_timestamp: oldest,
            newest_timestamp: newest,
        }
    }

    /// Get current sync latency estimate (time from oldest buffered to now)
    #[instrument(name = "sync_engine_estimated_latency", skip(self))]
    pub fn estimated_latency(&self, current_time: f64) -> f64 {
        self.buffer_stats()
            .oldest_timestamp
            .map(|oldest| current_time - oldest)
            .unwrap_or(0.0)
    }

    /// Get frame counter
    pub fn frame_count(&self) -> u64 {
        self.frame_counter
    }

    /// Get current motion intensity
    pub fn motion_intensity(&self) -> f64 {
        fuse_motion_pressure(self.motion_intensity, self.average_buffer_pressure())
    }

    fn update_motion_from_packet(&mut self, sensor_id: &str, packet: &SensorPacket) {
        if self.config.imu_sensor_id.as_deref() == Some(sensor_id) {
            if let SensorPayload::Imu(imu) = &packet.payload {
                self.latest_imu = Some(*imu);
                self.motion_intensity = compute_motion_intensity(imu);
            }
        }
    }

    fn reference_timestamp(&self) -> Option<f64> {
        self.sensors
            .get(self.reference_idx)
            .and_then(|s| s.buffer.peek().map(|packet| packet.timestamp))
    }

    #[instrument(
        name = "sync_engine_collect_frames",
        level = "trace",
        skip(self),
        fields(t_ref = t_ref, window = window)
    )]
    fn collect_frames(&mut self, t_ref: f64, window: f64, min_window_s: f64) -> FrameSelection {
        let num_required = self.config.required_sensors.len();
        let mut selection = FrameSelection::with_capacity(num_required);

        // Collect sensor indices for required sensors
        let sensor_indices: Vec<(SensorId, Option<usize>)> = self
            .config
            .required_sensors
            .iter()
            .map(|id| (id.clone(), self.find_sensor(id)))
            .collect();

        for (sensor_id, idx_opt) in sensor_indices {
            let idx = match idx_opt {
                Some(i) => i,
                None => {
                    selection.missing_sensors.push(sensor_id);
                    continue;
                }
            };

            let offset = self.sensors[idx].estimator.offset();
            let t_target = t_ref + offset;

            let packet_opt = self.sensors[idx]
                .buffer
                .find_closest_in_window(t_target, window)
                .cloned();

            let packet = match packet_opt {
                Some(p) => p,
                None => {
                    selection.missing_sensors.push(sensor_id);
                    continue;
                }
            };

            let time_delta = packet.timestamp - t_target;
            let load_index = self.buffer_pressure(&self.sensors[idx].buffer);
            let dt = self.estimator_dt(idx, t_ref);
            let (time_offset, kf_residual) = self.sensors[idx]
                .estimator
                .update(time_delta, dt, load_index);

            let quality_score = self.compute_quality_score(
                &packet,
                time_delta,
                kf_residual,
                window,
                min_window_s,
                load_index,
            );
            if quality_score < self.quality_threshold(packet.sensor_type) {
                selection.missing_sensors.push(sensor_id);
                continue;
            }

            // Aggregate all sensor data in one struct
            selection.selected.push(SelectedSensor {
                sensor_id,
                packet,
                time_offset,
                kf_residual,
                quality_score,
            });
        }

        // Update adaptive threshold based on this frame's outcomes
        let total = num_required;
        let accepted = selection.selected.len();
        self.update_adaptive_threshold(accepted, total);

        selection
    }

    #[instrument(
        name = "sync_engine_missing_policy",
        level = "debug",
        skip(self, missing_sensors),
        fields(strategy = ?self.config.missing_strategy, missing = missing_sensors.len())
    )]
    fn should_drop_for_missing(&self, missing_sensors: &[SensorId]) -> bool {
        match self.config.missing_strategy {
            MissingDataStrategy::Drop => {
                if missing_sensors.is_empty() {
                    false
                } else {
                    self.record_missing_drop(missing_sensors);
                    true
                }
            }
            MissingDataStrategy::Empty => false,
            MissingDataStrategy::Interpolate => {
                if missing_sensors.is_empty() {
                    false
                } else {
                    self.emit_interpolation_warning(missing_sensors);
                    false
                }
            }
        }
    }

    #[instrument(
        name = "sync_engine_drop_missing",
        level = "debug",
        skip_all,
        fields(missing = ?_missing_sensors)
    )]
    fn record_missing_drop(&self, _missing_sensors: &[SensorId]) {}

    #[instrument(
        name = "sync_engine_interpolation_placeholder",
        level = "warn",
        skip_all,
        fields(missing = ?_missing_sensors)
    )]
    fn emit_interpolation_warning(&self, _missing_sensors: &[SensorId]) {}

    fn aggregate_buffer_counts(&self) -> (u32, u32) {
        self.sensors.iter().fold((0u32, 0u32), |mut acc, sensor| {
            acc.0 += sensor.buffer.dropped_count() as u32;
            acc.1 += sensor.buffer.out_of_order_count() as u32;
            acc
        })
    }

    fn record_frame_metrics(
        &mut self,
        t_ref: f64,
        frames: &HashMap<SensorId, SensorPacket>,
        time_offsets: &HashMap<SensorId, f64>,
        quality_scores: &HashMap<SensorId, f64>,
    ) {
        metrics::counter!("sync_frames_total", "status" => "ok").increment(1);

        let completeness = frames.len() as f64 / self.config.required_sensors.len() as f64;
        metrics::histogram!("sync_completeness_ratio").record(completeness);

        if let Some(last_t) = self.last_sync_time {
            let jitter = (t_ref - last_t).abs();
            metrics::histogram!("sync_jitter").record(jitter);
        }
        self.last_sync_time = Some(t_ref);

        for (sensor_id, packet) in frames {
            let offset = time_offsets.get(sensor_id).copied().unwrap_or(0.0);
            let t_target = t_ref + offset;
            let error = (packet.timestamp - t_target).abs();
            metrics::histogram!(
                "sync_alignment_error",
                "sensor_id" => sensor_id.to_string()
            )
            .record(error);
        }

        for (sensor_id, quality) in quality_scores {
            metrics::histogram!(
                "sync_quality_score",
                "sensor_id" => sensor_id.to_string()
            )
            .record(*quality);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use contracts::{ImageData, ImageFormat, PointCloudData, SensorType, Vector3};

    fn make_camera_packet(sensor_id: &str, timestamp: f64) -> SensorPacket {
        SensorPacket {
            sensor_id: sensor_id.into(),
            sensor_type: SensorType::Camera,
            timestamp,
            frame_id: None,
            payload: SensorPayload::Image(ImageData {
                width: 100,
                height: 100,
                format: ImageFormat::Rgb8,
                data: Bytes::from(vec![0u8; 30000]),
            }),
        }
    }

    fn make_lidar_packet(sensor_id: &str, timestamp: f64) -> SensorPacket {
        SensorPacket {
            sensor_id: sensor_id.into(),
            sensor_type: SensorType::Lidar,
            timestamp,
            frame_id: None,
            payload: SensorPayload::PointCloud(PointCloudData {
                num_points: 1000,
                point_stride: 16,
                data: Bytes::from(vec![0u8; 16000]),
            }),
        }
    }

    fn make_imu_packet(sensor_id: &str, timestamp: f64) -> SensorPacket {
        SensorPacket {
            sensor_id: sensor_id.into(),
            sensor_type: SensorType::Imu,
            timestamp,
            frame_id: None,
            payload: SensorPayload::Imu(ImuData {
                accelerometer: Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 9.8,
                },
                gyroscope: Vector3::default(),
                compass: 0.0,
            }),
        }
    }

    fn default_config() -> SyncEngineConfig {
        SyncEngineConfig {
            reference_sensor_id: "cam".into(),
            required_sensors: vec!["cam".into(), "lidar".into()],
            imu_sensor_id: Some("imu".into()),
            window: Default::default(),
            buffer: Default::default(),
            adakf: Default::default(),
            missing_strategy: MissingDataStrategy::Drop,
            sensor_intervals: HashMap::new(),
        }
    }

    #[test]
    fn test_sync_normal_sequence() {
        let config = default_config();
        let mut engine = SyncEngine::new(config);

        // Push camera and lidar at same time
        engine.push(make_camera_packet("cam", 0.1));
        let result = engine.push(make_lidar_packet("lidar", 0.1));

        assert!(result.is_some());
        let frame = result.unwrap();
        assert_eq!(frame.t_sync, 0.1);
        assert_eq!(frame.frames.len(), 2);
    }

    #[test]
    fn test_sync_missing_sensor_drop() {
        let config = default_config();
        let mut engine = SyncEngine::new(config);

        // Push only camera
        let result = engine.push(make_camera_packet("cam", 0.1));

        // Should not produce output (lidar missing)
        assert!(result.is_none());
    }

    #[test]
    fn test_sync_out_of_order() {
        let config = default_config();
        let mut engine = SyncEngine::new(config);

        // Push out of order
        engine.push(make_camera_packet("cam", 0.2));
        engine.push(make_camera_packet("cam", 0.1)); // Earlier but arrives later
        engine.push(make_lidar_packet("lidar", 0.1));

        let result = engine.push(make_lidar_packet("lidar", 0.2));

        // Should still produce valid output
        assert!(result.is_some());
    }

    #[test]
    fn test_imu_affects_window() {
        let config = default_config();
        let mut engine = SyncEngine::new(config);

        // Push IMU with high motion
        let mut imu_packet = make_imu_packet("imu", 0.0);
        if let SensorPayload::Imu(ref mut imu) = imu_packet.payload {
            imu.accelerometer.x = 10.0; // High acceleration
            imu.gyroscope.x = 2.0; // High angular velocity
        }
        engine.push(imu_packet);

        // Should have higher motion intensity
        assert!(engine.motion_intensity() > 0.3);
    }

    #[test]
    fn test_frame_counter() {
        let config = default_config();
        let mut engine = SyncEngine::new(config);

        engine.push(make_camera_packet("cam", 0.1));
        engine.push(make_lidar_packet("lidar", 0.1));
        assert_eq!(engine.frame_count(), 1);

        engine.push(make_camera_packet("cam", 0.2));
        engine.push(make_lidar_packet("lidar", 0.2));
        assert_eq!(engine.frame_count(), 2);
    }
}
