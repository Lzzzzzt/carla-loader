//! Mock sensor source
//!
//! For testing without CARLA environment.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use async_channel::{bounded, Receiver};
use bytes::Bytes;
use contracts::{
    GnssData, ImageData, ImageFormat, ImuData, PointCloudData, SensorPacket, SensorPayload,
    SensorType, Vector3,
};
use tracing::{debug, trace};

use crate::config::IngestionMetrics;

/// Mock sensor source configuration
#[derive(Debug, Clone)]
pub struct MockSensorConfig {
    /// Sensor ID
    pub sensor_id: String,

    /// Sensor type
    pub sensor_type: SensorType,

    /// Send frequency (Hz)
    pub frequency_hz: f64,

    /// Image width (Camera only)
    pub image_width: u32,

    /// Image height (Camera only)
    pub image_height: u32,

    /// LiDAR point count (Lidar only)
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

/// Mock sensor source
///
/// Generates simulated sensor data for testing.
pub struct MockSensorSource {
    config: MockSensorConfig,
    running: Arc<AtomicBool>,
}

impl MockSensorSource {
    /// Create new Mock sensor source
    pub fn new(config: MockSensorConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create Mock Camera source
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

    /// Create Mock LiDAR source
    pub fn lidar(sensor_id: &str, frequency_hz: f64, num_points: u32) -> Self {
        Self::new(MockSensorConfig {
            sensor_id: sensor_id.to_string(),
            sensor_type: SensorType::Lidar,
            frequency_hz,
            lidar_points: num_points,
            ..Default::default()
        })
    }

    /// Create Mock IMU source
    pub fn imu(sensor_id: &str, frequency_hz: f64) -> Self {
        Self::new(MockSensorConfig {
            sensor_id: sensor_id.to_string(),
            sensor_type: SensorType::Imu,
            frequency_hz,
            ..Default::default()
        })
    }

    /// Create Mock GNSS source
    pub fn gnss(sensor_id: &str, frequency_hz: f64) -> Self {
        Self::new(MockSensorConfig {
            sensor_id: sensor_id.to_string(),
            sensor_type: SensorType::Gnss,
            frequency_hz,
            ..Default::default()
        })
    }

    /// Start Mock source, returns data stream receiver
    ///
    /// # Arguments
    /// * `channel_capacity` - Channel capacity
    /// * `metrics` - Optional metrics instance
    pub fn start(
        &self,
        channel_capacity: usize,
        metrics: Option<Arc<IngestionMetrics>>,
    ) -> Receiver<SensorPacket> {
        let (tx, rx) = bounded(channel_capacity);
        let config = self.config.clone();
        let running = self.running.clone();
        let metrics = metrics.unwrap_or_else(|| Arc::new(IngestionMetrics::new()));

        running.store(true, Ordering::SeqCst);

        thread::spawn(move || {
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
                    sensor_id: config.sensor_id.clone().into(),
                    sensor_type: config.sensor_type,
                    timestamp,
                    frame_id: Some(frame_id),
                    payload,
                };

                metrics.record_received();

                if tx.send_blocking(packet).is_err() {
                    debug!(sensor_id = %config.sensor_id, "mock sensor channel closed");
                    break;
                }

                trace!(
                    sensor_id = %config.sensor_id,
                    frame_id,
                    timestamp,
                    "mock packet sent"
                );

                thread::sleep(interval);
            }

            debug!(sensor_id = %config.sensor_id, "mock sensor source stopped");
        });

        rx
    }

    /// Stop Mock source
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_camera_source() {
        let source = MockSensorSource::camera("test_cam", 100.0, 100, 100);
        let rx = source.start(10, None);

        // Receive a few packets
        for _ in 0..3 {
            let packet = rx.recv_blocking().unwrap();
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

    #[test]
    fn test_mock_imu_source() {
        let source = MockSensorSource::imu("test_imu", 100.0);
        let rx = source.start(10, None);

        let packet = rx.recv_blocking().unwrap();
        assert_eq!(packet.sensor_type, SensorType::Imu);

        if let SensorPayload::Imu(imu) = packet.payload {
            assert!((imu.accelerometer.z - 9.81).abs() < 0.01);
        } else {
            panic!("expected Imu payload");
        }

        source.stop();
    }
}
