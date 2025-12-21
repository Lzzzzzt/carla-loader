//! # Dispatcher
//!
//! 数据分发模块。
//!
//! 负责：
//! - 消费 `SyncedFrame`
//! - Fan-out 到多个 sinks
//! - 隔离慢 sink，不阻塞主链路

pub mod dispatcher;
pub mod error;
pub mod handle;
pub mod metrics;
pub mod sinks;

pub use contracts::{DataSink, SyncedFrame};
pub use dispatcher::{Dispatcher, DispatcherBuilder, DispatcherConfig, create_dispatcher};
pub use error::DispatcherError;
pub use handle::SinkHandle;
pub use metrics::{MetricsSnapshot, SinkMetrics};
pub use sinks::{FileSink, LogSink, NetworkSink};
