//! IMU 传感器适配器

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use contracts::{DropPolicy, ImuData, SensorPacket, SensorPayload, SensorType, Vector3};
use tokio::sync::mpsc;
use tracing::{debug, trace, warn};

#[cfg(feature = "real-carla")]
use carla::client::Sensor;
#[cfg(feature = "real-carla")]
use carla::sensor::SensorDataBase;
#[cfg(feature = "real-carla")]
use carla::sensor::data::ImuMeasurement;

use crate::adapter::SensorAdapter;
use crate::config::{BackpressureConfig, IngestionMetrics};

/// IMU 传感器适配器
pub struct ImuAdapter {
    /// 传感器 ID
    sensor_id: String,

    /// 背压配置
    config: BackpressureConfig,

    /// 是否正在监听
    listening: Arc<AtomicBool>,

    /// CARLA 传感器
    #[cfg(feature = "real-carla")]
    sensor: Sensor,
}

impl ImuAdapter {
    /// 创建新的 IMU 适配器
    #[cfg(feature = "real-carla")]
    pub fn new(sensor_id: String, sensor: Sensor, config: BackpressureConfig) -> Self {
        Self {
            sensor_id,
            config,
            listening: Arc::new(AtomicBool::new(false)),
            sensor,
        }
    }

    /// 创建新的 IMU 适配器（无 CARLA）
    #[cfg(not(feature = "real-carla"))]
    pub fn new_mock(sensor_id: String, config: BackpressureConfig) -> Self {
        Self {
            sensor_id,
            config,
            listening: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl SensorAdapter for ImuAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn sensor_type(&self) -> SensorType {
        SensorType::Imu
    }

    #[cfg(feature = "real-carla")]
    fn start(&self, tx: mpsc::Sender<SensorPacket>, metrics: Arc<IngestionMetrics>) {
        if self.listening.swap(true, Ordering::SeqCst) {
            warn!(sensor_id = %self.sensor_id, "imu adapter already listening");
            return;
        }

        let sensor_id = self.sensor_id.clone();
        let drop_policy = self.config.drop_policy;
        let listening = self.listening.clone();

        debug!(sensor_id = %sensor_id, "starting imu adapter");

        self.sensor.listen(move |sensor_data| {
            if !listening.load(Ordering::Relaxed) {
                return;
            }

            let imu = match ImuMeasurement::try_from(sensor_data.clone()) {
                Ok(i) => i,
                Err(_) => {
                    metrics.record_parse_error();
                    trace!(sensor_id = %sensor_id, "failed to parse imu data");
                    return;
                }
            };

            let timestamp = sensor_data.timestamp();
            let frame_id = Some(sensor_data.frame() as u64);

            // 转换 IMU 数据
            let accel = imu.accelerometer();
            let gyro = imu.gyroscope();
            let compass = imu.compass();

            let imu_data = ImuData {
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
                compass: compass as f64,
            };

            let packet = SensorPacket {
                sensor_id: sensor_id.clone(),
                sensor_type: SensorType::Imu,
                timestamp,
                frame_id,
                payload: SensorPayload::Imu(imu_data),
            };

            metrics.record_received();

            match tx.try_send(packet) {
                Ok(_) => {
                    trace!(sensor_id = %sensor_id, "imu packet sent");
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    metrics.record_dropped();
                    match drop_policy {
                        DropPolicy::DropNewest => {
                            trace!(sensor_id = %sensor_id, "imu packet dropped (newest)");
                        }
                        DropPolicy::DropOldest => {
                            trace!(sensor_id = %sensor_id, "imu packet dropped (oldest fallback)");
                        }
                    }
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    warn!(sensor_id = %sensor_id, "imu channel closed");
                }
            }
        });
    }

    #[cfg(not(feature = "real-carla"))]
    fn start(&self, _tx: mpsc::Sender<SensorPacket>, _metrics: Arc<IngestionMetrics>) {
        self.listening.store(true, Ordering::SeqCst);
        warn!(sensor_id = %self.sensor_id, "imu adapter started in mock mode (no data)");
    }

    fn stop(&self) {
        if self.listening.swap(false, Ordering::SeqCst) {
            debug!(sensor_id = %self.sensor_id, "stopping imu adapter");
            #[cfg(feature = "real-carla")]
            self.sensor.stop();
        }
    }

    fn is_listening(&self) -> bool {
        self.listening.load(Ordering::Relaxed)
    }
}
