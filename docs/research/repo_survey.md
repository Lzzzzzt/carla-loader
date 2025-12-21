# 仓库调研报告

## 1. 仓库结构

```
carla-syncer/
├── contracts/       # 接口契约 crate
├── config_loader/   # 配置加载
├── actor_factory/   # CARLA actor 工厂
├── ingestion/       # 数据摄取
├── sync_engine/     # 同步引擎
├── dispatcher/      # 数据分发
├── observability/   # 可观测性
└── tests/           # 集成测试
```

## 2. 依赖

| 依赖 | 版本 | 用途 |
|------|------|------|
| tokio | 1.48.0 | 异步运行时 |
| serde | 1.0 | 序列化 |
| thiserror | 2.0 | 错误处理 |
| bytes | 1.10 | 零拷贝缓冲 |

## 3. Rust 版本

- Edition: 2024 (nightly)
- 需要 nightly toolchain

## 4. CI

- GitHub Actions
- 检查项: fmt, clippy, test

## 5. 风险点

- Rust 2024 edition 尚未稳定，可能需要 nightly
- CARLA FFI 集成需要额外绑定库
