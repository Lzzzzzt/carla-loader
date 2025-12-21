# Agent 7 — QA / Systems Test（系统测试与回归）

> 目标：先**调研**现有测试覆盖与可运行入口，再**规划**“无 CARLA 也能跑”的端到端测试链路与回归基线，最后**实现**集成测试、乱序重放回归、性能基线与 CI 门禁。

---

## 1) 调研（必须先做）

- [ ] 仓库已有单测/集成测试结构（tests/、dev-deps、fixture）
- [ ] 是否能在 CI 起 CARLA（通常不行）：若不行，必须提供 mock/模拟源 e2e
- [ ] 关键质量目标：吞吐、延迟、内存占用、丢包率
- [ ] 与编排器对齐：contracts 快照测试策略、CI gating

**调研产出**
- `docs/research/test_survey.md`：现状、缺口、CI 可运行策略

---

## 2) 规划（测试分层与基线）

### 2.1 合约测试（contracts）
- [ ] 对 `contracts` 的结构定义做快照/编译期约束测试
- [ ] 任何 breaking change 必须更新快照并说明

### 2.2 模拟 e2e（无需 CARLA）
- [ ] 使用 Ingestion 的 `MockSensorSource` 生成 cam/lidar/imu 包
- [ ] 跑通 ingestion -> sync -> dispatcher -> sinks（file/log）
- [ ] 覆盖乱序/缺失/抖动用例（与 Sync Engine 的测试计划一致）

### 2.3 性能回归（可选但建议）
- [ ] 基准：固定持续时间内的 frames_out_rate、sync_latency 分布、内存峰值
- [ ] 输出 `bench_report.md`（可手工生成，但数据必须来自可重复脚本）

**规划产出**
- `docs/design/test_plan.md`：分层策略、用例矩阵、CI 跑哪些、nightly 跑哪些

---

## 3) 实现

- [ ] 新建 `tests` crate（或 `/tests` 集成测试目录）
- [ ] 实现：
  - 合约快照测试
  - 模拟 e2e 测试（至少 2 个：正常流 + 乱序流）
  - 故障注入：慢 sink、sink 失败
- [ ] 提供脚本：`scripts/run_e2e.sh`（本地一键跑）
- [ ] 将关键测试接入 CI gating（无 CARLA）

---

## 4) 验收（Acceptance Tests）

- [ ] CI 上 `cargo test` 能跑完核心 e2e（无 CARLA）
- [ ] 乱序/缺失用例输出稳定（golden/快照）
- [ ] 故障注入：系统不中断、指标/日志能定位问题点

---

## 5) 协作约束

- 测试必须只通过公开接口驱动（contracts + 各 crate 公共 API）
- 不可在测试中依赖本地特定路径/手工步骤（除非明确标注为 manual test）
