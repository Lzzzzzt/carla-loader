//! Mock sensor implementation
//!
//! Implements `SensorSource` trait, generates simulated sensor data.
//! Used for testing and development without CARLA environment.

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

/// Mock sensor configuration
#[derive(Debug, Clone)]
pub struct MockSensorConfig {
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
            frequency_hz: 20.0,
            image_width: 800,
            image_height: 600,
            lidar_points: 10000,
        }
    }
}

/// Mock sensor
///
/// Implements `SensorSource` trait, generates simulated data at specified frequency in background thread.
/// Data is sent through callback function, consistent with real CARLA sensor behavior.
pub struct MockSensor {
    sensor_id: String,
    sensor_type: SensorType,
    config: MockSensorConfig,
    listening: Arc<AtomicBool>,
}

impl MockSensor {
    /// Create new Mock sensor
    pub fn new(sensor_id: String, sensor_type: SensorType, config: MockSensorConfig) -> Self {
        Self {
            sensor_id,
            sensor_type,
            config,
            listening: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create Mock sensor with default configuration
    pub fn with_defaults(sensor_id: String, sensor_type: SensorType) -> Self {
        Self::new(sensor_id, sensor_type, MockSensorConfig::default())
    }

    /// Generate simulated data payload
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
        // Idempotent: if already listening, don't start again
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

        // Wait for a few packets
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

        // First call
        sensor.listen(Arc::new(move |_| {
            count1.fetch_add(1, Ordering::Relaxed);
        }));

        // Second call should be ignored
        sensor.listen(Arc::new(move |_| {
            count2.fetch_add(100, Ordering::Relaxed);
        }));

        thread::sleep(Duration::from_millis(100));
        sensor.stop();

        // Should only have count from first callback
        let final_count = count.load(Ordering::Relaxed);
        assert!(final_count > 0);
        assert!(final_count < 50); // 100ms max ~20 packets (default 20Hz)
    }
}
