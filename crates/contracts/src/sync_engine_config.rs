//! Sync engine configuration contracts that can be shared across crates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{MissingFramePolicy, SensorId};

/// Sync engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEngineConfig {
    /// Reference sensor ID (main clock source)
    pub reference_sensor_id: SensorId,

    /// Required sensor IDs (must all have data to output)
    pub required_sensors: Vec<SensorId>,

    /// IMU sensor ID (for adaptive window calculation)
    pub imu_sensor_id: Option<SensorId>,

    /// Window configuration
    #[serde(default)]
    pub window: WindowConfig,

    /// Buffer configuration
    #[serde(default)]
    pub buffer: BufferConfig,

    /// AdaKF configuration
    #[serde(default)]
    pub adakf: AdaKFConfig,

    /// Missing data strategy
    #[serde(default)]
    pub missing_strategy: MissingDataStrategy,

    /// Expected interval per sensor (seconds)
    #[serde(default)]
    pub sensor_intervals: HashMap<SensorId, f64>,
}

/// IMU adaptive window configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    /// Minimum window size in milliseconds (high motion)
    pub min_ms: f64,
    /// Maximum window size in milliseconds (low motion)
    pub max_ms: f64,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            min_ms: 20.0,
            max_ms: 100.0,
        }
    }
}

/// Buffer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferConfig {
    /// Maximum buffer size per sensor
    pub max_size: usize,
    /// Buffer timeout in seconds before eviction
    pub timeout_s: f64,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            max_size: 1000,
            timeout_s: 1.0,
        }
    }
}

/// AdaKF (Adaptive Kalman Filter) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaKFConfig {
    /// Initial time offset estimate
    pub initial_offset: f64,
    /// Process noise (Q)
    pub process_noise: f64,
    /// Measurement noise (R)
    pub measurement_noise: f64,
    /// Residual window size for adaptive tuning
    pub residual_window: usize,
    /// Expected interval (seconds) - used as prior for noise scaling
    pub expected_interval: Option<f64>,
}

impl Default for AdaKFConfig {
    fn default() -> Self {
        Self {
            initial_offset: 0.0,
            process_noise: 0.0001,
            measurement_noise: 0.001,
            residual_window: 20,
            expected_interval: None,
        }
    }
}

/// Strategy for handling missing sensor data
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingDataStrategy {
    /// Drop the frame if any required sensor is missing
    #[default]
    Drop,
    /// Output frame with empty slot for missing sensor
    Empty,
    /// Interpolate from adjacent frames
    Interpolate,
}

impl From<MissingFramePolicy> for MissingDataStrategy {
    fn from(policy: MissingFramePolicy) -> Self {
        match policy {
            MissingFramePolicy::Drop => MissingDataStrategy::Drop,
            MissingFramePolicy::Empty => MissingDataStrategy::Empty,
            MissingFramePolicy::Interpolate => MissingDataStrategy::Interpolate,
        }
    }
}
