//! Main sync engine implementation.
#![allow(unused)]

use std::collections::HashMap;

use contracts::{ImuData, SensorPacket, SensorPayload, SensorType, SyncMeta, SyncedFrame};
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

#[derive(Default)]
struct FrameSelection {
    frames: HashMap<String, SensorPacket>,
    time_offsets: HashMap<String, f64>,
    kf_residuals: HashMap<String, f64>,
    quality_scores: HashMap<String, f64>,
    missing_sensors: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct SyncContext {
    reference_time: f64,
    window: f64,
    fused_intensity: f64,
    min_window_s: f64,
}

/// Multi-sensor synchronization engine
#[derive(Debug)]
pub struct SyncEngine {
    /// Configuration
    config: SyncEngineConfig,
    /// Per-sensor buffers
    buffers: HashMap<String, SensorBuffer>,
    /// Per-sensor AdaKF estimators
    estimators: HashMap<String, AdaKF>,
    /// Current state
    state: SyncState,
    /// Frame counter
    frame_counter: u64,
    /// Latest IMU data for window calculation
    latest_imu: Option<ImuData>,
    /// Current motion intensity
    motion_intensity: f64,
    /// Total dropped count
    total_dropped: u64,
    /// Total out-of-order count
    total_out_of_order: u64,
    /// Last synced timestamp for jitter calculation
    last_sync_time: Option<f64>,
    /// Last reference time processed per sensor (for KF Δt)
    last_estimator_update: HashMap<String, f64>,
    /// Last emitted timestamp per sensor (for jitter watchdog)
    sensor_last_emit: HashMap<String, f64>,
}

impl SyncEngine {
    /// Create a new sync engine with the given configuration
    pub fn new(config: SyncEngineConfig) -> Self {
        let mut buffers = HashMap::new();
        let mut estimators = HashMap::new();

        // Initialize buffers and estimators for all required sensors
        for sensor_id in &config.required_sensors {
            buffers.insert(
                sensor_id.clone(),
                SensorBuffer::new(config.buffer.max_size, config.buffer.timeout_s),
            );

            let mut kf_config = config.adakf.clone();
            if let Some(&interval) = config.sensor_intervals.get(sensor_id) {
                kf_config.expected_interval = Some(interval);
            }
            estimators.insert(sensor_id.clone(), AdaKF::new(&kf_config));
        }

        // Also add reference sensor if not in required list
        if !config
            .required_sensors
            .contains(&config.reference_sensor_id)
        {
            buffers.insert(
                config.reference_sensor_id.clone(),
                SensorBuffer::new(config.buffer.max_size, config.buffer.timeout_s),
            );
            estimators.insert(
                config.reference_sensor_id.clone(),
                AdaKF::new(&config.adakf),
            );
        }

        Self {
            config,
            buffers,
            estimators,
            state: SyncState::Idle,
            frame_counter: 0,
            latest_imu: None,
            motion_intensity: 0.0,
            total_dropped: 0,
            total_out_of_order: 0,
            last_sync_time: None,
            last_estimator_update: HashMap::new(),
            sensor_last_emit: HashMap::new(),
        }
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
        self.buffer_mut(&sensor_id).push(packet);

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
        self.buffers.values().all(|b| b.is_empty())
    }

    /// Check if all required sensors have at least one packet
    fn all_required_sensors_have_data(&self) -> bool {
        self.config
            .required_sensors
            .iter()
            .all(|id| self.buffers.get(id).map(|b| !b.is_empty()).unwrap_or(false))
    }

    fn average_buffer_pressure(&self) -> f64 {
        if self.buffers.is_empty() {
            return 0.0;
        }

        let mut total = 0.0;
        let mut count = 0.0;
        for (sensor_id, buffer) in &self.buffers {
            total += self.buffer_pressure_for(sensor_id, buffer);
            count += 1.0;
        }

        if count == 0.0 {
            0.0
        } else {
            (total / count).clamp(0.0, 1.0)
        }
    }

    fn sensor_load_index(&self, sensor_id: &str) -> f64 {
        self.buffers
            .get(sensor_id)
            .map(|buffer| self.buffer_pressure_for(sensor_id, buffer))
            .unwrap_or(0.0)
    }

    fn buffer_pressure_for(&self, sensor_id: &str, buffer: &SensorBuffer) -> f64 {
        let capacity = self.config.buffer.max_size.max(1) as f64;
        let depth = buffer.len() as f64 / capacity;
        let drop = buffer.dropped_count() as f64 / capacity;
        let out_of_order = buffer.out_of_order_count() as f64 / capacity;
        let penalty = 0.25 * (drop + out_of_order);
        (depth + penalty).clamp(0.0, 1.0)
    }

    fn sensor_expected_interval(&self, sensor_id: &str) -> f64 {
        self.config
            .sensor_intervals
            .get(sensor_id)
            .copied()
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

    fn estimator_dt(&mut self, sensor_id: &str, t_ref: f64) -> f64 {
        let entry = self
            .last_estimator_update
            .entry(sensor_id.to_string())
            .or_insert(t_ref);
        let dt = (t_ref - *entry).abs();
        *entry = t_ref;
        if dt > 0.0 {
            dt
        } else {
            self.sensor_expected_interval(sensor_id)
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

    fn quality_threshold(&self, sensor_type: SensorType) -> f64 {
        match sensor_type {
            SensorType::Camera => 0.05,
            SensorType::Lidar => 0.04,
            SensorType::Imu => 0.02,
            _ => 0.03,
        }
    }

    fn check_sensor_jitter(&mut self, frames: &HashMap<String, SensorPacket>) {
        for (sensor_id, packet) in frames {
            let entry = self
                .sensor_last_emit
                .entry(sensor_id.clone())
                .or_insert(packet.timestamp);
            let interval = (packet.timestamp - *entry).abs();
            let budget = Self::sensor_jitter_budget(packet.sensor_type);
            if interval > budget {
                tracing::warn!(
                    sensor_id = %sensor_id,
                    jitter = interval,
                    budget,
                    "sensor jitter budget exceeded"
                );
                metrics::counter!(
                    "sync_sensor_jitter_exceeded",
                    "sensor_id" => sensor_id.clone()
                )
                .increment(1);
            }
            *entry = packet.timestamp;
        }
    }

    fn sensor_jitter_budget(sensor_type: SensorType) -> f64 {
        match sensor_type {
            SensorType::Camera => 0.265,
            SensorType::Lidar => 0.4,
            SensorType::Imu => 0.12,
            SensorType::Gnss => 0.5,
            SensorType::Radar => 0.3,
            _ => 0.3,
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

        self.record_frame_metrics(
            context.reference_time,
            &selection.frames,
            &selection.time_offsets,
            &selection.quality_scores,
        );

        let FrameSelection {
            frames,
            time_offsets,
            kf_residuals,
            quality_scores: _,
            missing_sensors,
        } = selection;

        self.check_sensor_jitter(&frames);

        let sync_meta = self.build_sync_meta(
            context.window,
            context.fused_intensity,
            time_offsets,
            kf_residuals,
            missing_sensors,
            dropped_count,
            out_of_order_count,
        );

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
        for buffer in self.buffers.values_mut() {
            buffer.remove_consumed(up_to);
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

        for buffer in self.buffers.values() {
            let len = buffer.len();
            total += len;

            // Get sensor type from first packet if available
            if let Some(packet) = buffer.peek() {
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

    fn buffer_mut(&mut self, sensor_id: &str) -> &mut SensorBuffer {
        self.buffers
            .entry(sensor_id.to_string())
            .or_insert_with(|| {
                SensorBuffer::new(self.config.buffer.max_size, self.config.buffer.timeout_s)
            })
    }

    fn reference_timestamp(&self) -> Option<f64> {
        self.buffers
            .get(&self.config.reference_sensor_id)
            .and_then(|buffer| buffer.peek().map(|packet| packet.timestamp))
    }

    #[instrument(
        name = "sync_engine_collect_frames",
        level = "trace",
        skip(self),
        fields(t_ref = t_ref, window = window)
    )]
    fn collect_frames(&mut self, t_ref: f64, window: f64, min_window_s: f64) -> FrameSelection {
        let mut selection = FrameSelection::default();

        let required_sensors = self.config.required_sensors.clone();
        for sensor_id in required_sensors {
            let offset = self
                .estimators
                .get(&sensor_id)
                .map(|e| e.offset())
                .unwrap_or(0.0);

            let t_target = t_ref + offset;

            let packet_opt = {
                let buffer = match self.buffers.get(&sensor_id) {
                    Some(b) => b,
                    None => {
                        selection.missing_sensors.push(sensor_id.clone());
                        continue;
                    }
                };
                buffer.find_closest_in_window(t_target, window).cloned()
            };

            let packet = match packet_opt {
                Some(packet) => packet,
                None => {
                    selection.missing_sensors.push(sensor_id.clone());
                    continue;
                }
            };

            let time_delta = packet.timestamp - t_target;
            let load_index = self.sensor_load_index(&sensor_id);
            let dt = self.estimator_dt(&sensor_id, t_ref);
            let (estimate, residual) = if let Some(estimator) = self.estimators.get_mut(&sensor_id)
            {
                estimator.update(time_delta, dt, load_index)
            } else {
                (offset, time_delta)
            };

            selection.time_offsets.insert(sensor_id.clone(), estimate);

            let quality = self.compute_quality_score(
                &packet,
                time_delta,
                residual,
                window,
                min_window_s,
                load_index,
            );
            if quality < self.quality_threshold(packet.sensor_type) {
                selection.missing_sensors.push(sensor_id.clone());
                continue;
            }

            selection.kf_residuals.insert(sensor_id.clone(), residual);
            selection.quality_scores.insert(sensor_id.clone(), quality);
            selection.frames.insert(sensor_id.clone(), packet);
        }

        selection
    }

    #[instrument(
        name = "sync_engine_missing_policy",
        level = "debug",
        skip(self, missing_sensors),
        fields(strategy = ?self.config.missing_strategy, missing = missing_sensors.len())
    )]
    fn should_drop_for_missing(&self, missing_sensors: &[String]) -> bool {
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
    fn record_missing_drop(&self, _missing_sensors: &[String]) {}

    #[instrument(
        name = "sync_engine_interpolation_placeholder",
        level = "warn",
        skip_all,
        fields(missing = ?_missing_sensors)
    )]
    fn emit_interpolation_warning(&self, _missing_sensors: &[String]) {}

    fn aggregate_buffer_counts(&self) -> (u32, u32) {
        self.buffers.values().fold((0u32, 0u32), |mut acc, buffer| {
            acc.0 += buffer.dropped_count() as u32;
            acc.1 += buffer.out_of_order_count() as u32;
            acc
        })
    }

    fn build_sync_meta(
        &self,
        window: f64,
        motion_intensity: f64,
        time_offsets: HashMap<String, f64>,
        kf_residuals: HashMap<String, f64>,
        missing_sensors: Vec<String>,
        dropped_count: u32,
        out_of_order_count: u32,
    ) -> SyncMeta {
        SyncMeta {
            reference_sensor_id: self.config.reference_sensor_id.clone(),
            window_size: window,
            motion_intensity: Some(motion_intensity),
            time_offsets,
            kf_residuals,
            missing_sensors,
            dropped_count,
            out_of_order_count,
        }
    }

    fn record_frame_metrics(
        &mut self,
        t_ref: f64,
        frames: &HashMap<String, SensorPacket>,
        time_offsets: &HashMap<String, f64>,
        quality_scores: &HashMap<String, f64>,
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
            sensor_id: sensor_id.to_string(),
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
            sensor_id: sensor_id.to_string(),
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
            sensor_id: sensor_id.to_string(),
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
            reference_sensor_id: "cam".to_string(),
            required_sensors: vec!["cam".to_string(), "lidar".to_string()],
            imu_sensor_id: Some("imu".to_string()),
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
