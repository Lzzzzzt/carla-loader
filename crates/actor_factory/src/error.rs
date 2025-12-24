//! Actor Factory error types

use contracts::ContractError;
use thiserror::Error;

/// Actor Factory specific error
#[derive(Debug, Error)]
pub enum ActorFactoryError {
    /// CARLA connection error
    #[error("failed to connect to CARLA: {message}")]
    ConnectionFailed { message: String },

    /// Vehicle spawn error
    #[error("failed to spawn vehicle '{vehicle_id}': {message}")]
    VehicleSpawnFailed { vehicle_id: String, message: String },

    /// Sensor spawn error
    #[error("failed to spawn sensor '{sensor_id}' on vehicle '{vehicle_id}': {message}")]
    SensorSpawnFailed {
        sensor_id: String,
        vehicle_id: String,
        message: String,
    },

    /// Attach error
    #[error("failed to attach sensor '{sensor_id}' to vehicle '{vehicle_id}': {message}")]
    AttachFailed {
        sensor_id: String,
        vehicle_id: String,
        message: String,
    },

    /// Destroy error
    #[error("failed to destroy actor {actor_id}: {message}")]
    DestroyFailed { actor_id: u32, message: String },

    /// Wrapped ContractError
    #[error(transparent)]
    Contract(#[from] ContractError),
}

impl ActorFactoryError {
    /// Create vehicle spawn error
    pub fn vehicle_spawn(vehicle_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::VehicleSpawnFailed {
            vehicle_id: vehicle_id.into(),
            message: message.into(),
        }
    }

    /// Create sensor spawn error
    pub fn sensor_spawn(
        sensor_id: impl Into<String>,
        vehicle_id: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::SensorSpawnFailed {
            sensor_id: sensor_id.into(),
            vehicle_id: vehicle_id.into(),
            message: message.into(),
        }
    }
}

/// Result alias
pub type Result<T> = std::result::Result<T, ActorFactoryError>;
