//! Adapter common utility functions

use std::sync::Arc;

use async_channel::{Sender, TrySendError};
use contracts::{DropPolicy, SensorPacket};
use tracing::trace;

use crate::config::IngestionMetrics;

/// Send packet, handling backpressure policy
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
                    // TODO: Need to use a channel that supports pop to implement true DropOldest
                    trace!(sensor_id = %sensor_id, "packet dropped (oldest fallback)");
                }
            }
        }
        Err(TrySendError::Closed(_)) => {
            tracing::warn!(sensor_id = %sensor_id, "channel closed");
        }
    }
}

/// Convert POD slice to bytes::Bytes (zero-copy version, shared memory)
///
/// # Safety
/// Caller must ensure:
/// 1. T is a POD type (plain old data)
/// 2. slice lifetime is long enough, or data will be consumed immediately
#[inline]
#[allow(dead_code)] // Used by real-carla feature adapters
pub unsafe fn pod_slice_to_bytes_unchecked<T>(slice: &[T]) -> bytes::Bytes {
    let ptr = slice.as_ptr() as *const u8;
    let len = std::mem::size_of_val(slice);
    // Note: Must copy here because CARLA callback data may become invalid after callback returns
    bytes::Bytes::copy_from_slice(std::slice::from_raw_parts(ptr, len))
}

/// Safely convert slice implementing bytemuck::Pod to bytes::Bytes
#[inline]
#[allow(dead_code)]
pub fn pod_slice_to_bytes<T: bytemuck::Pod>(slice: &[T]) -> bytes::Bytes {
    bytes::Bytes::copy_from_slice(bytemuck::cast_slice(slice))
}
