//! Dispatcher error types

use thiserror::Error;

/// Dispatcher-specific errors
#[derive(Debug, Error)]
pub enum DispatcherError {
    /// Sink creation error
    #[error("failed to create sink '{name}': {message}")]
    SinkCreation { name: String, message: String },

    /// Queue full - frame dropped
    #[error("queue full for sink '{sink_name}', frame {frame_id} dropped")]
    QueueFull { sink_name: String, frame_id: u64 },

    /// Sink write error (from contract)
    #[error("sink error: {0}")]
    Contract(#[from] contracts::ContractError),

    /// IO error
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl DispatcherError {
    /// Create a sink creation error
    pub fn sink_creation(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::SinkCreation {
            name: name.into(),
            message: message.into(),
        }
    }
}
