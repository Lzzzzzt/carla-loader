//! Camera 传感器适配器

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bytes::Bytes;
use contracts::{DropPolicy, ImageData, ImageFormat, SensorPacket, SensorPayload, SensorType};
use tokio::sync::mpsc;
use tracing::{debug, trace, warn};

#[cfg(feature = "real-carla")]
use carla::client::Sensor;
#[cfg(feature = "real-carla")]
use carla::sensor::data::Image;
#[cfg(feature = "real-carla")]
use carla::sensor::SensorDataBase;

use crate::adapter::SensorAdapter;
use crate::config::{BackpressureConfig, IngestionMetrics};

/// Camera 传感器适配器
pub struct CameraAdapter {
    /// 传感器 ID
    sensor_id: String,

    /// 背压配置
    config: BackpressureConfig,

    /// 是否正在监听
    listening: Arc<AtomicBool>,

    /// CARLA 传感器（仅在启用 real-carla 时存在）
    #[cfg(feature = "real-carla")]
    sensor: Sensor,
}

impl CameraAdapter {
    /// 创建新的 Camera 适配器
    #[cfg(feature = "real-carla")]
    pub fn new(sensor_id: String, sensor: Sensor, config: BackpressureConfig) -> Self {
        Self {
            sensor_id,
            config,
            listening: Arc::new(AtomicBool::new(false)),
            sensor,
        }
    }

    /// 创建新的 Camera 适配器（无 CARLA）
    #[cfg(not(feature = "real-carla"))]
    pub fn new_mock(sensor_id: String, config: BackpressureConfig) -> Self {
        Self {
            sensor_id,
            config,
            listening: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl SensorAdapter for CameraAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn sensor_type(&self) -> SensorType {
        SensorType::Camera
    }

    #[cfg(feature = "real-carla")]
    fn start(&self, tx: mpsc::Sender<SensorPacket>, metrics: Arc<IngestionMetrics>) {
        if self.listening.swap(true, Ordering::SeqCst) {
            warn!(sensor_id = %self.sensor_id, "camera adapter already listening");
            return;
        }

        let sensor_id = self.sensor_id.clone();
        let drop_policy = self.config.drop_policy;
        let listening = self.listening.clone();

        debug!(sensor_id = %sensor_id, "starting camera adapter");

        self.sensor.listen(move |sensor_data| {
            // 检查是否应该继续监听
            if !listening.load(Ordering::Relaxed) {
                return;
            }

            // 尝试转换为 Image
            let image = match Image::try_from(sensor_data.clone()) {
                Ok(img) => img,
                Err(_) => {
                    metrics.record_parse_error();
                    trace!(sensor_id = %sensor_id, "failed to parse camera data");
                    return;
                }
            };

            // 获取时间戳和帧 ID
            let timestamp = sensor_data.timestamp();
            let frame_id = Some(sensor_data.frame() as u64);

            // 复制图像数据到 Bytes
            let raw_bytes = image.as_raw_bytes();
            let data = Bytes::copy_from_slice(raw_bytes);
            let payload_size = data.len() as u64;

            // 构建 ImageData
            let image_data = ImageData {
                width: image.width() as u32,
                height: image.height() as u32,
                format: ImageFormat::Bgra8, // CARLA 默认 BGRA
                data,
            };

            // 构建 SensorPacket
            let packet = SensorPacket {
                sensor_id: sensor_id.clone(),
                sensor_type: SensorType::Camera,
                timestamp,
                frame_id,
                payload: SensorPayload::Image(image_data),
            };

            metrics.record_received();
            metrics::counter!("ingestion_packets_total", "sensor_id" => sensor_id.clone(), "status" => "ok")
                .increment(1);
            metrics::counter!("ingestion_bytes_total", "sensor_id" => sensor_id.clone())
                .increment(payload_size);

            // 非阻塞发送
            match tx.try_send(packet) {
                Ok(_) => {
                    trace!(sensor_id = %sensor_id, "camera packet sent");
                }
                Err(mpsc::error::TrySendError::Full(packet)) => {
                    match drop_policy {
                        DropPolicy::DropNewest => {
                            // 丢弃当前包
                            metrics.record_dropped();
                            metrics::counter!(
                                "ingestion_packets_total",
                                "sensor_id" => sensor_id.clone(),
                                "status" => "dropped"
                            )
                            .increment(1);
                            trace!(sensor_id = %sensor_id, "camera packet dropped (newest)");
                        }
                        DropPolicy::DropOldest => {
                            // TODO: 需要使用支持 pop 的通道实现
                            // 目前简单丢弃当前包
                            metrics.record_dropped();
                            metrics::counter!(
                                "ingestion_packets_total",
                                "sensor_id" => sensor_id.clone(),
                                "status" => "dropped"
                            )
                            .increment(1);
                            trace!(sensor_id = %sensor_id, "camera packet dropped (oldest fallback)");
                            let _ = tx.try_send(packet);
                        }
                    }
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    warn!(sensor_id = %sensor_id, "camera channel closed");
                }
            }
        });
    }

    #[cfg(not(feature = "real-carla"))]
    fn start(&self, _tx: mpsc::Sender<SensorPacket>, _metrics: Arc<IngestionMetrics>) {
        self.listening.store(true, Ordering::SeqCst);
        warn!(sensor_id = %self.sensor_id, "camera adapter started in mock mode (no data)");
    }

    fn stop(&self) {
        if self.listening.swap(false, Ordering::SeqCst) {
            debug!(sensor_id = %self.sensor_id, "stopping camera adapter");
            #[cfg(feature = "real-carla")]
            self.sensor.stop();
        }
    }

    fn is_listening(&self) -> bool {
        self.listening.load(Ordering::Relaxed)
    }
}
