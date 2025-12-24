//! WorldBlueprint - Config Loader output
//!
//! Describes the complete world configuration: map, weather, vehicles, sensors, sync policy, output routing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use validator::Validate;

use crate::{AdaKFConfig, BufferConfig, MissingDataStrategy, SyncEngineConfig, WindowConfig};

/// Configuration version
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ConfigVersion {
    #[default]
    V1,
}

/// Complete world configuration blueprint
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct WorldBlueprint {
    /// Configuration version
    #[serde(default)]
    pub version: ConfigVersion,

    /// World settings
    #[validate(nested)]
    pub world: WorldConfig,

    /// Vehicle definition list
    #[validate(nested)]
    pub vehicles: Vec<VehicleConfig>,

    /// Sync policy configuration
    #[validate(nested)]
    pub sync: SyncConfig,

    /// Output routing configuration
    #[validate(nested)]
    pub sinks: Vec<SinkConfig>,
}

/// World configuration: map, weather, etc.
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct WorldConfig {
    /// Map name (e.g., "Town01")
    #[validate(length(min = 1, message = "map name cannot be empty"))]
    pub map: String,

    /// Weather preset (optional)
    #[serde(default)]
    pub weather: Option<WeatherPreset>,

    /// CARLA server address
    #[serde(default = "default_carla_host")]
    pub carla_host: String,

    /// CARLA server port
    #[serde(default = "default_carla_port")]
    #[validate(range(min = 1, max = 65535))]
    pub carla_port: u16,
}

fn default_carla_host() -> String {
    "localhost".to_string()
}

fn default_carla_port() -> u16 {
    2000
}

/// Weather preset
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

/// Custom weather parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherParams {
    pub cloudiness: f32,
    pub precipitation: f32,
    pub sun_altitude_angle: f32,
}

/// Vehicle configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct VehicleConfig {
    /// Unique identifier
    #[validate(length(min = 1, message = "vehicle id cannot be empty"))]
    pub id: String,

    /// Blueprint name (e.g., "vehicle.tesla.model3")
    #[validate(length(min = 1, message = "blueprint name cannot be empty"))]
    pub blueprint: String,

    /// Initial pose
    pub spawn_point: Option<Transform>,

    /// Attached sensor list
    #[serde(default)]
    #[validate(nested)]
    pub sensors: Vec<SensorConfig>,
}

/// 3D transform: position + rotation
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Transform {
    /// Position (x, y, z) in meters
    pub location: Location,

    /// Rotation (pitch, yaw, roll) in degrees
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

/// Sensor configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SensorConfig {
    /// Unique identifier
    #[validate(length(min = 1, message = "sensor id cannot be empty"))]
    pub id: String,

    /// Sensor type
    pub sensor_type: SensorType,

    /// Mount pose relative to parent actor
    pub transform: Transform,

    /// Sampling frequency (Hz), must be > 0
    #[validate(range(exclusive_min = 0.0, message = "frequency_hz must be > 0"))]
    pub frequency_hz: f64,

    /// Sensor-specific attributes
    #[serde(default)]
    pub attributes: HashMap<String, String>,
}

/// Sensor type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorType {
    Camera,
    Lidar,
    Imu,
    Gnss,
    Radar,
}

/// Sync policy configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
#[validate(schema(function = "validate_sync_window"))]
pub struct SyncConfig {
    /// Primary clock sensor ID (used to determine reference time)
    #[validate(length(min = 1, message = "primary_sensor_id cannot be empty"))]
    pub primary_sensor_id: String,

    /// Sync window lower bound (seconds)
    #[serde(default = "default_min_window")]
    #[validate(range(min = 0.0))]
    pub min_window_sec: f64,

    /// Sync window upper bound (seconds)
    #[serde(default = "default_max_window")]
    #[validate(range(min = 0.0))]
    pub max_window_sec: f64,

    /// Missing frame policy
    #[serde(default)]
    pub missing_frame_policy: MissingFramePolicy,

    /// Drop policy
    #[serde(default)]
    pub drop_policy: DropPolicy,

    /// Additional sync engine tuning parameters
    #[serde(default)]
    pub engine: SyncEngineOverrides,
}

/// Validate sync window (min <= max)
fn validate_sync_window(config: &SyncConfig) -> Result<(), validator::ValidationError> {
    if config.min_window_sec > config.max_window_sec {
        let mut err = validator::ValidationError::new("window_range");
        err.message = Some(std::borrow::Cow::Borrowed(
            "min_window_sec must be <= max_window_sec",
        ));
        return Err(err);
    }
    Ok(())
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

/// Missing frame handling policy
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingFramePolicy {
    /// Drop the sync frame
    #[default]
    Drop,
    /// Use empty frame as placeholder
    Empty,
    /// Interpolate to fill
    Interpolate,
}

/// Drop policy (when backpressure is full)
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropPolicy {
    /// Drop oldest packets
    #[default]
    DropOldest,
    /// Drop newest packets
    DropNewest,
}

/// Sink output configuration
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SinkConfig {
    /// Sink name
    #[validate(length(min = 1, message = "sink name cannot be empty"))]
    pub name: String,

    /// Sink type
    pub sink_type: SinkType,

    /// Queue capacity
    #[serde(default = "default_queue_capacity")]
    pub queue_capacity: usize,

    /// Type-specific parameters
    #[serde(default)]
    pub params: HashMap<String, String>,
}

fn default_queue_capacity() -> usize {
    100
}

/// Sink type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SinkType {
    /// Log output
    Log,
    /// File output
    File,
    /// Network output (UDP)
    Network,
}

impl WorldBlueprint {
    /// Build a SyncEngineConfig using blueprint data and optional overrides
    pub fn to_sync_engine_config(&self) -> SyncEngineConfig {
        use crate::SensorId;

        let overrides = &self.sync.engine;

        let required_sensors: Vec<SensorId> = if overrides.required_sensor_ids.is_empty() {
            self.default_required_sensors()
                .into_iter()
                .map(SensorId::from)
                .collect()
        } else {
            overrides
                .required_sensor_ids
                .iter()
                .map(|s| SensorId::from(s.as_str()))
                .collect()
        };

        let imu_sensor_id: Option<SensorId> = overrides
            .imu_sensor_id
            .as_ref()
            .map(|s| SensorId::from(s.as_str()))
            .or_else(|| {
                self.first_sensor_of_type(SensorType::Imu)
                    .map(|s| SensorId::from(s.id.as_str()))
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

        let mut sensor_intervals: std::collections::HashMap<SensorId, f64> = overrides
            .sensor_intervals
            .iter()
            .map(|(k, v)| (SensorId::from(k.as_str()), *v))
            .collect();
        for sensor in self.all_sensors() {
            if sensor.frequency_hz > 0.0 {
                let key = SensorId::from(sensor.id.as_str());
                sensor_intervals
                    .entry(key)
                    .or_insert_with(|| 1.0 / sensor.frequency_hz);
            }
        }

        SyncEngineConfig {
            reference_sensor_id: SensorId::from(self.sync.primary_sensor_id.as_str()),
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
