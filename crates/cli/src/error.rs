//! Error types for CLI operations.

use thiserror::Error;

/// CLI-specific error types
#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum CliError {
    /// Configuration file not found
    #[error("Configuration file not found: {path}")]
    ConfigNotFound { path: String },

    /// Configuration parsing error
    #[error("Failed to parse configuration: {message}")]
    ConfigParse { message: String },

    /// Configuration validation error
    #[error("Configuration validation failed: {message}")]
    ConfigValidation { message: String },

    /// CARLA connection error
    #[error("Failed to connect to CARLA server at {host}:{port}: {message}")]
    CarlaConnection {
        host: String,
        port: u16,
        message: String,
    },

    /// Pipeline execution error
    #[error("Pipeline execution failed: {message}")]
    PipelineExecution { message: String },

    /// Graceful shutdown error
    #[error("Error during shutdown: {message}")]
    Shutdown { message: String },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error wrapper
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

#[allow(dead_code)]
impl CliError {
    pub fn config_not_found(path: impl Into<String>) -> Self {
        Self::ConfigNotFound { path: path.into() }
    }

    pub fn config_parse(message: impl Into<String>) -> Self {
        Self::ConfigParse {
            message: message.into(),
        }
    }

    pub fn config_validation(message: impl Into<String>) -> Self {
        Self::ConfigValidation {
            message: message.into(),
        }
    }

    pub fn carla_connection(
        host: impl Into<String>,
        port: u16,
        message: impl Into<String>,
    ) -> Self {
        Self::CarlaConnection {
            host: host.into(),
            port,
            message: message.into(),
        }
    }

    pub fn pipeline_execution(message: impl Into<String>) -> Self {
        Self::PipelineExecution {
            message: message.into(),
        }
    }

    pub fn shutdown(message: impl Into<String>) -> Self {
        Self::Shutdown {
            message: message.into(),
        }
    }
}

/// Result type alias for CLI operations
#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, CliError>;
