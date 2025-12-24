//! # Dispatcher
//!
//! Data dispatch module.
//!
//! Responsibilities:
//! - Consume `SyncedFrame`
//! - Fan-out to multiple sinks
//! - Isolate slow sinks without blocking main pipeline

pub mod dispatcher;
pub mod error;
pub mod handle;
pub mod metrics;
pub mod sinks;

pub use contracts::{DataSink, SyncedFrame};
pub use dispatcher::{create_dispatcher, Dispatcher, DispatcherBuilder, DispatcherConfig};
pub use error::DispatcherError;
pub use handle::SinkHandle;
pub use metrics::{MetricsSnapshot, SinkMetrics};
pub use sinks::{FileSink, LogSink, NetworkSink};
