//! AGI Proof Harness — Nightly runner that publishes contract metrics.
//!
//! Publishes:
//! - Success rate
//! - Cost per solve
//! - Robustness under noise
//! - Policy compliance
//! - Contradiction rate
//! - Rollback correctness
//! - Viability checklist status
//! - Autonomy level
//!
//! Usage:
//!   cargo run --bin agi-proof-harness
//!   cargo run --bin agi-proof-harness -- --holdout 1000 --cycles 10 --verbose
//!   cargo run --bin agi-proof-harness -- --full  # 10K training, 1K holdout, 10 cycles

use anyhow::Result;
use clap::Parser;
use ruvector_benchmarks::acceptance_test::{
    run_ablation_comparison, run_acceptance_test, HoldoutConfig,
};
use ruvector_benchmarks::agi_contract::{AutonomyEvaluator, ContractHealth, ViabilityChecklist};
use ruvector_benchmarks::intelligence_metrics::IntelligenceCalculator;
use ruvector_benchmarks::superintelligence::{run_pathway, SIConfig};

#[derive(Parser, Debug)]
#[command(name = "agi-proof-harness")]
#[command(about = "AGI contract proof harness — publishes nightly metrics")]
struct Args {
    /// Holdout evaluation set size
    #[arg(long, default_value = "200")]
    holdout: usize,

    /// Training tasks per cycle
    #[arg(long, default_value = "200")]
    training: usize,

    /// Number of improvement cycles
    #[arg(long, default_value = "5")]
    cycles: usize,

    /// Frozen holdout seed
    #[arg(long, default_value = "3735928559")]
    holdout_seed: u64,

    /// Training seed
    #[arg(long, default_value = "42")]
    training_seed: u64,

    /// Noise injection rate
    #[arg(long, default_value = "0.25")]
    noise: f64,

    /// Step budget per task
    #[arg(long, default_value = "400")]
    step_budget: usize,

    /// Full acceptance test (10K training, 1K holdout, 10 cycles)
    #[arg(long)]
    full: bool,

    /// Minimum accuracy threshold
    #[arg(long, default_value = "0.80")]
    min_accuracy: f64,

    /// Run three-mode ablation comparison (A/B/C)
    #[arg(long)]
    ablation: bool,

    /// Also run the 5-level SI pathway
    #[arg(long)]
    pathway: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              AGI PROOF HARNESS                               ║");
    println!("║   Contract-based intelligence measurement                    ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let config = if args.full {
        HoldoutConfig {
            holdout_size: 1000,
            training_per_cycle: 1000,
            cycles: 10,
            holdout_seed: args.holdout_seed,
            training_seed: args.training_seed,
            noise_rate: args.noise,
            step_budget: args.step_budget,
            min_accuracy: 0.95,
            min_dimensions_improved: 2,
            verbose: args.verbose,
        }
    } else {
        HoldoutConfig {
            holdout_size: args.holdout,
            training_per_cycle: args.training,
            cycles: args.cycles,
            holdout_seed: args.holdout_seed,
            training_seed: args.training_seed,
            noise_rate: args.noise,
            step_budget: args.step_budget,
            min_accuracy: args.min_accuracy,
            min_dimensions_improved: 2,
            verbose: args.verbose,
        }
    };

    println!(
        "  Config: holdout={}, training/cycle={}, cycles={}, noise={:.0}%",
        config.holdout_size,
        config.training_per_cycle,
        config.cycles,
        config.noise_rate * 100.0
    );
    println!(
        "  Seeds: holdout=0x{:X}, training={}",
        config.holdout_seed, config.training_seed
    );
    println!();

    // ─── Run Acceptance Test ─────────────────────────────────────────
    println!("  Running acceptance test...");
    let result = run_acceptance_test(&config)?;
    result.print();

    // ─── Ablation Comparison ─────────────────────────────────────────
    if args.ablation {
        println!("  Running ablation comparison (A / B / C)...");
        let comparison = run_ablation_comparison(&config)?;
        comparison.print();
    }

    // ─── Contract Health Summary ─────────────────────────────────────
    if let Some(last_cycle) = result.cycles.last() {
        println!();
        last_cycle.contract_health.print();

        // ─── Autonomy Level ──────────────────────────────────────────
        let health_history: Vec<ContractHealth> = result
            .cycles
            .iter()
            .map(|c| c.contract_health.clone())
            .collect();
        let evaluator = AutonomyEvaluator::default();
        let level = evaluator.evaluate(&health_history);
        println!();
        evaluator.print_status(level, &last_cycle.contract_health);

        // ─── Viability Checklist ─────────────────────────────────────
        let viability = ViabilityChecklist::evaluate(&health_history);
        println!();
        viability.print();
    }

    // ─── Optional: SI Pathway ────────────────────────────────────────
    if args.pathway {
        println!();
        println!("  Running 5-level SI pathway...");
        let si_config = SIConfig {
            episodes_per_level: 6,
            tasks_per_episode: 15,
            verbose: args.verbose,
            ..Default::default()
        };
        let pathway_result = run_pathway(&si_config)?;
        pathway_result.print();

        // Show contract health for peak level
        if let Some(peak) = pathway_result
            .levels
            .iter()
            .max_by(|a, b| a.iq_score.partial_cmp(&b.iq_score).unwrap())
        {
            let health = ContractHealth::from_raw(&peak.raw_metrics);
            println!("  Peak Level ({}) Contract:", peak.name);
            health.print();

            let calculator = IntelligenceCalculator::default();
            let assessment = calculator.calculate(&peak.raw_metrics);
            println!("  Multi-dimensional IQ: {:.1}", assessment.overall_score);
            println!(
                "    Cost efficiency:  {:.2}",
                assessment.cost.cost_efficiency
            );
            println!(
                "    Robustness score: {:.2}",
                assessment.robustness.robustness_score
            );
        }
    }

    println!();
    Ok(())
}
