# 配置调研报告

## 1. CARLA 传感器类型

| 传感器类型 | CARLA Blueprint | 输出数据 |
|-----------|-----------------|---------|
| **Camera** | `sensor.camera.rgb`, `sensor.camera.depth`, `sensor.camera.semantic_segmentation` | RGBA/Depth/Seg 图像 |
| **LiDAR** | `sensor.lidar.ray_cast`, `sensor.lidar.semantic_ray_cast` | XYZI 点云 |
| **Radar** | `sensor.other.radar` | 检测点 (altitude, azimuth, depth, velocity) |
| **IMU** | `sensor.other.imu` | 加速度、陀螺仪、指南针 |
| **GNSS** | `sensor.other.gnss` | 经纬度、高度 |

## 2. 现有配置状态

- 仓库中无现有配置文件示例
- `contracts` crate 已定义完整数据模型
- 无 CLI 入口定义

## 3. 配置格式选择

| 格式 | 优点 | 缺点 |
|-----|------|-----|
| **TOML** (推荐) | 可读性强、注释支持 | 嵌套复杂时冗长 |
| JSON | 通用、工具支持好 | 无注释 |
| YAML | 简洁 | 缩进敏感 |

**决策**: TOML 为主，JSON 可选

## 4. 兼容性建议

1. 配置版本字段 (`version = "V1"`) 便于迁移
2. 提供合理的默认值减少必填项
3. 清晰的错误信息包含字段路径
