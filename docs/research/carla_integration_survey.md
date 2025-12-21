# CARLA 集成调研报告

## 1. CARLA 版本与连接方式

### 版本选择
- 推荐 CARLA 0.9.15+（最新稳定版）
- 支持 headless 模式运行

### 连接配置
| 参数 | 默认值 | 说明 |
|------|--------|------|
| host | localhost | CARLA 服务器地址 |
| port | 2000 | CARLA 服务器端口 |
| timeout | 10s | 连接超时 |

### 同步/异步模式
- **同步模式**（推荐）：由客户端控制 `world.tick()`，确保传感器数据一致性
- **异步模式**：服务器自主 tick，适合实时仿真

## 2. Rust 绑定方案

### 现状分析
- 官方无 Rust 绑定，主要支持 Python/C++
- 社区 Rust 绑定（如 `carla-rust`）不够成熟

### 推荐方案：Trait 抽象 + Mock

```rust
#[async_trait]
pub trait CarlaClient: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn spawn_vehicle(...) -> Result<ActorId>;
    async fn spawn_sensor(...) -> Result<ActorId>;
    async fn destroy_actor(...) -> Result<()>;
}
```

优势：
1. 单元测试无需真实 CARLA
2. 未来可替换 FFI 实现
3. 支持注入失败场景测试

## 3. Actor/传感器蓝图

### Blueprint Library
- Vehicle: `vehicle.tesla.model3`, `vehicle.audi.tt` 等
- Sensor: `sensor.camera.rgb`, `sensor.lidar.ray_cast` 等

### 属性设置
| 传感器 | 关键属性 |
|--------|----------|
| Camera | image_size_x, image_size_y, fov |
| LiDAR | channels, range, points_per_second |
| IMU | sensor_tick |
| GNSS | sensor_tick |

### Transform 表达
- Location: (x, y, z) 单位：米
- Rotation: (pitch, yaw, roll) 单位：度

## 4. Teardown 约束

### 幽灵 Actor 问题
- Actor 创建后如果不显式销毁，会残留在仿真中
- 连接断开时不会自动清理

### 销毁策略
1. 先销毁 sensors，后销毁 vehicles（依赖顺序）
2. 销毁前检查 actor 是否存在（幂等）
3. 记录日志便于排查泄露

## 5. 与 RuntimeGraph 对齐

`RuntimeGraph` 需保存：
- `vehicles: HashMap<String, ActorId>` - 配置 ID → CARLA actor ID
- `sensors: HashMap<String, ActorId>` - 配置 ID → CARLA actor ID
- `sensor_to_vehicle: HashMap<String, String>` - 传感器归属
- `actor_to_id: HashMap<ActorId, String>` - 反查

## 6. 风险点

| 风险 | 缓解措施 |
|------|----------|
| 连接不稳定 | 指数退避重试 |
| spawn 失败 | 全量回滚已创建 actors |
| 同步 tick 阻塞 | 超时机制 |
| Actor 泄露 | teardown + 日志审计 |
