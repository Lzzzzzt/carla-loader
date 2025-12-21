# Agent 2 — Actor Factory（CARLA 资产工厂）

> 目标：先**调研** CARLA 接入方式与现有封装，再**规划** spawn/attach/teardown 的可靠性设计，最后**实现** `WorldBlueprint -> RuntimeGraph` 的落地，并提供可复用的 mock/合约测试。

---

## 1) 调研（必须先做）

- [ ] 确认 CARLA 版本、连接方式（host/port、同步/异步模式）、是否 headless
- [ ] 现有 client：Python/C++/Rust wrapper/FFI 现状（可重用代码与风险）
- [ ] Actor/传感器蓝图获取方式：blueprint library、attribute 设置、transform 表达
- [ ] teardown 约束：如何销毁 actor，避免幽灵 actor
- [ ] 与编排器对齐 `contracts::RuntimeGraph` 结构（需要保存哪些句柄与映射）

**调研产出**
- `docs/research/carla_integration_survey.md`：接入方式、API 选型、潜在坑（连接重试、同步 tick、actor 泄露）

---

## 2) 规划（可靠性与并发时序）

### 2.1 生命周期与回滚
- [ ] spawn vehicle 成功、spawn sensor 失败：必须回滚销毁已创建对象
- [ ] attach 失败：必须释放已创建 sensor
- [ ] 提供 `teardown(RuntimeGraph)`：保证幂等

### 2.2 并发与时序
- [ ] 先 vehicle 后 sensors
- [ ] sensors attach 完成后再“允许 ingestion 注册回调”（避免 race）
- [ ] 若使用 CARLA 同步模式：明确 tick 驱动策略（由谁 tick）

**规划产出**
- `docs/design/actor_factory.md`：spawn 流程图、错误处理、回滚策略、teardown 语义

---

## 3) 实现

- [ ] 新建 `actor_factory` crate
- [ ] 实现 `spawn_from_blueprint(&WorldBlueprint) -> Result<RuntimeGraph>`
- [ ] 映射表：vehicle_id/sensor_id -> handle（以及 handle -> id 反查可选）
- [ ] 日志：每次 spawn/attach/teardown 记录 id 与关键参数
- [ ] 测试：
  - 合约测试：mock CARLA 接口（无 CARLA 也能跑）
  - （可选）真实集成测试：若 CI 不可跑，则提供 `scripts/run_carla_local.md`

---

## 4) 验收（Acceptance Tests）

- [ ] mock 测试覆盖：spawn 成功、sensor spawn 失败回滚、teardown 幂等
- [ ] 本地接真实 CARLA：能生成指定数量 vehicle + sensors，挂载关系正确
- [ ] teardown 后无残留 actor（可通过 CARLA 查询或日志确认）

---

## 5) 协作约束

- 仅依赖 `contracts`；CARLA wrapper 可在本 crate 内部或单独子模块
- 不要在本 crate 内定义 `SensorPacket`/同步逻辑；回调注册由 Ingestion 负责
