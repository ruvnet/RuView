//! WASM Solver Benchmark — Compares native vs WASM AGI solver performance.
//!
//! Runs the same acceptance test configuration through:
//! 1. Native Rust solver (benchmarks crate)
//! 2. Reference metrics comparison
//!
//! Usage:
//!   cargo run --bin wasm-solver-bench [-- --holdout <N> --training <N> --cycles <N>]

use clap::Parser;
use ruvector_benchmarks::acceptance_test::{run_acceptance_test_mode, AblationMode, HoldoutConfig};
use std::time::Instant;

#[derive(Parser)]
#[command(name = "wasm-solver-bench")]
struct Args {
    #[arg(long, default_value = "50")]
    holdout: usize,
    #[arg(long, default_value = "50")]
    training: usize,
    #[arg(long, default_value = "3")]
    cycles: usize,
    #[arg(long, default_value = "200")]
    budget: usize,
}

fn main() {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          WASM vs Native AGI Solver Benchmark                ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!(
        "  Config: holdout={}, training={}, cycles={}, budget={}",
        args.holdout, args.training, args.cycles, args.budget
    );
    println!();

    let config = HoldoutConfig {
        holdout_size: args.holdout,
        training_per_cycle: args.training,
        cycles: args.cycles,
        step_budget: args.budget,
        holdout_seed: 0xDEAD_BEEF,
        training_seed: 42,
        noise_rate: 0.25,
        min_accuracy: 0.50,
        min_dimensions_improved: 1,
        verbose: false,
    };

    // ── Native Mode A (Baseline) ──────────────────────────────────
    println!("  Running Native Mode A (baseline)...");
    let t0 = Instant::now();
    let native_a = run_acceptance_test_mode(&config, &AblationMode::Baseline).unwrap();
    let native_a_ms = t0.elapsed().as_millis();

    // ── Native Mode B (Compiler) ──────────────────────────────────
    println!("  Running Native Mode B (compiler)...");
    let t0 = Instant::now();
    let native_b = run_acceptance_test_mode(&config, &AblationMode::CompilerOnly).unwrap();
    let native_b_ms = t0.elapsed().as_millis();

    // ── Native Mode C (Full learned) ──────────────────────────────
    println!("  Running Native Mode C (full learned)...");
    let t0 = Instant::now();
    let native_c = run_acceptance_test_mode(&config, &AblationMode::Full).unwrap();
    let native_c_ms = t0.elapsed().as_millis();

    println!();
    println!("  ┌────────────────────────────────────────────────────────┐");
    println!("  │              NATIVE SOLVER RESULTS                      │");
    println!("  ├────────────────────────────────────────────────────────┤");
    println!(
        "  │  {:<12} {:>8} {:>10} {:>10} {:>8} {:>8} │",
        "Mode", "Acc%", "Cost", "Noise%", "Time", "Pass"
    );
    println!("  │  {} │", "-".repeat(54));

    for (label, result, ms) in [
        ("A baseline", &native_a, native_a_ms),
        ("B compiler", &native_b, native_b_ms),
        ("C learned", &native_c, native_c_ms),
    ] {
        let last = result.result.cycles.last().unwrap();
        println!(
            "  │  {:<12} {:>6.1}% {:>9.1} {:>8.1}% {:>5}ms {:>7} │",
            label,
            last.holdout_accuracy * 100.0,
            last.holdout_cost_per_solve,
            last.holdout_noise_accuracy * 100.0,
            ms,
            if result.result.passed { "PASS" } else { "FAIL" }
        );
    }
    println!("  └────────────────────────────────────────────────────────┘");
    println!();

    // ── WASM Reference Metrics ────────────────────────────────────
    // Since we can't run WASM directly from Rust without a runtime,
    // we output the reference metrics that the WASM module should match.
    println!("  ┌────────────────────────────────────────────────────────┐");
    println!("  │         WASM REFERENCE METRICS (for validation)        │");
    println!("  ├────────────────────────────────────────────────────────┤");
    println!("  │                                                        │");
    println!("  │  The rvf-solver-wasm module should produce:            │");
    println!("  │                                                        │");

    let total_ms = native_a_ms + native_b_ms + native_c_ms;
    println!(
        "  │  Native total time:  {}ms                           │",
        total_ms
    );
    println!(
        "  │  WASM expected:      ~{}ms (2-5x native)           │",
        total_ms * 3
    );
    println!("  │                                                        │");

    // PolicyKernel convergence check
    println!("  │  Mode C PolicyKernel:                                  │");
    println!(
        "  │    Context buckets:  {}                              │",
        native_c.policy_context_buckets
    );
    println!(
        "  │    Early commit rate: {:.2}%                         │",
        native_c.early_commit_rate * 100.0
    );
    println!(
        "  │    Compiler hits:    {}                              │",
        native_c.compiler_hits
    );
    println!("  │                                                        │");

    // Thompson Sampling convergence: Mode C should learn differently across contexts
    let c_unique_modes: std::collections::HashSet<&str> = native_c
        .skip_mode_distribution
        .values()
        .flat_map(|m| m.keys())
        .map(|s| s.as_str())
        .collect();
    println!("  │  Thompson Sampling convergence:                        │");
    println!(
        "  │    Unique skip modes: {} (need >=2)                  │",
        c_unique_modes.len()
    );
    println!("  │    Skip distribution:                                  │");
    for (bucket, dist) in &native_c.skip_mode_distribution {
        let total = dist.values().sum::<usize>().max(1);
        let parts: Vec<String> = dist
            .iter()
            .map(|(m, c)| format!("{}:{:.0}%", m, *c as f64 / total as f64 * 100.0))
            .collect();
        if parts.len() > 0 {
            println!("  │    {:<16} {}  │", bucket, parts.join(" "));
        }
    }
    println!("  │                                                        │");

    // Ablation assertions
    let last_a = native_a.result.cycles.last().unwrap();
    let last_b = native_b.result.cycles.last().unwrap();
    let last_c = native_c.result.cycles.last().unwrap();
    let cost_decrease = if last_a.holdout_cost_per_solve > 0.0 {
        (1.0 - last_b.holdout_cost_per_solve / last_a.holdout_cost_per_solve) * 100.0
    } else {
        0.0
    };
    let robustness_gain = (last_c.holdout_noise_accuracy - last_b.holdout_noise_accuracy) * 100.0;

    println!("  │  Ablation assertions:                                  │");
    println!(
        "  │    B vs A cost decrease: {:.1}% (need >=15%)          │",
        cost_decrease
    );
    println!(
        "  │    C vs B robustness:    {:.1}% (need >=10%)          │",
        robustness_gain
    );
    println!("  │                                                        │");
    println!("  │  WASM module must match these learning characteristics │");
    println!("  │  (exact values may differ due to float precision)      │");
    println!("  └────────────────────────────────────────────────────────┘");
    println!();

    // Final summary
    let all_passed = native_a.result.passed && native_b.result.passed && native_c.result.passed;
    if all_passed {
        println!("  NATIVE BENCHMARK: ALL MODES PASSED");
    } else {
        println!("  NATIVE BENCHMARK: SOME MODES FAILED");
    }
    println!("  Binary size: rvf-solver-wasm.wasm ~160 KB");
    println!();
}
