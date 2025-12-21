//! # CARLA Syncer CLI
//!
//! 命令行接口入口点。
//!
//! 提供：
//! - 配置加载与验证
//! - 管道编排与生命周期管理
//! - 优雅关闭处理

mod cli;
mod commands;
mod error;
mod pipeline;

use anyhow::Result;
use clap::Parser;
use tracing::info;
use tracing_subscriber::Layer;

use cli::{Cli, Commands};
use commands::{run_info, run_pipeline, run_validate};

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if present
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    // Initialize logging based on CLI options
    init_logging(&cli)?;

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "CARLA Syncer CLI starting"
    );

    // Execute command
    let result = match &cli.command {
        Commands::Run(args) => run_pipeline(args).await,
        Commands::Validate(args) => run_validate(args),
        Commands::Info(args) => run_info(args),
    };

    if let Err(ref e) = result {
        tracing::error!(error = %e, "Command failed");
    }

    result
}

/// Initialize logging based on CLI options
fn init_logging(cli: &Cli) -> Result<()> {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let filter = if cli.quiet {
        EnvFilter::new("warn")
    } else {
        let default_level = match cli.verbose {
            0 => "info",
            1 => "debug",
            _ => "trace",
        };
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level))
    };

    let fmt_layer = match cli.log_format {
        cli::LogFormat::Json => fmt::layer()
            .json()
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .boxed(),
        cli::LogFormat::Pretty => fmt::layer().pretty().boxed(),
        cli::LogFormat::Compact => fmt::layer().compact().boxed(),
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init()
        .map_err(|e| anyhow::anyhow!("Failed to initialize logging: {}", e))?;

    Ok(())
}
