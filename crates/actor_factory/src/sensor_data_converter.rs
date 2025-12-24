//! CARLA 传感器数据转换
//!
//! 将 CARLA 原生传感器数据转换为 `SensorPacket`。
//! 仅在 `real-carla` feature 启用时编译。

use bytes::Bytes;
use carla::sensor::data::{
    GnssMeasurement, Image, ImuMeasurement, LidarMeasurement, RadarMeasurement,
};
use carla::sensor::{SensorData, SensorDataBase};
use contracts::{
    GnssData, ImageData, ImageFormat, ImuData, PointCloudData, RadarData, SensorPacket,
    SensorPayload, SensorType, Vector3,
};

/// 将 POD 切片转换为 bytes::Bytes
///
/// # Safety
/// 调用者必须确保 T 是 POD 类型
#[inline]
unsafe fn pod_slice_to_bytes_unchecked<T>(slice: &[T]) -> Bytes {
    let ptr = slice.as_ptr() as *const u8;
    let len = std::mem::size_of_val(slice);
    Bytes::copy_from_slice(std::slice::from_raw_parts(ptr, len))
}

/// 将 CARLA Image 转换为 SensorPayload
fn image_to_payload(image: &Image) -> SensorPayload {
    let data = Bytes::copy_from_slice(image.as_raw_bytes());
    SensorPayload::Image(ImageData {
        width: image.width() as u32,
        height: image.height() as u32,
        format: ImageFormat::Bgra8,
        data,
    })
}

/// 将 CARLA LidarMeasurement 转换为 SensorPayload
fn lidar_to_payload(lidar: &LidarMeasurement) -> SensorPayload {
    let points = lidar.as_slice();
    let data = unsafe { pod_slice_to_bytes_unchecked(points) };
    SensorPayload::PointCloud(PointCloudData {
        num_points: points.len() as u32,
        point_stride: 16, // x, y, z, intensity: f32 each
        data,
    })
}

/// 将 CARLA ImuMeasurement 转换为 SensorPayload
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

/// 将 CARLA GnssMeasurement 转换为 SensorPayload
fn gnss_to_payload(gnss: &GnssMeasurement) -> SensorPayload {
    SensorPayload::Gnss(GnssData {
        latitude: gnss.latitude(),
        longitude: gnss.longitude(),
        altitude: gnss.attitude(), // Note: carla-rust uses attitude() for altitude
    })
}

/// 将 CARLA RadarMeasurement 转换为 SensorPayload
fn radar_to_payload(radar: &RadarMeasurement) -> SensorPayload {
    let detections = radar.as_slice();
    let data = unsafe { pod_slice_to_bytes_unchecked(detections) };
    SensorPayload::Radar(RadarData {
        num_detections: detections.len() as u32,
        data,
    })
}

/// 将 CARLA 传感器数据转换为 SensorPacket
///
/// 根据传感器类型自动选择合适的转换函数。
/// 如果数据类型与传感器类型不匹配，返回 None。
pub fn convert_sensor_data(
    sensor_id: &str,
    sensor_type: SensorType,
    data: &SensorData,
) -> Option<SensorPacket> {
    let timestamp = data.timestamp();
    let frame_id = data.frame() as u64;

    let payload = match sensor_type {
        SensorType::Camera => {
            let image = Image::try_from(data.clone()).ok()?;
            image_to_payload(&image)
        }
        SensorType::Lidar => {
            let lidar = LidarMeasurement::try_from(data.clone()).ok()?;
            lidar_to_payload(&lidar)
        }
        SensorType::Imu => {
            let imu = ImuMeasurement::try_from(data.clone()).ok()?;
            imu_to_payload(&imu)
        }
        SensorType::Gnss => {
            let gnss = GnssMeasurement::try_from(data.clone()).ok()?;
            gnss_to_payload(&gnss)
        }
        SensorType::Radar => {
            let radar = RadarMeasurement::try_from(data.clone()).ok()?;
            radar_to_payload(&radar)
        }
    };

    Some(SensorPacket {
        sensor_id: sensor_id.to_string().into(),
        sensor_type,
        timestamp,
        frame_id: Some(frame_id),
        payload,
    })
}
