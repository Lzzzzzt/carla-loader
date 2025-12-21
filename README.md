# CARLA Syncer

基于 CARLA 的多传感器数据导入与同步模块。

## 项目结构

```
carla-syncer/
├── contracts/       # 冻结的接口契约 (ICD)
├── config_loader/   # 配置解析 → WorldBlueprint
├── actor_factory/   # CARLA actor spawn → RuntimeGraph
├── ingestion/       # 传感器回调 → SensorPacket
├── sync_engine/     # 多传感器同步 → SyncedFrame
├── dispatcher/      # 数据分发到 sinks
├── observability/   # 日志与指标
└── tests/           # 集成测试
```

## 数据流

```
Config → WorldBlueprint → RuntimeGraph
                              ↓
                         Sensors
                              ↓
                      SensorPacket (ingestion)
                              ↓
                      SyncedFrame (sync_engine)
                              ↓
                        DataSink (dispatcher)
```

## 快速开始

```bash
# 构建
cargo build --workspace

# 运行测试
cargo test --workspace

# 格式检查
cargo fmt --all --check

# Lint 检查
cargo clippy --workspace -- -D warnings
```

## 文档

- `docs/research/` - 调研报告
- `docs/design/` - 设计文档
- `docs/architecture.md` - 架构总览
