# carla-syncer-cli

CARLA 传感器同步管道的命令行接口。

## 功能

- 从 TOML/JSON 配置文件加载完整管道配置
- 连接 CARLA 服务器并自动 spawn actors
- 运行传感器同步引擎
- 将同步帧分发到多个 sinks
- 支持优雅关闭和清理

## 安装

```bash
cargo install --path crates/cli
```

## 使用方法

### 基础使用

```bash
# 使用默认配置运行
carla-syncer run

# 指定配置文件
carla-syncer run --config /path/to/config.toml

# 指定 CARLA 服务器地址（覆盖配置文件）
carla-syncer run --config config.toml --host 192.168.1.100 --port 2000
```

### 命令行选项

```bash
carla-syncer --help
```

#### 全局选项

| 选项 | 环境变量 | 描述 |
|------|----------|------|
| `-v, --verbose` | `CARLA_SYNCER_VERBOSE` | 增加日志详细程度 |
| `-q, --quiet` | - | 减少日志输出 |
| `--log-format` | `CARLA_SYNCER_LOG_FORMAT` | 日志格式: `json`, `pretty`, `compact` |

#### run 命令选项

| 选项 | 环境变量 | 默认值 | 描述 |
|------|----------|--------|------|
| `-c, --config` | `CARLA_SYNCER_CONFIG` | `config.toml` | 配置文件路径 |
| `--host` | `CARLA_HOST` | - | CARLA 服务器地址（覆盖配置） |
| `--port` | `CARLA_PORT` | - | CARLA 服务器端口（覆盖配置） |
| `--max-frames` | `CARLA_SYNCER_MAX_FRAMES` | 无限制 | 最大同步帧数 |
| `--timeout` | `CARLA_SYNCER_TIMEOUT` | 0 (无限制) | 运行超时秒数 |
| `--dry-run` | - | false | 验证配置但不实际运行 |

### 子命令

#### `run` - 运行同步管道

```bash
carla-syncer run --config config.toml
```

#### `validate` - 验证配置文件

```bash
carla-syncer validate --config config.toml
```

#### `info` - 显示配置信息

```bash
carla-syncer info --config config.toml
```

### 环境变量

可以通过 `.env` 文件或环境变量配置：

```bash
# .env
CARLA_SYNCER_CONFIG=config.toml
CARLA_HOST=localhost
CARLA_PORT=2000
CARLA_SYNCER_LOG_FORMAT=json
CARLA_SYNCER_VERBOSE=true
RUST_LOG=info
```

## 配置文件示例

参见 `crates/config_loader/examples/full.toml`

## 架构

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│ CLI Parser  │────▶│ Config       │────▶│ Pipeline    │
│ (clap)      │     │ Loader       │     │ Orchestrator│
└─────────────┘     └──────────────┘     └─────────────┘
                                                │
       ┌────────────────────────────────────────┼────────────────────────────────┐
       │                                        │                                │
       ▼                                        ▼                                ▼
┌─────────────┐                          ┌─────────────┐                  ┌─────────────┐
│ Actor       │                          │ Ingestion   │                  │ Sync        │
│ Factory     │                          │ Pipeline    │                  │ Engine      │
└─────────────┘                          └─────────────┘                  └─────────────┘
       │                                        │                                │
       │                                        └────────────────────────────────┘
       │                                                       │
       │                                                       ▼
       │                                                ┌─────────────┐
       │                                                │ Dispatcher  │
       │                                                └─────────────┘
       │                                                       │
       └───────────────────────────────────────────────────────┘
                               (cleanup)
```

## License

MIT
