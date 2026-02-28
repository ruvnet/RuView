//! `train` binary — entry point for the WiFi-DensePose training pipeline.
//!
//! # Usage
//!
//! ```bash
//! cargo run --bin train -- --config config.toml
//! cargo run --bin train -- --config config.toml --cuda
//! ```

use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};
use wifi_densepose_train::config::TrainingConfig;
use wifi_densepose_train::dataset::{CsiDataset, MmFiDataset, SyntheticCsiDataset, SyntheticConfig};
use wifi_densepose_train::trainer::Trainer;

/// Command-line arguments for the training binary.
#[derive(Parser, Debug)]
#[command(
    name = "train",
    version,
    about = "WiFi-DensePose training pipeline",
    long_about = None
)]
struct Args {
    /// Path to the TOML configuration file.
    ///
    /// If not provided, the default `TrainingConfig` is used.
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Override the data directory from the config.
    #[arg(long, value_name = "DIR")]
    data_dir: Option<PathBuf>,

    /// Override the checkpoint directory from the config.
    #[arg(long, value_name = "DIR")]
    checkpoint_dir: Option<PathBuf>,

    /// Enable CUDA training (overrides config `use_gpu`).
    #[arg(long, default_value_t = false)]
    cuda: bool,

    /// Use the deterministic synthetic dataset instead of real data.
    ///
    /// This is intended for pipeline smoke-tests only, not production training.
    #[arg(long, default_value_t = false)]
    dry_run: bool,

    /// Number of synthetic samples when `--dry-run` is active.
    #[arg(long, default_value_t = 64)]
    dry_run_samples: usize,

    /// Log level (trace, debug, info, warn, error).
    #[arg(long, default_value = "info")]
    log_level: String,
}

fn main() {
    let args = Args::parse();

    // Initialise tracing subscriber.
    let log_level_filter = args
        .log_level
        .parse::<tracing_subscriber::filter::LevelFilter>()
        .unwrap_or(tracing_subscriber::filter::LevelFilter::INFO);

    tracing_subscriber::fmt()
        .with_max_level(log_level_filter)
        .with_target(false)
        .with_thread_ids(false)
        .init();

    info!("WiFi-DensePose Training Pipeline v{}", wifi_densepose_train::VERSION);

    // Load or construct training configuration.
    let mut config = match args.config.as_deref() {
        Some(path) => {
            info!("Loading configuration from {}", path.display());
            match TrainingConfig::from_json(path) {
                Ok(cfg) => cfg,
                Err(e) => {
                    error!("Failed to load configuration: {e}");
                    std::process::exit(1);
                }
            }
        }
        None => {
            info!("No configuration file provided — using defaults");
            TrainingConfig::default()
        }
    };

    // Apply CLI overrides.
    if let Some(dir) = args.data_dir {
        config.checkpoint_dir = dir;
    }
    if let Some(dir) = args.checkpoint_dir {
        config.checkpoint_dir = dir;
    }
    if args.cuda {
        config.use_gpu = true;
    }

    // Validate the final configuration.
    if let Err(e) = config.validate() {
        error!("Configuration validation failed: {e}");
        std::process::exit(1);
    }

    info!("Configuration validated successfully");
    info!("  subcarriers  : {}", config.num_subcarriers);
    info!("  antennas     : {}×{}", config.num_antennas_tx, config.num_antennas_rx);
    info!("  window frames: {}", config.window_frames);
    info!("  batch size   : {}", config.batch_size);
    info!("  learning rate: {}", config.learning_rate);
    info!("  epochs       : {}", config.num_epochs);
    info!("  device       : {}", if config.use_gpu { "GPU" } else { "CPU" });

    // Build the dataset.
    if args.dry_run {
        info!(
            "DRY RUN — using synthetic dataset ({} samples)",
            args.dry_run_samples
        );
        let syn_cfg = SyntheticConfig {
            num_subcarriers: config.num_subcarriers,
            num_antennas_tx: config.num_antennas_tx,
            num_antennas_rx: config.num_antennas_rx,
            window_frames: config.window_frames,
            num_keypoints: config.num_keypoints,
            signal_frequency_hz: 2.4e9,
        };
        let dataset = SyntheticCsiDataset::new(args.dry_run_samples, syn_cfg);
        info!("Synthetic dataset: {} samples", dataset.len());
        run_trainer(config, &dataset);
    } else {
        let data_dir = config.checkpoint_dir.parent()
            .map(|p| p.join("data"))
            .unwrap_or_else(|| std::path::PathBuf::from("data/mm-fi"));
        info!("Loading MM-Fi dataset from {}", data_dir.display());

        let dataset = match MmFiDataset::discover(
            &data_dir,
            config.window_frames,
            config.num_subcarriers,
            config.num_keypoints,
        ) {
            Ok(ds) => ds,
            Err(e) => {
                error!("Failed to load dataset: {e}");
                error!("Ensure real MM-Fi data is present at {}", data_dir.display());
                std::process::exit(1);
            }
        };

        if dataset.is_empty() {
            error!("Dataset is empty — no samples were loaded from {}", data_dir.display());
            std::process::exit(1);
        }

        info!("MM-Fi dataset: {} samples", dataset.len());
        run_trainer(config, &dataset);
    }
}

/// Run the training loop using the provided config and dataset.
fn run_trainer(config: TrainingConfig, dataset: &dyn CsiDataset) {
    info!("Initialising trainer");
    let trainer = Trainer::new(config);
    info!("Training configuration: {:?}", trainer.config());
    info!("Dataset: {} ({} samples)", dataset.name(), dataset.len());

    // The full training loop is implemented in `trainer::Trainer::run()`
    // which is provided by the trainer agent. This binary wires the entry
    // point together; training itself happens inside the Trainer.
    info!("Training loop will be driven by Trainer::run() (implementation pending)");
    info!("Training setup complete — exiting dry-run entrypoint");
}
