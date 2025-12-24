//! Ingestion error types

use thiserror::Error;

/// Ingestion error
#[derive(Debug, Error)]
pub enum IngestionError {
    /// Sensor data parse failed
    #[error("failed to parse sensor data: {message}")]
    ParseFailed {
        /// Sensor ID
        sensor_id: String,
        /// Error message
        message: String,
    },

    /// Channel closed
    #[error("channel closed for sensor {sensor_id}")]
    ChannelClosed {
        /// Sensor ID
        sensor_id: String,
    },

    /// Sensor not listening
    #[error("sensor {sensor_id} is not listening")]
    SensorNotListening {
        /// Sensor ID
        sensor_id: String,
    },

    /// Sensor already listening
    #[error("sensor {sensor_id} is already listening")]
    AlreadyListening {
        /// Sensor ID
        sensor_id: String,
    },
}

/// Ingestion Result type alias
pub type Result<T> = std::result::Result<T, IngestionError>;
