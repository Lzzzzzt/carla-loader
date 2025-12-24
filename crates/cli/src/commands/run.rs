//! `run` command implementation.

use anyhow::{Context, Result};
use std::time::Duration;
use tracing::{info, warn};

use crate::cli::RunArgs;
use crate::pipeline::{Pipeline, PipelineConfig};

/// Execute the `run` command
pub async fn run_pipeline(args: &RunArgs) -> Result<()> {
    info!(config = %args.config.display(), "Loading configuration");

    // Validate config path
    if !args.config.exists() {
        anyhow::bail!("Configuration file not found: {}", args.config.display());
    }

    // Load and parse configuration
    let mut blueprint = config_loader::ConfigLoader::load_from_path(&args.config)
        .with_context(|| format!("Failed to load config from {}", args.config.display()))?;

    // Apply CLI overrides
    if let Some(ref host) = args.host {
        info!(host = %host, "Overriding CARLA host from CLI");
        blueprint.world.carla_host = host.clone();
    }
    if let Some(port) = args.port {
        info!(port = %port, "Overriding CARLA port from CLI");
        blueprint.world.carla_port = port;
    }

    info!(
        map = %blueprint.world.map,
        host = %blueprint.world.carla_host,
        port = blueprint.world.carla_port,
        vehicles = blueprint.vehicles.len(),
        sinks = blueprint.sinks.len(),
        "Configuration loaded"
    );

    // Dry run - just validate and exit
    if args.dry_run {
        info!("Dry run mode - configuration is valid, exiting");
        print_config_summary(&blueprint);
        return Ok(());
    }

    // Build pipeline configuration
    let pipeline_config = PipelineConfig {
        blueprint,
        max_frames: if args.max_frames == 0 {
            None
        } else {
            Some(args.max_frames)
        },
        timeout: if args.timeout == 0 {
            None
        } else {
            Some(Duration::from_secs(args.timeout))
        },
        buffer_size: args.buffer_size,
        metrics_port: if args.metrics_port == 0 {
            None
        } else {
            Some(args.metrics_port)
        },
        replay_path: args.replay.clone(),
        replay_speed: args.replay_speed,
        replay_loop: args.replay_loop,
    };

    // Create and run pipeline
    let pipeline = Pipeline::new(pipeline_config);

    // Setup graceful shutdown handler
    let shutdown_signal = setup_shutdown_signal();

    info!("Starting pipeline...");

    // Run pipeline with shutdown signal
    tokio::select! {
        result = pipeline.run() => {
            match result {
                Ok(stats) => {
                    info!(
                        frames_synced = stats.frames_synced,
                        frames_dropped = stats.frames_dropped,
                        duration_secs = stats.duration.as_secs_f64(),
                        fps = format!("{:.2}", stats.fps()),
                        "Pipeline completed successfully"
                    );

                    // Print detailed statistics
                    stats.print_summary();
                }
                Err(e) => {
                    return Err(e).context("Pipeline execution failed");
                }
            }
        }
        _ = shutdown_signal => {
            warn!("Received shutdown signal, stopping pipeline...");
        }
    }

    info!("CARLA Syncer finished");
    Ok(())
}

/// Setup Ctrl+C and SIGTERM signal handlers
async fn setup_shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

/// Print configuration summary for dry-run mode
fn print_config_summary(blueprint: &contracts::WorldBlueprint) {
    println!("\n=== Configuration Summary ===\n");
    println!("World:");
    println!("  Map: {}", blueprint.world.map);
    println!(
        "  CARLA: {}:{}",
        blueprint.world.carla_host, blueprint.world.carla_port
    );
    println!("\nVehicles ({}):", blueprint.vehicles.len());
    for vehicle in &blueprint.vehicles {
        let sensor_count: usize = vehicle.sensors.len();
        println!(
            "  - {} ({}) - {} sensors",
            vehicle.id, vehicle.blueprint, sensor_count
        );
    }

    if !blueprint.sinks.is_empty() {
        println!("\nSinks ({}):", blueprint.sinks.len());
        for sink in &blueprint.sinks {
            println!("  - {} ({:?})", sink.name, sink.sink_type);
        }
    }

    if let Some(sync) = Some(&blueprint.sync) {
        println!("\nSync Settings:");
        println!("  Reference sensor: {}", sync.primary_sensor_id);
        if !sync.engine.required_sensor_ids.is_empty() {
            println!("  Required sensors: {:?}", sync.engine.required_sensor_ids);
        }
        if let Some(ref imu) = sync.engine.imu_sensor_id {
            println!("  IMU sensor: {}", imu);
        }
    }

    println!();
}
