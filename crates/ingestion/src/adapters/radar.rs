//! Radar 传感器适配器

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bytes::Bytes;
use contracts::{DropPolicy, RadarData, SensorPacket, SensorPayload, SensorType};
use tokio::sync::mpsc;
use tracing::{debug, trace, warn};

#[cfg(feature = "real-carla")]
use carla::client::Sensor;
#[cfg(feature = "real-carla")]
use carla::sensor::SensorDataBase;
#[cfg(feature = "real-carla")]
use carla::sensor::data::RadarMeasurement;

use crate::adapter::SensorAdapter;
use crate::config::{BackpressureConfig, IngestionMetrics};

/// RadarDetection 每个检测 16 字节 (velocity, azimuth, altitude, depth)
const DETECTION_STRIDE: usize = 16;

/// Radar 传感器适配器
pub struct RadarAdapter {
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

impl RadarAdapter {
    /// 创建新的 Radar 适配器
    #[cfg(feature = "real-carla")]
    pub fn new(sensor_id: String, sensor: Sensor, config: BackpressureConfig) -> Self {
        Self {
            sensor_id,
            config,
            listening: Arc::new(AtomicBool::new(false)),
            sensor,
        }
    }

    /// 创建新的 Radar 适配器（无 CARLA）
    #[cfg(not(feature = "real-carla"))]
    pub fn new_mock(sensor_id: String, config: BackpressureConfig) -> Self {
        Self {
            sensor_id,
            config,
            listening: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl SensorAdapter for RadarAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn sensor_type(&self) -> SensorType {
        SensorType::Radar
    }

    #[cfg(feature = "real-carla")]
    fn start(&self, tx: mpsc::Sender<SensorPacket>, metrics: Arc<IngestionMetrics>) {
        if self.listening.swap(true, Ordering::SeqCst) {
            warn!(sensor_id = %self.sensor_id, "radar adapter already listening");
            return;
        }

        let sensor_id = self.sensor_id.clone();
        let drop_policy = self.config.drop_policy;
        let listening = self.listening.clone();

        debug!(sensor_id = %sensor_id, "starting radar adapter");

        self.sensor.listen(move |sensor_data| {
            if !listening.load(Ordering::Relaxed) {
                return;
            }

            let radar = match RadarMeasurement::try_from(sensor_data.clone()) {
                Ok(r) => r,
                Err(_) => {
                    metrics.record_parse_error();
                    trace!(sensor_id = %sensor_id, "failed to parse radar data");
                    return;
                }
            };

            let timestamp = sensor_data.timestamp();
            let frame_id = Some(sensor_data.frame() as u64);

            // 获取检测数据
            let detections = radar.as_slice();
            let num_detections = detections.len() as u32;

            // 将检测数据转换为字节
            let data = unsafe {
                let ptr = detections.as_ptr() as *const u8;
                let len = detections.len() * DETECTION_STRIDE;
                Bytes::copy_from_slice(std::slice::from_raw_parts(ptr, len))
            };

            let radar_data = RadarData {
                num_detections,
                data,
            };

            let packet = SensorPacket {
                sensor_id: sensor_id.clone(),
                sensor_type: SensorType::Radar,
                timestamp,
                frame_id,
                payload: SensorPayload::Radar(radar_data),
            };

            metrics.record_received();

            match tx.try_send(packet) {
                Ok(_) => {
                    trace!(sensor_id = %sensor_id, num_detections, "radar packet sent");
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    metrics.record_dropped();
                    match drop_policy {
                        DropPolicy::DropNewest => {
                            trace!(sensor_id = %sensor_id, "radar packet dropped (newest)");
                        }
                        DropPolicy::DropOldest => {
                            trace!(sensor_id = %sensor_id, "radar packet dropped (oldest fallback)");
                        }
                    }
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    warn!(sensor_id = %sensor_id, "radar channel closed");
                }
            }
        });
    }

    #[cfg(not(feature = "real-carla"))]
    fn start(&self, _tx: mpsc::Sender<SensorPacket>, _metrics: Arc<IngestionMetrics>) {
        self.listening.store(true, Ordering::SeqCst);
        warn!(sensor_id = %self.sensor_id, "radar adapter started in mock mode (no data)");
    }

    fn stop(&self) {
        if self.listening.swap(false, Ordering::SeqCst) {
            debug!(sensor_id = %self.sensor_id, "stopping radar adapter");
            #[cfg(feature = "real-carla")]
            self.sensor.stop();
        }
    }

    fn is_listening(&self) -> bool {
        self.listening.load(Ordering::Relaxed)
    }
}
