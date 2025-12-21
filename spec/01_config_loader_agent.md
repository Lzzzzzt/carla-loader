# Agent 1 — Config Loader（配置加载器）

> 目标：先**调研**现有配置/字段/运行入口，再**规划**配置 schema 与校验规则，最后**实现**配置解析与 `WorldBlueprint` 生成，配套单测与示例配置。

---

## 1) 调研（必须先做）

- [ ] 搜索仓库是否已有配置文件、示例、环境变量约定（TOML/JSON/YAML）
- [ ] 查明启动参数入口（CLI / config path / defaults）
- [ ] 列出需要支持的配置域：
  - 地图/天气/世界参数
  - 车辆定义（数量、蓝图、初始位姿）
  - 传感器定义（类型、挂载、频率、外参/内参、输出编码）
  - 同步策略（窗口上下限、主时钟传感器、丢包策略）
  - 输出路由（多个 sink，sink 参数）
- [ ] 与编排器对齐 `contracts::WorldBlueprint` 的字段骨架与扩展方式

**调研产出（提交到 `docs/research/`）**
- `docs/research/config_survey.md`：现有配置现状、缺口、兼容性建议

---

## 2) 规划（Schema + 校验）

### 2.1 Schema 设计
- [ ] 定义 `ConfigVersion`（例如 `v1`）
- [ ] 明确字段默认值（频率、窗口、sink 参数等）
- [ ] 明确枚举与单位（Hz、秒、角度/弧度、坐标系）

### 2.2 校验规则（必须可定位错误）
- [ ] sensor_id 唯一；vehicle_id 唯一
- [ ] 传感器挂载拓扑合法：sensor 只能挂载在存在的 vehicle 上
- [ ] 采样率合法（>0）；同步窗口上下限合法（min<=max）
- [ ] sink 配置合法：必填字段齐全、目标地址/路径可解析
- [ ] 禁止“静默修正”：所有自动修正必须写入日志并可被测试覆盖

**规划产出**
- `docs/design/config_schema.md`：字段列表、示例、默认值、校验规则、迁移策略

---

## 3) 实现

- [ ] 新建 `config_loader` crate
- [ ] serde 解析 TOML/JSON（建议 TOML 为主；JSON 作为可选）
- [ ] 生成 `contracts::WorldBlueprint`
- [ ] 提供清晰错误（带字段路径/上下文），建议用 `thiserror`
- [ ] 提供 `examples/minimal.toml` + `examples/full.toml`
- [ ] 单测覆盖：>=10 组（合法/非法），含失败路径

---

## 4) 验收（Acceptance Tests）

- [ ] `cargo test -p config_loader` 全过
- [ ] 最小配置可生成 Blueprint（并能被后续模块消费）
- [ ] 非法配置报错信息可定位（字段路径 + 原因）
- [ ] Blueprint（可选）支持 round-trip：serialize -> deserialize 一致

---

## 5) 协作约束

- 仅依赖 `contracts` crate；不得依赖其他业务 crate
- 任何 `WorldBlueprint` 字段新增/修改：先向编排器提案，走 contracts 变更流程
