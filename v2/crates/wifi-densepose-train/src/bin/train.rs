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
use tracing::{error, info, warn};

use wifi_densepose_train::{
    config::TrainingConfig,
    dataset::{CsiDataset, MmFiDataset, SyntheticConfig, SyntheticCsiDataset},
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

    /// MM-Fi directory to use as the REAL validation set (e.g. a held-out
    /// subject or session). Takes precedence over the automatic
    /// subject-disjoint split of `--data-dir`. Must share no subject with
    /// `--data-dir` (checked; overlap is refused as a train/val leak).
    #[arg(long, value_name = "DIR")]
    val_dir: Option<PathBuf>,

    /// Evaluation-only mode: load `--checkpoint`, run the validation loop over
    /// `--val-dir` (no training), print PCK@0.2 / OKS and exit. Requires the
    /// `tch-backend` feature.
    #[arg(long, default_value_t = false, requires_all = ["checkpoint", "val_dir"])]
    eval_only: bool,

    /// Checkpoint (tch VarStore `.pt` or `.safetensors`) to load in
    /// `--eval-only` mode.
    #[arg(long, value_name = "FILE", requires = "eval_only")]
    checkpoint: Option<PathBuf>,

    /// In `--eval-only` mode, dump per-sample predicted + GT keypoints as JSONL
    /// to this path (dataset order, one line per window).
    #[arg(long, value_name = "FILE", requires = "eval_only")]
    dump_preds: Option<PathBuf>,

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
    // Eval-only mode: load checkpoint, run the validation loop, exit
    // ------------------------------------------------------------------

    if args.eval_only {
        // clap enforces `--checkpoint` and `--val-dir` alongside `--eval-only`.
        let (Some(ckpt), Some(val_dir)) = (args.checkpoint.as_deref(), args.val_dir.as_deref())
        else {
            error!("--eval-only requires --checkpoint <FILE> and --val-dir <DIR>");
            std::process::exit(2);
        };
        let eval_ds = load_mmfi_or_exit(val_dir, &config, "eval");
        run_eval(config, &eval_ds, ckpt, args.dump_preds.as_deref());
        return;
    }

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
        warn!(
            "[SMOKE-TEST ONLY] --dry-run trains and validates on SYNTHETIC data. \
             Any val_pck/val_oks is a pipeline smoke-test and MUST NOT be reported \
             as accuracy (ADR-155 §Tier-1.2)."
        );

        run_smoke_test(config, &train_ds, &val_ds);
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
                error!("Ensure MM-Fi data exists at {}", data_dir.display());
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

        // An explicit held-out validation directory takes precedence over the
        // automatic split. Refuse any subject overlap with the training set:
        // that would be exactly the leak ADR-155 §Tier-1.2 forbids.
        if let Some(val_dir) = args.val_dir.as_deref() {
            let val_ds = load_mmfi_or_exit(val_dir, &config, "validation");
            let train_subjects = train_ds.subjects();
            let shared: Vec<u32> = val_ds
                .subjects()
                .into_iter()
                .filter(|s| train_subjects.contains(s))
                .collect();
            if !shared.is_empty() {
                error!(
                    "--val-dir shares subject(s) {shared:?} with --data-dir; a \
                     subject-overlapping validation set leaks (ADR-155 §Tier-1.2). \
                     Use a held-out subject directory."
                );
                std::process::exit(1);
            }
            info!(
                "Using REAL held-out validation set: {} windows (subjects {:?}) from {}",
                val_ds.len(),
                val_ds.subjects(),
                val_dir.display()
            );
            run_training(config, &train_ds, &val_ds);
            return;
        }

        // ADR-155 §Tier-1.2: prefer a REAL, leak-free, subject-disjoint split so
        // any reported PCK/OKS is honest. MM-Fi windows are stride-1 (≈99%
        // overlap), so an index-level split would leak; a synthetic val set
        // makes the metric meaningless. Split at the subject level when the
        // dataset has ≥2 subjects.
        match train_ds.subject_disjoint_split(0.2, config.seed) {
            Ok((train_view, val_view)) => {
                info!(
                    "Leak-free subject-disjoint split: {} train windows (subjects {:?}) / \
                     {} val windows (subjects {:?})",
                    train_view.len(),
                    train_view.subjects(),
                    val_view.len(),
                    val_view.subjects(),
                );
                run_training(config, &train_view, &val_view);
            }
            Err(e) => {
                // Cannot form a real split (e.g. a single subject). Fall back to
                // a SYNTHETIC val set, but make it UNMISTAKABLE that this is a
                // smoke-test only — its metric is NOT a reportable number.
                warn!("Cannot build a leak-free subject-disjoint split: {e}");
                warn!(
                    "[SMOKE-TEST ONLY] Falling back to a SYNTHETIC validation set. \
                     ANY val_pck/val_oks printed below is a PIPELINE SMOKE-TEST on \
                     synthetic data and MUST NOT be reported or claimed as accuracy \
                     (ADR-155 §Tier-1.2). Provide a multi-subject dataset for a real \
                     measurement."
                );
                let val_syn_cfg = SyntheticConfig {
                    num_subcarriers: config.num_subcarriers,
                    num_antennas_tx: config.num_antennas_tx,
                    num_antennas_rx: config.num_antennas_rx,
                    window_frames: config.window_frames,
                    num_keypoints: config.num_keypoints,
                    signal_frequency_hz: 2.4e9,
                };
                let val_ds = SyntheticCsiDataset::new(config.batch_size.max(1), val_syn_cfg);
                run_smoke_test(config, &train_ds, &val_ds);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// run_training — conditionally compiled on tch-backend
// ---------------------------------------------------------------------------

#[cfg(feature = "tch-backend")]
fn run_training(config: TrainingConfig, train_ds: &dyn CsiDataset, val_ds: &dyn CsiDataset) {
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
fn run_training(_config: TrainingConfig, train_ds: &dyn CsiDataset, val_ds: &dyn CsiDataset) {
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
// --val-dir / --eval-only helpers
// ---------------------------------------------------------------------------

/// Load an MM-Fi directory with the run's window/subcarrier/keypoint shape, or
/// exit with an error if it cannot be loaded or holds no windows.
fn load_mmfi_or_exit(dir: &std::path::Path, config: &TrainingConfig, what: &str) -> MmFiDataset {
    let ds = match MmFiDataset::discover(
        dir,
        config.window_frames,
        config.num_subcarriers,
        config.num_keypoints,
    ) {
        Ok(ds) => ds,
        Err(e) => {
            error!("Failed to load {what} dataset from {}: {e}", dir.display());
            std::process::exit(1);
        }
    };
    if ds.is_empty() {
        error!("{what} dataset is empty — {}", dir.display());
        std::process::exit(1);
    }
    ds
}

/// `--eval-only`: load a checkpoint into a fresh model and run the same
/// validation loop training uses ([`Trainer::evaluate`]).
#[cfg(feature = "tch-backend")]
fn run_eval(
    config: TrainingConfig,
    ds: &dyn CsiDataset,
    checkpoint: &std::path::Path,
    dump: Option<&std::path::Path>,
) {
    use wifi_densepose_train::trainer::Trainer;

    let mut trainer = Trainer::new(config);
    match trainer.load_checkpoint(checkpoint) {
        Ok(epoch) => info!("Loaded checkpoint {} (epoch {epoch})", checkpoint.display()),
        Err(e) => {
            error!("Failed to load checkpoint: {e}");
            std::process::exit(1);
        }
    }
    info!("Evaluating on {} windows", ds.len());

    let result = match dump {
        Some(path) => {
            info!("Dumping per-sample predictions to {}", path.display());
            trainer.evaluate_with_dump(ds, path)
        }
        None => trainer.evaluate(ds),
    };
    match result {
        Ok(m) => info!("EVAL RESULT: {}", m.summary()),
        Err(e) => {
            error!("Evaluation failed: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(feature = "tch-backend"))]
fn run_eval(
    _config: TrainingConfig,
    _ds: &dyn CsiDataset,
    _checkpoint: &std::path::Path,
    _dump: Option<&std::path::Path>,
) {
    error!(
        "--eval-only requires the `tch-backend` feature: \
         cargo run --features tch-backend --bin train"
    );
    std::process::exit(1);
}

// ---------------------------------------------------------------------------
// run_smoke_test — synthetic-validation path (NOT a reportable metric)
// ---------------------------------------------------------------------------
//
// ADR-155 §Tier-1.2: identical to `run_training` but every metric it surfaces
// is prefixed/labelled as a SMOKE-TEST so a synthetic-val PCK can never be
// mistaken for a measured accuracy number.

#[cfg(feature = "tch-backend")]
fn run_smoke_test(config: TrainingConfig, train_ds: &dyn CsiDataset, val_ds: &dyn CsiDataset) {
    use wifi_densepose_train::trainer::Trainer;

    warn!(
        "[SMOKE-TEST] Starting SYNTHETIC-validation run: {} train / {} val samples. \
         Reported PCK/OKS below are NOT measurements.",
        train_ds.len(),
        val_ds.len()
    );

    let mut trainer = Trainer::new(config);
    match trainer.train(train_ds, val_ds) {
        Ok(result) => {
            warn!("[SMOKE-TEST] Pipeline ran end-to-end (no crash). Metrics are synthetic:");
            warn!(
                "[SMOKE-TEST] (DO NOT REPORT) best_pck@0.2={:.4} @ epoch {} — synthetic val",
                result.best_pck, result.best_epoch
            );
            info!(
                "[SMOKE-TEST] Final train loss: {:.6}",
                result.final_train_loss
            );
        }
        Err(e) => {
            error!("[SMOKE-TEST] Pipeline failed: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(feature = "tch-backend"))]
fn run_smoke_test(_config: TrainingConfig, train_ds: &dyn CsiDataset, val_ds: &dyn CsiDataset) {
    warn!(
        "[SMOKE-TEST] Pipeline verification only: {} train / {} synthetic-val samples loaded. \
         No metric is produced; build with --features tch-backend to run the pipeline.",
        train_ds.len(),
        val_ds.len()
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Log a human-readable summary of the active training configuration.
fn log_config_summary(config: &TrainingConfig) {
    info!("Training configuration:");
    info!(
        "  subcarriers  : {} (native: {})",
        config.num_subcarriers, config.native_subcarriers
    );
    info!(
        "  antennas     : {}×{}",
        config.num_antennas_tx, config.num_antennas_rx
    );
    info!("  window frames: {}", config.window_frames);
    info!("  batch size   : {}", config.batch_size);
    info!("  learning rate: {:.2e}", config.learning_rate);
    info!("  epochs       : {}", config.num_epochs);
    info!(
        "  device       : {}",
        if config.use_gpu { "GPU" } else { "CPU" }
    );
    info!("  checkpoint   : {}", config.checkpoint_dir.display());
}
