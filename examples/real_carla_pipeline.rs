//! Real CARLA Pipeline Example
//!
//! Connects to CARLA server (192.168.31.193:2000), spawns actors, and runs the sync pipeline.
//!
//! Run with: cargo run --example real_carla_pipeline

use std::path::PathBuf;
use std::time::Duration;

use actor_factory::{ActorFactory, CarlaClient, RealCarlaClient};
use config_loader::ConfigLoader;
use contracts::{SensorConfig, SyncedFrame, WorldBlueprint};
use dispatcher::create_dispatcher;
use ingestion::IngestionPipeline;
use sync_engine::SyncEngine;
use tokio::sync::mpsc;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    // Initialize observability (Tracing + Prometheus)
    observability::init()?;

    info!("Starting Real CARLA Pipeline Demo");

    // ==== Stage 1: Configure Blueprint ====
    let config_path = resolve_config_path();
    info!(path = %config_path.display(), "Loading blueprint config");
    let blueprint = ConfigLoader::load_from_path(config_path.as_path())?;

    // ==== Stage 2: Connect and Spawn Actors ====
    info!(
        host = blueprint.world.carla_host,
        port = blueprint.world.carla_port,
        "Connecting to CARLA..."
    );

    let mut client = RealCarlaClient::new();
    client
        .connect(&blueprint.world.carla_host, blueprint.world.carla_port)
        .await?;

    // Create factory with a clone of the client (RealCarlaClient uses Arc internally)
    let factory = ActorFactory::new(client.clone());

    info!("Spawning actors...");
    let runtime_graph = factory.spawn_from_blueprint(&blueprint).await?;
    info!("Actors spawned successfully");

    // ==== Stage 3: Setup Ingestion Pipeline ====
    info!("Setting up ingestion pipeline...");
    let mut ingestion = IngestionPipeline::new(100);

    for (sensor_config_id, actor_id) in &runtime_graph.sensors {
        // Use unified get_sensor_source interface
        if let Some(sensor_config) = find_sensor(&blueprint, sensor_config_id) {
            // New unified interface - works the same for Mock and Real!
            if let Some(sensor_source) = client.get_sensor_source(
                *actor_id,
                sensor_config_id.clone(),
                sensor_config.sensor_type,
            ) {
                ingestion.register_sensor_source(sensor_config_id.clone(), sensor_source, None);
            } else {
                warn!(sensor_id = %sensor_config_id, "Failed to retrieve sensor source");
            }
        }
    }

    // ==== Stage 4: Setup Sync Engine ====
    info!("Configuring sync engine...");
    let sync_config = blueprint.to_sync_engine_config();
    let mut sync_engine = SyncEngine::new(sync_config.clone());

    // ==== Stage 5: Setup Dispatcher ====
    info!("Setting up dispatcher...");
    let (sync_tx, sync_rx) = mpsc::channel::<SyncedFrame>(100);

    if blueprint.sinks.is_empty() {
        warn!("No sinks configured; dispatcher will drop frames");
    }

    let dispatcher = create_dispatcher(blueprint.sinks.clone(), sync_rx).await?;
    let dispatcher_handle = dispatcher.spawn();

    // ==== Stage 6: Start Pipeline ====
    info!("Starting pipeline...");
    ingestion.start_all();
    let ingestion_rx = ingestion.take_receiver().unwrap();

    let target_frames = 10000u64;
    let sync_tx_clone = sync_tx;

    info!("Running pipeline, target: {} synced frames", target_frames);

    // async-channel is natively async, no bridge needed
    let pipeline_handle = tokio::spawn(async move {
        let mut synced_count = 0u64;

        while let Ok(packet) = ingestion_rx.recv().await {
            if let Some(frame) = sync_engine.push(packet) {
                synced_count += 1;
                info!(
                    frame_id = frame.frame_id,
                    t_sync = format!("{:.3}", frame.t_sync),
                    sensors = frame.frames.len(),
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
        synced_count
    });

    // Wait for pipeline or timeout
    let result = tokio::time::timeout(Duration::from_secs(1000), pipeline_handle).await;

    // ==== Stage 7: Cleanup ====
    info!("Shutting down and cleaning up...");

    ingestion.stop_all();
    factory.teardown(&runtime_graph).await?;

    // Wait for dispatcher
    let _ = tokio::time::timeout(Duration::from_secs(20), dispatcher_handle).await;

    match result {
        Ok(Ok(count)) => info!(frames = count, "Pipeline completed successfully"),
        Ok(Err(e)) => warn!("Pipeline error: {:?}", e),
        Err(_) => warn!("Pipeline timed out"),
    }

    Ok(())
}

fn resolve_config_path() -> PathBuf {
    std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("crates/config_loader/examples/full.toml"))
}

fn find_sensor<'a>(blueprint: &'a WorldBlueprint, sensor_id: &str) -> Option<&'a SensorConfig> {
    blueprint
        .vehicles
        .iter()
        .flat_map(|vehicle| vehicle.sensors.iter())
        .find(|sensor| sensor.id == sensor_id)
}
