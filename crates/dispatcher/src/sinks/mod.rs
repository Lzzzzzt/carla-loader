//! Sink implementations
//!
//! Contains LogSink, FileSink, and NetworkSink.

mod file;
mod log;
mod network;

pub use self::file::FileSink;
pub use self::log::LogSink;
pub use self::network::NetworkSink;
