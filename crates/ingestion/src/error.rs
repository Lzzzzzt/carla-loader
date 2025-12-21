//! Ingestion 错误类型

use thiserror::Error;

/// Ingestion 错误
#[derive(Debug, Error)]
pub enum IngestionError {
    /// 传感器数据解析失败
    #[error("failed to parse sensor data: {message}")]
    ParseFailed {
        /// 传感器 ID
        sensor_id: String,
        /// 错误消息
        message: String,
    },

    /// 通道已关闭
    #[error("channel closed for sensor {sensor_id}")]
    ChannelClosed {
        /// 传感器 ID
        sensor_id: String,
    },

    /// 传感器未在监听
    #[error("sensor {sensor_id} is not listening")]
    SensorNotListening {
        /// 传感器 ID
        sensor_id: String,
    },

    /// 传感器已在监听
    #[error("sensor {sensor_id} is already listening")]
    AlreadyListening {
        /// 传感器 ID
        sensor_id: String,
    },
}

/// Ingestion Result 类型别名
pub type Result<T> = std::result::Result<T, IngestionError>;
