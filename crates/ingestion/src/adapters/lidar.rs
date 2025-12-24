//! LiDAR 传感器适配器

#[cfg(feature = "real-carla")]
use contracts::{PointCloudData, SensorPayload};

#[cfg(feature = "real-carla")]
use carla::sensor::data::LidarMeasurement;

#[cfg(feature = "real-carla")]
use crate::adapters::common::pod_slice_to_bytes_unchecked;

/// LidarDetection 每点 16 字节 (x: f32, y: f32, z: f32, intensity: f32)
#[cfg(feature = "real-carla")]
const POINT_STRIDE: u32 = 16;

/// 将 LiDAR 测量转换为 SensorPayload
#[cfg(feature = "real-carla")]
#[inline]
fn lidar_to_payload(lidar: &LidarMeasurement) -> SensorPayload {
    let points = lidar.as_slice();
    // SAFETY: LidarDetection 是 POD 类型 (x, y, z, intensity: f32)
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
