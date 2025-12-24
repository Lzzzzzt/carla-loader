//! Layered error definitions
//!
//! Categorized by source: config / carla / ffi / sync / sink

use thiserror::Error;

/// Unified error type
#[derive(Debug, Error)]
pub enum ContractError {
    // ===== Configuration Errors =====
    /// Configuration parse error
    #[error("config parse error: {message}")]
    ConfigParse {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Configuration validation error
    #[error("config validation error at '{field}': {message}")]
    ConfigValidation { field: String, message: String },

    // ===== CARLA Errors =====
    /// CARLA connection error
    #[error("carla connection error: {message}")]
    CarlaConnection { message: String },

    /// CARLA spawn error
    #[error("carla spawn error for '{actor_id}': {message}")]
    CarlaSpawn { actor_id: String, message: String },

    /// CARLA actor not found
    #[error("carla actor not found: {actor_id}")]
    CarlaActorNotFound { actor_id: String },

    // ===== FFI Errors =====
    /// FFI call error
    #[error("ffi error: {message}")]
    Ffi { message: String },

    /// Data parse error
    #[error("payload parse error for sensor '{sensor_id}': {message}")]
    PayloadParse { sensor_id: String, message: String },

    // ===== Sync Errors =====
    /// Sync timeout
    #[error("sync timeout: waited {waited_ms}ms for sensors: {missing:?}")]
    SyncTimeout {
        waited_ms: u64,
        missing: Vec<String>,
    },

    /// Buffer overflow
    #[error("buffer overflow for sensor '{sensor_id}': depth={depth}, max={max}")]
    BufferOverflow {
        sensor_id: String,
        depth: usize,
        max: usize,
    },

    // ===== Sink Errors =====
    /// Sink write error
    #[error("sink '{sink_name}' write error: {message}")]
    SinkWrite { sink_name: String, message: String },

    /// Sink connection error
    #[error("sink '{sink_name}' connection error: {message}")]
    SinkConnection { sink_name: String, message: String },

    // ===== General Errors =====
    /// IO error
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Other error
    #[error("{0}")]
    Other(String),
}

impl ContractError {
    /// Create configuration parse error
    pub fn config_parse(message: impl Into<String>) -> Self {
        Self::ConfigParse {
            message: message.into(),
            source: None,
        }
    }

    /// Create configuration validation error
    pub fn config_validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ConfigValidation {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Create CARLA spawn error
    pub fn carla_spawn(actor_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::CarlaSpawn {
            actor_id: actor_id.into(),
            message: message.into(),
        }
    }

    /// Create sink write error
    pub fn sink_write(sink_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::SinkWrite {
            sink_name: sink_name.into(),
            message: message.into(),
        }
    }
}
