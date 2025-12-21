//! LiDAR 传感器适配器

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bytes::Bytes;
use contracts::{DropPolicy, PointCloudData, SensorPacket, SensorPayload, SensorType};
use tokio::sync::mpsc;
use tracing::{debug, trace, warn};

#[cfg(feature = "real-carla")]
use carla::client::Sensor;
#[cfg(feature = "real-carla")]
use carla::sensor::SensorDataBase;
#[cfg(feature = "real-carla")]
use carla::sensor::data::LidarMeasurement;

use crate::adapter::SensorAdapter;
use crate::config::{BackpressureConfig, IngestionMetrics};

/// LidarDetection 每点 16 字节 (x: f32, y: f32, z: f32, intensity: f32)
const POINT_STRIDE: u32 = 16;

/// LiDAR 传感器适配器
pub struct LidarAdapter {
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

impl LidarAdapter {
    /// 创建新的 LiDAR 适配器
    #[cfg(feature = "real-carla")]
    pub fn new(sensor_id: String, sensor: Sensor, config: BackpressureConfig) -> Self {
        Self {
            sensor_id,
            config,
            listening: Arc::new(AtomicBool::new(false)),
            sensor,
        }
    }

    /// 创建新的 LiDAR 适配器（无 CARLA）
    #[cfg(not(feature = "real-carla"))]
    pub fn new_mock(sensor_id: String, config: BackpressureConfig) -> Self {
        Self {
            sensor_id,
            config,
            listening: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl SensorAdapter for LidarAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn sensor_type(&self) -> SensorType {
        SensorType::Lidar
    }

    #[cfg(feature = "real-carla")]
    fn start(&self, tx: mpsc::Sender<SensorPacket>, metrics: Arc<IngestionMetrics>) {
        if self.listening.swap(true, Ordering::SeqCst) {
            warn!(sensor_id = %self.sensor_id, "lidar adapter already listening");
            return;
        }

        let sensor_id = self.sensor_id.clone();
        let drop_policy = self.config.drop_policy;
        let listening = self.listening.clone();

        debug!(sensor_id = %sensor_id, "starting lidar adapter");

        self.sensor.listen(move |sensor_data| {
            if !listening.load(Ordering::Relaxed) {
                return;
            }

            let lidar = match LidarMeasurement::try_from(sensor_data.clone()) {
                Ok(l) => l,
                Err(_) => {
                    metrics.record_parse_error();
                    trace!(sensor_id = %sensor_id, "failed to parse lidar data");
                    return;
                }
            };

            let timestamp = sensor_data.timestamp();
            let frame_id = Some(sensor_data.frame() as u64);

            // 获取点云数据
            let points = lidar.as_slice();
            let num_points = points.len() as u32;

            // 将点云数据转换为字节
            // SAFETY: LidarDetection 是 POD 类型，可以安全地转换为字节
            let data = unsafe {
                let ptr = points.as_ptr() as *const u8;
                let len = points.len() * POINT_STRIDE as usize;
                Bytes::copy_from_slice(std::slice::from_raw_parts(ptr, len))
            };

            let point_cloud = PointCloudData {
                num_points,
                point_stride: POINT_STRIDE,
                data,
            };

            let packet = SensorPacket {
                sensor_id: sensor_id.clone(),
                sensor_type: SensorType::Lidar,
                timestamp,
                frame_id,
                payload: SensorPayload::PointCloud(point_cloud),
            };

            metrics.record_received();

            match tx.try_send(packet) {
                Ok(_) => {
                    trace!(sensor_id = %sensor_id, num_points, "lidar packet sent");
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    metrics.record_dropped();
                    match drop_policy {
                        DropPolicy::DropNewest => {
                            trace!(sensor_id = %sensor_id, "lidar packet dropped (newest)");
                        }
                        DropPolicy::DropOldest => {
                            trace!(sensor_id = %sensor_id, "lidar packet dropped (oldest fallback)");
                        }
                    }
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    warn!(sensor_id = %sensor_id, "lidar channel closed");
                }
            }
        });
    }

    #[cfg(not(feature = "real-carla"))]
    fn start(&self, _tx: mpsc::Sender<SensorPacket>, _metrics: Arc<IngestionMetrics>) {
        self.listening.store(true, Ordering::SeqCst);
        warn!(sensor_id = %self.sensor_id, "lidar adapter started in mock mode (no data)");
    }

    fn stop(&self) {
        if self.listening.swap(false, Ordering::SeqCst) {
            debug!(sensor_id = %self.sensor_id, "stopping lidar adapter");
            #[cfg(feature = "real-carla")]
            self.sensor.stop();
        }
    }

    fn is_listening(&self) -> bool {
        self.listening.load(Ordering::Relaxed)
    }
}
