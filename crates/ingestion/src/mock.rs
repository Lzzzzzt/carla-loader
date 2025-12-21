//! Mock 传感器源
//!
//! 用于无 CARLA 环境的测试。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bytes::Bytes;
use contracts::{
    GnssData, ImageData, ImageFormat, ImuData, PointCloudData, SensorPacket, SensorPayload,
    SensorType, Vector3,
};
use tokio::sync::mpsc;
use tracing::{debug, trace};

use crate::config::IngestionMetrics;

/// Mock 传感器源配置
#[derive(Debug, Clone)]
pub struct MockSensorConfig {
    /// 传感器 ID
    pub sensor_id: String,

    /// 传感器类型
    pub sensor_type: SensorType,

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
            sensor_id: "mock_sensor".to_string(),
            sensor_type: SensorType::Camera,
            frequency_hz: 10.0,
            image_width: 800,
            image_height: 600,
            lidar_points: 10000,
        }
    }
}

/// Mock 传感器源
///
/// 生成模拟的传感器数据用于测试。
pub struct MockSensorSource {
    config: MockSensorConfig,
    running: Arc<AtomicBool>,
}

impl MockSensorSource {
    /// 创建新的 Mock 传感器源
    pub fn new(config: MockSensorConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 创建 Mock Camera 源
    pub fn camera(sensor_id: &str, frequency_hz: f64, width: u32, height: u32) -> Self {
        Self::new(MockSensorConfig {
            sensor_id: sensor_id.to_string(),
            sensor_type: SensorType::Camera,
            frequency_hz,
            image_width: width,
            image_height: height,
            ..Default::default()
        })
    }

    /// 创建 Mock LiDAR 源
    pub fn lidar(sensor_id: &str, frequency_hz: f64, num_points: u32) -> Self {
        Self::new(MockSensorConfig {
            sensor_id: sensor_id.to_string(),
            sensor_type: SensorType::Lidar,
            frequency_hz,
            lidar_points: num_points,
            ..Default::default()
        })
    }

    /// 创建 Mock IMU 源
    pub fn imu(sensor_id: &str, frequency_hz: f64) -> Self {
        Self::new(MockSensorConfig {
            sensor_id: sensor_id.to_string(),
            sensor_type: SensorType::Imu,
            frequency_hz,
            ..Default::default()
        })
    }

    /// 创建 Mock GNSS 源
    pub fn gnss(sensor_id: &str, frequency_hz: f64) -> Self {
        Self::new(MockSensorConfig {
            sensor_id: sensor_id.to_string(),
            sensor_type: SensorType::Gnss,
            frequency_hz,
            ..Default::default()
        })
    }

    /// 启动 Mock 源，返回数据流接收端
    ///
    /// # Arguments
    /// * `channel_capacity` - 通道容量
    /// * `metrics` - 可选的 metrics 实例
    pub fn start(
        &self,
        channel_capacity: usize,
        metrics: Option<Arc<IngestionMetrics>>,
    ) -> mpsc::Receiver<SensorPacket> {
        let (tx, rx) = mpsc::channel(channel_capacity);
        let config = self.config.clone();
        let running = self.running.clone();
        let metrics = metrics.unwrap_or_else(|| Arc::new(IngestionMetrics::new()));

        running.store(true, Ordering::SeqCst);

        tokio::spawn(async move {
            let interval = Duration::from_secs_f64(1.0 / config.frequency_hz);
            let mut frame_id: u64 = 0;
            let start_time = std::time::Instant::now();

            debug!(
                sensor_id = %config.sensor_id,
                sensor_type = ?config.sensor_type,
                frequency_hz = config.frequency_hz,
                "mock sensor source started"
            );

            while running.load(Ordering::Relaxed) {
                let timestamp = start_time.elapsed().as_secs_f64();
                frame_id += 1;

                let payload = match config.sensor_type {
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
                    SensorType::Radar => SensorPayload::Radar(contracts::RadarData {
                        num_detections: 5,
                        data: Bytes::from(vec![0u8; 5 * 16]),
                    }),
                };

                let packet = SensorPacket {
                    sensor_id: config.sensor_id.clone(),
                    sensor_type: config.sensor_type,
                    timestamp,
                    frame_id: Some(frame_id),
                    payload,
                };

                metrics.record_received();

                if tx.send(packet).await.is_err() {
                    debug!(sensor_id = %config.sensor_id, "mock sensor channel closed");
                    break;
                }

                trace!(
                    sensor_id = %config.sensor_id,
                    frame_id,
                    timestamp,
                    "mock packet sent"
                );

                tokio::time::sleep(interval).await;
            }

            debug!(sensor_id = %config.sensor_id, "mock sensor source stopped");
        });

        rx
    }

    /// 停止 Mock 源
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// 检查是否正在运行
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_camera_source() {
        let source = MockSensorSource::camera("test_cam", 100.0, 100, 100);
        let mut rx = source.start(10, None);

        // 接收几个包
        for _ in 0..3 {
            let packet = rx.recv().await.unwrap();
            assert_eq!(packet.sensor_id, "test_cam");
            assert_eq!(packet.sensor_type, SensorType::Camera);
            assert!(packet.frame_id.is_some());

            if let SensorPayload::Image(img) = packet.payload {
                assert_eq!(img.width, 100);
                assert_eq!(img.height, 100);
            } else {
                panic!("expected Image payload");
            }
        }

        source.stop();
    }

    #[tokio::test]
    async fn test_mock_imu_source() {
        let source = MockSensorSource::imu("test_imu", 100.0);
        let mut rx = source.start(10, None);

        let packet = rx.recv().await.unwrap();
        assert_eq!(packet.sensor_type, SensorType::Imu);

        if let SensorPayload::Imu(imu) = packet.payload {
            assert!((imu.accelerometer.z - 9.81).abs() < 0.01);
        } else {
            panic!("expected Imu payload");
        }

        source.stop();
    }
}
