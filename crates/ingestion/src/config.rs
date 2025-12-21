//! 背压配置和指标

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub use contracts::DropPolicy;

/// 背压配置
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// 通道容量
    pub channel_capacity: usize,

    /// 满时丢包策略
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
    /// 创建新的背压配置
    pub fn new(channel_capacity: usize, drop_policy: DropPolicy) -> Self {
        Self {
            channel_capacity,
            drop_policy,
        }
    }
}

/// Ingestion 指标
#[derive(Debug, Default)]
pub struct IngestionMetrics {
    /// 接收的数据包总数
    pub packets_received: AtomicU64,

    /// 丢弃的数据包总数
    pub packets_dropped: AtomicU64,

    /// 当前队列长度
    pub queue_len: AtomicUsize,

    /// 解析错误数
    pub parse_errors: AtomicU64,
}

impl IngestionMetrics {
    /// 创建新的指标实例
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录接收一个包
    pub fn record_received(&self) {
        self.packets_received.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录丢弃一个包
    pub fn record_dropped(&self) {
        self.packets_dropped.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录解析错误
    pub fn record_parse_error(&self) {
        self.parse_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// 更新队列长度
    pub fn update_queue_len(&self, len: usize) {
        self.queue_len.store(len, Ordering::Relaxed);
    }

    /// 获取快照
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            packets_received: self.packets_received.load(Ordering::Relaxed),
            packets_dropped: self.packets_dropped.load(Ordering::Relaxed),
            queue_len: self.queue_len.load(Ordering::Relaxed),
            parse_errors: self.parse_errors.load(Ordering::Relaxed),
        }
    }
}

/// 指标快照
#[derive(Debug, Clone, Default)]
pub struct MetricsSnapshot {
    /// 接收的数据包总数
    pub packets_received: u64,

    /// 丢弃的数据包总数
    pub packets_dropped: u64,

    /// 当前队列长度
    pub queue_len: usize,

    /// 解析错误数
    pub parse_errors: u64,
}
