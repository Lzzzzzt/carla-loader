# FFI 与 Ingestion 调研报告

## 1. FFI 层与回调注册接口

### carla-rust 传感器回调 API
`carla-rust` (v0.13) 提供了传感器回调接口：

```rust
// carla::client::Sensor
impl Sensor {
    pub fn listen<F>(&self, callback: F)
    where
        F: FnMut(SensorData) + Send + 'static;
    
    pub fn stop(&self);
    pub fn is_listening(&self) -> bool;
}
```

**关键点：**
- 回调运行在 **独立线程**（非 tokio 线程）
- `SensorData` 是 FFI 桥接类型，封装了 C++ 指针
- 必须在回调返回前完成数据复制（否则悬垂引用）

### 数据类型转换
```rust
// SensorData 转换为具体类型
Image::try_from(sensor_data)           // camera
LidarMeasurement::try_from(sensor_data) // lidar
ImuMeasurement::try_from(sensor_data)   // imu
GnssMeasurement::try_from(sensor_data)  // gnss
RadarMeasurement::try_from(sensor_data) // radar
```

---

## 2. Payload 数据结构

### SensorDataBase trait
所有 sensor data 类型实现此 trait：
```rust
trait SensorDataBase {
    fn frame(&self) -> usize;       // 帧序号
    fn timestamp(&self) -> f64;     // CARLA simulation time (秒)
    fn sensor_transform(&self) -> Transform;
}
```

### 具体数据类型

| 类型 | 关键方法 | 数据格式 |
|------|----------|----------|
| `Image` | `as_raw_bytes()`, `as_slice()` -> `&[Color]` | BGRA, width*height*4 bytes |
| `LidarMeasurement` | `as_slice()` -> `&[LidarDetection]` | x,y,z,intensity per point |
| `ImuMeasurement` | `accelerometer()`, `gyroscope()`, `compass()` | 3D vectors |
| `GnssMeasurement` | `latitude()`, `longitude()`, `altitude()` | f64 度/米 |
| `RadarMeasurement` | `as_slice()` -> `&[RadarDetection]` | velocity, azimuth, altitude, depth |

---

## 3. 内存所有权

### CARLA 回调 Buffer 生命周期
- `SensorData` 内部持有 `SharedPtr<FfiSensorData>`（C++ shared_ptr）
- **生命周期**：仅在回调函数调用期间有效
- **所有权**：CARLA C++ 端分配，回调返回后可能释放
- **结论**：必须在回调中立即复制数据

### 零拷贝策略
- `Image::as_raw_bytes()` 返回 `&[u8]`，可直接 `Bytes::copy_from_slice()`
- `LidarMeasurement::as_slice()` 返回 `&[LidarDetection]`，需转为 bytes
- IMU/GNSS 数据量小，直接复制

### 推荐实现
```rust
// 在回调中
let payload = Bytes::copy_from_slice(image.as_raw_bytes());
// 或使用 Arc<[u8]> 减少后续克隆开销
```

---

## 4. 线程模型

### 当前架构
- `actor_factory` 中未启动 tokio runtime
- 传感器回调运行在 CARLA 的 C++ 线程

### 需解决的问题
1. 回调线程 → tokio 通道桥接
2. 避免在回调中阻塞（不能 block on channel send）

### 推荐方案
```rust
// 使用 crossbeam 或 tokio::sync::mpsc 的 try_send
use tokio::sync::mpsc;

let (tx, rx) = mpsc::channel(capacity);

sensor.listen(move |data| {
    // 在回调线程中解析数据
    let packet = parse_sensor_data(data);
    
    // 非阻塞发送
    match tx.try_send(packet) {
        Ok(_) => {},
        Err(TrySendError::Full(_)) => {
            // 背压：根据策略丢包
        },
        Err(TrySendError::Closed(_)) => {},
    }
});
```

---

## 5. 与 contracts::SensorPacket 对齐

### 现有契约
```rust
pub struct SensorPacket {
    pub sensor_id: String,
    pub sensor_type: SensorType,
    pub timestamp: f64,      // CARLA simulation time ✓
    pub frame_id: Option<u64>,
    pub payload: SensorPayload,
}

pub enum SensorPayload {
    Image(ImageData),        // width, height, format, data: Bytes
    PointCloud(PointCloudData), // num_points, point_stride, data: Bytes
    Imu(ImuData),            // accelerometer, gyroscope, compass
    Gnss(GnssData),          // latitude, longitude, altitude
    Radar(RadarData),        // num_detections, data: Bytes
    Raw(Bytes),              // fallback
}
```

### 映射关系
| carla-rust 类型 | → contracts 类型 |
|-----------------|------------------|
| `Image` | `SensorPayload::Image(ImageData)` |
| `LidarMeasurement` | `SensorPayload::PointCloud(PointCloudData)` |
| `ImuMeasurement` | `SensorPayload::Imu(ImuData)` |
| `GnssMeasurement` | `SensorPayload::Gnss(GnssData)` |
| `RadarMeasurement` | `SensorPayload::Radar(RadarData)` |

---

## 6. 风险清单

| 风险 | 严重度 | 缓解措施 |
|------|--------|----------|
| 回调线程阻塞 | 高 | 使用 try_send + 背压策略 |
| 内存泄漏/悬垂引用 | 高 | 在回调中立即复制数据 |
| 高频数据背压 | 中 | 可配置通道容量 + drop policy |
| 数据解析失败 | 中 | TryFrom 失败时记录 metric + 跳过 |
| 通道关闭 | 中 | 优雅关闭：先 stop() sensor，再 drop channel |

---

## 7. 可复用代码

### actor_factory 中的 Sensor 存储
```rust
// crates/actor_factory/src/carla_client.rs
enum ActorType {
    Vehicle(Vehicle),
    Sensor(Sensor),  // 可用于注册 listen
}
```

### contracts 中的数据结构
- `SensorPacket`, `SensorPayload` 及所有子类型
- `SensorType` 枚举
- `DropPolicy` 枚举

---

## 8. 结论

carla-rust 提供了完整的传感器回调 API，与 contracts 定义的数据结构可良好对接。主要工作：

1. **Adapter 层**：为每类传感器实现 `TryFrom<SensorData> -> SensorPacket`
2. **桥接层**：回调线程 → tokio mpsc 通道
3. **背压层**：可配置的 drop policy + metrics
4. **Unsafe 封装**：所有 FFI 操作限制在 adapter 内部
