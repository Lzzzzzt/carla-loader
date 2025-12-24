//! SensorSource trait - Sensor data source abstraction
//!
//! Defines a unified interface for sensor data sources, decoupling adapters from concrete sensor implementations.
//! Supports unified handling of real CARLA sensors and Mock sensors.

use std::sync::Arc;

use crate::{SensorPacket, SensorType};

/// Sensor data callback type
///
/// When a sensor produces data, it sends `SensorPacket` through this callback.
/// Uses `Arc` to allow callback sharing across multiple contexts.
pub type SensorDataCallback = Arc<dyn Fn(SensorPacket) + Send + Sync>;

/// Sensor data source trait
///
/// Abstracts the common behavior of real CARLA sensors and Mock sensors.
/// All sensor data sources implement this trait for use by IngestionPipeline.
///
/// # Design Principles
///
/// 1. **Decoupling**: Separates sensor data generation from data consumption
/// 2. **Unified Interface**: Mock and Real sensors use the same API
/// 3. **Callback Pattern**: Uses callbacks instead of channels, consistent with CARLA's native pattern
///
/// # Example
///
/// ```ignore
/// let sensor: Box<dyn SensorSource> = get_sensor_source();
/// sensor.listen(Arc::new(|packet| {
///     println!("Received packet: {:?}", packet.sensor_id);
/// }));
/// // ... use sensor ...
/// sensor.stop();
/// ```
pub trait SensorSource: Send + Sync {
    /// Get sensor ID
    fn sensor_id(&self) -> &str;

    /// Get sensor type
    fn sensor_type(&self) -> SensorType;

    /// Register data callback
    ///
    /// When the sensor produces data, it calls the callback function to send `SensorPacket`.
    /// If already listening, repeated calls should be idempotent (won't register multiple callbacks).
    ///
    /// # Arguments
    /// * `callback` - Data callback function, receives `SensorPacket`
    fn listen(&self, callback: SensorDataCallback);

    /// Stop listening
    ///
    /// Stops sensor data generation. For Mock sensors, stops background thread;
    /// For Real sensors, calls CARLA sensor.stop().
    fn stop(&self);

    /// Check if currently listening
    fn is_listening(&self) -> bool;
}
