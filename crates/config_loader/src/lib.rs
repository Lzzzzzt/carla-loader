//! # Config Loader
//!
//! Configuration loading and parsing module.
//!
//! Responsibilities:
//! - Parse TOML/JSON configuration files
//! - Validate configuration legality
//! - Generate `WorldBlueprint`
//!
//! # Example
//!
//! ```no_run
//! use config_loader::ConfigLoader;
//! use std::path::Path;
//!
//! let blueprint = ConfigLoader::load_from_path(Path::new("config.toml")).unwrap();
//! println!("Map: {}", blueprint.world.map);
//! ```

mod parser;
mod validator;

pub use contracts::WorldBlueprint;
pub use parser::ConfigFormat;

use contracts::ContractError;
use std::path::Path;

/// Configuration loader
///
/// Provides static methods to load configuration from files or strings.
pub struct ConfigLoader;

impl ConfigLoader {
    /// Load configuration from file path
    ///
    /// Automatically detects format from file extension (.toml / .json).
    ///
    /// # Errors
    /// - File read failure
    /// - Unsupported format
    /// - Parse failure
    /// - Validation failure
    pub fn load_from_path(path: &Path) -> Result<WorldBlueprint, ContractError> {
        let format = Self::detect_format(path)?;
        let content = Self::read_file(path)?;
        Self::load_from_str(&content, format)
    }

    /// Load configuration from string
    ///
    /// # Errors
    /// - Parse failure
    /// - Validation failure
    pub fn load_from_str(
        content: &str,
        format: ConfigFormat,
    ) -> Result<WorldBlueprint, ContractError> {
        Self::parse_and_validate(content, format)
    }

    /// Serialize WorldBlueprint to TOML string
    pub fn to_toml(blueprint: &WorldBlueprint) -> Result<String, ContractError> {
        toml::to_string_pretty(blueprint)
            .map_err(|e| ContractError::config_parse(format!("TOML serialize error: {e}")))
    }

    /// Serialize WorldBlueprint to JSON string
    pub fn to_json(blueprint: &WorldBlueprint) -> Result<String, ContractError> {
        serde_json::to_string_pretty(blueprint)
            .map_err(|e| ContractError::config_parse(format!("JSON serialize error: {e}")))
    }
}

impl ConfigLoader {
    /// Infer configuration format from file extension
    fn detect_format(path: &Path) -> Result<ConfigFormat, ContractError> {
        let ext = path.extension().and_then(|e| e.to_str()).ok_or_else(|| {
            ContractError::config_parse("cannot determine file format from extension")
        })?;

        ConfigFormat::from_extension(ext).ok_or_else(|| {
            ContractError::config_parse(format!("unsupported config format: .{ext}"))
        })
    }

    /// Read configuration file content
    fn read_file(path: &Path) -> Result<String, ContractError> {
        Ok(std::fs::read_to_string(path)?)
    }

    /// Parse and validate configuration content
    fn parse_and_validate(
        content: &str,
        format: ConfigFormat,
    ) -> Result<WorldBlueprint, ContractError> {
        let blueprint = parser::parse(content, format)?;
        validator::validate(&blueprint)?;
        Ok(blueprint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_TOML: &str = r#"
[world]
map = "Town01"

[[vehicles]]
id = "ego"
blueprint = "vehicle.tesla.model3"
[vehicles.spawn_point.location]
x = 0.0
y = 0.0
z = 0.0
[vehicles.spawn_point.rotation]
pitch = 0.0
yaw = 0.0
roll = 0.0

[[vehicles.sensors]]
id = "front_camera"
sensor_type = "camera"
frequency_hz = 20.0
[vehicles.sensors.transform.location]
x = 2.0
y = 0.0
z = 1.5
[vehicles.sensors.transform.rotation]
pitch = 0.0
yaw = 0.0
roll = 0.0

[sync]
primary_sensor_id = "front_camera"

[[sinks]]
name = "log_sink"
sink_type = "log"
"#;

    #[test]
    fn test_load_from_str_toml() {
        let result = ConfigLoader::load_from_str(MINIMAL_TOML, ConfigFormat::Toml);
        assert!(result.is_ok(), "Failed: {:?}", result.err());
        let bp = result.unwrap();
        assert_eq!(bp.world.map, "Town01");
    }

    #[test]
    fn test_round_trip_toml() {
        let bp = ConfigLoader::load_from_str(MINIMAL_TOML, ConfigFormat::Toml).unwrap();
        let serialized = ConfigLoader::to_toml(&bp).unwrap();
        let bp2 = ConfigLoader::load_from_str(&serialized, ConfigFormat::Toml).unwrap();
        assert_eq!(bp.world.map, bp2.world.map);
        assert_eq!(bp.vehicles.len(), bp2.vehicles.len());
        assert_eq!(bp.vehicles[0].id, bp2.vehicles[0].id);
    }

    #[test]
    fn test_round_trip_json() {
        let bp = ConfigLoader::load_from_str(MINIMAL_TOML, ConfigFormat::Toml).unwrap();
        let json = ConfigLoader::to_json(&bp).unwrap();
        let bp2 = ConfigLoader::load_from_str(&json, ConfigFormat::Json).unwrap();
        assert_eq!(bp.world.map, bp2.world.map);
    }

    #[test]
    fn test_validation_runs_after_parse() {
        // Duplicate sensor id should fail validation
        let content = r#"
[world]
map = "Town01"

[[vehicles]]
id = "ego"
blueprint = "vehicle.test"
[vehicles.spawn_point.location]
x = 0.0
y = 0.0
z = 0.0
[vehicles.spawn_point.rotation]
pitch = 0.0
yaw = 0.0
roll = 0.0

[[vehicles.sensors]]
id = "cam"
sensor_type = "camera"
frequency_hz = 10.0
[vehicles.sensors.transform.location]
x = 0.0
y = 0.0
z = 0.0
[vehicles.sensors.transform.rotation]
pitch = 0.0
yaw = 0.0
roll = 0.0

[[vehicles.sensors]]
id = "cam"
sensor_type = "lidar"
frequency_hz = 10.0
[vehicles.sensors.transform.location]
x = 0.0
y = 0.0
z = 1.0
[vehicles.sensors.transform.rotation]
pitch = 0.0
yaw = 0.0
roll = 0.0

[sync]
primary_sensor_id = "cam"

[[sinks]]
name = "log"
sink_type = "log"
"#;
        let result = ConfigLoader::load_from_str(content, ConfigFormat::Toml);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("duplicate"));
    }
}
