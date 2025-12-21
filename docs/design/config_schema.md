# Config Schema 设计

## 配置版本

```toml
version = "V1"
```

## 字段说明

### world 配置

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|-----|------|-----|-------|------|
| `map` | string | ✓ | - | 地图名 (e.g., "Town01") |
| `carla_host` | string | | `"localhost"` | CARLA 服务器地址 |
| `carla_port` | u16 | | `2000` | CARLA 端口 |
| `weather` | enum | | - | 天气预设或自定义 |

天气预设: `clear_noon`, `cloudy_noon`, `wet_noon`, `rainy_noon`, `clear_sunset`, `custom`

### vehicles 配置

| 字段 | 类型 | 必填 | 说明 |
|-----|------|-----|------|
| `id` | string | ✓ | 唯一标识 |
| `blueprint` | string | ✓ | CARLA 蓝图 |
| `spawn_point` | Transform | ✓ | 初始位姿 |
| `sensors` | array | | 传感器列表 |

### sensors 配置

| 字段 | 类型 | 必填 | 说明 |
|-----|------|-----|------|
| `id` | string | ✓ | 全局唯一标识 |
| `sensor_type` | enum | ✓ | `camera/lidar/radar/imu/gnss` |
| `frequency_hz` | f64 | ✓ | 采样率 (>0) |
| `transform` | Transform | ✓ | 相对挂载位姿 |
| `attributes` | map | | CARLA 传感器属性 |

### sync 配置

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|-----|------|-----|-------|------|
| `primary_sensor_id` | string | ✓ | - | 主时钟传感器 ID |
| `min_window_sec` | f64 | | `0.020` | 同步窗口下限 |
| `max_window_sec` | f64 | | `0.100` | 同步窗口上限 |
| `missing_frame_policy` | enum | | `drop` | `drop/empty/interpolate` |
| `drop_policy` | enum | | `drop_oldest` | `drop_oldest/drop_newest` |

#### sync.engine 调优

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|-----|------|-----|-------|------|
| `required_sensor_ids` | array | | 所有主车传感器 | 要求完整同步的传感器列表 |
| `imu_sensor_id` | string | | 自动检测 IMU | 用于自适应窗口的 IMU ID |
| `window.min_ms` | f64 | | `min_window_sec*1000` | 覆盖窗口最小值 (毫秒) |
| `window.max_ms` | f64 | | `max_window_sec*1000` | 覆盖窗口最大值 (毫秒) |
| `buffer.max_size` | usize | | `1000` | 每个传感器的最大缓冲深度 |
| `buffer.timeout_s` | f64 | | `1.0` | 过期驱逐阈值 |
| `adakf.*` | table | | 见默认值 | AdaKF 噪声/窗口设置 |
| `sensor_intervals` | map | | 来自 `frequency_hz` | 每个传感器的期望采样间隔 (秒) |

### sinks 配置

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|-----|------|-----|-------|------|
| `name` | string | ✓ | - | Sink 名称 |
| `sink_type` | enum | ✓ | - | `log/file/network` |
| `queue_capacity` | usize | | `100` | 队列容量 |
| `params` | map | | - | 类型特定参数 |

## 校验规则

1. **vehicle_id 唯一**
2. **sensor_id 全局唯一**
3. **primary_sensor_id 存在于某车辆传感器中**
4. **frequency_hz > 0**
5. **min_window_sec ≤ max_window_sec**
6. **sink.name 非空**

## 示例

参考 `config_loader/examples/minimal.toml` 和 `config_loader/examples/full.toml`
