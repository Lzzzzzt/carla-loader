//! # Sync Engine
//!
//! Multi-sensor data synchronization engine (per PDF specification).
//!
//! Responsibilities:
//! - Event-driven sync triggering
//! - IMU adaptive windowing
//! - KF/AdaKF time offset correction
//! - Output `SyncedFrame`
//!
//! ## Usage Example
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
