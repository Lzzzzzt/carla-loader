//! # Ingestion Pipeline
//!
//! 传感器数据摄取模块。
//!
//! 负责：
//! - 注册 CARLA 传感器回调
//! - 解析传感器数据为 `SensorPacket`
//! - 背压管理与丢包策略
//! - 通过 tokio 通道发送给下游
//!
//! ## 使用示例
//!
//! ```ignore
//! use ingestion::{IngestionPipeline, BackpressureConfig};
//!
//! let mut pipeline = IngestionPipeline::new(100);
//! pipeline.register_sensor(sensor_id, sensor_type, sensor, None);
//! pipeline.start_all();
//!
//! let rx = pipeline.take_receiver().unwrap();
//! while let Some(packet) = rx.recv().await {
//!     // 处理数据包
//! }
//! ```
//!
//! ## Mock 测试
//!
//! ```ignore
//! use ingestion::MockSensorSource;
//!
//! let source = MockSensorSource::camera("test_cam", 20.0, 800, 600);
//! let rx = source.start(100, None);
//! ```

mod adapter;
mod adapters;
mod config;
mod error;
mod mock;
mod pipeline;

// Re-exports
pub use adapter::SensorAdapter;
pub use adapters::{CameraAdapter, GnssAdapter, ImuAdapter, LidarAdapter, RadarAdapter};
pub use config::{BackpressureConfig, DropPolicy, IngestionMetrics, MetricsSnapshot};
pub use contracts::SensorPacket;
pub use error::{IngestionError, Result};
pub use mock::{MockSensorConfig, MockSensorSource};
pub use pipeline::IngestionPipeline;
