//! # Ingestion Pipeline
//!
//! Sensor data ingestion module.
//!
//! Responsibilities:
//! - Register sensor data sources (supports Mock and Real)
//! - Parse sensor data into `SensorPacket`
//! - Backpressure management and drop policy
//! - Send to downstream via async-channel
//!
//! ## Usage Example (Unified Interface)
//!
//! ```ignore
//! use ingestion::{IngestionPipeline, BackpressureConfig};
//! use contracts::SensorSource;
//!
//! let mut pipeline = IngestionPipeline::new(100);
//!
//! // Use unified SensorSource interface
//! let sensor_source: Box<dyn SensorSource> = client.get_sensor_source(
//!     actor_id, sensor_id, sensor_type,
//! );
//! pipeline.register_sensor_source(sensor_id, sensor_source, None);
//!
//! pipeline.start_all();
//! let rx = pipeline.take_receiver().unwrap();
//! while let Some(packet) = rx.recv().await {
//!     // Process data packet
//! }
//! ```
//!
//! ## Mock Testing
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
mod generic_adapter;
mod mock;
mod pipeline;

// Re-exports
pub use adapter::SensorAdapter;
pub use adapters::{CameraAdapter, GnssAdapter, ImuAdapter, LidarAdapter, RadarAdapter};
pub use config::{BackpressureConfig, DropPolicy, IngestionMetrics, MetricsSnapshot};
pub use contracts::SensorPacket;
pub use error::{IngestionError, Result};
pub use generic_adapter::GenericSensorAdapter;
pub use mock::{MockSensorConfig, MockSensorSource};
pub use pipeline::IngestionPipeline;
