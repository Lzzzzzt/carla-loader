# Agent 3 — Ingestion Pipeline / FFI（摄取流水线与内存安全）

> 目标：先**调研**现有回调/FFI 边界与数据格式，再**规划**安全封装与背压策略，最后**实现**从传感器回调到 `SensorPacket` 通道的高吞吐路径，并提供压测与失败路径测试。

---

## 1) 调研（必须先做）

- [ ] 仓库内是否已有 FFI 层（C++/C）与回调注册接口
- [ ] 现有 payload：图像、点云、IMU 等的数据结构/序列化方式
- [ ] 内存所有权：CARLA 回调提供的 buffer 生命周期（谁分配/谁释放）
- [ ] 是否已有 tokio runtime；线程模型（回调线程 vs async 任务）
- [ ] 与编排器对齐 `contracts::SensorPacket` payload 形态（Bytes/Arc<[u8]>）

**调研产出**
- `docs/research/ffi_ingestion_survey.md`：unsafe 边界、内存所有权、可复用代码、风险清单

---

## 2) 规划（安全封装 + 背压）

### 2.1 Unsafe 最小化
- [ ] 把所有 unsafe 封在 `ffi.rs`/`unsafe_mod` 中，外部只暴露安全 API
- [ ] 明确生命周期：回调数据必须在返回前完成拷贝或转移所有权（禁止悬垂引用）
- [ ] 对大 payload（图像/点云）优先 bytes/Arc 零拷贝策略（可通过自定义 allocator/引用计数实现）

### 2.2 背压策略（必须可配置、可观测）
- [ ] 通道容量：按 sensor_type 配置
- [ ] 满时策略：drop_oldest / drop_newest / block（默认不得 block 回调线程）
- [ ] 记录 metrics：ingest_rate、dropped_packets、queue_len

**规划产出**
- `docs/design/ingestion_pipeline.md`：回调线程模型、内存策略、背压策略、payload 类型约定

---

## 3) 实现

- [ ] 新建 `ingestion` crate
- [ ] 为每类传感器实现 `SensorAdapter`：
  - 注册回调
  - 解析/封装为 `SensorPacket`
  - 送入 `tokio::mpsc` 或 `crossbeam` 通道（按编排器契约）
- [ ] 提供 `MockSensorSource`（用于无 CARLA 的 e2e）
- [ ] 压测：模拟 20Hz camera + 10Hz lidar + 100Hz imu，持续运行 N 秒输出统计
- [ ] 失败路径测试：
  - 通道满时的丢包策略正确
  - payload 非法/解析失败有明确错误与计数

---

## 4) 验收（Acceptance Tests）

- [ ] `cargo test -p ingestion` 通过
- [ ] 压测运行稳定（无崩溃、无内存暴涨），丢包与队列长度可观测
- [ ] sanitizer（可选）跑过关键回调路径（ASan/UBSan 任选）

---

## 5) 协作约束

- 仅依赖 `contracts`（以及必要的 FFI 依赖）
- 不实现同步/分发逻辑；只负责把原始数据稳定变成 `SensorPacket` 流
