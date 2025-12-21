//! GNSS 传感器适配器

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use contracts::{DropPolicy, GnssData, SensorPacket, SensorPayload, SensorType};
use tokio::sync::mpsc;
use tracing::{debug, trace, warn};

#[cfg(feature = "real-carla")]
use carla::client::Sensor;
#[cfg(feature = "real-carla")]
use carla::sensor::SensorDataBase;
#[cfg(feature = "real-carla")]
use carla::sensor::data::GnssMeasurement;

use crate::adapter::SensorAdapter;
use crate::config::{BackpressureConfig, IngestionMetrics};

/// GNSS 传感器适配器
pub struct GnssAdapter {
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

impl GnssAdapter {
    /// 创建新的 GNSS 适配器
    #[cfg(feature = "real-carla")]
    pub fn new(sensor_id: String, sensor: Sensor, config: BackpressureConfig) -> Self {
        Self {
            sensor_id,
            config,
            listening: Arc::new(AtomicBool::new(false)),
            sensor,
        }
    }

    /// 创建新的 GNSS 适配器（无 CARLA）
    #[cfg(not(feature = "real-carla"))]
    pub fn new_mock(sensor_id: String, config: BackpressureConfig) -> Self {
        Self {
            sensor_id,
            config,
            listening: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl SensorAdapter for GnssAdapter {
    fn sensor_id(&self) -> &str {
        &self.sensor_id
    }

    fn sensor_type(&self) -> SensorType {
        SensorType::Gnss
    }

    #[cfg(feature = "real-carla")]
    fn start(&self, tx: mpsc::Sender<SensorPacket>, metrics: Arc<IngestionMetrics>) {
        if self.listening.swap(true, Ordering::SeqCst) {
            warn!(sensor_id = %self.sensor_id, "gnss adapter already listening");
            return;
        }

        let sensor_id = self.sensor_id.clone();
        let drop_policy = self.config.drop_policy;
        let listening = self.listening.clone();

        debug!(sensor_id = %sensor_id, "starting gnss adapter");

        self.sensor.listen(move |sensor_data| {
            if !listening.load(Ordering::Relaxed) {
                return;
            }

            let gnss = match GnssMeasurement::try_from(sensor_data.clone()) {
                Ok(g) => g,
                Err(_) => {
                    metrics.record_parse_error();
                    trace!(sensor_id = %sensor_id, "failed to parse gnss data");
                    return;
                }
            };

            let timestamp = sensor_data.timestamp();
            let frame_id = Some(sensor_data.frame() as u64);

            let gnss_data = GnssData {
                latitude: gnss.latitude(),
                longitude: gnss.longitude(),
                altitude: gnss.attitude(), // Note: carla-rust uses attitude() for altitude
            };

            let packet = SensorPacket {
                sensor_id: sensor_id.clone(),
                sensor_type: SensorType::Gnss,
                timestamp,
                frame_id,
                payload: SensorPayload::Gnss(gnss_data),
            };

            metrics.record_received();

            match tx.try_send(packet) {
                Ok(_) => {
                    trace!(sensor_id = %sensor_id, "gnss packet sent");
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    metrics.record_dropped();
                    match drop_policy {
                        DropPolicy::DropNewest => {
                            trace!(sensor_id = %sensor_id, "gnss packet dropped (newest)");
                        }
                        DropPolicy::DropOldest => {
                            trace!(sensor_id = %sensor_id, "gnss packet dropped (oldest fallback)");
                        }
                    }
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    warn!(sensor_id = %sensor_id, "gnss channel closed");
                }
            }
        });
    }

    #[cfg(not(feature = "real-carla"))]
    fn start(&self, _tx: mpsc::Sender<SensorPacket>, _metrics: Arc<IngestionMetrics>) {
        self.listening.store(true, Ordering::SeqCst);
        warn!(sensor_id = %self.sensor_id, "gnss adapter started in mock mode (no data)");
    }

    fn stop(&self) {
        if self.listening.swap(false, Ordering::SeqCst) {
            debug!(sensor_id = %self.sensor_id, "stopping gnss adapter");
            #[cfg(feature = "real-carla")]
            self.sensor.stop();
        }
    }

    fn is_listening(&self) -> bool {
        self.listening.load(Ordering::Relaxed)
    }
}
