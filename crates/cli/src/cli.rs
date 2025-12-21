//! CLI argument definitions using clap.

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// CARLA Syncer - Multi-sensor synchronization pipeline for CARLA simulator
#[derive(Parser, Debug)]
#[command(
    name = "carla-syncer",
    author,
    version,
    about = "CARLA multi-sensor synchronization pipeline",
    long_about = "A high-performance sensor synchronization pipeline for CARLA simulator.\n\n\
                  Connects to CARLA, spawns actors from configuration, synchronizes \n\
                  multi-sensor data streams, and dispatches to configured sinks."
)]
pub struct Cli {
    /// Increase logging verbosity (-v for debug, -vv for trace)
    #[arg(short, long, action = clap::ArgAction::Count, global = true, env = "CARLA_SYNCER_VERBOSE")]
    pub verbose: u8,

    /// Suppress all output except errors
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Log output format
    #[arg(
        long,
        value_enum,
        default_value = "pretty",
        global = true,
        env = "CARLA_SYNCER_LOG_FORMAT"
    )]
    pub log_format: LogFormat,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run the synchronization pipeline
    Run(RunArgs),

    /// Validate configuration file without running
    Validate(ValidateArgs),

    /// Display configuration information
    Info(InfoArgs),
}

/// Arguments for the `run` command
#[derive(Parser, Debug, Clone)]
pub struct RunArgs {
    /// Path to configuration file (TOML or JSON)
    #[arg(
        short,
        long,
        default_value = "config.toml",
        env = "CARLA_SYNCER_CONFIG"
    )]
    pub config: PathBuf,

    /// Override CARLA server host from configuration
    #[arg(long, env = "CARLA_HOST")]
    pub host: Option<String>,

    /// Override CARLA server port from configuration
    #[arg(long, env = "CARLA_PORT")]
    pub port: Option<u16>,

    /// Maximum number of synced frames to produce (0 = unlimited)
    #[arg(long, default_value = "0", env = "CARLA_SYNCER_MAX_FRAMES")]
    pub max_frames: u64,

    /// Pipeline timeout in seconds (0 = no timeout)
    #[arg(long, default_value = "0", env = "CARLA_SYNCER_TIMEOUT")]
    pub timeout: u64,

    /// Validate configuration and exit without running pipeline
    #[arg(long)]
    pub dry_run: bool,

    /// Channel buffer size for internal queues
    #[arg(long, default_value = "100", env = "CARLA_SYNCER_BUFFER_SIZE")]
    pub buffer_size: usize,

    /// Metrics server port (0 = disabled)
    #[arg(long, default_value = "9000", env = "CARLA_SYNCER_METRICS_PORT")]
    pub metrics_port: u16,
}

/// Arguments for the `validate` command
#[derive(Parser, Debug)]
pub struct ValidateArgs {
    /// Path to configuration file to validate
    #[arg(short, long, default_value = "config.toml")]
    pub config: PathBuf,

    /// Output validation result as JSON
    #[arg(long)]
    pub json: bool,
}

/// Arguments for the `info` command
#[derive(Parser, Debug)]
pub struct InfoArgs {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.toml")]
    pub config: PathBuf,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Show detailed sensor information
    #[arg(long)]
    pub sensors: bool,

    /// Show sink configuration
    #[arg(long)]
    pub sinks: bool,
}

/// Log output format
#[derive(ValueEnum, Clone, Debug, Default)]
pub enum LogFormat {
    /// JSON structured logging
    Json,
    /// Human-readable pretty format
    #[default]
    Pretty,
    /// Compact single-line format
    Compact,
}
