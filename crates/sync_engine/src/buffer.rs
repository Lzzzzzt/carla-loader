//! Per-sensor packet buffer with timestamp-based ordering.
//!
//! Uses index-based separation for better performance:
//! - HeapRb stores lightweight metadata (timestamp + slab key)
//! - Slab stores actual SensorPacket data
//!
//! This avoids moving large payloads during buffer operations.

use std::cmp::Ordering;
use std::fmt;

use contracts::SensorPacket;
use ringbuf::{traits::*, HeapRb};
use slab::Slab;

/// Lightweight metadata stored in ring buffer
#[derive(Debug, Clone, Copy)]
struct PacketMeta {
    /// Timestamp for ordering
    timestamp: f64,
    /// Key into the slab storage
    slab_key: usize,
}

/// Per-sensor buffer with timeout eviction
///
/// Uses index separation: HeapRb stores only lightweight metadata,
/// while actual SensorPacket data lives in a Slab. This minimizes
/// memory movement for large payloads (images, point clouds).
pub struct SensorBuffer {
    /// Ring buffer of metadata (timestamp + slab key)
    index: HeapRb<PacketMeta>,
    /// Actual packet storage
    storage: Slab<SensorPacket>,
    max_size: usize,
    dropped_count: u64,
    out_of_order_count: u64,
    last_timestamp: Option<f64>,
}

impl fmt::Debug for SensorBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SensorBuffer")
            .field("len", &self.index.occupied_len())
            .field("max_size", &self.max_size)
            .field("dropped", &self.dropped_count)
            .finish()
    }
}

impl SensorBuffer {
    /// Create a new sensor buffer
    #[inline]
    pub fn new(max_size: usize, _timeout_s: f64) -> Self {
        Self {
            index: HeapRb::new(max_size),
            storage: Slab::with_capacity(max_size),
            max_size,
            dropped_count: 0,
            out_of_order_count: 0,
            last_timestamp: None,
        }
    }

    /// Push a packet into the buffer
    ///
    /// If buffer is full, overwrites the oldest packet.
    #[inline]
    pub fn push(&mut self, packet: SensorPacket) {
        let timestamp = packet.timestamp;

        // Track out-of-order arrivals
        if let Some(last) = self.last_timestamp {
            if timestamp < last {
                self.out_of_order_count += 1;
            }
        }
        self.last_timestamp = Some(timestamp);

        // If full, remove oldest entry from both index and storage
        if self.index.is_full() {
            if let Some(old_meta) = self.index.try_pop() {
                self.storage.remove(old_meta.slab_key);
            }
            self.dropped_count += 1;
        }

        // Insert packet into slab and metadata into ring buffer
        let slab_key = self.storage.insert(packet);
        let meta = PacketMeta {
            timestamp,
            slab_key,
        };
        let _ = self.index.try_push(meta);
    }

    /// Peek at the earliest packet (by timestamp) without removing
    #[inline]
    pub fn peek(&self) -> Option<&SensorPacket> {
        self.index
            .iter()
            .min_by(|a, b| {
                a.timestamp
                    .partial_cmp(&b.timestamp)
                    .unwrap_or(Ordering::Equal)
            })
            .and_then(|meta| self.storage.get(meta.slab_key))
    }

    /// Remove and return the earliest packet (by timestamp)
    #[inline]
    #[allow(dead_code)]
    pub fn pop(&mut self) -> Option<SensorPacket> {
        if self.index.is_empty() {
            return None;
        }

        // Find index of minimum timestamp
        let min_idx = self
            .index
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                a.timestamp
                    .partial_cmp(&b.timestamp)
                    .unwrap_or(Ordering::Equal)
            })
            .map(|(i, _)| i)?;

        // Collect all metadata, remove target, rebuild index
        let mut metas: Vec<PacketMeta> = self.index.pop_iter().collect();
        let removed_meta = metas.remove(min_idx);

        // Rebuild index (only moves small metadata, not payloads)
        for m in metas {
            let _ = self.index.try_push(m);
        }

        // Remove and return actual packet from storage
        Some(self.storage.remove(removed_meta.slab_key))
    }

    /// Get the number of packets in the buffer
    #[inline]
    pub fn len(&self) -> usize {
        self.index.occupied_len()
    }

    /// Check if the buffer is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Evict packets older than (now - timeout_s)
    #[inline]
    #[allow(dead_code)]
    pub fn evict_expired(&mut self, now: f64, timeout_s: f64) -> usize {
        let cutoff = now - timeout_s;
        let mut evicted = 0;

        // Collect metadata, filtering expired entries
        let remaining: Vec<PacketMeta> = self
            .index
            .pop_iter()
            .filter(|m| {
                if m.timestamp >= cutoff {
                    true
                } else {
                    // Remove expired packet from storage
                    self.storage.remove(m.slab_key);
                    evicted += 1;
                    false
                }
            })
            .collect();

        // Rebuild index with remaining metadata
        for m in remaining {
            let _ = self.index.try_push(m);
        }

        self.dropped_count += evicted as u64;
        evicted
    }

    /// Find the closest packet to target timestamp within window
    #[inline]
    pub fn find_closest_in_window(&self, target: f64, window: f64) -> Option<&SensorPacket> {
        let half = window / 2.0;
        let (min_t, max_t) = (target - half, target + half);

        self.index
            .iter()
            .filter(|m| m.timestamp >= min_t && m.timestamp <= max_t)
            .min_by(|a, b| {
                let da = (a.timestamp - target).abs();
                let db = (b.timestamp - target).abs();
                da.partial_cmp(&db).unwrap_or(Ordering::Equal)
            })
            .and_then(|meta| self.storage.get(meta.slab_key))
    }

    /// Remove consumed packets up to and including the given timestamp
    #[inline]
    pub fn remove_consumed(&mut self, up_to_timestamp: f64) {
        // Collect metadata, removing consumed entries from storage
        let remaining: Vec<PacketMeta> = self
            .index
            .pop_iter()
            .filter(|m| {
                if m.timestamp > up_to_timestamp {
                    true
                } else {
                    self.storage.remove(m.slab_key);
                    false
                }
            })
            .collect();

        // Rebuild index
        for m in remaining {
            let _ = self.index.try_push(m);
        }
    }

    /// Get dropped packet count
    #[inline]
    pub fn dropped_count(&self) -> u64 {
        self.dropped_count
    }

    /// Get out-of-order packet count
    #[inline]
    pub fn out_of_order_count(&self) -> u64 {
        self.out_of_order_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use contracts::{SensorPayload, SensorType};

    fn make_packet(sensor_id: &str, timestamp: f64) -> SensorPacket {
        SensorPacket {
            sensor_id: sensor_id.into(),
            sensor_type: SensorType::Camera,
            timestamp,
            frame_id: None,
            payload: SensorPayload::Raw(Bytes::new()),
        }
    }

    #[test]
    fn test_buffer_push_order() {
        let mut buffer = SensorBuffer::new(10, 10.0);

        buffer.push(make_packet("cam", 3.0));
        buffer.push(make_packet("cam", 1.0));
        buffer.push(make_packet("cam", 2.0));

        // Pop returns earliest by timestamp
        assert_eq!(buffer.pop().unwrap().timestamp, 1.0);
        assert_eq!(buffer.pop().unwrap().timestamp, 2.0);
        assert_eq!(buffer.pop().unwrap().timestamp, 3.0);
    }

    #[test]
    fn test_buffer_capacity() {
        let mut buffer = SensorBuffer::new(3, 10.0);

        buffer.push(make_packet("cam", 1.0));
        buffer.push(make_packet("cam", 2.0));
        buffer.push(make_packet("cam", 3.0));
        buffer.push(make_packet("cam", 4.0)); // Should evict oldest

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.dropped_count(), 1);
    }

    #[test]
    fn test_buffer_timeout() {
        let mut buffer = SensorBuffer::new(10, 1.0);

        buffer.push(make_packet("cam", 0.0));
        buffer.push(make_packet("cam", 0.5));
        buffer.push(make_packet("cam", 1.5));

        let evicted = buffer.evict_expired(2.0, 1.0);
        assert_eq!(evicted, 2); // 0.0 and 0.5 expired
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn test_find_closest_in_window() {
        let mut buffer = SensorBuffer::new(10, 10.0);

        buffer.push(make_packet("cam", 1.0));
        buffer.push(make_packet("cam", 1.05));
        buffer.push(make_packet("cam", 1.1));

        let closest = buffer.find_closest_in_window(1.04, 0.1);
        assert!(closest.is_some());
        assert_eq!(closest.unwrap().timestamp, 1.05);
    }

    #[test]
    fn test_out_of_order_detection() {
        let mut buffer = SensorBuffer::new(10, 10.0);

        buffer.push(make_packet("cam", 1.0));
        buffer.push(make_packet("cam", 3.0));
        buffer.push(make_packet("cam", 2.0)); // Out of order

        assert_eq!(buffer.out_of_order_count(), 1);
    }
}
