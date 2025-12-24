//! LiDAR sensor adapter

#[cfg(feature = "real-carla")]
use contracts::{PointCloudData, SensorPayload};

#[cfg(feature = "real-carla")]
use carla::sensor::data::LidarMeasurement;

#[cfg(feature = "real-carla")]
use crate::adapters::common::pod_slice_to_bytes_unchecked;

/// LidarDetection 16 bytes per point (x: f32, y: f32, z: f32, intensity: f32)
#[cfg(feature = "real-carla")]
const POINT_STRIDE: u32 = 16;

/// Convert LiDAR measurement to SensorPayload
#[cfg(feature = "real-carla")]
#[inline]
fn lidar_to_payload(lidar: &LidarMeasurement) -> SensorPayload {
    let points = lidar.as_slice();
    // SAFETY: LidarDetection is a POD type (x, y, z, intensity: f32)
    let data = unsafe { pod_slice_to_bytes_unchecked(points) };
    SensorPayload::PointCloud(PointCloudData {
        num_points: points.len() as u32,
        point_stride: POINT_STRIDE,
        data,
    })
}

define_sensor_adapter!(
    LidarAdapter,
    SensorType::Lidar,
    LidarMeasurement,
    lidar_to_payload
);
