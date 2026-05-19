//! `cog-pose-estimation` — Cognitum Cog binary entrypoint.
//!
//! Implements the ADR-100 runtime contract:
//!     cog-pose-estimation version
//!     cog-pose-estimation manifest
//!     cog-pose-estimation health
//!     cog-pose-estimation run --config <path>
//!
//! Each subcommand writes structured JSON to stdout. `run` is long-running
//! and emits one `pose.frame` event per inferred CSI window.

use clap::{Parser, Subcommand};
use cog_pose_estimation::{
    config::CogConfig,
    inference::{InferenceEngine, SyntheticInput},
    manifest::ManifestSpec,
    publisher::{emit_event, Event},
};
use std::path::PathBuf;

const COG_ID: &str = "pose-estimation";
const COG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = COG_ID, version = COG_VERSION)]
#[command(about = "Cognitum Cog: 17-keypoint pose estimation from WiFi CSI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print `<id> <version>` and exit.
    Version,
    /// Print the embedded manifest as JSON.
    Manifest,
    /// One-shot health check. Exit 0 if the cog can come up healthy.
    Health,
    /// Long-running inference loop.
    Run {
        /// Path to runtime config JSON. See `cog/config.schema.json`.
        #[arg(long, value_name = "PATH")]
        config: PathBuf,
    },
}

fn main() -> std::process::ExitCode {
    init_logging();

    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::Version => cmd_version(),
        Cmd::Manifest => cmd_manifest(),
        Cmd::Health => cmd_health(),
        Cmd::Run { config } => cmd_run(config),
    };

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{COG_ID}: {err}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .json()
        .try_init();
}

fn cmd_version() -> Result<(), Box<dyn std::error::Error>> {
    println!("{COG_ID} {COG_VERSION}");
    Ok(())
}

fn cmd_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let spec = ManifestSpec::embedded(COG_ID, COG_VERSION);
    println!("{}", serde_json::to_string_pretty(&spec)?);
    Ok(())
}

fn cmd_health() -> Result<(), Box<dyn std::error::Error>> {
    let engine = InferenceEngine::new()?;
    let synthetic = SyntheticInput::default();
    let out = engine.infer(&synthetic.as_window())?;
    if out.is_finite() {
        emit_event(&Event::health_ok(COG_ID));
        Ok(())
    } else {
        Err("inference produced non-finite output".into())
    }
}

fn cmd_run(config_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = CogConfig::load(&config_path)?;
    emit_event(&Event::run_started(COG_ID, &cfg));

    let engine = InferenceEngine::new()?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(cog_pose_estimation::runtime::run_loop(cfg, engine))?;
    Ok(())
}
