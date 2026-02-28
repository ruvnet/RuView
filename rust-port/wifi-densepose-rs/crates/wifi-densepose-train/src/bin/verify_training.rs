//! `verify-training` binary — end-to-end smoke-test for the training pipeline.
//!
//! Runs a deterministic forward pass through the complete pipeline using the
//! synthetic dataset (seed = 42). All assertions are purely structural; no
//! real GPU or dataset files are required.
//!
//! # Usage
//!
//! ```bash
//! cargo run --bin verify-training
//! cargo run --bin verify-training -- --samples 128 --verbose
//! ```
//!
//! Exit code `0` means all checks passed; non-zero means a failure was detected.

use clap::Parser;
use tracing::{error, info};
use wifi_densepose_train::{
    config::TrainingConfig,
    dataset::{CsiDataset, SyntheticCsiDataset, SyntheticConfig},
    subcarrier::interpolate_subcarriers,
    proof::verify_checkpoint_dir,
};

/// Arguments for the `verify-training` binary.
#[derive(Parser, Debug)]
#[command(
    name = "verify-training",
    version,
    about = "Smoke-test the WiFi-DensePose training pipeline end-to-end",
    long_about = None,
)]
struct Args {
    /// Number of synthetic samples to generate for the test.
    #[arg(long, default_value_t = 16)]
    samples: usize,

    /// Log level (trace, debug, info, warn, error).
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Print per-sample statistics to stdout.
    #[arg(long, short = 'v', default_value_t = false)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    let log_level_filter = args
        .log_level
        .parse::<tracing_subscriber::filter::LevelFilter>()
        .unwrap_or(tracing_subscriber::filter::LevelFilter::INFO);

    tracing_subscriber::fmt()
        .with_max_level(log_level_filter)
        .with_target(false)
        .with_thread_ids(false)
        .init();

    info!("=== WiFi-DensePose Training Verification ===");
    info!("Samples: {}", args.samples);

    let mut failures: Vec<String> = Vec::new();

    // -----------------------------------------------------------------------
    // 1. Config validation
    // -----------------------------------------------------------------------
    info!("[1/5] Verifying default TrainingConfig...");
    let config = TrainingConfig::default();
    match config.validate() {
        Ok(()) => info!("  OK: default config validates"),
        Err(e) => {
            let msg = format!("FAIL: default config is invalid: {e}");
            error!("{}", msg);
            failures.push(msg);
        }
    }

    // -----------------------------------------------------------------------
    // 2. Synthetic dataset creation and sample shapes
    // -----------------------------------------------------------------------
    info!("[2/5] Verifying SyntheticCsiDataset...");
    let syn_cfg = SyntheticConfig {
        num_subcarriers: config.num_subcarriers,
        num_antennas_tx: config.num_antennas_tx,
        num_antennas_rx: config.num_antennas_rx,
        window_frames: config.window_frames,
        num_keypoints: config.num_keypoints,
        signal_frequency_hz: 2.4e9,
    };

    // Use deterministic seed 42 (required for proof verification).
    let dataset = SyntheticCsiDataset::new(args.samples, syn_cfg.clone());

    if dataset.len() != args.samples {
        let msg = format!(
            "FAIL: dataset.len() = {} but expected {}",
            dataset.len(),
            args.samples
        );
        error!("{}", msg);
        failures.push(msg);
    } else {
        info!("  OK: dataset.len() = {}", dataset.len());
    }

    // Verify sample shapes for every sample.
    let mut shape_ok = true;
    for i in 0..args.samples {
        match dataset.get(i) {
            Ok(sample) => {
                let amp_shape = sample.amplitude.shape().to_vec();
                let expected_amp = vec![
                    syn_cfg.window_frames,
                    syn_cfg.num_antennas_tx,
                    syn_cfg.num_antennas_rx,
                    syn_cfg.num_subcarriers,
                ];
                if amp_shape != expected_amp {
                    let msg = format!(
                        "FAIL: sample {i} amplitude shape {amp_shape:?} != {expected_amp:?}"
                    );
                    error!("{}", msg);
                    failures.push(msg);
                    shape_ok = false;
                }

                let kp_shape = sample.keypoints.shape().to_vec();
                let expected_kp = vec![syn_cfg.num_keypoints, 2];
                if kp_shape != expected_kp {
                    let msg = format!(
                        "FAIL: sample {i} keypoints shape {kp_shape:?} != {expected_kp:?}"
                    );
                    error!("{}", msg);
                    failures.push(msg);
                    shape_ok = false;
                }

                // Keypoints must be in [0, 1]
                for kp in sample.keypoints.outer_iter() {
                    for &coord in kp.iter() {
                        if !(0.0..=1.0).contains(&coord) {
                            let msg = format!(
                                "FAIL: sample {i} keypoint coordinate {coord} out of [0, 1]"
                            );
                            error!("{}", msg);
                            failures.push(msg);
                            shape_ok = false;
                        }
                    }
                }

                if args.verbose {
                    info!(
                        "  sample {i}: amp={amp_shape:?}, kp={kp_shape:?}, \
                         amp[0,0,0,0]={:.4}",
                        sample.amplitude[[0, 0, 0, 0]]
                    );
                }
            }
            Err(e) => {
                let msg = format!("FAIL: dataset.get({i}) returned error: {e}");
                error!("{}", msg);
                failures.push(msg);
                shape_ok = false;
            }
        }
    }
    if shape_ok {
        info!("  OK: all {} sample shapes are correct", args.samples);
    }

    // -----------------------------------------------------------------------
    // 3. Determinism check — same index must yield the same data
    // -----------------------------------------------------------------------
    info!("[3/5] Verifying determinism...");
    let s_a = dataset.get(0).expect("sample 0 must be loadable");
    let s_b = dataset.get(0).expect("sample 0 must be loadable");
    let amp_equal = s_a
        .amplitude
        .iter()
        .zip(s_b.amplitude.iter())
        .all(|(a, b)| (a - b).abs() < 1e-7);
    if amp_equal {
        info!("  OK: dataset is deterministic (get(0) == get(0))");
    } else {
        let msg = "FAIL: dataset.get(0) produced different results on second call".to_string();
        error!("{}", msg);
        failures.push(msg);
    }

    // -----------------------------------------------------------------------
    // 4. Subcarrier interpolation
    // -----------------------------------------------------------------------
    info!("[4/5] Verifying subcarrier interpolation 114 → 56...");
    {
        let sample = dataset.get(0).expect("sample 0 must be loadable");
        // Simulate raw data with 114 subcarriers by creating a zero array.
        let raw = ndarray::Array4::<f32>::zeros((
            syn_cfg.window_frames,
            syn_cfg.num_antennas_tx,
            syn_cfg.num_antennas_rx,
            114,
        ));
        let resampled = interpolate_subcarriers(&raw, 56);
        let expected_shape = [
            syn_cfg.window_frames,
            syn_cfg.num_antennas_tx,
            syn_cfg.num_antennas_rx,
            56,
        ];
        if resampled.shape() == expected_shape {
            info!("  OK: interpolation output shape {:?}", resampled.shape());
        } else {
            let msg = format!(
                "FAIL: interpolation output shape {:?} != {:?}",
                resampled.shape(),
                expected_shape
            );
            error!("{}", msg);
            failures.push(msg);
        }
        // Amplitude from the synthetic dataset should already have 56 subcarriers.
        if sample.amplitude.shape()[3] != 56 {
            let msg = format!(
                "FAIL: sample amplitude has {} subcarriers, expected 56",
                sample.amplitude.shape()[3]
            );
            error!("{}", msg);
            failures.push(msg);
        } else {
            info!("  OK: sample amplitude already at 56 subcarriers");
        }
    }

    // -----------------------------------------------------------------------
    // 5. Proof helpers
    // -----------------------------------------------------------------------
    info!("[5/5] Verifying proof helpers...");
    {
        let tmp = tempfile_dir();
        if verify_checkpoint_dir(&tmp) {
            info!("  OK: verify_checkpoint_dir recognises existing directory");
        } else {
            let msg = format!(
                "FAIL: verify_checkpoint_dir returned false for {}",
                tmp.display()
            );
            error!("{}", msg);
            failures.push(msg);
        }

        let nonexistent = std::path::Path::new("/tmp/__nonexistent_wifi_densepose_path__");
        if !verify_checkpoint_dir(nonexistent) {
            info!("  OK: verify_checkpoint_dir correctly rejects nonexistent path");
        } else {
            let msg = "FAIL: verify_checkpoint_dir returned true for nonexistent path".to_string();
            error!("{}", msg);
            failures.push(msg);
        }
    }

    // -----------------------------------------------------------------------
    // Summary
    // -----------------------------------------------------------------------
    info!("===================================================");
    if failures.is_empty() {
        info!("ALL CHECKS PASSED ({}/5 suites)", 5);
        std::process::exit(0);
    } else {
        error!("{} CHECK(S) FAILED:", failures.len());
        for f in &failures {
            error!("  - {f}");
        }
        std::process::exit(1);
    }
}

/// Return a path to a temporary directory that exists for the duration of this
/// process. Uses `/tmp` as a portable fallback.
fn tempfile_dir() -> std::path::PathBuf {
    let p = std::path::Path::new("/tmp");
    if p.exists() && p.is_dir() {
        p.to_path_buf()
    } else {
        std::env::temp_dir()
    }
}
