# Output Sinks Survey

## 1. Existing Output Modules

### Current State
- **`dispatcher` crate**: Skeleton exists with basic `lib.rs` and empty `sinks.rs`
- **`contracts::DataSink` trait**: Already defined with:
  - `name()` - Sink identifier
  - `write(&mut self, frame: &SyncedFrame)` - Write operation
  - `flush()` - Buffer flush
  - `close()` - Cleanup

### Sink Configuration in Blueprint
`SinkConfig` structure in `contracts/src/blueprint.rs`:
- `name: String` - Unique identifier
- `sink_type: SinkType` - Log/File/Network
- `queue_capacity: usize` - Default 100
- `params: HashMap<String, String>` - Type-specific parameters

## 2. Output Format Requirements

### SyncedFrame Structure
```rust
SyncedFrame {
    t_sync: f64,              // Sync timestamp (CARLA sim time)
    frame_id: u64,            // Monotonic frame ID
    frames: HashMap<String, SensorPacket>,  // Sensor data
    sync_meta: SyncMeta,      // Metadata
}
```

### Recommended Formats
| Sink Type | Format | Use Case |
|-----------|--------|----------|
| Log | JSON/text | Debug, real-time monitoring |
| File | Binary (raw payload) / JSON | Offline analysis, replay |
| Network | Bincode/MessagePack | Low-latency streaming |

## 3. Sink Priority & Requirements

| Priority | Sink | Description |
|----------|------|-------------|
| P0 | LogSink | Minimal viable for debugging |
| P0 | FileSink | Essential for data collection |
| P1 | NetworkSink (UDP) | Optional: online streaming |

## 4. File Sink Specifics
- **Rolling Strategy**: By time (e.g., hourly) or by frame count
- **Naming Convention**: `{sensor_id}_{frame_id}_{timestamp}.bin`
- **Directory Structure**: `output/{date}/{sensor_type}/`

## 5. Network Sink Specifics
- **Protocol**: UDP (low-latency, no backpressure needed)
- **Future**: QUIC for reliable streaming
- **No retry**: Fire-and-forget semantics

## 6. Failure Requirements
- Per-sink failure isolation: One sink failure must not affect others
- Error metrics exposed for monitoring
- Optional circuit breaker for repeated failures
