//! `validate` command implementation.

use anyhow::{Context, Result};
use serde::Serialize;
use tracing::info;

use crate::cli::ValidateArgs;

/// Validation result for JSON output
#[derive(Serialize)]
struct ValidationResult {
    valid: bool,
    config_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    warnings: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<ConfigSummary>,
}

#[derive(Serialize)]
struct ConfigSummary {
    version: String,
    map: String,
    vehicle_count: usize,
    sensor_count: usize,
    sink_count: usize,
}

/// Execute the `validate` command
pub fn run_validate(args: &ValidateArgs) -> Result<()> {
    info!(config = %args.config.display(), "Validating configuration");

    let result = validate_config(args);

    if args.json {
        let json = serde_json::to_string_pretty(&result)
            .context("Failed to serialize validation result")?;
        println!("{}", json);
    } else {
        print_validation_result(&result);
    }

    if result.valid {
        Ok(())
    } else {
        anyhow::bail!("Configuration validation failed")
    }
}

fn validate_config(args: &ValidateArgs) -> ValidationResult {
    let config_path = args.config.display().to_string();

    // Check file exists
    if !args.config.exists() {
        return ValidationResult {
            valid: false,
            config_path,
            error: Some(format!("File not found: {}", args.config.display())),
            warnings: None,
            summary: None,
        };
    }

    // Try to load and validate
    match config_loader::ConfigLoader::load_from_path(&args.config) {
        Ok(blueprint) => {
            let warnings = collect_warnings(&blueprint);
            let sensor_count: usize = blueprint
                .vehicles
                .iter()
                .map(|v| v.sensors.len())
                .sum();

            ValidationResult {
                valid: true,
                config_path,
                error: None,
                warnings: if warnings.is_empty() {
                    None
                } else {
                    Some(warnings)
                },
                summary: Some(ConfigSummary {
                    version: format!("{:?}", blueprint.version),
                    map: blueprint.world.map.clone(),
                    vehicle_count: blueprint.vehicles.len(),
                    sensor_count,
                    sink_count: blueprint.sinks.len(),
                }),
            }
        }
        Err(e) => ValidationResult {
            valid: false,
            config_path,
            error: Some(e.to_string()),
            warnings: None,
            summary: None,
        },
    }
}

/// Collect configuration warnings (non-fatal issues)
fn collect_warnings(blueprint: &contracts::WorldBlueprint) -> Vec<String> {
    let mut warnings = Vec::new();

    // Check for empty sinks
    if blueprint.sinks.is_empty() {
        warnings.push("No sinks configured - synced frames will be dropped".to_string());
    }

    // Check for vehicles without sensors
    for vehicle in &blueprint.vehicles {
        if vehicle.sensors.is_empty() {
            warnings.push(format!(
                "Vehicle '{}' has no sensors configured",
                vehicle.id
            ));
        }
    }

    // Check sync settings
    if blueprint.sync.engine.required_sensor_ids.is_empty() {
        warnings.push("sync.engine.required_sensor_ids is empty - using default sensors".to_string());
    }

    warnings
}

fn print_validation_result(result: &ValidationResult) {
    if result.valid {
        println!("✓ Configuration is valid: {}", result.config_path);

        if let Some(ref summary) = result.summary {
            println!("\n  Version: {}", summary.version);
            println!("  Map: {}", summary.map);
            println!("  Vehicles: {}", summary.vehicle_count);
            println!("  Sensors: {}", summary.sensor_count);
            println!("  Sinks: {}", summary.sink_count);
        }

        if let Some(ref warnings) = result.warnings {
            println!("\n⚠ Warnings:");
            for warning in warnings {
                println!("  - {}", warning);
            }
        }
    } else {
        println!("✗ Configuration is invalid: {}", result.config_path);
        if let Some(ref error) = result.error {
            println!("\n  Error: {}", error);
        }
    }
}
