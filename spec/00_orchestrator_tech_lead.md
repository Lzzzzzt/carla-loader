# Agent 0 — Orchestrator / Tech Lead（编排器）

> 目标：先**调研**现有代码/依赖/运行方式，再**规划**并冻结接口契约（ICD），最后**实现**工程骨架、CI 与集成门禁，确保多 Agent 并行开发不翻车。

---

## 1) 调研（必须先做，产出可复用结论）

### 1.1 代码与环境调研
- [ ] 拉起当前仓库（workspace/crates/模块边界、Rust edition、tokio 版本、依赖管理方式）
- [ ] 现有 CARLA 接入方式（Python client / C++ client / Rust wrapper / FFI）
- [ ] 当前数据格式（图像/LiDAR/IMU）是否已有定义；是否已有日志/metrics 方案
- [ ] 现有 CI（若无：确定 GitHub Actions / GitLab CI 目标平台）

**产出（必须提交到 `docs/research/`）**
- `docs/research/repo_survey.md`：仓库结构、依赖、可运行入口、风险点
- `docs/research/toolchain.md`：Rust/cargo、fmt/clippy、sanitizer、CARLA 版本/运行方式

### 1.2 需求与约束调研（必读）
- [ ] 阅读用户提供的设计 PDF，提炼“必须遵循”的 Sync Engine 行为（作为硬约束）
- [ ] 与其他 Agent 对齐：contracts 冻结原则、接口变更流程、PR 评审规则

**产出**
- `docs/research/pdf_key_requirements.md`：Sync Engine 的“不可改”清单（引用章节/页码即可）

---

## 2) 规划（先冻结接口，再分解依赖）

### 2.1 冻结 ICD（接口契约）
在 `contracts` crate 中冻结以下结构与 trait（只允许兼容扩展）：
- `WorldBlueprint`（Config Loader 输出）
- `RuntimeGraph`（Actor Factory 输出）
- `SensorPacket`（Ingestion 输出）
- `SyncedFrame/SyncedPacket`（Sync Engine 输出）
- `DataSink` trait（Dispatcher 输出端接口）

**必须明确**
- 时间模型：以 CARLA simulation timestamp（seconds, f64）为主；frame_id 可选用于排序/诊断
- 通道拓扑：Ingestion -> Sync -> Dispatcher 的消息边界与背压策略
- 错误分层：config/carla/ffi/sync/sink
- 指标命名与 label 约束：ingest_rate、buffer_depth、out_of_order、sync_latency、sink_queue_len 等

**产出**
- `contracts/` crate（代码 + rustdoc）
- `docs/architecture.md`：数据流、线程/任务模型、背压策略、错误/指标规范
- PR 模板：要求失败路径测试、非作者 review、接口变更需更新合约测试

### 2.2 分支策略与合入门禁（Gating）
- [ ] 主分支保护：fmt + clippy + unit tests + integration tests
- [ ] 合约快照测试（contracts 的结构变更必须更新快照）
- [ ] 组件依赖方向：所有业务 crate **只能依赖 contracts**，禁止互相反向依赖

---

## 3) 实现（工程骨架 + CI + 集成跑通）

### 3.1 Repo / crate 骨架
建议 workspace：
- `contracts/`
- `config_loader/`
- `actor_factory/`
- `ingestion/`
- `sync_engine/`（按 PDF）
- `dispatcher/` + `sinks/`
- `observability/`（可选但建议）
- `tests/`（集成与回归）

### 3.2 CI
- [ ] `cargo fmt --check`
- [ ] `cargo clippy -- -D warnings`
- [ ] `cargo test`
- [ ] （可选）`cargo test -p tests` e2e（无需 CARLA 的模拟链路必须可跑）

---

## 4) 交付物（Done 的定义）

- [ ] `contracts` crate 冻结并合入主干
- [ ] `docs/architecture.md` + `docs/research/*` 齐全
- [ ] CI 通过、PR 模板可用
- [ ] 提供最小可运行示例（可仅为“模拟 ingestion -> sync -> sinks”）

---

## 5) 风险与注意事项
- **禁止**各 Agent 私自修改 `contracts`：必须走编排器发起的接口变更流程
- Sync Engine 的实现细节必须以 PDF 为唯一权威规格；若与现有接口冲突，优先调整 contracts（通过流程）
