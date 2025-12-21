# Observability Design Specification

## 1. Tracing & Logging Standards

### Span Attributes
All spans should include consistent attributes to allow correlation:

- `component`: The high-level component name (e.g., `ingestion`, `sync_engine`, `dispatcher`).
- `sensor_id`: If applicable, the ID of the sensor (e.g., `camera_front`, `lidar_top`).
- `frame_id`: The global synced frame ID (when available).

### Key Events
- **Component Lifecycle**: `start`, `shutdown`, `error`.
- **Packet Flow**:
    - `packet_received` (Ingestion)
    - `frame_sync_start` (Sync)
    - `frame_sync_complete` (Sync)
    - `dispatch_start` (Dispatcher)
    - `sink_write_error` (Dispatcher)

### Coding Guidelines
- **Async Functions**: Use `#[tracing::instrument]` macro on async functions to automatically generate spans.
- **Complex Logging**: Extract complex logging blocks into separate helper functions decorated with `#[instrument]` to keep business logic clean.

### Levels
- `ERROR`: Recoverable or fatal errors requiring attention.
- `WARN`: Unusual states (e.g., queue full backpressure, out-of-order packets dropped).
- `INFO`: Lifecycle events, periodic status summaries (every N frames).
- `DEBUG`: Per-packet flow details.
- `TRACE`: Raw data details (payload sizes, non-critical specific values).

## 2. Metrics Definition

Prefix all metrics with `carla_syncer_`.

### Ingestion (`ingestion`)
| Metric Name | Type | Labels | Description |
| :--- | :--- | :--- | :--- |
| `ingestion_packets_total` | Counter | `sensor_id`, `status` | Total packets received. Status: `ok`, `dropped`. |
| `ingestion_queue_size` | Gauge | `sensor_id` | Current depth of the internal channel buffer. |
| `ingestion_bytes_total` | Counter | `sensor_id` | Total bytes ingested. |

### Sync Engine (`sync_engine`)
| Metric Name | Type | Labels | Description |
| :--- | :--- | :--- | :--- |
| `sync_frames_total` | Counter | `status` | Total synced frames produced. Status: `ok`, `incomplete`. |
| `sync_buffer_depth` | Gauge | `sensor_id` | Number of packets waiting in the time-window buffer. |
| `sync_latency_seconds` | Histogram | - | Time from oldest packet timestamp to sync emission. |
| `sync_kf_residual` | Histogram | `sensor_id` | Kalman Filter prediction residual (measurement - prediction). |
| `sync_out_of_order_total` | Counter | `sensor_id` | Packets arrived too late/out of order. |
| `sync_alignment_error` | Histogram | `sensor_id` | Time difference between packet timestamp and frame center time (absolute value). |
| `sync_jitter` | Histogram | - | Variation in inter-frame interval time. |
| `sync_completeness_ratio` | Histogram | - | Ratio of sensors present vs expected per frame (effectiveness). |

### Dispatcher (`dispatcher`)
| Metric Name | Type | Labels | Description |
| :--- | :--- | :--- | :--- |
| `dispatcher_sink_queue_size` | Gauge | `sink_id` | Pending items for a sink. |
| `dispatcher_sink_writes_total` | Counter | `sink_id`, `status` | Writes to sink. Status: `ok`, `error`, `dropped`. |
| `dispatcher_sink_latency_seconds` | Histogram | `sink_id` | Time taken for sink I/O. |

## 3. Alerting Rules

| Alert Name | Condition | Severity | Description |
| :--- | :--- | :--- | :--- |
| `HighLatency` | `P95(sync_latency_seconds) > 0.1s` | Warning | Sync process is lagging behind real-time. |
| `BufferSaturation` | `ingestion_queue_size > 90% capacity` | Critical | Risk of packet drops due to backpressure. |
| `SinkFailureBurst` | `rate(dispatcher_sink_writes_total{status="error"}) > 5/s` | Critical | A sink is consistently failing (circuit breaker may trigger). |
| `SensorDataLoss` | `rate(ingestion_packets_total{status="dropped"}) > 0` | Warning | Packets are being dropped at ingestion. |
