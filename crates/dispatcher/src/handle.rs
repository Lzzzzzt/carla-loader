//! SinkHandle - manages a sink with isolated queue and worker task

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, instrument, warn};

use contracts::{DataSink, SyncedFrame};

use crate::metrics::SinkMetrics;

/// Handle to a running sink worker
pub struct SinkHandle {
    /// Sink name
    name: String,
    /// Channel to send frames to worker
    tx: mpsc::Sender<SyncedFrame>,
    /// Shared metrics
    metrics: Arc<SinkMetrics>,
    /// Worker task handle
    worker_handle: JoinHandle<()>,
}

impl SinkHandle {
    /// Create a new SinkHandle and spawn the worker task
    pub fn spawn<S: DataSink + Send + 'static>(sink: S, queue_capacity: usize) -> Self {
        let name = sink.name().to_string();
        let (tx, rx) = mpsc::channel(queue_capacity);
        let metrics = Arc::new(SinkMetrics::new());

        let worker_metrics = Arc::clone(&metrics);
        let worker_name = name.clone();

        let worker_handle = tokio::spawn(async move {
            sink_worker(sink, rx, worker_metrics, worker_name).await;
        });

        Self {
            name,
            tx,
            metrics,
            worker_handle,
        }
    }

    /// Get sink name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get current metrics
    pub fn metrics(&self) -> &Arc<SinkMetrics> {
        &self.metrics
    }

    /// Send a frame to the sink (non-blocking)
    ///
    /// Returns true if sent, false if queue full (frame dropped)
    pub fn try_send(&self, frame: SyncedFrame) -> bool {
        match self.tx.try_send(frame) {
            Ok(()) => {
                // Update queue length approximation
                self.metrics.set_queue_len(self.tx.capacity());
                true
            }
            Err(mpsc::error::TrySendError::Full(f)) => {
                self.metrics.inc_dropped_count();
                warn!(
                    sink = %self.name,
                    frame_id = f.frame_id,
                    "Queue full, frame dropped"
                );
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                error!(sink = %self.name, "Sink worker closed unexpectedly");
                false
            }
        }
    }

    /// Shutdown the sink worker gracefully
    #[instrument(name = "sink_handle_shutdown", skip(self))]
    pub async fn shutdown(self) {
        // Drop sender to signal worker to stop
        drop(self.tx);
        // Wait for worker to finish
        if let Err(e) = self.worker_handle.await {
            error!(sink = %self.name, error = ?e, "Worker task panicked");
        }
        debug!(sink = %self.name, "SinkHandle shutdown complete");
    }
}

/// Worker task that consumes frames and writes to sink
#[instrument(
    name = "sink_worker_loop",
    skip(sink, rx, metrics),
    fields(sink = %name)
)]
async fn sink_worker<S: DataSink>(
    mut sink: S,
    mut rx: mpsc::Receiver<SyncedFrame>,
    metrics: Arc<SinkMetrics>,
    name: String,
) {
    debug!(sink = %name, "Sink worker started");

    while let Some(frame) = rx.recv().await {
        // Update queue length
        metrics.set_queue_len(rx.len());

        match sink.write(&frame).await {
            Ok(()) => {
                metrics.inc_write_count();
            }
            Err(e) => {
                metrics.inc_failure_count();
                error!(
                    sink = %name,
                    frame_id = frame.frame_id,
                    error = %e,
                    "Write failed"
                );
                // Continue processing - don't crash on single failure
            }
        }
    }

    // Cleanup
    if let Err(e) = sink.flush().await {
        error!(sink = %name, error = %e, "Flush failed on shutdown");
    }
    if let Err(e) = sink.close().await {
        error!(sink = %name, error = %e, "Close failed on shutdown");
    }

    debug!(sink = %name, "Sink worker stopped");
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{ContractError, SyncMeta};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::time::{sleep, Duration};

    /// Mock sink for testing
    struct MockSink {
        name: String,
        write_count: Arc<AtomicU64>,
        should_fail: bool,
        delay_ms: u64,
    }

    impl DataSink for MockSink {
        fn name(&self) -> &str {
            &self.name
        }

        async fn write(&mut self, _frame: &SyncedFrame) -> Result<(), ContractError> {
            if self.delay_ms > 0 {
                sleep(Duration::from_millis(self.delay_ms)).await;
            }
            if self.should_fail {
                return Err(ContractError::sink_write(&self.name, "mock failure"));
            }
            self.write_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        async fn flush(&mut self) -> Result<(), ContractError> {
            Ok(())
        }

        async fn close(&mut self) -> Result<(), ContractError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_sink_handle_basic() {
        let write_count = Arc::new(AtomicU64::new(0));
        let sink = MockSink {
            name: "test".to_string(),
            write_count: Arc::clone(&write_count),
            should_fail: false,
            delay_ms: 0,
        };

        let handle = SinkHandle::spawn(sink, 10);

        for i in 0..5 {
            let frame = SyncedFrame {
                t_sync: i as f64,
                frame_id: i,
                frames: HashMap::new(),
                sync_meta: SyncMeta::default(),
            };
            assert!(handle.try_send(frame));
        }

        handle.shutdown().await;
        assert_eq!(write_count.load(Ordering::Relaxed), 5);
    }

    #[tokio::test]
    async fn test_sink_handle_queue_full() {
        let write_count = Arc::new(AtomicU64::new(0));
        let sink = MockSink {
            name: "slow".to_string(),
            write_count: Arc::clone(&write_count),
            should_fail: false,
            delay_ms: 100, // Slow sink
        };

        // Small queue capacity
        let handle = SinkHandle::spawn(sink, 2);

        // Send more than queue can hold
        for i in 0..10 {
            let frame = SyncedFrame {
                t_sync: i as f64,
                frame_id: i,
                frames: HashMap::new(),
                sync_meta: SyncMeta::default(),
            };
            handle.try_send(frame);
        }

        // Some should have been dropped
        assert!(handle.metrics().dropped_count() > 0);

        handle.shutdown().await;
    }

    #[tokio::test]
    async fn test_sink_handle_failure_isolation() {
        let sink = MockSink {
            name: "failing".to_string(),
            write_count: Arc::new(AtomicU64::new(0)),
            should_fail: true,
            delay_ms: 0,
        };

        let handle = SinkHandle::spawn(sink, 10);

        for i in 0..3 {
            let frame = SyncedFrame {
                t_sync: i as f64,
                frame_id: i,
                frames: HashMap::new(),
                sync_meta: SyncMeta::default(),
            };
            handle.try_send(frame);
        }

        // Give worker time to process
        sleep(Duration::from_millis(50)).await;

        // Should have recorded failures
        assert!(handle.metrics().failure_count() > 0);

        handle.shutdown().await;
    }
}
