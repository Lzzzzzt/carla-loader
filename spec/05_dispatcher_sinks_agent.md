# Agent 5 — Data Dispatcher + Sinks（分发器与输出端）

> 目标：先**调研**现有输出需求/格式与可能的 sink 类型，再**规划**隔离慢 sink 的并发模型，最后**实现** Dispatcher 主循环 + 至少两类 Sink，并具备失败隔离与可观测性。

---

## 1) 调研（必须先做）

- [ ] 仓库内是否已有输出模块（写盘、网络、日志、ROS 等）
- [ ] 输出格式要求：原始 payload vs 编码后；文件命名/分片/滚动策略
- [ ] 网络要求：UDP/TCP/QUIC？是否需要 backpressure/重试
- [ ] 与编排器对齐 `contracts::DataSink` trait 以及 `SyncedFrame` 的字段使用方式

**调研产出**
- `docs/research/output_sinks_survey.md`：需要支持的 sink 列表、优先级、格式约束

---

## 2) 规划（隔离与失败策略）

### 2.1 并发模型（必须隔离慢 sink）
- [ ] Dispatcher 消费 `SyncedFrame` 后，按路由表 fan-out
- [ ] 每个 sink 拥有独立队列 + worker 任务（tokio task 或线程）
- [ ] 队列满策略：丢弃/阻塞/降级（可配置，默认不阻塞主链路）

### 2.2 失败策略（最小必需）
- [ ] 单 sink 写失败：记录失败率/错误，不影响其他 sink
- [ ] 可选：重试（指数退避）与断路器（失败阈值后短暂停用）

**规划产出**
- `docs/design/dispatcher_sinks.md`：并发拓扑、队列策略、失败策略、可观测性指标

---

## 3) 实现

- [ ] 新建 `dispatcher` crate 与 `sinks` crate（或 `dispatcher` 内含 sinks 模块）
- [ ] 实现 `LogSink`（最小可用）与 `FileSink`（建议：按时间/帧滚动）
- [ ] （可选）实现 `NetworkSink`（UDP）用于在线流式传输
- [ ] 提供路由配置解析（从 `WorldBlueprint`/config 中读取）
- [ ] Metrics：sink_queue_len、sink_write_rate、sink_failures、dropped_by_sink

---

## 4) 验收（Acceptance Tests）

- [ ] 同一帧可同时写入 Log + File（内容/数量一致）
- [ ] 注入慢 sink（sleep/阻塞）：主链路吞吐不显著下降；慢 sink 队列长度上升并被指标观测
- [ ] 注入 sink 写失败：错误计数增长但系统继续运行
- [ ] `cargo test -p dispatcher -p sinks` 全过

---

## 5) 协作约束

- 仅依赖 `contracts`
- 禁止在 sinks 中做同步/对齐逻辑；只消费 `SyncedFrame`
