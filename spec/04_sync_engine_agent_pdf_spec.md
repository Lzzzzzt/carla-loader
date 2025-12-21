# Agent 4 — Sync Engine（同步引擎，必须以 PDF 为准）

> 目标：先**调研** PDF 中同步引擎的“硬约束”与现有实现，再**规划**可测试的状态机/数据结构与接口，最后**实现**事件驱动同步、IMU 自适应窗口、KF/AdaKF 校正、可选运动补偿，并提供乱序重放测试集。

---

## 1) 调研（必须先做）

### 1.1 PDF 必读与硬约束提取（不可改）
- [ ] 逐段阅读 PDF 中 Sync Engine 相关章节，提取以下要点为硬约束清单：
  - 事件驱动触发 + 每传感器缓冲队列
  - IMU 动态窗口（下限 20ms，上限 100ms 的示例区间映射）
  - 窗口内选帧 + 时间戳插值/卡尔曼校正（AdaKF）
  - 输出后移除已用帧；记录残差用于自适应调参
  - 输出通过抽象接口分发到 sinks
- [ ] 与编排器对齐 `contracts::SensorPacket` / `SyncedFrame` / `SyncMeta` 字段是否足够表达 PDF 需求

**调研产出**
- `docs/research/sync_engine_pdf_requirements.md`：硬约束清单（标注 PDF 页码/章节）
- `docs/research/sync_engine_gap_analysis.md`：contracts/现有代码与 PDF 需求的差距与提案

---

## 2) 规划（算法与工程设计，必须可测试）

### 2.1 数据结构与状态机
- [ ] 每传感器缓冲：有序队列/小顶堆（按 timestamp 排序，支持乱序插入）
- [ ] 参考时钟选择：以主传感器为参考（由配置指定）；若缺失则等待/降级策略（必须可配置）
- [ ] 同步触发：任一新包到达触发尝试；若未满足“各类至少一帧”则不输出；

### 2.2 IMU 自适应窗口（Δt）
- [ ] 从最新 IMU 估计运动强度指标（线速度/角速度等）
- [ ] 映射到窗口：min_dt=20ms，max_dt=100ms（示例遵循 PDF），并支持配置覆盖
- [ ] 窗口趋近 0 时：必须插值（IMU 高采样）

### 2.3 AdaKF 时间偏移估计
- [ ] 定义状态：每传感器相对主时钟 offset（可一维）
- [ ] 观测：选帧时间戳与参考时刻差；更新 offset，输出校正时间
- [ ] AdaKF：依据残差统计调整噪声协方差（需设计触发条件与边界）

### 2.4 输出与淘汰策略
- [ ] 输出 `SyncedFrame { t_sync, frames, sync_meta }`
- [ ] 移除已消费帧；对过期帧执行淘汰（计数 dropped/out_of_order）
- [ ] 所有策略（超时、缺失补齐：drop/empty/interpolate）必须可配置，并有明确默认值

**规划产出（必须提交）**
- `docs/design/sync_engine_design.md`：状态机、队列策略、窗口策略、AdaKF 公式/伪代码、复杂度分析
- `docs/design/sync_engine_test_plan.md`：乱序/延迟/缺失/抖动用例与期望输出

---

## 3) 实现

- [ ] 新建 `sync_engine` crate
- [ ] 提供入口：
  - `push(packet) -> Option<SyncedFrame>`（同步输出）或 `async Stream<SyncedFrame>`（以 contracts 冻结为准）
- [ ] 实现：
  - 乱序缓存与重排序
  - IMU 动态窗口计算
  - 窗口内选帧
  - KF offset 更新与时间校正；AdaKF 调参闭环
  - IMU 插值/积分；（可选）LiDAR 运动补偿模块（可 feature flag）
- [ ] Metrics：buffer_depth、sync_latency、kf_residual、dropped/out_of_order

---

## 4) 验收（Acceptance Tests）

必须提供“可重放的固定输入序列”测试集：
- [ ] 正常序列：cam20Hz + lidar10Hz + imu100Hz → 连续输出 SyncedFrame
- [ ] 乱序序列：人为打乱到达顺序 → 输出仍可解释（t_sync 单调或符合策略）
- [ ] 缺失：某传感器短暂缺帧 → 按配置策略 drop/empty/interpolate
- [ ] 抖动：时间戳噪声 → KF/AdaKF 残差收敛趋势可观测

**验收门槛**
- [ ] `cargo test -p sync_engine` 全过
- [ ] 乱序重放测试：输出快照稳定（golden file）
- [ ] 不允许静默丢包：所有丢弃必须计数并可观测

---

## 5) 协作约束

- Sync Engine 的行为以 PDF 为唯一权威规格；若发现冲突，提交 `gap_analysis` 给编排器决策
- 仅依赖 `contracts`；不得在本 crate 引入 sinks/dispatcher

## AdaKF 设计

