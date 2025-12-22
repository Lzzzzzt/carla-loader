//! 适配器公共工具函数

use std::sync::Arc;

use async_channel::{Sender, TrySendError};
use contracts::{DropPolicy, SensorPacket};
use tracing::trace;

use crate::config::IngestionMetrics;

/// 发送数据包，处理背压策略
#[inline]
pub fn send_packet(
    tx: &Sender<SensorPacket>,
    packet: SensorPacket,
    metrics: &Arc<IngestionMetrics>,
    sensor_id: &str,
    drop_policy: DropPolicy,
) {
    match tx.try_send(packet) {
        Ok(_) => {
            trace!(sensor_id = %sensor_id, "packet sent");
        }
        Err(TrySendError::Full(_)) => {
            metrics.record_dropped();
            match drop_policy {
                DropPolicy::DropNewest => {
                    trace!(sensor_id = %sensor_id, "packet dropped (newest)");
                }
                DropPolicy::DropOldest => {
                    // TODO: 需要使用支持 pop 的通道实现真正的 DropOldest
                    trace!(sensor_id = %sensor_id, "packet dropped (oldest fallback)");
                }
            }
        }
        Err(TrySendError::Closed(_)) => {
            tracing::warn!(sensor_id = %sensor_id, "channel closed");
        }
    }
}

/// 将 POD 切片转换为 bytes::Bytes (零拷贝版本，共享内存)
///
/// # Safety
/// 调用者必须确保:
/// 1. T 是 POD 类型（plain old data）
/// 2. slice 的生命周期足够长，或数据会被立即消费
#[inline]
pub unsafe fn pod_slice_to_bytes_unchecked<T>(slice: &[T]) -> bytes::Bytes {
    let ptr = slice.as_ptr() as *const u8;
    let len = std::mem::size_of_val(slice);
    // 注意: 这里必须复制，因为 CARLA 回调数据在回调返回后可能失效
    bytes::Bytes::copy_from_slice(std::slice::from_raw_parts(ptr, len))
}

/// 将实现 bytemuck::Pod 的切片安全转换为 bytes::Bytes
#[inline]
#[allow(dead_code)]
pub fn pod_slice_to_bytes<T: bytemuck::Pod>(slice: &[T]) -> bytes::Bytes {
    bytes::Bytes::copy_from_slice(bytemuck::cast_slice(slice))
}
