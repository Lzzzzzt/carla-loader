# Agent 6 — Observability & Reliability（可观测性与稳定性）

> 目标：先**调研**现有日志/指标/错误处理方式，再**规划**统一规范与埋点清单，最后**实现**跨组件可复用的 observability 模块与关键告警建议，确保问题可定位。

---

## 1) 调研（必须先做）

- [ ] 仓库现有日志库：log/tracing；格式化输出（json/plain）
- [ ] 是否已有 metrics（prometheus/opentelemetry）与导出方式
- [ ] 错误处理现状：anyhow/thiserror/自定义 error
- [ ] 与编排器对齐错误分层与指标命名约束

**调研产出**
- `docs/research/observability_survey.md`：现状、缺口、推荐选型

---

## 2) 规划（规范与指标）

### 2.1 日志/Tracing 规范
- [ ] 统一 span：component=..., sensor_id=..., sink=...
- [ ] 关键事件：spawn/attach、callback 注册、queue 满、sync 输出、sink 失败

### 2.2 Metrics 清单（必须覆盖端到端）
- [ ] ingestion：ingest_rate、dropped_packets、queue_len
- [ ] sync：buffer_depth、out_of_order、sync_latency、kf_residual、frames_out_rate
- [ ] dispatcher/sink：sink_queue_len、sink_write_rate、sink_failures、dropped_by_sink
- [ ] system：memory、cpu（可选）

### 2.3 告警建议（阈值建议 + 说明）
- [ ] sync_latency p95 超阈值
- [ ] dropped_packets 持续增长
- [ ] 某 sink failures 连续爆发、断路器触发

**规划产出**
- `docs/design/observability_spec.md`：命名规范、label、指标定义、告警建议

---

## 3) 实现

- [ ] 新建 `observability` crate（或公共模块）
- [ ] 提供初始化函数：tracing subscriber + metrics exporter（按选型）
- [ ] 提供宏/helper：统一创建 span、统一计数器/直方图
- [ ] 在关键路径提供示例接入（至少 2 个组件演示）

---

## 4) 验收（Acceptance Tests）

- [ ] 本地运行模拟链路可看到关键日志与指标输出
- [ ] 指标命名与 label 符合规范（可写静态检查或快照测试）
- [ ] `cargo test -p observability` 通过

---

## 5) 协作约束

- 不改变业务逻辑；仅提供“可复用的埋点与规范”
- 不引入重依赖导致二进制暴涨（选型需在调研中说明权衡）
