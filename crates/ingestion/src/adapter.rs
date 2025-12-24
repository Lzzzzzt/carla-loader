//! Sensor adapter trait

use std::sync::Arc;

use async_channel::Sender;
use contracts::{SensorPacket, SensorType};

use crate::config::IngestionMetrics;

/// Sensor adapter trait
///
/// Implement this trait for each sensor type, responsible for:
/// 1. Registering CARLA sensor callbacks
/// 2. Parsing sensor data
/// 3. Wrapping into `SensorPacket`
/// 4. Sending to channel (handling backpressure)
pub trait SensorAdapter: Send + Sync {
    /// Get sensor ID
    fn sensor_id(&self) -> &str;

    /// Get sensor type
    fn sensor_type(&self) -> SensorType;

    /// Start sensor data collection
    ///
    /// # Arguments
    /// * `tx` - Data packet sending channel
    /// * `metrics` - Shared ingestion metrics
    fn start(&self, tx: Sender<SensorPacket>, metrics: Arc<IngestionMetrics>);

    /// Stop sensor data collection
    fn stop(&self);

    /// Check if sensor is listening
    fn is_listening(&self) -> bool;
}
