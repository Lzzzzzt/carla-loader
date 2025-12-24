//! CARLA client abstraction
//!
//! Defines traits for interacting with CARLA, supporting real implementation and mock testing.

use std::future::Future;

use contracts::{ActorId, SensorSource, SensorType, Transform};

use crate::error::Result;

/// CARLA client trait
///
/// Abstracts CARLA core operations for testing and future implementation replacement.
/// Supports unified interface for real CARLA client and Mock client.
pub trait CarlaClient: Send + Sync {
    /// Connect to CARLA server
    fn connect(&mut self, host: &str, port: u16) -> impl Future<Output = Result<()>> + Send;

    /// Spawn vehicle
    ///
    /// # Arguments
    /// * `blueprint` - Blueprint name, e.g., "vehicle.tesla.model3"
    /// * `transform` - Initial pose
    ///
    /// # Returns
    /// Newly created actor ID
    fn spawn_vehicle(
        &self,
        blueprint: &str,
        transform: Option<Transform>,
    ) -> impl Future<Output = Result<ActorId>> + Send;

    /// Spawn sensor and attach to parent actor
    ///
    /// # Arguments
    /// * `blueprint` - Blueprint name, e.g., "sensor.camera.rgb"
    /// * `transform` - Pose relative to parent actor
    /// * `parent_id` - Parent actor ID
    /// * `attributes` - Sensor attributes
    ///
    /// # Returns
    /// Newly created sensor actor ID
    fn spawn_sensor(
        &self,
        blueprint: &str,
        transform: Transform,
        parent_id: ActorId,
        attributes: &std::collections::HashMap<String, String>,
    ) -> impl Future<Output = Result<ActorId>> + Send;

    /// Destroy actor
    ///
    /// Idempotent operation: returns Ok if actor doesn't exist
    fn destroy_actor(&self, actor_id: ActorId) -> impl Future<Output = Result<()>> + Send;

    /// Check if actor exists
    fn actor_exists(&self, actor_id: ActorId) -> impl Future<Output = Result<bool>> + Send;

    /// Get sensor data source
    ///
    /// Returns an object implementing `SensorSource`, usable by IngestionPipeline.
    /// This is the core interface for unifying Mock and Real sensors.
    ///
    /// # Arguments
    /// * `actor_id` - Sensor's actor ID
    /// * `sensor_id` - Sensor configuration ID (for logging and tracing)
    /// * `sensor_type` - Sensor type
    ///
    /// # Returns
    /// Boxed trait object implementing `SensorSource`, None if actor doesn't exist
    fn get_sensor_source(
        &self,
        actor_id: ActorId,
        sensor_id: String,
        sensor_type: SensorType,
    ) -> Option<Box<dyn SensorSource>>;
}
