//! Pipeline orchestrator - coordinates all components.

use std::time::{Duration, Instant};

use actor_factory::CarlaClient;
use anyhow::{Context, Result};
use contracts::{SensorConfig, SyncedFrame, WorldBlueprint};
use observability::record_sync_metrics;
use tokio::sync::mpsc;
use tracing::{info, warn};

use super::PipelineStats;

/// Pipeline configuration
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// The world blueprint configuration
    pub blueprint: WorldBlueprint,

    /// Maximum number of frames to sync (None = unlimited)
    pub max_frames: Option<u64>,

    /// Pipeline timeout (None = no timeout)
    pub timeout: Option<Duration>,

    /// Channel buffer size
    pub buffer_size: usize,

    /// Metrics server port (None = disabled)
    pub metrics_port: Option<u16>,
}

/// Main pipeline orchestrator
pub struct Pipeline {
    config: PipelineConfig,
}

impl Pipeline {
    /// Create a new pipeline with the given configuration
    pub fn new(config: PipelineConfig) -> Self {
        Self { config }
    }

    /// Run the pipeline to completion
    pub async fn run(self) -> Result<PipelineStats> {
        let start_time = Instant::now();
        let blueprint = &self.config.blueprint;

        // ==== Stage 1: Initialize Metrics (optional) ====
        if let Some(port) = self.config.metrics_port {
            observability::init_metrics_only(port)?;
            info!("Metrics endpoint available on port 9000");
        }

        // ==== Stage 2: Connect to CARLA ====
        info!(
            host = %blueprint.world.carla_host,
            port = blueprint.world.carla_port,
            "Connecting to CARLA server..."
        );

        let mut client = actor_factory::RealCarlaClient::new();
        client
            .connect(&blueprint.world.carla_host, blueprint.world.carla_port)
            .await
            .with_context(|| {
                format!(
                    "Failed to connect to CARLA at {}:{}",
                    blueprint.world.carla_host, blueprint.world.carla_port
                )
            })?;

        info!("Connected to CARLA server");

        // ==== Stage 3: Spawn Actors ====
        info!("Spawning actors from blueprint...");
        let factory = actor_factory::ActorFactory::new(client.clone());
        let runtime_graph = factory
            .spawn_from_blueprint(blueprint)
            .await
            .context("Failed to spawn actors")?;

        info!(
            vehicles = runtime_graph.vehicles.len(),
            sensors = runtime_graph.sensors.len(),
            "Actors spawned successfully"
        );

        // ==== Stage 4: Setup Ingestion Pipeline ====
        info!("Setting up ingestion pipeline...");
        let mut ingestion = ingestion::IngestionPipeline::new(self.config.buffer_size);
        let mut active_sensors = 0usize;

        for (sensor_config_id, actor_id) in &runtime_graph.sensors {
            if let Some(sensor_config) = find_sensor(blueprint, sensor_config_id) {
                if let Some(sensor) = client.get_sensor(*actor_id) {
                    ingestion.register_sensor(
                        sensor_config_id.clone(),
                        sensor_config.sensor_type,
                        sensor,
                        None,
                    );
                    active_sensors += 1;
                } else {
                    warn!(sensor_id = %sensor_config_id, "Failed to retrieve CARLA sensor object");
                }
            }
        }

        info!(active_sensors, "Ingestion pipeline configured");

        // ==== Stage 5: Setup Sync Engine ====
        info!("Configuring sync engine...");
        let sync_config = blueprint.to_sync_engine_config();
        let mut sync_engine = sync_engine::SyncEngine::new(sync_config.clone());

        info!(
            reference_sensor = %sync_config.reference_sensor_id,
            required_sensors = ?sync_config.required_sensors,
            "Sync engine configured"
        );

        // ==== Stage 6: Setup Dispatcher ====
        info!("Setting up dispatcher...");
        let (sync_tx, sync_rx) = mpsc::channel::<SyncedFrame>(self.config.buffer_size);

        if blueprint.sinks.is_empty() {
            warn!("No sinks configured - synced frames will be dropped");
        }

        let dispatcher = dispatcher::create_dispatcher(blueprint.sinks.clone(), sync_rx)
            .await
            .context("Failed to create dispatcher")?;

        let active_sinks = blueprint.sinks.len();
        let dispatcher_handle = dispatcher.spawn();

        info!(active_sinks, "Dispatcher started");

        // ==== Stage 7: Start Pipeline ====
        info!("Starting sensor data ingestion...");
        ingestion.start_all();
        let mut ingestion_rx = ingestion
            .take_receiver()
            .context("Failed to get ingestion receiver")?;

        let max_frames = self.config.max_frames;
        let sync_tx_clone = sync_tx;

        info!(
            max_frames = ?max_frames,
            "Pipeline running"
        );

        // Pipeline processing task
        let pipeline_task = async move {
            let mut stats = PipelineStats::default();
            stats.active_sensors = active_sensors;
            stats.active_sinks = active_sinks;

            while let Some(packet) = ingestion_rx.recv().await {
                stats.packets_received += 1;

                if let Some(frame) = sync_engine.push(packet) {
                    stats.frames_synced += 1;

                    // Record metrics from SyncMeta
                    record_sync_metrics(&frame.sync_meta, frame.frame_id);
                    stats.sync_metrics.update(&frame.sync_meta);

                    // Update dropped count from sync meta
                    stats.frames_dropped += frame.sync_meta.dropped_count as u64;

                    info!(
                        frame_id = frame.frame_id,
                        t_sync = format!("{:.3}", frame.t_sync),
                        sensors = frame.frames.len(),
                        window_ms = format!("{:.2}", frame.sync_meta.window_size * 1000.0),
                        dropped = frame.sync_meta.dropped_count,
                        missing = frame.sync_meta.missing_sensors.len(),
                        "Synced frame produced"
                    );

                    if sync_tx_clone.send(frame).await.is_err() {
                        warn!("Dispatcher channel closed");
                        break;
                    }

                    // Check max frames limit
                    if let Some(max) = max_frames {
                        if stats.frames_synced >= max {
                            info!(frames = stats.frames_synced, "Reached max frames limit");
                            break;
                        }
                    }
                }
            }

            stats
        };

        // Run with optional timeout
        let stats = if let Some(timeout) = self.config.timeout {
            match tokio::time::timeout(timeout, pipeline_task).await {
                Ok(stats) => stats,
                Err(_) => {
                    warn!(timeout_secs = timeout.as_secs(), "Pipeline timed out");
                    PipelineStats::default()
                }
            }
        } else {
            pipeline_task.await
        };

        // ==== Stage 8: Cleanup ====
        info!("Shutting down pipeline...");

        // Stop ingestion
        ingestion.stop_all();

        // Teardown actors
        if let Err(e) = factory.teardown(&runtime_graph).await {
            warn!(error = %e, "Error during actor teardown");
        }

        // Wait for dispatcher to flush
        let _ = tokio::time::timeout(Duration::from_secs(5), dispatcher_handle).await;

        let mut final_stats = stats;
        final_stats.duration = start_time.elapsed();

        info!(
            duration_secs = final_stats.duration.as_secs_f64(),
            fps = format!("{:.2}", final_stats.fps()),
            "Pipeline shutdown complete"
        );

        Ok(final_stats)
    }
}

/// Find a sensor configuration by ID in the blueprint
fn find_sensor<'a>(blueprint: &'a WorldBlueprint, sensor_id: &str) -> Option<&'a SensorConfig> {
    blueprint
        .vehicles
        .iter()
        .flat_map(|vehicle| vehicle.sensors.iter())
        .find(|sensor| sensor.id == sensor_id)
}
