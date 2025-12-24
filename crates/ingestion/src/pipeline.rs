//! Ingestion Pipeline main entry

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
/// Manages multiple sensor adapters, provides unified data stream output.
/// Supports unified registration of Mock and Real sensors.
pub struct IngestionPipeline {
    /// Registered adapters
    adapters: HashMap<String, Box<dyn SensorAdapter>>,

    /// Shared metrics
    metrics: Arc<IngestionMetrics>,

    /// Data sender (shared by all adapters)
    tx: Sender<SensorPacket>,

    /// Data receiver
    rx: Option<Receiver<SensorPacket>>,

    /// Default backpressure configuration
    default_config: BackpressureConfig,
}

impl IngestionPipeline {
    /// Create new Ingestion Pipeline
    ///
    /// # Arguments
    /// * `channel_capacity` - Channel capacity
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

    /// Create with custom backpressure configuration
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

    /// Register sensor data source (unified interface)
    ///
    /// This is the recommended registration method, supports Mock and Real sensors.
    ///
    /// # Arguments
    /// * `sensor_id` - Sensor configuration ID
    /// * `source` - Data source implementing `SensorSource` trait
    /// * `config` - Optional backpressure configuration
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

    /// Register sensor (using carla-rust Sensor)
    ///
    /// Retained for backward compatibility, recommend using `register_sensor_source`.
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

    /// Start all registered sensors
    #[instrument(name = "ingestion_start_all", skip(self))]
    pub fn start_all(&self) {
        info!(count = self.adapters.len(), "starting all sensor adapters");
        for (sensor_id, adapter) in &self.adapters {
            self.start_adapter(sensor_id, adapter.as_ref());
        }
    }

    /// Stop all sensors
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

    /// Get data stream receiver
    ///
    /// Note: Can only be called once, subsequent calls return None
    pub fn take_receiver(&mut self) -> Option<Receiver<SensorPacket>> {
        self.rx.take()
    }

    /// Get metrics reference
    pub fn metrics(&self) -> Arc<IngestionMetrics> {
        self.metrics.clone()
    }

    /// Get registered sensor count
    pub fn sensor_count(&self) -> usize {
        self.adapters.len()
    }

    /// Check if specified sensor is listening
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
