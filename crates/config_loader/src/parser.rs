//! 配置解析模块
//!
//! 支持 TOML (主要) 和 JSON (可选) 格式。

use contracts::{ContractError, WorldBlueprint};

/// 配置文件格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigFormat {
    /// TOML 格式 (推荐)
    Toml,
    /// JSON 格式
    Json,
}

impl ConfigFormat {
    /// 从文件扩展名推断格式
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "toml" => Some(Self::Toml),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

/// 解析 TOML 格式配置
pub fn parse_toml(content: &str) -> Result<WorldBlueprint, ContractError> {
    toml::from_str(content).map_err(|e| ContractError::ConfigParse {
        message: format!("TOML parse error: {e}"),
        source: Some(Box::new(e)),
    })
}

/// 解析 JSON 格式配置
pub fn parse_json(content: &str) -> Result<WorldBlueprint, ContractError> {
    serde_json::from_str(content).map_err(|e| ContractError::ConfigParse {
        message: format!("JSON parse error: {e}"),
        source: Some(Box::new(e)),
    })
}

/// 根据格式解析配置
pub fn parse(content: &str, format: ConfigFormat) -> Result<WorldBlueprint, ContractError> {
    match format {
        ConfigFormat::Toml => parse_toml(content),
        ConfigFormat::Json => parse_json(content),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_toml_minimal() {
        let content = r#"
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
        let result = parse_toml(content);
        assert!(result.is_ok(), "Failed: {:?}", result.err());
        let bp = result.unwrap();
        assert_eq!(bp.world.map, "Town01");
        assert_eq!(bp.vehicles.len(), 1);
        assert_eq!(bp.vehicles[0].sensors.len(), 1);
    }

    #[test]
    fn test_parse_json_minimal() {
        let content = r#"{
            "world": { "map": "Town01" },
            "vehicles": [{
                "id": "ego",
                "blueprint": "vehicle.tesla.model3",
                "spawn_point": {
                    "location": { "x": 0.0, "y": 0.0, "z": 0.0 },
                    "rotation": { "pitch": 0.0, "yaw": 0.0, "roll": 0.0 }
                },
                "sensors": [{
                    "id": "cam1",
                    "sensor_type": "camera",
                    "frequency_hz": 10.0,
                    "transform": {
                        "location": { "x": 0.0, "y": 0.0, "z": 2.0 },
                        "rotation": { "pitch": 0.0, "yaw": 0.0, "roll": 0.0 }
                    }
                }]
            }],
            "sync": { "primary_sensor_id": "cam1" },
            "sinks": [{ "name": "log", "sink_type": "log" }]
        }"#;
        let result = parse_json(content);
        assert!(result.is_ok(), "Failed: {:?}", result.err());
    }

    #[test]
    fn test_parse_toml_syntax_error() {
        let content = "invalid toml [[[";
        let result = parse_toml(content);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ContractError::ConfigParse { .. }));
    }

    #[test]
    fn test_format_from_extension() {
        assert_eq!(
            ConfigFormat::from_extension("toml"),
            Some(ConfigFormat::Toml)
        );
        assert_eq!(
            ConfigFormat::from_extension("TOML"),
            Some(ConfigFormat::Toml)
        );
        assert_eq!(
            ConfigFormat::from_extension("json"),
            Some(ConfigFormat::Json)
        );
        assert_eq!(ConfigFormat::from_extension("yaml"), None);
    }
}
