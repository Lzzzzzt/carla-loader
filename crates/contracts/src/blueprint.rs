//! WorldBlueprint - Config Loader 输出
//!
//! 描述完整的世界配置：地图、天气、车辆、传感器、同步策略、输出路由。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{AdaKFConfig, BufferConfig, MissingDataStrategy, SyncEngineConfig, WindowConfig};

/// 配置版本
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ConfigVersion {
    #[default]
    V1,
}

/// 完整的世界配置蓝图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBlueprint {
    /// 配置版本
    #[serde(default)]
    pub version: ConfigVersion,

    /// 世界设置
    pub world: WorldConfig,

    /// 车辆定义列表
    pub vehicles: Vec<VehicleConfig>,

    /// 同步策略配置
    pub sync: SyncConfig,

    /// 输出路由配置
    pub sinks: Vec<SinkConfig>,
}

/// 世界配置：地图、天气等
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldConfig {
    /// 地图名称 (e.g., "Town01")
    pub map: String,

    /// 天气预设 (可选)
    #[serde(default)]
    pub weather: Option<WeatherPreset>,

    /// CARLA 服务器地址
    #[serde(default = "default_carla_host")]
    pub carla_host: String,

    /// CARLA 服务器端口
    #[serde(default = "default_carla_port")]
    pub carla_port: u16,
}

fn default_carla_host() -> String {
    "localhost".to_string()
}

fn default_carla_port() -> u16 {
    2000
}

/// 天气预设
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherPreset {
    ClearNoon,
    CloudyNoon,
    WetNoon,
    RainyNoon,
    ClearSunset,
    Custom(WeatherParams),
}

/// 自定义天气参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherParams {
    pub cloudiness: f32,
    pub precipitation: f32,
    pub sun_altitude_angle: f32,
}

/// 车辆配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VehicleConfig {
    /// 唯一标识符
    pub id: String,

    /// 蓝图名称 (e.g., "vehicle.tesla.model3")
    pub blueprint: String,

    /// 初始位姿
    pub spawn_point: Option<Transform>,

    /// 挂载的传感器列表
    #[serde(default)]
    pub sensors: Vec<SensorConfig>,
}

/// 3D 变换：位置 + 旋转
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Transform {
    /// 位置 (x, y, z) 单位：米
    pub location: Location,

    /// 旋转 (pitch, yaw, roll) 单位：度
    pub rotation: Rotation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Location {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rotation {
    pub pitch: f64,
    pub yaw: f64,
    pub roll: f64,
}

/// 传感器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorConfig {
    /// 唯一标识符
    pub id: String,

    /// 传感器类型
    pub sensor_type: SensorType,

    /// 相对于父 actor 的挂载位姿
    pub transform: Transform,

    /// 采样频率 (Hz)，必须 > 0
    pub frequency_hz: f64,

    /// 传感器特定属性
    #[serde(default)]
    pub attributes: HashMap<String, String>,
}

/// 传感器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorType {
    Camera,
    Lidar,
    Imu,
    Gnss,
    Radar,
}

/// 同步策略配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// 主时钟传感器 ID (用于确定参考时间)
    pub primary_sensor_id: String,

    /// 同步窗口下限 (秒)
    #[serde(default = "default_min_window")]
    pub min_window_sec: f64,

    /// 同步窗口上限 (秒)
    #[serde(default = "default_max_window")]
    pub max_window_sec: f64,

    /// 缺帧策略
    #[serde(default)]
    pub missing_frame_policy: MissingFramePolicy,

    /// 丢包策略
    #[serde(default)]
    pub drop_policy: DropPolicy,

    /// Additional sync engine tuning parameters
    #[serde(default)]
    pub engine: SyncEngineOverrides,
}

/// Optional overrides for the runtime sync engine
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncEngineOverrides {
    /// List of sensors that must be present for a synced frame
    #[serde(default)]
    pub required_sensor_ids: Vec<String>,

    /// IMU sensor used for adaptive windowing
    #[serde(default)]
    pub imu_sensor_id: Option<String>,

    /// Custom window bounds in milliseconds
    #[serde(default)]
    pub window: Option<WindowConfig>,

    /// Buffer behavior adjustments
    #[serde(default)]
    pub buffer: Option<BufferConfig>,

    /// AdaKF tuning parameters
    #[serde(default)]
    pub adakf: Option<AdaKFConfig>,

    /// Expected interval per sensor (seconds)
    #[serde(default)]
    pub sensor_intervals: HashMap<String, f64>,
}

fn default_min_window() -> f64 {
    0.020 // 20ms
}

fn default_max_window() -> f64 {
    0.100 // 100ms
}

/// 缺帧处理策略
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingFramePolicy {
    /// 丢弃该同步帧
    #[default]
    Drop,
    /// 使用空帧占位
    Empty,
    /// 插值填充
    Interpolate,
}

/// 丢包策略 (背压满时)
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropPolicy {
    /// 丢弃最旧的包
    #[default]
    DropOldest,
    /// 丢弃最新的包
    DropNewest,
}

/// Sink 输出配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SinkConfig {
    /// Sink 名称
    pub name: String,

    /// Sink 类型
    pub sink_type: SinkType,

    /// 队列容量
    #[serde(default = "default_queue_capacity")]
    pub queue_capacity: usize,

    /// 类型特定参数
    #[serde(default)]
    pub params: HashMap<String, String>,
}

fn default_queue_capacity() -> usize {
    100
}

/// Sink 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SinkType {
    /// 日志输出
    Log,
    /// 文件输出
    File,
    /// 网络输出 (UDP)
    Network,
}

impl WorldBlueprint {
    /// Build a SyncEngineConfig using blueprint data and optional overrides
    pub fn to_sync_engine_config(&self) -> SyncEngineConfig {
        let overrides = &self.sync.engine;

        let required_sensors = if overrides.required_sensor_ids.is_empty() {
            self.default_required_sensors()
        } else {
            overrides.required_sensor_ids.clone()
        };

        let imu_sensor_id = overrides.imu_sensor_id.clone().or_else(|| {
            self.first_sensor_of_type(SensorType::Imu)
                .map(|s| s.id.clone())
        });

        let mut window = overrides.window.clone().unwrap_or(WindowConfig {
            min_ms: self.sync.min_window_sec * 1000.0,
            max_ms: self.sync.max_window_sec * 1000.0,
        });
        if window.min_ms > window.max_ms {
            std::mem::swap(&mut window.min_ms, &mut window.max_ms);
        }

        let buffer = overrides.buffer.clone().unwrap_or_default();
        let adakf = overrides.adakf.clone().unwrap_or_default();

        let mut sensor_intervals = overrides.sensor_intervals.clone();
        for sensor in self.all_sensors() {
            if sensor.frequency_hz > 0.0 {
                sensor_intervals
                    .entry(sensor.id.clone())
                    .or_insert_with(|| 1.0 / sensor.frequency_hz);
            }
        }

        SyncEngineConfig {
            reference_sensor_id: self.sync.primary_sensor_id.clone(),
            required_sensors,
            imu_sensor_id,
            window,
            buffer,
            adakf,
            missing_strategy: MissingDataStrategy::from(self.sync.missing_frame_policy),
            sensor_intervals,
        }
    }

    fn all_sensors(&self) -> impl Iterator<Item = &SensorConfig> {
        self.vehicles
            .iter()
            .flat_map(|vehicle| vehicle.sensors.iter())
    }

    fn default_required_sensors(&self) -> Vec<String> {
        if let Some(vehicle) = self.vehicles.iter().find(|vehicle| {
            vehicle
                .sensors
                .iter()
                .any(|sensor| sensor.id == self.sync.primary_sensor_id)
        }) {
            return vehicle
                .sensors
                .iter()
                .map(|sensor| sensor.id.clone())
                .collect();
        }

        self.all_sensors().map(|sensor| sensor.id.clone()).collect()
    }

    fn first_sensor_of_type(&self, kind: SensorType) -> Option<&SensorConfig> {
        self.all_sensors().find(|sensor| sensor.sensor_type == kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sensor(id: &str, sensor_type: SensorType, frequency_hz: f64) -> SensorConfig {
        SensorConfig {
            id: id.to_string(),
            sensor_type,
            transform: Transform {
                location: Location {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                rotation: Rotation {
                    pitch: 0.0,
                    yaw: 0.0,
                    roll: 0.0,
                },
            },
            frequency_hz,
            attributes: HashMap::new(),
        }
    }

    fn sample_blueprint() -> WorldBlueprint {
        WorldBlueprint {
            version: ConfigVersion::V1,
            world: WorldConfig {
                map: "Town01".into(),
                weather: None,
                carla_host: "localhost".into(),
                carla_port: 2000,
            },
            vehicles: vec![VehicleConfig {
                id: "ego".into(),
                blueprint: "vehicle.test".into(),
                spawn_point: None,
                sensors: vec![
                    sample_sensor("cam_main", SensorType::Camera, 20.0),
                    sample_sensor("lidar_top", SensorType::Lidar, 10.0),
                    sample_sensor("imu_sensor", SensorType::Imu, 100.0),
                ],
            }],
            sync: SyncConfig {
                primary_sensor_id: "cam_main".into(),
                min_window_sec: 0.02,
                max_window_sec: 0.1,
                missing_frame_policy: MissingFramePolicy::Drop,
                drop_policy: DropPolicy::DropOldest,
                engine: SyncEngineOverrides::default(),
            },
            sinks: vec![],
        }
    }

    #[test]
    fn sync_engine_config_defaults() {
        let blueprint = sample_blueprint();
        let config = blueprint.to_sync_engine_config();
        assert_eq!(config.reference_sensor_id, "cam_main");
        assert_eq!(config.required_sensors.len(), 3);
        assert_eq!(config.window.min_ms, 20.0);
        assert_eq!(config.window.max_ms, 100.0);
        assert_eq!(config.missing_strategy, MissingDataStrategy::Drop);
        assert_eq!(config.sensor_intervals.get("cam_main").copied(), Some(0.05));
    }

    #[test]
    fn sync_engine_config_overrides() {
        let mut blueprint = sample_blueprint();
        blueprint.sync.engine.required_sensor_ids = vec!["cam_main".into(), "lidar_top".into()];
        blueprint.sync.engine.imu_sensor_id = Some("imu_sensor".into());
        blueprint.sync.engine.window = Some(WindowConfig {
            min_ms: 10.0,
            max_ms: 80.0,
        });
        blueprint.sync.engine.buffer = Some(BufferConfig {
            max_size: 256,
            timeout_s: 0.5,
        });
        blueprint.sync.engine.adakf = Some(AdaKFConfig {
            initial_offset: 0.0,
            process_noise: 0.0002,
            measurement_noise: 0.0005,
            residual_window: 10,
            expected_interval: Some(0.05),
        });
        blueprint.sync.engine.sensor_intervals =
            HashMap::from([("cam_main".into(), 0.05), ("lidar_top".into(), 0.1)]);

        let config = blueprint.to_sync_engine_config();
        assert_eq!(config.window.min_ms, 10.0);
        assert_eq!(config.window.max_ms, 80.0);
        assert_eq!(config.buffer.max_size, 256);
        assert_eq!(config.adakf.residual_window, 10);
        assert_eq!(config.required_sensors.len(), 2);
        assert_eq!(config.sensor_intervals.get("lidar_top").copied(), Some(0.1));
    }
}
