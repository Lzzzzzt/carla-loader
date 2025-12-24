//! SensorPacket - Ingestion output
//!
//! Raw sensor data packet structure.

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::{SensorId, SensorType};

/// Sensor data packet
///
/// Raw data received from CARLA sensor callbacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorPacket {
    /// Sensor ID (cheap clone via Arc<str>)
    pub sensor_id: SensorId,

    /// Sensor type
    pub sensor_type: SensorType,

    /// CARLA simulation timestamp (seconds, f64) - primary clock
    pub timestamp: f64,

    /// Optional frame sequence number (used for ordering/diagnostics)
    pub frame_id: Option<u64>,

    /// Data payload (zero-copy)
    pub payload: SensorPayload,
}

/// Sensor data payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensorPayload {
    /// Image data (RGB/Depth/SemanticSeg)
    Image(ImageData),

    /// LiDAR point cloud
    PointCloud(PointCloudData),

    /// IMU data
    Imu(ImuData),

    /// GNSS data
    Gnss(GnssData),

    /// Radar data
    Radar(RadarData),

    /// Raw bytes (fallback)
    Raw(Bytes),
}

/// Image data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    /// Image width
    pub width: u32,

    /// Image height
    pub height: u32,

    /// Pixel format
    pub format: ImageFormat,

    /// Raw pixel data
    pub data: Bytes,
}

/// Image format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Rgb8,
    Rgba8,
    Bgra8,
    Depth,
    SemanticSeg,
}

/// LiDAR point cloud data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointCloudData {
    /// Number of points
    pub num_points: u32,

    /// Bytes per point (typically 16: x,y,z,intensity)
    pub point_stride: u32,

    /// Point cloud data
    pub data: Bytes,
}

/// IMU data
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ImuData {
    /// Accelerometer (m/s²)
    pub accelerometer: Vector3,

    /// Gyroscope (rad/s)
    pub gyroscope: Vector3,

    /// Compass (rad)
    pub compass: f64,
}

/// GNSS data
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GnssData {
    /// Latitude (degrees)
    pub latitude: f64,

    /// Longitude (degrees)
    pub longitude: f64,

    /// Altitude (meters)
    pub altitude: f64,
}

/// Radar data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadarData {
    /// Number of detections
    pub num_detections: u32,

    /// Detection data
    pub data: Bytes,
}

/// 3D vector
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
