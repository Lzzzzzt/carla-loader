//! Backpressure configuration and metrics

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub use contracts::DropPolicy;

/// Backpressure configuration
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// Channel capacity
    pub channel_capacity: usize,

    /// Drop policy when full
    pub drop_policy: DropPolicy,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 100,
            drop_policy: DropPolicy::DropNewest,
        }
    }
}

impl BackpressureConfig {
    /// Create new backpressure configuration
    pub fn new(channel_capacity: usize, drop_policy: DropPolicy) -> Self {
        Self {
            channel_capacity,
            drop_policy,
        }
    }
}

/// Ingestion metrics
#[derive(Debug, Default)]
pub struct IngestionMetrics {
    /// Total packets received
    pub packets_received: AtomicU64,

    /// Total packets dropped
    pub packets_dropped: AtomicU64,

    /// Current queue length
    pub queue_len: AtomicUsize,

    /// Parse error count
    pub parse_errors: AtomicU64,
}

impl IngestionMetrics {
    /// Create new metrics instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Record packet received
    pub fn record_received(&self) {
        self.packets_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Record packet dropped
    pub fn record_dropped(&self) {
        self.packets_dropped.fetch_add(1, Ordering::Relaxed);
    }

    /// Record parse error
    pub fn record_parse_error(&self) {
        self.parse_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Update queue length
    pub fn update_queue_len(&self, len: usize) {
        self.queue_len.store(len, Ordering::Relaxed);
    }

    /// Get snapshot
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            packets_received: self.packets_received.load(Ordering::Relaxed),
            packets_dropped: self.packets_dropped.load(Ordering::Relaxed),
            queue_len: self.queue_len.load(Ordering::Relaxed),
            parse_errors: self.parse_errors.load(Ordering::Relaxed),
        }
    }
}

/// Metrics snapshot
#[derive(Debug, Clone, Default)]
pub struct MetricsSnapshot {
    /// Total packets received
    pub packets_received: u64,

    /// Total packets dropped
    pub packets_dropped: u64,

    /// Current queue length
    pub queue_len: usize,

    /// Parse error count
    pub parse_errors: u64,
}
