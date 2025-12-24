//! Radar sensor adapter

#[cfg(feature = "real-carla")]
use contracts::{RadarData, SensorPayload};

#[cfg(feature = "real-carla")]
use carla::sensor::data::RadarMeasurement;

#[cfg(feature = "real-carla")]
use crate::adapters::common::pod_slice_to_bytes_unchecked;

/// Convert Radar measurement to SensorPayload
#[cfg(feature = "real-carla")]
#[inline]
fn radar_to_payload(radar: &RadarMeasurement) -> SensorPayload {
    let detections = radar.as_slice();
    // SAFETY: RadarDetection is a POD type (velocity, azimuth, altitude, depth: f32)
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
