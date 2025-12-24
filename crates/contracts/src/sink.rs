//! DataSink trait - Dispatcher output interface
//!
//! Defines the abstract interface for Sinks.

use crate::{ContractError, SyncedFrame};

/// Data output trait
///
/// All sink implementations must implement this trait.
#[trait_variant::make(DataSink: Send)]
pub trait LocalDataSink {
    /// Sink name (used for logging/metrics)
    fn name(&self) -> &str;

    /// Write synchronized frame
    ///
    /// # Errors
    /// Returns write error (should include context)
    async fn write(&mut self, frame: &SyncedFrame) -> Result<(), ContractError>;

    /// Flush buffer (if any)
    async fn flush(&mut self) -> Result<(), ContractError>;

    /// Close sink
    async fn close(&mut self) -> Result<(), ContractError>;
}
