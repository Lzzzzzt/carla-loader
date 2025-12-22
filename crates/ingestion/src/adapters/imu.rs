//! IMU 传感器适配器

use contracts::{ImuData, SensorPayload, Vector3};

#[cfg(feature = "real-carla")]
use carla::sensor::data::ImuMeasurement;

/// 将 IMU 测量转换为 SensorPayload
#[cfg(feature = "real-carla")]
#[inline]
fn imu_to_payload(imu: &ImuMeasurement) -> SensorPayload {
    let accel = imu.accelerometer();
    let gyro = imu.gyroscope();
    SensorPayload::Imu(ImuData {
        accelerometer: Vector3 {
            x: accel.x as f64,
            y: accel.y as f64,
            z: accel.z as f64,
        },
        gyroscope: Vector3 {
            x: gyro.x as f64,
            y: gyro.y as f64,
            z: gyro.z as f64,
        },
        compass: imu.compass() as f64,
    })
}

define_sensor_adapter!(ImuAdapter, SensorType::Imu, ImuMeasurement, imu_to_payload);
