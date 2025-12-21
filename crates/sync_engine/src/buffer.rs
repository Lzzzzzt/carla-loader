//! Per-sensor buffer with min-heap ordering by timestamp.
#![allow(unused)]

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use contracts::SensorPacket;

/// Wrapper for min-heap ordering by timestamp
#[derive(Debug, Clone)]
pub struct TimestampedPacket {
    pub packet: SensorPacket,
    /// Sequence number for tie-breaking same timestamps
    sequence: u64,
}

impl TimestampedPacket {
    pub fn new(packet: SensorPacket, sequence: u64) -> Self {
        Self { packet, sequence }
    }

    pub fn timestamp(&self) -> f64 {
        self.packet.timestamp
    }
}

impl PartialEq for TimestampedPacket {
    fn eq(&self, other: &Self) -> bool {
        self.packet.timestamp == other.packet.timestamp && self.sequence == other.sequence
    }
}

impl Eq for TimestampedPacket {}

impl PartialOrd for TimestampedPacket {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimestampedPacket {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap (earliest timestamp first)
        match other.packet.timestamp.partial_cmp(&self.packet.timestamp) {
            Some(Ordering::Equal) | None => other.sequence.cmp(&self.sequence),
            Some(ord) => ord,
        }
    }
}

/// Per-sensor buffer with timeout eviction
#[derive(Debug)]
pub struct SensorBuffer {
    heap: BinaryHeap<TimestampedPacket>,
    max_size: usize,
    timeout_s: f64,
    sequence_counter: u64,

    // Metrics
    dropped_count: u64,
    out_of_order_count: u64,
    last_timestamp: Option<f64>,
}

impl SensorBuffer {
    /// Create a new sensor buffer
    pub fn new(max_size: usize, timeout_s: f64) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(max_size),
            max_size,
            timeout_s,
            sequence_counter: 0,
            dropped_count: 0,
            out_of_order_count: 0,
            last_timestamp: None,
        }
    }

    /// Push a packet into the buffer
    pub fn push(&mut self, packet: SensorPacket) {
        let timestamp = packet.timestamp;

        // Track out-of-order arrivals
        if let Some(last) = self.last_timestamp {
            if timestamp < last {
                self.out_of_order_count += 1;
            }
        }
        self.last_timestamp = Some(timestamp);

        // Evict if at capacity
        if self.heap.len() >= self.max_size {
            // Remove oldest (which is at the top of our min-heap)
            self.heap.pop();
            self.dropped_count += 1;
        }

        self.sequence_counter += 1;
        let wrapped = TimestampedPacket::new(packet, self.sequence_counter);
        self.heap.push(wrapped);
    }

    /// Peek at the earliest packet without removing
    pub fn peek(&self) -> Option<&SensorPacket> {
        self.heap.peek().map(|w| &w.packet)
    }

    /// Remove and return the earliest packet
    pub fn pop(&mut self) -> Option<SensorPacket> {
        self.heap.pop().map(|w| w.packet)
    }

    /// Get the number of packets in the buffer
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Evict packets older than (now - timeout)
    pub fn evict_expired(&mut self, now: f64) -> usize {
        let cutoff = now - self.timeout_s;
        let mut evicted = 0;

        // We need to rebuild the heap without expired packets
        let mut remaining = Vec::with_capacity(self.heap.len());
        while let Some(item) = self.heap.pop() {
            if item.packet.timestamp >= cutoff {
                remaining.push(item);
            } else {
                evicted += 1;
                self.dropped_count += 1;
            }
        }

        self.heap = remaining.into_iter().collect();
        evicted
    }

    /// Find the closest packet to target timestamp within window
    pub fn find_closest_in_window(&self, target: f64, window: f64) -> Option<&SensorPacket> {
        let half_window = window / 2.0;
        let min_t = target - half_window;
        let max_t = target + half_window;

        self.heap
            .iter()
            .filter(|p| p.packet.timestamp >= min_t && p.packet.timestamp <= max_t)
            .min_by(|a, b| {
                let diff_a = (a.packet.timestamp - target).abs();
                let diff_b = (b.packet.timestamp - target).abs();
                diff_a.partial_cmp(&diff_b).unwrap_or(Ordering::Equal)
            })
            .map(|w| &w.packet)
    }

    /// Remove consumed packets up to and including the given timestamp
    pub fn remove_consumed(&mut self, up_to_timestamp: f64) {
        let mut remaining = Vec::with_capacity(self.heap.len());
        while let Some(item) = self.heap.pop() {
            if item.packet.timestamp > up_to_timestamp {
                remaining.push(item);
            }
        }
        self.heap = remaining.into_iter().collect();
    }

    /// Get dropped packet count
    pub fn dropped_count(&self) -> u64 {
        self.dropped_count
    }

    /// Get out-of-order packet count
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
            sensor_id: sensor_id.to_string(),
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

        let evicted = buffer.evict_expired(2.0);
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
