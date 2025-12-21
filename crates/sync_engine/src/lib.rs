//! # Sync Engine
//!
//! 多传感器数据同步引擎（以 PDF 规格为准）。
//!
//! 负责：
//! - 事件驱动同步触发
//! - IMU 自适应窗口
//! - KF/AdaKF 时间偏移校正
//! - 输出 `SyncedFrame`
//!
//! ## 使用示例
//!
//! ```ignore
//! use sync_engine::{SyncEngine, SyncEngineConfig};
//!
//! let config = SyncEngineConfig {
//!     reference_sensor_id: "cam".to_string(),
//!     required_sensors: vec!["cam".to_string(), "lidar".to_string()],
//!     imu_sensor_id: Some("imu".to_string()),
//!     ..Default::default()
//! };
//!
//! let mut engine = SyncEngine::new(config);
//!
//! // Push packets as they arrive
//! if let Some(frame) = engine.push(packet) {
//!     // Handle synchronized frame
//! }
//! ```

mod adakf;
mod buffer;
mod engine;
mod window;

// Re-exports
pub use contracts::{
    AdaKFConfig, BufferConfig, MissingDataStrategy, SyncEngineConfig, WindowConfig,
};
pub use engine::SyncEngine;

// Re-export contracts types
pub use contracts::{BufferStats, SensorPacket, SyncMeta, SyncedFrame};
