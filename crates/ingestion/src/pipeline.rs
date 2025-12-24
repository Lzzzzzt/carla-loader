//! Ingestion Pipeline 主入口

use std::collections::HashMap;
use std::sync::Arc;

use async_channel::{bounded, Receiver, Sender};
use contracts::{SensorPacket, SensorSource};
use tracing::{debug, info, instrument};

#[cfg(feature = "real-carla")]
use carla::client::Sensor;
#[cfg(feature = "real-carla")]
use contracts::SensorType;

use crate::adapter::SensorAdapter;
#[cfg(feature = "real-carla")]
use crate::adapters::{CameraAdapter, GnssAdapter, ImuAdapter, LidarAdapter, RadarAdapter};
use crate::config::{BackpressureConfig, IngestionMetrics};
use crate::generic_adapter::GenericSensorAdapter;

/// Ingestion Pipeline
///
/// 管理多个传感器适配器，提供统一的数据流输出。
/// 支持 Mock 和 Real 传感器的统一注册。
pub struct IngestionPipeline {
    /// 已注册的适配器
    adapters: HashMap<String, Box<dyn SensorAdapter>>,

    /// 共享的 metrics
    metrics: Arc<IngestionMetrics>,

    /// 数据发送端（所有 adapter 共享）
    tx: Sender<SensorPacket>,

    /// 数据接收端
    rx: Option<Receiver<SensorPacket>>,

    /// 默认背压配置
    default_config: BackpressureConfig,
}

impl IngestionPipeline {
    /// 创建新的 Ingestion Pipeline
    ///
    /// # Arguments
    /// * `channel_capacity` - 通道容量
    pub fn new(channel_capacity: usize) -> Self {
        let (tx, rx) = bounded(channel_capacity);

        Self {
            adapters: HashMap::new(),
            metrics: Arc::new(IngestionMetrics::new()),
            tx,
            rx: Some(rx),
            default_config: BackpressureConfig {
                channel_capacity,
                ..Default::default()
            },
        }
    }

    /// 使用自定义背压配置创建
    pub fn with_config(config: BackpressureConfig) -> Self {
        let (tx, rx) = bounded(config.channel_capacity);

        Self {
            adapters: HashMap::new(),
            metrics: Arc::new(IngestionMetrics::new()),
            tx,
            rx: Some(rx),
            default_config: config,
        }
    }

    /// 注册传感器数据源（统一接口）
    ///
    /// 这是推荐的注册方法，支持 Mock 和 Real 传感器。
    ///
    /// # Arguments
    /// * `sensor_id` - 传感器配置 ID
    /// * `source` - 实现 `SensorSource` trait 的数据源
    /// * `config` - 可选的背压配置
    #[instrument(
        name = "ingestion_register_sensor_source",
        skip(self, source, config),
        fields(sensor_id = %sensor_id)
    )]
    pub fn register_sensor_source(
        &mut self,
        sensor_id: String,
        source: Box<dyn SensorSource>,
        config: Option<BackpressureConfig>,
    ) {
        let adapter = GenericSensorAdapter::new(
            sensor_id.clone(),
            source,
            config.unwrap_or_else(|| self.default_config.clone()),
        );
        debug!(sensor_id = %sensor_id, "registered sensor source");
        self.adapters.insert(sensor_id, Box::new(adapter));
    }

    /// 注册传感器（使用 carla-rust Sensor）
    ///
    /// 保留用于向后兼容，建议使用 `register_sensor_source`。
    #[cfg(feature = "real-carla")]
    #[instrument(
        name = "ingestion_register_sensor",
        skip(self, sensor, config),
        fields(sensor_id = %sensor_id, sensor_type = ?sensor_type)
    )]
    pub fn register_sensor(
        &mut self,
        sensor_id: String,
        sensor_type: SensorType,
        sensor: Sensor,
        config: Option<BackpressureConfig>,
    ) {
        let adapter = Self::create_adapter(
            &sensor_id,
            sensor_type,
            sensor,
            config.unwrap_or_else(|| self.default_config.clone()),
        );
        debug!(sensor_id = %sensor_id, "registered sensor adapter");
        self.adapters.insert(sensor_id, adapter);
    }

    #[cfg(feature = "real-carla")]
    fn create_adapter(
        sensor_id: &str,
        sensor_type: SensorType,
        sensor: Sensor,
        config: BackpressureConfig,
    ) -> Box<dyn SensorAdapter> {
        match sensor_type {
            SensorType::Camera => {
                Box::new(CameraAdapter::new(sensor_id.to_string(), sensor, config))
            }
            SensorType::Lidar => Box::new(LidarAdapter::new(sensor_id.to_string(), sensor, config)),
            SensorType::Imu => Box::new(ImuAdapter::new(sensor_id.to_string(), sensor, config)),
            SensorType::Gnss => Box::new(GnssAdapter::new(sensor_id.to_string(), sensor, config)),
            SensorType::Radar => Box::new(RadarAdapter::new(sensor_id.to_string(), sensor, config)),
        }
    }

    /// 启动所有已注册的传感器
    #[instrument(name = "ingestion_start_all", skip(self))]
    pub fn start_all(&self) {
        info!(count = self.adapters.len(), "starting all sensor adapters");
        for (sensor_id, adapter) in &self.adapters {
            self.start_adapter(sensor_id, adapter.as_ref());
        }
    }

    /// 停止所有传感器
    #[instrument(name = "ingestion_stop_all", skip(self))]
    pub fn stop_all(&self) {
        info!(count = self.adapters.len(), "stopping all sensor adapters");
        for (sensor_id, adapter) in &self.adapters {
            self.stop_adapter(sensor_id, adapter.as_ref());
        }
    }

    fn start_adapter(&self, sensor_id: &str, adapter: &dyn SensorAdapter) {
        if !adapter.is_listening() {
            debug!(sensor_id = %sensor_id, "starting adapter");
            adapter.start(self.tx.clone(), self.metrics.clone());
        }
    }

    fn stop_adapter(&self, sensor_id: &str, adapter: &dyn SensorAdapter) {
        if adapter.is_listening() {
            debug!(sensor_id = %sensor_id, "stopping adapter");
            adapter.stop();
        }
    }

    /// 获取数据流接收端
    ///
    /// 注意：只能调用一次，后续调用返回 None
    pub fn take_receiver(&mut self) -> Option<Receiver<SensorPacket>> {
        self.rx.take()
    }

    /// 获取 metrics 引用
    pub fn metrics(&self) -> Arc<IngestionMetrics> {
        self.metrics.clone()
    }

    /// 获取已注册的传感器数量
    pub fn sensor_count(&self) -> usize {
        self.adapters.len()
    }

    /// 检查指定传感器是否正在监听
    pub fn is_sensor_listening(&self, sensor_id: &str) -> bool {
        self.adapters
            .get(sensor_id)
            .map(|a| a.is_listening())
            .unwrap_or(false)
    }
}

impl Drop for IngestionPipeline {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_creation() {
        let pipeline = IngestionPipeline::new(100);
        assert_eq!(pipeline.sensor_count(), 0);
    }

    #[test]
    fn test_take_receiver_once() {
        let mut pipeline = IngestionPipeline::new(100);
        assert!(pipeline.take_receiver().is_some());
        assert!(pipeline.take_receiver().is_none());
    }
}
