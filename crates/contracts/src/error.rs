//! 错误分层定义
//!
//! 按来源分层：config / carla / ffi / sync / sink

use thiserror::Error;

/// 统一错误类型
#[derive(Debug, Error)]
pub enum ContractError {
    // ===== 配置错误 =====
    /// 配置解析错误
    #[error("config parse error: {message}")]
    ConfigParse {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// 配置校验错误
    #[error("config validation error at '{field}': {message}")]
    ConfigValidation { field: String, message: String },

    // ===== CARLA 错误 =====
    /// CARLA 连接错误
    #[error("carla connection error: {message}")]
    CarlaConnection { message: String },

    /// CARLA spawn 错误
    #[error("carla spawn error for '{actor_id}': {message}")]
    CarlaSpawn { actor_id: String, message: String },

    /// CARLA actor 不存在
    #[error("carla actor not found: {actor_id}")]
    CarlaActorNotFound { actor_id: String },

    // ===== FFI 错误 =====
    /// FFI 调用错误
    #[error("ffi error: {message}")]
    Ffi { message: String },

    /// 数据解析错误
    #[error("payload parse error for sensor '{sensor_id}': {message}")]
    PayloadParse { sensor_id: String, message: String },

    // ===== 同步错误 =====
    /// 同步超时
    #[error("sync timeout: waited {waited_ms}ms for sensors: {missing:?}")]
    SyncTimeout {
        waited_ms: u64,
        missing: Vec<String>,
    },

    /// 缓冲区溢出
    #[error("buffer overflow for sensor '{sensor_id}': depth={depth}, max={max}")]
    BufferOverflow {
        sensor_id: String,
        depth: usize,
        max: usize,
    },

    // ===== Sink 错误 =====
    /// Sink 写入错误
    #[error("sink '{sink_name}' write error: {message}")]
    SinkWrite { sink_name: String, message: String },

    /// Sink 连接错误
    #[error("sink '{sink_name}' connection error: {message}")]
    SinkConnection { sink_name: String, message: String },

    // ===== 通用错误 =====
    /// IO 错误
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// 其他错误
    #[error("{0}")]
    Other(String),
}

impl ContractError {
    /// 创建配置解析错误
    pub fn config_parse(message: impl Into<String>) -> Self {
        Self::ConfigParse {
            message: message.into(),
            source: None,
        }
    }

    /// 创建配置校验错误
    pub fn config_validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ConfigValidation {
            field: field.into(),
            message: message.into(),
        }
    }

    /// 创建 CARLA spawn 错误
    pub fn carla_spawn(actor_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::CarlaSpawn {
            actor_id: actor_id.into(),
            message: message.into(),
        }
    }

    /// 创建 sink 写入错误
    pub fn sink_write(sink_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::SinkWrite {
            sink_name: sink_name.into(),
            message: message.into(),
        }
    }
}
