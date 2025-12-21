//! Sink metrics for observability

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Metrics for a single sink
#[derive(Debug, Default)]
pub struct SinkMetrics {
    /// Current queue length
    queue_len: AtomicUsize,
    /// Total successful writes
    write_count: AtomicU64,
    /// Total write failures
    failure_count: AtomicU64,
    /// Total frames dropped due to full queue
    dropped_count: AtomicU64,
}

impl SinkMetrics {
    /// Create new metrics instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Get current queue length
    pub fn queue_len(&self) -> usize {
        self.queue_len.load(Ordering::Relaxed)
    }

    /// Set current queue length
    pub fn set_queue_len(&self, len: usize) {
        self.queue_len.store(len, Ordering::Relaxed);
    }

    /// Get total write count
    pub fn write_count(&self) -> u64 {
        self.write_count.load(Ordering::Relaxed)
    }

    /// Increment write count
    pub fn inc_write_count(&self) {
        self.write_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get failure count
    pub fn failure_count(&self) -> u64 {
        self.failure_count.load(Ordering::Relaxed)
    }

    /// Increment failure count
    pub fn inc_failure_count(&self) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get dropped count
    pub fn dropped_count(&self) -> u64 {
        self.dropped_count.load(Ordering::Relaxed)
    }

    /// Increment dropped count
    pub fn inc_dropped_count(&self) {
        self.dropped_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get snapshot of all metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            queue_len: self.queue_len(),
            write_count: self.write_count(),
            failure_count: self.failure_count(),
            dropped_count: self.dropped_count(),
        }
    }
}

/// Snapshot of sink metrics (for reporting)
#[derive(Debug, Clone, Copy)]
pub struct MetricsSnapshot {
    pub queue_len: usize,
    pub write_count: u64,
    pub failure_count: u64,
    pub dropped_count: u64,
}
