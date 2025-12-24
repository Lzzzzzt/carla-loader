//! Mock 传感器实现
//!
//! 实现 `SensorSource` trait，生成模拟传感器数据。
//! 用于无 CARLA 环境时的测试和开发。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use bytes::Bytes;
use contracts::{
    GnssData, ImageData, ImageFormat, ImuData, PointCloudData, RadarData, SensorDataCallback,
    SensorPacket, SensorPayload, SensorSource, SensorType, Vector3,
};
use tracing::{debug, trace};

/// Mock 传感器配置
#[derive(Debug, Clone)]
pub struct MockSensorConfig {
    /// 发送频率 (Hz)
    pub frequency_hz: f64,
    /// 图像宽度（仅 Camera）
    pub image_width: u32,
    /// 图像高度（仅 Camera）
    pub image_height: u32,
    /// LiDAR 点数（仅 Lidar）
    pub lidar_points: u32,
}

impl Default for MockSensorConfig {
    fn default() -> Self {
        Self {
            frequency_hz: 20.0,
            image_width: 800,
            image_height: 600,
            lidar_points: 10000,
        }
    }
}

/// Mock 传感器
///
/// 实现 `SensorSource` trait，在后台线程中按指定频率生成模拟数据。
/// 数据通过回调函数发送，与真实 CARLA 传感器的行为一致。
pub struct MockSensor {
    sensor_id: String,
    sensor_type: SensorType,
    config: MockSensorConfig,
    listening: Arc<AtomicBool>,
}

impl MockSensor {
    /// 创建新的 Mock 传感器
    pub fn new(sensor_id: String, sensor_type: SensorType, config: MockSensorConfig) -> Self {
        Self {
            sensor_id,
            sensor_type,
            config,
            listening: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 使用默认配置创建 Mock 传感器
    pub fn with_defaults(sensor_id: String, sensor_type: SensorType) -> Self {
        Self::new(sensor_id, sensor_type, MockSensorConfig::default())
    }

    /// 生成模拟数据载荷
    fn generate_payload(
        config: &MockSensorConfig,
        sensor_type: SensorType,
        frame_id: u64,
    ) -> SensorPayload {
        match sensor_type {
            SensorType::Camera => {
                let size = (config.image_width * config.image_height * 4) as usize;
                SensorPayload::Image(ImageData {
                    width: config.image_width,
                    height: config.image_height,
                    format: ImageFormat::Bgra8,
                    data: Bytes::from(vec![128u8; size]),
                })
            }
            SensorType::Lidar => {
                let size = (config.lidar_points * 16) as usize;
                SensorPayload::PointCloud(PointCloudData {
                    num_points: config.lidar_points,
                    point_stride: 16,
                    data: Bytes::from(vec![0u8; size]),
                })
            }
            SensorType::Imu => SensorPayload::Imu(ImuData {
                accelerometer: Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 9.81,
                },
                gyroscope: Vector3::default(),
                compass: 0.0,
            }),
            SensorType::Gnss => SensorPayload::Gnss(GnssData {
                latitude: 40.0 + (frame_id as f64 * 0.0001),
                longitude: -74.0 + (frame_id as f64 * 0.0001),
                altitude: 100.0,
            }),
            SensorType::Radar => SensorPayload::Radar(RadarData {
                num_detections: 5,
                data: Bytes::from(vec![0u8; 5 * 16]),
            }),
        }
    }
}

impl SensorSource for MockSensor {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn sensor_type(&self) -> SensorType {
        self.sensor_type
    }

    fn listen(&self, callback: SensorDataCallback) {
        // 幂等：如果已经在监听，不重复启动
        if self.listening.swap(true, Ordering::SeqCst) {
            return;
        }

        let sensor_id = self.sensor_id.clone();
        let sensor_type = self.sensor_type;
        let config = self.config.clone();
        let listening = self.listening.clone();

        let interval = Duration::from_secs_f64(1.0 / config.frequency_hz);

        thread::spawn(move || {
            let mut frame_id: u64 = 0;
            let start_time = std::time::Instant::now();

            debug!(
                sensor_id = %sensor_id,
                sensor_type = ?sensor_type,
                frequency_hz = config.frequency_hz,
                "mock sensor started"
            );

            while listening.load(Ordering::Relaxed) {
                frame_id += 1;
                let timestamp = start_time.elapsed().as_secs_f64();

                let payload = Self::generate_payload(&config, sensor_type, frame_id);

                let packet = SensorPacket {
                    sensor_id: sensor_id.clone().into(),
                    sensor_type,
                    timestamp,
                    frame_id: Some(frame_id),
                    payload,
                };

                callback(packet);

                trace!(
                    sensor_id = %sensor_id,
                    frame_id,
                    timestamp,
                    "mock packet sent"
                );

                thread::sleep(interval);
            }

            debug!(sensor_id = %sensor_id, "mock sensor stopped");
        });
    }

    fn stop(&self) {
        self.listening.store(false, Ordering::SeqCst);
    }

    fn is_listening(&self) -> bool {
        self.listening.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;
    use std::time::Duration;

    #[test]
    fn test_mock_sensor_camera() {
        let sensor = MockSensor::new(
            "test_camera".to_string(),
            SensorType::Camera,
            MockSensorConfig {
                frequency_hz: 100.0,
                image_width: 100,
                image_height: 100,
                ..Default::default()
            },
        );

        let count = Arc::new(AtomicU64::new(0));
        let count_clone = count.clone();

        sensor.listen(Arc::new(move |packet| {
            assert_eq!(packet.sensor_id, "test_camera");
            assert_eq!(packet.sensor_type, SensorType::Camera);
            count_clone.fetch_add(1, Ordering::Relaxed);
        }));

        // 等待几个数据包
        thread::sleep(Duration::from_millis(50));
        sensor.stop();

        assert!(count.load(Ordering::Relaxed) > 0);
        assert!(!sensor.is_listening());
    }

    #[test]
    fn test_mock_sensor_imu() {
        let sensor = MockSensor::with_defaults("test_imu".to_string(), SensorType::Imu);

        let received_imu = Arc::new(AtomicBool::new(false));
        let received_clone = received_imu.clone();

        sensor.listen(Arc::new(move |packet| {
            if let SensorPayload::Imu(imu) = packet.payload {
                assert!((imu.accelerometer.z - 9.81).abs() < 0.01);
                received_clone.store(true, Ordering::Relaxed);
            }
        }));

        thread::sleep(Duration::from_millis(100));
        sensor.stop();

        assert!(received_imu.load(Ordering::Relaxed));
    }

    #[test]
    fn test_mock_sensor_idempotent_listen() {
        let sensor = MockSensor::with_defaults("test".to_string(), SensorType::Camera);

        let count = Arc::new(AtomicU64::new(0));
        let count1 = count.clone();
        let count2 = count.clone();

        // 第一次调用
        sensor.listen(Arc::new(move |_| {
            count1.fetch_add(1, Ordering::Relaxed);
        }));

        // 第二次调用应该被忽略
        sensor.listen(Arc::new(move |_| {
            count2.fetch_add(100, Ordering::Relaxed);
        }));

        thread::sleep(Duration::from_millis(100));
        sensor.stop();

        // 应该只有来自第一个回调的计数
        let final_count = count.load(Ordering::Relaxed);
        assert!(final_count > 0);
        assert!(final_count < 50); // 100ms 最多约 20 个包（默认 20Hz）
    }
}
