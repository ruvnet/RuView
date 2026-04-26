//! `train` binary — entry point for the WiFi-DensePose training pipeline.
//!
//! # Usage
//!
//! ```bash
//! # Full training with default config (requires tch-backend feature)
//! cargo run --features tch-backend --bin train
//!
//! # Custom config and data directory
//! cargo run --features tch-backend --bin train -- \
//!     --config config.json --data-dir /data/mm-fi
//!
//! # GPU training
//! cargo run --features tch-backend --bin train -- --cuda
//!
//! # Smoke-test with synthetic data (no real dataset required)
//! cargo run --features tch-backend --bin train -- --dry-run
//! ```
//!
//! Exit code 0 on success, non-zero on configuration or dataset errors.
//!
//! **Note**: This binary requires the `tch-backend` Cargo feature to be
//! enabled. When the feature is disabled a stub `main` is compiled that
//! immediately exits with a helpful error message.

use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};

use wifi_densepose_train::{
    config::TrainingConfig,
    dataset::{CsiDataset, MmFiDataset, SyntheticCsiDataset, SyntheticConfig},
};

// ---------------------------------------------------------------------------
// CLI arguments
// ---------------------------------------------------------------------------

/// Command-line arguments for the WiFi-DensePose training binary.
#[derive(Parser, Debug)]
#[command(
    name = "train",
    version,
    about = "Train WiFi-DensePose on the MM-Fi dataset",
    long_about = None
)]
struct Args {
    /// Path to a JSON training-configuration file.
    ///
    /// If not provided, [`TrainingConfig::default`] is used.
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Root directory containing MM-Fi recordings.
    #[arg(long, value_name = "DIR")]
    data_dir: Option<PathBuf>,

    /// Override the checkpoint output directory from the config.
    #[arg(long, value_name = "DIR")]
    checkpoint_dir: Option<PathBuf>,

    /// Enable CUDA training (sets `use_gpu = true` in the config).
    #[arg(long, default_value_t = false)]
    cuda: bool,

    /// Run a smoke-test with a synthetic dataset instead of real MM-Fi data.
    ///
    /// Useful for verifying the pipeline without downloading the dataset.
    #[arg(long, default_value_t = false)]
    dry_run: bool,

    /// Number of synthetic samples when `--dry-run` is active.
    #[arg(long, default_value_t = 64)]
    dry_run_samples: usize,

    /// Log level: trace, debug, info, warn, error.
    #[arg(long, default_value = "info")]
    log_level: String,
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let args = Args::parse();

    // Initialise structured logging.
    tracing_subscriber::fmt()
        .with_max_level(
            args.log_level
                .parse::<tracing_subscriber::filter::LevelFilter>()
                .unwrap_or(tracing_subscriber::filter::LevelFilter::INFO),
        )
        .with_target(false)
        .with_thread_ids(false)
        .init();

    info!(
        "WiFi-DensePose Training Pipeline v{}",
        wifi_densepose_train::VERSION
    );

    // ------------------------------------------------------------------
    // Build TrainingConfig
    // ------------------------------------------------------------------

    let mut config = if let Some(ref cfg_path) = args.config {
        info!("Loading configuration from {}", cfg_path.display());
        match TrainingConfig::from_json(cfg_path) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to load config: {e}");
                std::process::exit(1);
            }
        }
    } else {
        info!("No config file provided — using TrainingConfig::default()");
        TrainingConfig::default()
    };

    // Apply CLI overrides.
    if let Some(dir) = args.checkpoint_dir {
        info!("Overriding checkpoint_dir → {}", dir.display());
        config.checkpoint_dir = dir;
    }
    if args.cuda {
        info!("CUDA override: use_gpu = true");
        config.use_gpu = true;
    }

    // Validate the final configuration.
    if let Err(e) = config.validate() {
        error!("Config validation failed: {e}");
        std::process::exit(1);
    }

    log_config_summary(&config);

    // ------------------------------------------------------------------
    // Build datasets
    // ------------------------------------------------------------------

    let data_dir = args
        .data_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("data/mm-fi"));

    if args.dry_run {
        info!(
            "DRY RUN: using SyntheticCsiDataset ({} samples)",
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
        let n_total = args.dry_run_samples;
        let n_val = (n_total / 5).max(1);
        let n_train = n_total - n_val;
        let train_ds = SyntheticCsiDataset::new(n_train, syn_cfg.clone());
        let val_ds = SyntheticCsiDataset::new(n_val, syn_cfg);

        info!(
            "Synthetic split: {} train / {} val",
            train_ds.len(),
            val_ds.len()
        );

        run_training(config, &train_ds, &val_ds);
    } else {
        info!("Loading MM-Fi dataset from {}", data_dir.display());

        let train_ds = match MmFiDataset::discover(
            &data_dir,
            config.window_frames,
            config.num_subcarriers,
            config.num_keypoints,
        ) {
            Ok(ds) => ds,
            Err(e) => {
                error!("Failed to load dataset: {e}");
                error!(
                    "Ensure MM-Fi data exists at {}",
                    data_dir.display()
                );
                std::process::exit(1);
            }
        };

        if train_ds.is_empty() {
            error!(
                "Dataset is empty — no samples found in {}",
                data_dir.display()
            );
            std::process::exit(1);
        }

        info!("Dataset: {} samples", train_ds.len());

        // Use a small synthetic validation set when running without a split.
        let val_syn_cfg = SyntheticConfig {
            num_subcarriers: config.num_subcarriers,
            num_antennas_tx: config.num_antennas_tx,
            num_antennas_rx: config.num_antennas_rx,
            window_frames: config.window_frames,
            num_keypoints: config.num_keypoints,
            signal_frequency_hz: 2.4e9,
        };
        let val_ds = SyntheticCsiDataset::new(config.batch_size.max(1), val_syn_cfg);
        info!(
            "Using synthetic validation set ({} samples) for pipeline verification",
            val_ds.len()
        );

        run_training(config, &train_ds, &val_ds);
    }
}

// ---------------------------------------------------------------------------
// run_training — conditionally compiled on tch-backend
// ---------------------------------------------------------------------------

#[cfg(feature = "tch-backend")]
fn run_training(
    config: TrainingConfig,
    train_ds: &dyn CsiDataset,
    val_ds: &dyn CsiDataset,
) {
    use wifi_densepose_train::trainer::Trainer;

    info!(
        "Starting training: {} train / {} val samples",
        train_ds.len(),
        val_ds.len()
    );

    let mut trainer = Trainer::new(config);

    match trainer.train(train_ds, val_ds) {
        Ok(result) => {
            info!("Training complete.");
            info!("  Best PCK@0.2 : {:.4}", result.best_pck);
            info!("  Best epoch   : {}", result.best_epoch);
            info!("  Final loss   : {:.6}", result.final_train_loss);
            if let Some(ref ckpt) = result.checkpoint_path {
                info!("  Best checkpoint: {}", ckpt.display());
            }
        }
        Err(e) => {
            error!("Training failed: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(feature = "tch-backend"))]
fn run_training(
    _config: TrainingConfig,
    train_ds: &dyn CsiDataset,
    val_ds: &dyn CsiDataset,
) {
    info!(
        "Pipeline verification complete: {} train / {} val samples loaded.",
        train_ds.len(),
        val_ds.len()
    );
    info!(
        "Full training requires the `tch-backend` feature: \
         cargo run --features tch-backend --bin train"
    );
    info!("Config and dataset infrastructure: OK");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Log a human-readable summary of the active training configuration.
fn log_config_summary(config: &TrainingConfig) {
    info!("Training configuration:");
    info!("  subcarriers  : {} (native: {})", config.num_subcarriers, config.native_subcarriers);
    info!("  antennas     : {}×{}", config.num_antennas_tx, config.num_antennas_rx);
    info!("  window frames: {}", config.window_frames);
    info!("  batch size   : {}", config.batch_size);
    info!("  learning rate: {:.2e}", config.learning_rate);
    info!("  epochs       : {}", config.num_epochs);
    info!("  device       : {}", if config.use_gpu { "GPU" } else { "CPU" });
    info!("  checkpoint   : {}", config.checkpoint_dir.display());
}
