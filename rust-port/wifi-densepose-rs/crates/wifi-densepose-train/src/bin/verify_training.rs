//! `verify-training` binary — deterministic training proof / trust kill switch.
//!
//! Runs a fixed-seed mini-training on [`SyntheticCsiDataset`] for
//! [`proof::N_PROOF_STEPS`] gradient steps, then:
//!
//!  1. Verifies the training loss **decreased** (the model genuinely learned).
//!  2. Computes a SHA-256 hash of all model weight tensors after training.
//!  3. Compares the hash against a pre-recorded expected value stored in
//!     `<proof-dir>/expected_proof.sha256`.
//!
//! # Exit codes
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0    | PASS — hash matches AND loss decreased |
//! | 1    | FAIL — hash mismatch OR loss did not decrease |
//! | 2    | SKIP — no expected hash file found; run `--generate-hash` first |
//!
//! # Usage
//!
//! ```bash
//! # Generate the expected hash (first time)
//! cargo run --bin verify-training -- --generate-hash
//!
//! # Verify (subsequent runs)
//! cargo run --bin verify-training
//!
//! # Verbose output (show full loss trajectory)
//! cargo run --bin verify-training -- --verbose
//!
//! # Custom proof directory
//! cargo run --bin verify-training -- --proof-dir /path/to/proof
//! ```

use clap::Parser;
use std::path::PathBuf;

use wifi_densepose_train::proof;

// ---------------------------------------------------------------------------
// CLI arguments
// ---------------------------------------------------------------------------

/// Arguments for the `verify-training` trust kill switch binary.
#[derive(Parser, Debug)]
#[command(
    name = "verify-training",
    version,
    about = "WiFi-DensePose training trust kill switch: deterministic proof via SHA-256",
    long_about = None,
)]
struct Args {
    /// Generate (or regenerate) the expected hash and exit.
    ///
    /// Run this once after implementing or changing the training pipeline.
    /// Commit the resulting `expected_proof.sha256` to version control.
    #[arg(long, default_value_t = false)]
    generate_hash: bool,

    /// Directory where `expected_proof.sha256` is read from / written to.
    #[arg(long, default_value = ".")]
    proof_dir: PathBuf,

    /// Print the full per-step loss trajectory.
    #[arg(long, short = 'v', default_value_t = false)]
    verbose: bool,

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

    print_banner();

    // ------------------------------------------------------------------
    // Generate-hash mode
    // ------------------------------------------------------------------

    if args.generate_hash {
        println!("[GENERATE] Running proof to compute expected hash ...");
        println!("  Proof dir:  {}", args.proof_dir.display());
        println!("  Steps:      {}", proof::N_PROOF_STEPS);
        println!("  Model seed: {}", proof::MODEL_SEED);
        println!("  Data seed:  {}", proof::PROOF_SEED);
        println!();

        match proof::generate_expected_hash(&args.proof_dir) {
            Ok(hash) => {
                println!("  Hash written: {hash}");
                println!();
                println!(
                    "  File: {}/expected_proof.sha256",
                    args.proof_dir.display()
                );
                println!();
                println!("  Commit this file to version control, then run");
                println!("  verify-training (without --generate-hash) to verify.");
            }
            Err(e) => {
                eprintln!("  ERROR: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // ------------------------------------------------------------------
    // Verification mode
    // ------------------------------------------------------------------

    // Step 1: display proof configuration.
    println!("[1/4] PROOF CONFIGURATION");
    let cfg = proof::proof_config();
    println!("  Steps:       {}", proof::N_PROOF_STEPS);
    println!("  Model seed:  {}", proof::MODEL_SEED);
    println!("  Data seed:   {}", proof::PROOF_SEED);
    println!("  Batch size:  {}", proof::PROOF_BATCH_SIZE);
    println!("  Dataset:     SyntheticCsiDataset ({} samples, deterministic)", proof::PROOF_DATASET_SIZE);
    println!("  Subcarriers: {}", cfg.num_subcarriers);
    println!("  Window len:  {}", cfg.window_frames);
    println!("  Heatmap:     {}×{}", cfg.heatmap_size, cfg.heatmap_size);
    println!("  Lambda_kp:   {}", cfg.lambda_kp);
    println!("  Lambda_dp:   {}", cfg.lambda_dp);
    println!("  Lambda_tr:   {}", cfg.lambda_tr);
    println!();

    // Step 2: run the proof.
    println!("[2/4] RUNNING TRAINING PROOF");
    let result = match proof::run_proof(&args.proof_dir) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("  ERROR: {e}");
            std::process::exit(1);
        }
    };

    println!("  Steps completed: {}", result.steps_completed);
    println!("  Initial loss:    {:.6}", result.initial_loss);
    println!("  Final loss:      {:.6}", result.final_loss);
    println!(
        "  Loss decreased:  {} ({:.6} → {:.6})",
        if result.loss_decreased { "YES" } else { "NO" },
        result.initial_loss,
        result.final_loss
    );

    if args.verbose {
        println!();
        println!("  Loss trajectory ({} steps):", result.steps_completed);
        for (i, &loss) in result.loss_trajectory.iter().enumerate() {
            println!("    step {:3}: {:.6}", i, loss);
        }
    }
    println!();

    // Step 3: hash comparison.
    println!("[3/4] SHA-256 HASH COMPARISON");
    println!("  Computed:  {}", result.model_hash);

    match &result.expected_hash {
        None => {
            println!("  Expected:  (none — run with --generate-hash first)");
            println!();
            println!("[4/4] VERDICT");
            println!("{}", "=".repeat(72));
            println!("  SKIP — no expected hash file found.");
            println!();
            println!("  Run the following to generate the expected hash:");
            println!("    verify-training --generate-hash --proof-dir {}", args.proof_dir.display());
            println!("{}", "=".repeat(72));
            std::process::exit(2);
        }
        Some(expected) => {
            println!("  Expected:  {expected}");
            let matched = result.hash_matches.unwrap_or(false);
            println!("  Status:    {}", if matched { "MATCH" } else { "MISMATCH" });
            println!();

            // Step 4: final verdict.
            println!("[4/4] VERDICT");
            println!("{}", "=".repeat(72));

            if matched && result.loss_decreased {
                println!("  PASS");
                println!();
                println!("  The training pipeline produced a SHA-256 hash matching");
                println!("  the expected value.  This proves:");
                println!();
                println!("    1. Training is DETERMINISTIC");
                println!("       Same seed → same weight trajectory → same hash.");
                println!();
                println!("    2. Loss DECREASED over {} steps", proof::N_PROOF_STEPS);
                println!("       ({:.6} → {:.6})", result.initial_loss, result.final_loss);
                println!("       The model is genuinely learning signal structure.");
                println!();
                println!("    3. No non-determinism was introduced");
                println!("       Any code/library change would produce a different hash.");
                println!();
                println!("    4. Signal processing, loss functions, and optimizer are REAL");
                println!("       A mock pipeline cannot reproduce this exact hash.");
                println!();
                println!("  Model hash: {}", result.model_hash);
                println!("{}", "=".repeat(72));
                std::process::exit(0);
            } else {
                println!("  FAIL");
                println!();
                if !result.loss_decreased {
                    println!(
                        "  REASON: Loss did not decrease ({:.6} → {:.6}).",
                        result.initial_loss, result.final_loss
                    );
                    println!("  The model is not learning.  Check loss function and optimizer.");
                }
                if !matched {
                    println!("  REASON: Hash mismatch.");
                    println!("    Computed:  {}", result.model_hash);
                    println!("    Expected:  {}", expected);
                    println!();
                    println!("  Possible causes:");
                    println!("    - Code change (model architecture, loss, data pipeline)");
                    println!("    - Library version change (tch, ndarray)");
                    println!("    - Non-determinism was introduced");
                    println!();
                    println!("  If the change is intentional, regenerate the hash:");
                    println!(
                        "    verify-training --generate-hash --proof-dir {}",
                        args.proof_dir.display()
                    );
                }
                println!("{}", "=".repeat(72));
                std::process::exit(1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Banner
// ---------------------------------------------------------------------------

fn print_banner() {
    println!("{}", "=".repeat(72));
    println!("  WiFi-DensePose Training: Trust Kill Switch / Proof Replay");
    println!("{}", "=".repeat(72));
    println!();
    println!("  \"If training is deterministic and loss decreases from a fixed");
    println!("   seed, 'it is mocked' becomes a falsifiable claim that fails");
    println!("   against SHA-256 evidence.\"");
    println!();
}
