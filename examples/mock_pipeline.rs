//! Mock Pipeline Example
//!
//! Demonstrates using the unified SensorSource interface with MockCarlaClient.
//! This example runs without requiring a real CARLA server.
//!
//! Run with: cargo run --example mock_pipeline --no-default-features

use std::time::Duration;

use actor_factory::{ActorFactory, CarlaClient, MockCarlaClient};
use config_loader::ConfigLoader;
use contracts::{SensorConfig, SyncedFrame};
use ingestion::IngestionPipeline;
use sync_engine::SyncEngine;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tracing::info!("Starting Mock Pipeline Demo");

    // ==== Stage 1: Use default config or load from file ====
    let blueprint = if let Some(path) = std::env::args().nth(1) {
        tracing::info!(path = %path, "Loading blueprint config");
        ConfigLoader::load_from_path(std::path::Path::new(&path))?
    } else {
        // Create a minimal test blueprint
        create_test_blueprint()
    };

    // ==== Stage 2: Connect and Spawn Actors (Mock) ====
    tracing::info!("Creating Mock CARLA client...");

    let mut client = MockCarlaClient::new();
    client
        .connect(&blueprint.world.carla_host, blueprint.world.carla_port)
        .await?;

    // Create factory with mock client
    let factory = ActorFactory::new(client.clone());

    tracing::info!("Spawning actors (mock)...");
    let runtime_graph = factory.spawn_from_blueprint(&blueprint).await?;
    tracing::info!(
        vehicles = runtime_graph.vehicles.len(),
        sensors = runtime_graph.sensors.len(),
        "Actors spawned successfully"
    );

    // ==== Stage 3: Setup Ingestion Pipeline ====
    tracing::info!("Setting up ingestion pipeline...");
    let mut ingestion = IngestionPipeline::new(100);

    // Use the unified get_sensor_source interface
    for (sensor_config_id, actor_id) in &runtime_graph.sensors {
        if let Some(sensor_config) = find_sensor(&blueprint, sensor_config_id) {
            // This is the key unified interface!
            if let Some(sensor_source) = client.get_sensor_source(
                *actor_id,
                sensor_config_id.clone(),
                sensor_config.sensor_type,
            ) {
                ingestion.register_sensor_source(sensor_config_id.clone(), sensor_source, None);
                tracing::info!(sensor_id = %sensor_config_id, "Registered sensor source");
            } else {
                tracing::warn!(sensor_id = %sensor_config_id, "Failed to get sensor source");
            }
        }
    }

    tracing::info!(
        sensor_count = ingestion.sensor_count(),
        "Ingestion pipeline configured"
    );

    // ==== Stage 4: Setup Sync Engine ====
    tracing::info!("Configuring sync engine...");
    let sync_config = blueprint.to_sync_engine_config();
    let mut sync_engine = SyncEngine::new(sync_config);

    // ==== Stage 5: Start Pipeline ====
    tracing::info!("Starting pipeline...");
    ingestion.start_all();
    let ingestion_rx = ingestion.take_receiver().unwrap();

    let target_frames = 50u64;

    tracing::info!("Running pipeline, target: {} synced frames", target_frames);

    let pipeline_handle = tokio::spawn(async move {
        let mut synced_count = 0u64;

        while let Ok(packet) = ingestion_rx.recv().await {
            tracing::debug!(
                sensor_id = %packet.sensor_id,
                frame_id = ?packet.frame_id,
                timestamp = packet.timestamp,
                "Received packet"
            );

            if let Some(frame) = sync_engine.push(packet) {
                synced_count += 1;
                tracing::info!(
                    frame_id = frame.frame_id,
                    t_sync = format!("{:.3}", frame.t_sync),
                    sensors = frame.frames.len(),
                    "Synced frame produced"
                );

                if synced_count >= target_frames {
                    break;
                }
            }
        }
        synced_count
    });

    // Wait for pipeline or timeout
    let result = tokio::time::timeout(Duration::from_secs(30), pipeline_handle).await;

    // ==== Stage 6: Cleanup ====
    tracing::info!("Shutting down and cleaning up...");
    ingestion.stop_all();
    factory.teardown(&runtime_graph).await?;

    match result {
        Ok(Ok(count)) => tracing::info!(frames = count, "Pipeline completed successfully"),
        Ok(Err(e)) => tracing::warn!("Pipeline error: {:?}", e),
        Err(_) => tracing::warn!("Pipeline timed out"),
    }

    Ok(())
}

fn find_sensor<'a>(
    blueprint: &'a contracts::WorldBlueprint,
    sensor_id: &str,
) -> Option<&'a SensorConfig> {
    blueprint
        .vehicles
        .iter()
        .flat_map(|vehicle| vehicle.sensors.iter())
        .find(|sensor| sensor.id == sensor_id)
}

fn create_test_blueprint() -> contracts::WorldBlueprint {
    use contracts::*;
    use std::collections::HashMap;

    WorldBlueprint {
        version: ConfigVersion::V1,
        world: WorldConfig {
            map: "Town01".to_string(),
            weather: None,
            carla_host: "localhost".to_string(),
            carla_port: 2000,
        },
        vehicles: vec![VehicleConfig {
            id: "ego_vehicle".to_string(),
            blueprint: "vehicle.tesla.model3".to_string(),
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
            sensors: vec![
                SensorConfig {
                    id: "front_camera".to_string(),
                    sensor_type: SensorType::Camera,
                    transform: Transform {
                        location: Location {
                            x: 2.0,
                            y: 0.0,
                            z: 1.5,
                        },
                        rotation: Rotation {
                            pitch: 0.0,
                            yaw: 0.0,
                            roll: 0.0,
                        },
                    },
                    frequency_hz: 30.0,
                    attributes: HashMap::new(),
                },
                SensorConfig {
                    id: "imu".to_string(),
                    sensor_type: SensorType::Imu,
                    transform: Transform {
                        location: Location {
                            x: 0.0,
                            y: 0.0,
                            z: 0.5,
                        },
                        rotation: Rotation {
                            pitch: 0.0,
                            yaw: 0.0,
                            roll: 0.0,
                        },
                    },
                    frequency_hz: 100.0,
                    attributes: HashMap::new(),
                },
            ],
        }],
        sync: SyncConfig {
            primary_sensor_id: "front_camera".to_string(),
            min_window_sec: 0.02,
            max_window_sec: 0.1,
            missing_frame_policy: MissingFramePolicy::Drop,
            drop_policy: DropPolicy::DropOldest,
            engine: SyncEngineOverrides::default(),
        },
        sinks: vec![],
    }
}
