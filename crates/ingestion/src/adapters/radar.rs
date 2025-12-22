//! Radar 传感器适配器

use contracts::{RadarData, SensorPayload};

#[cfg(feature = "real-carla")]
use carla::sensor::data::RadarMeasurement;

use crate::adapters::common::pod_slice_to_bytes_unchecked;

/// 将 Radar 测量转换为 SensorPayload
#[cfg(feature = "real-carla")]
#[inline]
fn radar_to_payload(radar: &RadarMeasurement) -> SensorPayload {
    let detections = radar.as_slice();
    // SAFETY: RadarDetection 是 POD 类型 (velocity, azimuth, altitude, depth: f32)
    let data = unsafe { pod_slice_to_bytes_unchecked(detections) };
    SensorPayload::Radar(RadarData {
        num_detections: detections.len() as u32,
        data,
    })
}

define_sensor_adapter!(
    RadarAdapter,
    SensorType::Radar,
    RadarMeasurement,
    radar_to_payload
);
