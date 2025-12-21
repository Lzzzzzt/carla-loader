//! Actor Factory 错误类型

use contracts::ContractError;
use thiserror::Error;

/// Actor Factory 专用错误
#[derive(Debug, Error)]
pub enum ActorFactoryError {
    /// CARLA 连接错误
    #[error("failed to connect to CARLA: {message}")]
    ConnectionFailed { message: String },

    /// Vehicle spawn 错误
    #[error("failed to spawn vehicle '{vehicle_id}': {message}")]
    VehicleSpawnFailed { vehicle_id: String, message: String },

    /// Sensor spawn 错误
    #[error("failed to spawn sensor '{sensor_id}' on vehicle '{vehicle_id}': {message}")]
    SensorSpawnFailed {
        sensor_id: String,
        vehicle_id: String,
        message: String,
    },

    /// Attach 错误
    #[error("failed to attach sensor '{sensor_id}' to vehicle '{vehicle_id}': {message}")]
    AttachFailed {
        sensor_id: String,
        vehicle_id: String,
        message: String,
    },

    /// Destroy 错误
    #[error("failed to destroy actor {actor_id}: {message}")]
    DestroyFailed { actor_id: u32, message: String },

    /// 包装的 ContractError
    #[error(transparent)]
    Contract(#[from] ContractError),
}

impl ActorFactoryError {
    /// 创建 vehicle spawn 错误
    pub fn vehicle_spawn(vehicle_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::VehicleSpawnFailed {
            vehicle_id: vehicle_id.into(),
            message: message.into(),
        }
    }

    /// 创建 sensor spawn 错误
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

/// Result 别名
pub type Result<T> = std::result::Result<T, ActorFactoryError>;
