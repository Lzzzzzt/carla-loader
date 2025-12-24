//! GNSS 传感器适配器

#[cfg(feature = "real-carla")]
use contracts::{GnssData, SensorPayload};

#[cfg(feature = "real-carla")]
use carla::sensor::data::GnssMeasurement;

/// 将 GNSS 测量转换为 SensorPayload
#[cfg(feature = "real-carla")]
#[inline]
fn gnss_to_payload(gnss: &GnssMeasurement) -> SensorPayload {
    SensorPayload::Gnss(GnssData {
        latitude: gnss.latitude(),
        longitude: gnss.longitude(),
        altitude: gnss.attitude(), // Note: carla-rust uses attitude() for altitude
    })
}

define_sensor_adapter!(
    GnssAdapter,
    SensorType::Gnss,
    GnssMeasurement,
    gnss_to_payload
);
