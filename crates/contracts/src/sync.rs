//! SyncedFrame - Sync Engine output
//!
//! Synchronized frame data structure.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{SensorId, SensorPacket, SensorType};

/// Synchronized frame
///
/// Contains aligned multi-sensor data within the same time window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncedFrame {
    /// Sync timestamp (CARLA simulation time, seconds)
    pub t_sync: f64,

    /// Frame sequence number (monotonically increasing)
    pub frame_id: u64,

    /// Sensor data packets (sensor_id -> packet)
    pub frames: HashMap<SensorId, SensorPacket>,

    /// Sync metadata
    pub sync_meta: SyncMeta,
}

/// Sync metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncMeta {
    /// Reference clock sensor ID
    pub reference_sensor_id: SensorId,

    /// Dynamic window size (seconds)
    pub window_size: f64,

    /// Motion intensity (0-1), used for adaptive windowing
    pub motion_intensity: Option<f64>,

    /// Time offset estimates per sensor (sensor_id -> offset)
    pub time_offsets: HashMap<SensorId, f64>,

    /// Kalman filter residuals (used for adaptive tuning)
    pub kf_residuals: HashMap<SensorId, f64>,

    /// Missing sensors (no data in this frame)
    pub missing_sensors: Vec<SensorId>,

    /// Dropped packet count (expired/out-of-order)
    pub dropped_count: u32,

    /// Out-of-order packet count
    pub out_of_order_count: u32,
}

/// Synchronized data packet (single sensor)
#[derive(Debug, Clone)]
pub struct SyncedPacket {
    /// Original data packet
    pub packet: SensorPacket,

    /// Corrected timestamp
    pub corrected_timestamp: f64,

    /// Whether interpolated
    pub interpolated: bool,

    /// Time delta between original timestamp and reference time
    pub time_delta: f64,
}

/// Sensor buffer status (for diagnostics)
#[derive(Debug, Clone, Default)]
pub struct BufferStats {
    /// Buffer depth per sensor type
    pub buffer_depths: HashMap<SensorType, usize>,

    /// Total buffered packets
    pub total_packets: usize,

    /// Oldest timestamp
    pub oldest_timestamp: Option<f64>,

    /// Newest timestamp
    pub newest_timestamp: Option<f64>,
}
