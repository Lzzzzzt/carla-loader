//! 配置校验模块
//!
//! 校验规则：
//! - sensor_id 唯一
//! - vehicle_id 唯一
//! - 传感器挂载拓扑合法 (primary_sensor_id 存在)
//! - frequency_hz > 0
//! - min_window_sec <= max_window_sec
//! - sink 必填字段齐全

use std::collections::HashSet;

use contracts::{ContractError, WorldBlueprint};

/// 校验 WorldBlueprint 配置
///
/// 返回第一个遇到的错误，或 Ok(())。
pub fn validate(blueprint: &WorldBlueprint) -> Result<(), ContractError> {
    validate_vehicle_ids(blueprint)?;
    validate_sensor_ids(blueprint)?;
    validate_sensor_frequencies(blueprint)?;
    validate_sync_config(blueprint)?;
    validate_sinks(blueprint)?;
    Ok(())
}

/// 校验 vehicle_id 唯一性  
fn validate_vehicle_ids(blueprint: &WorldBlueprint) -> Result<(), ContractError> {
    let mut seen = HashSet::new();
    for vehicle in &blueprint.vehicles {
        if !seen.insert(&vehicle.id) {
            return Err(ContractError::config_validation(
                format!("vehicles[id={}]", vehicle.id),
                "duplicate vehicle_id",
            ));
        }
    }
    Ok(())
}

/// 校验 sensor_id 唯一性 (全局)
fn validate_sensor_ids(blueprint: &WorldBlueprint) -> Result<(), ContractError> {
    let mut seen = HashSet::new();
    for vehicle in &blueprint.vehicles {
        for sensor in &vehicle.sensors {
            if !seen.insert(&sensor.id) {
                return Err(ContractError::config_validation(
                    format!("vehicles[{}].sensors[id={}]", vehicle.id, sensor.id),
                    "duplicate sensor_id",
                ));
            }
        }
    }
    Ok(())
}

/// 校验传感器采样率
fn validate_sensor_frequencies(blueprint: &WorldBlueprint) -> Result<(), ContractError> {
    for vehicle in &blueprint.vehicles {
        for sensor in &vehicle.sensors {
            if sensor.frequency_hz <= 0.0 {
                return Err(ContractError::config_validation(
                    format!(
                        "vehicles[{}].sensors[{}].frequency_hz",
                        vehicle.id, sensor.id
                    ),
                    format!("frequency_hz must be > 0, got {}", sensor.frequency_hz),
                ));
            }
        }
    }
    Ok(())
}

/// 校验同步配置
fn validate_sync_config(blueprint: &WorldBlueprint) -> Result<(), ContractError> {
    let sync = &blueprint.sync;

    // 校验窗口范围
    if sync.min_window_sec > sync.max_window_sec {
        return Err(ContractError::config_validation(
            "sync.min_window_sec / sync.max_window_sec",
            format!(
                "min_window_sec ({}) must be <= max_window_sec ({})",
                sync.min_window_sec, sync.max_window_sec
            ),
        ));
    }

    // 校验 primary_sensor_id 存在
    let all_sensor_ids: HashSet<_> = blueprint
        .vehicles
        .iter()
        .flat_map(|v| v.sensors.iter().map(|s| s.id.as_str()))
        .collect();

    if !all_sensor_ids.contains(sync.primary_sensor_id.as_str()) {
        return Err(ContractError::config_validation(
            "sync.primary_sensor_id",
            format!(
                "primary_sensor_id '{}' not found in any vehicle sensors",
                sync.primary_sensor_id
            ),
        ));
    }

    Ok(())
}

/// 校验 sink 配置
fn validate_sinks(blueprint: &WorldBlueprint) -> Result<(), ContractError> {
    for (idx, sink) in blueprint.sinks.iter().enumerate() {
        if sink.name.is_empty() {
            return Err(ContractError::config_validation(
                format!("sinks[{}].name", idx),
                "sink name cannot be empty",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{
        ConfigVersion, DropPolicy, Location, MissingFramePolicy, Rotation, SensorConfig,
        SensorType, SinkConfig, SinkType, SyncConfig, SyncEngineOverrides, Transform,
        VehicleConfig, WorldConfig,
    };

    fn minimal_blueprint() -> WorldBlueprint {
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
                blueprint: "vehicle.tesla.model3".into(),
                spawn_point: Some(Transform {
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
                }),
                sensors: vec![SensorConfig {
                    id: "cam1".into(),
                    sensor_type: SensorType::Camera,
                    transform: Transform {
                        location: Location {
                            x: 0.0,
                            y: 0.0,
                            z: 2.0,
                        },
                        rotation: Rotation {
                            pitch: 0.0,
                            yaw: 0.0,
                            roll: 0.0,
                        },
                    },
                    frequency_hz: 20.0,
                    attributes: Default::default(),
                }],
            }],
            sync: SyncConfig {
                primary_sensor_id: "cam1".into(),
                min_window_sec: 0.02,
                max_window_sec: 0.1,
                missing_frame_policy: MissingFramePolicy::Drop,
                drop_policy: DropPolicy::DropOldest,
                engine: SyncEngineOverrides::default(),
            },
            sinks: vec![SinkConfig {
                name: "log".into(),
                sink_type: SinkType::Log,
                queue_capacity: 100,
                params: Default::default(),
            }],
        }
    }

    #[test]
    fn test_valid_config() {
        let bp = minimal_blueprint();
        assert!(validate(&bp).is_ok());
    }

    #[test]
    fn test_duplicate_vehicle_id() {
        let mut bp = minimal_blueprint();
        bp.vehicles.push(bp.vehicles[0].clone());
        let result = validate(&bp);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("duplicate vehicle_id"), "got: {err}");
    }

    #[test]
    fn test_duplicate_sensor_id() {
        let mut bp = minimal_blueprint();
        let dup_sensor = bp.vehicles[0].sensors[0].clone();
        bp.vehicles[0].sensors.push(dup_sensor);
        let result = validate(&bp);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("duplicate sensor_id"), "got: {err}");
    }

    #[test]
    fn test_invalid_frequency() {
        let mut bp = minimal_blueprint();
        bp.vehicles[0].sensors[0].frequency_hz = -5.0;
        let result = validate(&bp);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("frequency_hz must be > 0"), "got: {err}");
    }

    #[test]
    fn test_invalid_window_range() {
        let mut bp = minimal_blueprint();
        bp.sync.min_window_sec = 0.5;
        bp.sync.max_window_sec = 0.1;
        let result = validate(&bp);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("min_window_sec"), "got: {err}");
    }

    #[test]
    fn test_primary_sensor_not_found() {
        let mut bp = minimal_blueprint();
        bp.sync.primary_sensor_id = "nonexistent".into();
        let result = validate(&bp);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"), "got: {err}");
    }

    #[test]
    fn test_empty_sink_name() {
        let mut bp = minimal_blueprint();
        bp.sinks[0].name = String::new();
        let result = validate(&bp);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cannot be empty"), "got: {err}");
    }
}
