//! `info` command implementation.

use anyhow::{Context, Result};
use serde::Serialize;
use tracing::info;

use crate::cli::InfoArgs;

/// Configuration info for JSON output
#[derive(Serialize)]
struct ConfigInfo {
    version: String,
    world: WorldInfo,
    vehicles: Vec<VehicleInfo>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    sinks: Vec<SinkInfo>,
    sync_settings: SyncInfo,
}

#[derive(Serialize)]
struct WorldInfo {
    map: String,
    carla_host: String,
    carla_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    weather: Option<String>,
}

#[derive(Serialize)]
struct VehicleInfo {
    id: String,
    blueprint: String,
    sensors: Vec<SensorInfo>,
}

#[derive(Serialize)]
struct SensorInfo {
    id: String,
    sensor_type: String,
    frequency_hz: f64,
    #[serde(skip_serializing_if = "std::collections::HashMap::is_empty")]
    attributes: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
struct SinkInfo {
    name: String,
    sink_type: String,
}

#[derive(Serialize)]
struct SyncInfo {
    primary_sensor_id: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    required_sensor_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    imu_sensor_id: Option<String>,
}

/// Execute the `info` command
pub fn run_info(args: &InfoArgs) -> Result<()> {
    info!(config = %args.config.display(), "Loading configuration info");

    if !args.config.exists() {
        anyhow::bail!("Configuration file not found: {}", args.config.display());
    }

    let blueprint = config_loader::ConfigLoader::load_from_path(&args.config)
        .with_context(|| format!("Failed to load config from {}", args.config.display()))?;

    if args.json {
        let info = build_config_info(&blueprint, args);
        let json =
            serde_json::to_string_pretty(&info).context("Failed to serialize config info")?;
        println!("{}", json);
    } else {
        print_config_info(&blueprint, args);
    }

    Ok(())
}

fn build_config_info(blueprint: &contracts::WorldBlueprint, args: &InfoArgs) -> ConfigInfo {
    let weather_desc = blueprint
        .world
        .weather
        .as_ref()
        .map(|w| format!("{:?}", w));

    let mut vehicles: Vec<VehicleInfo> = Vec::new();
    for v in &blueprint.vehicles {
        let sensors = if args.sensors {
            v.sensors
                .iter()
                .map(|s| SensorInfo {
                    id: s.id.clone(),
                    sensor_type: format!("{:?}", s.sensor_type),
                    frequency_hz: s.frequency_hz,
                    attributes: s.attributes.clone(),
                })
                .collect()
        } else {
            Vec::new()
        };

        vehicles.push(VehicleInfo {
            id: v.id.clone(),
            blueprint: v.blueprint.clone(),
            sensors,
        });
    }

    let sinks = if args.sinks {
        blueprint
            .sinks
            .iter()
            .map(|s| SinkInfo {
                name: s.name.clone(),
                sink_type: format!("{:?}", s.sink_type),
            })
            .collect()
    } else {
        Vec::new()
    };

    let sync_settings = SyncInfo {
        primary_sensor_id: blueprint.sync.primary_sensor_id.clone(),
        required_sensor_ids: blueprint.sync.engine.required_sensor_ids.clone(),
        imu_sensor_id: blueprint.sync.engine.imu_sensor_id.clone(),
    };

    ConfigInfo {
        version: format!("{:?}", blueprint.version),
        world: WorldInfo {
            map: blueprint.world.map.clone(),
            carla_host: blueprint.world.carla_host.clone(),
            carla_port: blueprint.world.carla_port,
            weather: weather_desc,
        },
        vehicles,
        sinks,
        sync_settings,
    }
}

fn print_config_info(blueprint: &contracts::WorldBlueprint, args: &InfoArgs) {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║               CARLA Syncer Configuration                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // World info
    println!("📍 World");
    println!("   ├─ Version: {:?}", blueprint.version);
    println!("   ├─ Map: {}", blueprint.world.map);
    println!(
        "   ├─ CARLA Server: {}:{}",
        blueprint.world.carla_host, blueprint.world.carla_port
    );
    match &blueprint.world.weather {
        Some(weather) => {
            println!("   └─ Weather: {:?}", weather);
        }
        None => {
            println!("   └─ Weather: Default");
        }
    }

    // Vehicles
    println!("\n🚗 Vehicles ({})", blueprint.vehicles.len());
    for (i, vehicle) in blueprint.vehicles.iter().enumerate() {
        let is_last = i == blueprint.vehicles.len() - 1;
        let prefix = if is_last { "└─" } else { "├─" };
        let child_prefix = if is_last { "   " } else { "│  " };

        println!("   {} {} ({})", prefix, vehicle.id, vehicle.blueprint);

        if args.sensors && !vehicle.sensors.is_empty() {
            println!("   {}  📷 Sensors ({}):", child_prefix, vehicle.sensors.len());
            for (j, sensor) in vehicle.sensors.iter().enumerate() {
                let sensor_is_last = j == vehicle.sensors.len() - 1;
                let sensor_prefix = if sensor_is_last { "└─" } else { "├─" };
                println!(
                    "   {}     {} {} ({:?}, {} Hz)",
                    child_prefix, sensor_prefix, sensor.id, sensor.sensor_type, sensor.frequency_hz
                );
            }
        } else {
            println!(
                "   {}  └─ {} sensors",
                child_prefix,
                vehicle.sensors.len()
            );
        }
    }

    // Sync Settings
    let sync = &blueprint.sync;
    println!("\n⚙️  Sync Settings");
    println!("   ├─ Primary Sensor: {}", sync.primary_sensor_id);
    if !sync.engine.required_sensor_ids.is_empty() {
        println!("   ├─ Required Sensors: {:?}", sync.engine.required_sensor_ids);
    }
    if let Some(ref imu) = sync.engine.imu_sensor_id {
        println!("   └─ IMU Sensor: {}", imu);
    } else {
        println!("   └─ IMU Sensor: (auto-detect)");
    }

    // Sinks
    if !blueprint.sinks.is_empty() {
        println!("\n📤 Sinks ({})", blueprint.sinks.len());
        for (i, sink) in blueprint.sinks.iter().enumerate() {
            let is_last = i == blueprint.sinks.len() - 1;
            let prefix = if is_last { "└─" } else { "├─" };
            println!(
                "   {} {} ({:?})",
                prefix, sink.name, sink.sink_type
            );
        }
    }

    println!();
}
