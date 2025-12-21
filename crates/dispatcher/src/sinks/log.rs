//! LogSink - logs frame summary via tracing

use contracts::{ContractError, DataSink, SyncedFrame};
use tracing::{info, instrument};

/// Sink that logs frame summaries for debugging
pub struct LogSink {
    name: String,
}

impl LogSink {
    /// Create a new LogSink with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    fn log_frame_summary(&self, frame: &SyncedFrame) {
        let sensor_count = frame.frames.len();
        let missing_count = frame.sync_meta.missing_sensors.len();

        info!(
            sink = %self.name,
            frame_id = frame.frame_id,
            t_sync = frame.t_sync,
            sensors = sensor_count,
            missing = missing_count,
            dropped = frame.sync_meta.dropped_count,
            "SyncedFrame received"
        );
    }
}

impl DataSink for LogSink {
    fn name(&self) -> &str {
        &self.name
    }

    #[instrument(
        name = "log_sink_write",
        skip(self, frame),
        fields(sink = %self.name, frame_id = frame.frame_id)
    )]
    async fn write(&mut self, frame: &SyncedFrame) -> Result<(), ContractError> {
        self.log_frame_summary(frame);
        Ok(())
    }

    #[instrument(name = "log_sink_flush", skip(self))]
    async fn flush(&mut self) -> Result<(), ContractError> {
        // Nothing to flush for log sink
        Ok(())
    }

    #[instrument(name = "log_sink_close", skip(self))]
    async fn close(&mut self) -> Result<(), ContractError> {
        info!(sink = %self.name, "LogSink closed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::SyncMeta;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_log_sink_write() {
        let mut sink = LogSink::new("test_log");
        let frame = SyncedFrame {
            t_sync: 1.0,
            frame_id: 1,
            frames: HashMap::new(),
            sync_meta: SyncMeta::default(),
        };

        let result = sink.write(&frame).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_sink_name() {
        let sink = LogSink::new("my_logger");
        assert_eq!(sink.name(), "my_logger");
    }
}
