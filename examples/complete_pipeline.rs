//! Complete Pipeline Example
//!
//! Demonstrates reading a single configuration file, wiring mock sensors,
//! running the sync engine, and fanning out via the dispatcher.
//!
//! Run with: cargo run --example complete_pipeline [config_path]
#![allow(clippy::field_reassign_with_default)]

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use config_loader::ConfigLoader;
use contracts::{SensorConfig, SensorPacket, SensorType, SyncedFrame, WorldBlueprint};
use dispatcher::create_dispatcher;
use ingestion::{MockSensorConfig, MockSensorSource};
use sync_engine::{SyncEngine, SyncEngineConfig};
use tokio::sync::mpsc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting Complete Pipeline Demo");

    let config_path = resolve_config_path();
    info!(path = %config_path.display(), "Loading unified config file");
    let blueprint = ConfigLoader::load_from_path(config_path.as_path())?;
    info!(map = %blueprint.world.map, "Blueprint loaded");

    // ==== Stage 1: Configure Sync Engine ====
    let sync_engine_config = blueprint.to_sync_engine_config();
    let mut sync_engine = SyncEngine::new(sync_engine_config.clone());

    // ==== Stage 2: Create Dispatcher with sinks from config ====
    let (sync_tx, sync_rx) = mpsc::channel::<SyncedFrame>(100);
    let dispatcher = create_dispatcher(blueprint.sinks.clone(), sync_rx).await?;
    let dispatcher_handle = dispatcher.spawn();

    // ==== Stage 3: Start Mock Sources described by config ====
    let mock_sources = build_mock_sources(&blueprint, &sync_engine_config)?;
    info!(
        source_count = mock_sources.len(),
        "Starting mock sensor streams"
    );

    let mut receivers = Vec::new();
    for source in &mock_sources {
        receivers.push(source.start(100, None));
    }

    // Fan-in all sensor streams into a single channel (async-channel is natively async)
    let (packet_tx, packet_rx) = mpsc::channel::<SensorPacket>(512);
    for rx in receivers {
        let tx = packet_tx.clone();
        tokio::spawn(async move {
            while let Ok(packet) = rx.recv().await {
                if tx.send(packet).await.is_err() {
                    break;
                }
            }
        });
    }
    drop(packet_tx);

    // ==== Stage 4: Run Pipeline ====
    let target_frames = 20u64;
    let sync_tx_clone = sync_tx.clone();
    info!(
        required_sensors = sync_engine_config.required_sensors.len(),
        target_frames, "Running pipeline"
    );

    let pipeline_handle = tokio::spawn(async move {
        let mut synced_count = 0u64;
        let mut packet_rx = packet_rx;

        while let Some(packet) = packet_rx.recv().await {
            if let Some(frame) = sync_engine.push(packet) {
                synced_count += 1;
                info!(
                    frame_id = frame.frame_id,
                    sensors = frame.frames.len(),
                    t_sync = format!("{:.3}", frame.t_sync),
                    "Synced frame produced"
                );

                if sync_tx_clone.send(frame).await.is_err() {
                    break;
                }

                if synced_count >= target_frames {
                    break;
                }
            }
        }

        info!(
            total_synced = synced_count,
            motion_intensity = format!("{:.3}", sync_engine.motion_intensity()),
            "Pipeline complete"
        );

        synced_count
    });

    // Wait for pipeline with timeout
    let result = tokio::time::timeout(Duration::from_secs(10), pipeline_handle).await;

    // ==== Stage 5: Graceful Shutdown ====
    info!("Shutting down...");

    for source in &mock_sources {
        source.stop();
    }

    drop(sync_tx);
    let _ = tokio::time::timeout(Duration::from_secs(2), dispatcher_handle).await;

    match result {
        Ok(Ok(count)) => info!(frames = count, "Pipeline completed successfully"),
        Ok(Err(e)) => info!("Pipeline task error: {:?}", e),
        Err(_) => info!("Pipeline timed out"),
    }

    info!("Complete Pipeline Demo finished");
    Ok(())
}

fn resolve_config_path() -> PathBuf {
    std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("crates/config_loader/examples/full.toml"))
}

fn build_mock_sources(
    blueprint: &WorldBlueprint,
    sync_config: &SyncEngineConfig,
) -> Result<Vec<MockSensorSource>, PipelineConfigError> {
    let mut unique_ids = HashSet::new();
    let mut sensors: Vec<&SensorConfig> = Vec::new();

    for sensor_id in &sync_config.required_sensors {
        let sensor = find_sensor(blueprint, sensor_id).ok_or_else(|| {
            PipelineConfigError(format!("Sensor '{}' not defined in blueprint", sensor_id))
        })?;

        if unique_ids.insert(sensor_id.clone()) {
            sensors.push(sensor);
        }
    }

    if let Some(imu_id) = &sync_config.imu_sensor_id {
        if unique_ids.insert(imu_id.clone()) {
            if let Some(sensor) = find_sensor(blueprint, imu_id) {
                sensors.push(sensor);
            }
        }
    }

    sensors.into_iter().map(build_source_from_sensor).collect()
}

fn find_sensor<'a>(blueprint: &'a WorldBlueprint, sensor_id: &str) -> Option<&'a SensorConfig> {
    blueprint
        .vehicles
        .iter()
        .flat_map(|vehicle| vehicle.sensors.iter())
        .find(|sensor| sensor.id == sensor_id)
}

fn build_source_from_sensor(
    sensor: &SensorConfig,
) -> Result<MockSensorSource, PipelineConfigError> {
    let source = match sensor.sensor_type {
        SensorType::Camera => {
            let (width, height) = camera_dimensions(sensor);
            MockSensorSource::camera(&sensor.id, sensor.frequency_hz, width, height)
        }
        SensorType::Lidar => {
            let points = attribute_u32(sensor, "points_per_second", 10000);
            MockSensorSource::lidar(&sensor.id, sensor.frequency_hz, points)
        }
        SensorType::Imu => MockSensorSource::imu(&sensor.id, sensor.frequency_hz),
        SensorType::Gnss => MockSensorSource::gnss(&sensor.id, sensor.frequency_hz),
        SensorType::Radar => {
            let mut config = MockSensorConfig::default();
            config.sensor_id = sensor.id.clone();
            config.sensor_type = SensorType::Radar;
            config.frequency_hz = sensor.frequency_hz;
            MockSensorSource::new(config)
        }
    };

    Ok(source)
}

fn camera_dimensions(sensor: &SensorConfig) -> (u32, u32) {
    let width = attribute_u32(sensor, "image_size_x", 640);
    let height = attribute_u32(sensor, "image_size_y", 480);
    (width, height)
}

fn attribute_u32(sensor: &SensorConfig, key: &str, default: u32) -> u32 {
    sensor
        .attributes
        .get(key)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
}

#[derive(Debug)]
struct PipelineConfigError(String);

impl std::fmt::Display for PipelineConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PipelineConfigError {}
