# Observability

可观测性模块，提供 Tracing 日志和 Prometheus 指标支持。

## 功能

- **Tracing**: 结构化日志，支持 JSON/Pretty/Compact 格式
- **Prometheus 指标**: 通过 HTTP 端点导出指标
- **SyncMeta 指标收集**: 基于同步元信息的实时指标

## 使用方法

### 初始化

```rust
use observability::{init, init_with_config, ObservabilityConfig, LogFormat};

// 使用默认配置初始化
observability::init()?;

// 或自定义配置
let config = ObservabilityConfig {
    log_format: LogFormat::Pretty,
    metrics_port: Some(9000),
    default_log_level: "debug".to_string(),
};
observability::init_with_config(config)?;
```

### 记录 Sync 指标

```rust
use observability::record_sync_metrics;

// 在每次产生 SyncedFrame 时调用
if let Some(frame) = sync_engine.push(packet) {
    record_sync_metrics(&frame.sync_meta, frame.frame_id);
}
```

### 聚合统计

```rust
use observability::SyncMetricsAggregator;

let mut aggregator = SyncMetricsAggregator::new();

// 在每次产生 SyncedFrame 时更新
aggregator.update(&frame.sync_meta);

// 获取摘要
let summary = aggregator.summary();
println!("{}", summary);
```

## Prometheus 指标

访问 `http://localhost:9000/metrics` 获取指标。

### 可用指标

| 指标名称 | 类型 | 描述 |
|---------|------|------|
| `carla_syncer_frames_total` | Counter | 同步帧总数 |
| `carla_syncer_last_frame_id` | Gauge | 最新帧 ID |
| `carla_syncer_window_size_ms` | Histogram | 同步窗口大小 (ms) |
| `carla_syncer_motion_intensity` | Gauge | 当前运动强度 |
| `carla_syncer_motion_intensity_hist` | Histogram | 运动强度分布 |
| `carla_syncer_packets_dropped_total` | Counter | 丢包总数 |
| `carla_syncer_packets_dropped_current` | Gauge | 当前帧丢包数 |
| `carla_syncer_packets_out_of_order_total` | Counter | 乱序包总数 |
| `carla_syncer_packets_out_of_order_current` | Gauge | 当前帧乱序包数 |
| `carla_syncer_sensors_missing` | Gauge | 当前缺失传感器数 |
| `carla_syncer_frames_with_missing_sensors_total` | Counter | 有缺失传感器的帧总数 |
| `carla_syncer_sensor_missing_total` | Counter | 各传感器缺失次数 (by sensor_id) |
| `carla_syncer_time_offset_ms` | Gauge | 各传感器时间偏移 (by sensor_id) |
| `carla_syncer_time_offset_ms_hist` | Histogram | 时间偏移分布 (by sensor_id) |
| `carla_syncer_kf_residual` | Gauge | 卡尔曼滤波残差 (by sensor_id) |
| `carla_syncer_kf_residual_hist` | Histogram | KF 残差分布 (by sensor_id) |
| `carla_syncer_packets_received_total` | Counter | 接收包总数 (by sensor_id, sensor_type) |
| `carla_syncer_frames_dispatched_total` | Counter | 分发帧总数 (by sink, status) |
| `carla_syncer_sync_latency_ms` | Histogram | 同步延迟 (ms) |
| `carla_syncer_buffer_depth` | Gauge | 缓冲区深度 (by sensor_id) |

### 示例查询

```promql
# 同步帧率
rate(carla_syncer_frames_total[1m])

# 丢包率
rate(carla_syncer_packets_dropped_total[1m]) / rate(carla_syncer_frames_total[1m])

# 平均窗口大小
histogram_quantile(0.5, carla_syncer_window_size_ms_bucket)

# 各传感器缺失率
rate(carla_syncer_sensor_missing_total[1m])
```

## 日志格式

### JSON (默认)

```json
{
  "timestamp": "2025-12-21T07:30:00.000Z",
  "level": "INFO",
  "target": "carla_syncer",
  "fields": {
    "message": "Synced frame produced",
    "frame_id": 100,
    "sensors": 3
  },
  "threadId": "ThreadId(2)"
}
```

### Pretty

```
  2025-12-21T07:30:00.000Z  INFO carla_syncer: Synced frame produced
    frame_id: 100
    sensors: 3
```

### Compact

```
2025-12-21T07:30:00.000Z  INFO carla_syncer: Synced frame produced frame_id=100 sensors=3
```
