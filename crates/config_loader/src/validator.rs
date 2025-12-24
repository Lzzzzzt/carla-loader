//! Configuration validation module
//!
//! Uses the `validator` crate for structured validation while retaining custom validation rules.
//!
//! Validation rules:
//! - sensor_id must be unique
//! - vehicle_id must be unique
//! - Sensor mount topology must be valid (primary_sensor_id must exist)
//! - frequency_hz > 0 (handled by validator derive)
//! - min_window_sec <= max_window_sec (handled by validator schema)
//! - sink required fields must be present (handled by validator derive)

use std::collections::HashSet;

use contracts::{ContractError, WorldBlueprint};
use validator::Validate;

/// Validate WorldBlueprint configuration
///
/// First runs structured validator checks, then executes custom validation.
pub fn validate(blueprint: &WorldBlueprint) -> Result<(), ContractError> {
    // 1. Run validator derive defined rules
    blueprint
        .validate()
        .map_err(|e| ContractError::config_validation("validation", format!("{}", e)))?;

    // 2. Execute custom validation (ID uniqueness, reference integrity)
    validate_unique_vehicle_ids(blueprint)?;
    validate_unique_sensor_ids(blueprint)?;
    validate_primary_sensor_exists(blueprint)?;

    Ok(())
}

/// Validate vehicle_id uniqueness
fn validate_unique_vehicle_ids(blueprint: &WorldBlueprint) -> Result<(), ContractError> {
    let mut seen = HashSet::with_capacity(blueprint.vehicles.len());
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

/// Validate sensor_id uniqueness (global)
fn validate_unique_sensor_ids(blueprint: &WorldBlueprint) -> Result<(), ContractError> {
    let total_sensors: usize = blueprint.vehicles.iter().map(|v| v.sensors.len()).sum();
    let mut seen = HashSet::with_capacity(total_sensors);

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

/// Validate primary_sensor_id exists
fn validate_primary_sensor_exists(blueprint: &WorldBlueprint) -> Result<(), ContractError> {
    let all_sensor_ids: HashSet<_> = blueprint
        .vehicles
        .iter()
        .flat_map(|v| v.sensors.iter().map(|s| s.id.as_str()))
        .collect();

    if !all_sensor_ids.contains(blueprint.sync.primary_sensor_id.as_str()) {
        return Err(ContractError::config_validation(
            "sync.primary_sensor_id",
            format!(
                "primary_sensor_id '{}' not found in any vehicle sensors",
                blueprint.sync.primary_sensor_id
            ),
        ));
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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("duplicate vehicle_id"));
    }

    #[test]
    fn test_duplicate_sensor_id() {
        let mut bp = minimal_blueprint();
        let dup_sensor = bp.vehicles[0].sensors[0].clone();
        bp.vehicles[0].sensors.push(dup_sensor);
        let result = validate(&bp);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("duplicate sensor_id"));
    }

    #[test]
    fn test_invalid_frequency() {
        let mut bp = minimal_blueprint();
        bp.vehicles[0].sensors[0].frequency_hz = -5.0;
        let result = validate(&bp);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_window_range() {
        let mut bp = minimal_blueprint();
        bp.sync.min_window_sec = 0.5;
        bp.sync.max_window_sec = 0.1;
        let result = validate(&bp);
        assert!(result.is_err());
    }

    #[test]
    fn test_primary_sensor_not_found() {
        let mut bp = minimal_blueprint();
        bp.sync.primary_sensor_id = "nonexistent".into();
        let result = validate(&bp);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_empty_sink_name() {
        let mut bp = minimal_blueprint();
        bp.sinks[0].name = String::new();
        let result = validate(&bp);
        assert!(result.is_err());
    }
}
