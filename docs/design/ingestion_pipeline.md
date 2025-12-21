# Ingestion Pipeline 设计文档

## 概述

Ingestion Pipeline 负责从 CARLA 传感器回调接收原始数据，转换为 `SensorPacket`，并通过 tokio 通道发送给下游（Sync Engine）。

---

## 1. 架构

```
┌─────────────────────────────────────────────────────────────┐
│                        CARLA Simulator                      │
└─────────────┬───────────────┬───────────────┬──────────────┘
              │               │               │
        (C++ thread)    (C++ thread)    (C++ thread)
              │               │               │
              ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────┐
│   SensorAdapter (Camera)  SensorAdapter (Lidar)  ...        │
│   parse + copy to Bytes   parse + copy to Bytes             │
└─────────────┬───────────────┬───────────────┬──────────────┘
              │               │               │
         try_send()      try_send()      try_send()
              │               │               │
              ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────┐
│           tokio::sync::mpsc::Receiver<SensorPacket>         │
│                     (bounded channel)                        │
└─────────────────────────┬───────────────────────────────────┘
                          │
                          ▼
                   Sync Engine / Dispatcher
```

---

## 2. 线程模型

### 2.1 回调线程（CARLA C++ 线程）
- 每个传感器的 `listen()` 回调运行在独立线程
- **禁止阻塞**：不能调用阻塞 API（如 `channel.send().await`）
- 使用 `try_send()` 进行非阻塞发送

### 2.2 Tokio 运行时
- `IngestionPipeline` 返回 `Receiver<SensorPacket>`
- 下游（Sync Engine）在 tokio task 中消费

---

## 3. 内存策略

### 3.1 数据复制原则
- 所有 CARLA FFI 数据在回调返回前**必须复制**
- 使用 `Bytes::copy_from_slice()` 确保数据独立

### 3.2 零拷贝优化（可选）
对于大 payload（图像/点云），可使用 `Arc<[u8]>` 减少后续克隆：
```rust
let data: Arc<[u8]> = image.as_raw_bytes().into();
```

---

## 4. 背压策略

### 4.1 配置项
```rust
pub struct BackpressureConfig {
    /// 通道容量
    pub channel_capacity: usize,
    /// 满时策略
    pub drop_policy: DropPolicy,
}
```

### 4.2 丢包策略
| 策略 | 行为 |
|------|------|
| `DropNewest` | 通道满时丢弃当前包 |
| `DropOldest` | 通道满时从通道中取出最旧的包丢弃，再发送新包 |
| `Block` | ⚠️ 仅用于测试，会阻塞回调线程 |

**默认**：`DropNewest`（不阻塞回调线程）

### 4.3 Metrics
```rust
struct IngestionMetrics {
    packets_received: AtomicU64,
    packets_dropped: AtomicU64,
    queue_len: AtomicUsize,
    parse_errors: AtomicU64,
}
```

---

## 5. Payload 类型约定

### 5.1 Image
- Format: `BGRA8`（CARLA 默认），可能需要转换为 `RGBA8`
- Data: `width * height * 4` bytes

### 5.2 LiDAR
- Point stride: 16 bytes (x: f32, y: f32, z: f32, intensity: f32)
- Data: `num_points * 16` bytes

### 5.3 IMU
- Struct 直接复制（无 Bytes）

### 5.4 GNSS
- Struct 直接复制（无 Bytes）

### 5.5 Radar
- Detection: 16 bytes per detection
- Data: `num_detections * 16` bytes

---

## 6. 公开 API

```rust
/// 传感器适配器 trait
pub trait SensorAdapter: Send + Sync {
    /// 注册回调并开始接收数据
    fn start(&self) -> Receiver<SensorPacket>;
    
    /// 停止接收
    fn stop(&self);
}

/// Ingestion Pipeline 主入口
pub struct IngestionPipeline { ... }

impl IngestionPipeline {
    /// 为已创建的传感器注册回调
    pub fn register_sensor(
        &mut self,
        sensor_id: String,
        sensor_type: SensorType,
        sensor: Sensor,
        config: BackpressureConfig,
    );
    
    /// 获取合并后的数据流
    pub fn packet_stream(&self) -> Receiver<SensorPacket>;
    
    /// 停止所有传感器
    pub fn stop_all(&self);
}
```

---

## 7. Mock 支持

为无 CARLA 环境的测试提供 `MockSensorSource`：
```rust
pub struct MockSensorSource {
    pub sensor_id: String,
    pub sensor_type: SensorType,
    pub frequency_hz: f64,
}

impl MockSensorSource {
    /// 返回可配置频率的模拟数据流
    pub fn start(&self) -> Receiver<SensorPacket>;
}
```
