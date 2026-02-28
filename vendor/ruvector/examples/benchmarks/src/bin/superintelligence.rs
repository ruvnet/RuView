//! Superintelligence Pathway Runner
//!
//! Runs a 5-level recursive intelligence amplification pipeline and tracks
//! IQ progression from foundation (~85) toward superintelligence (~98+).
//!
//! Usage:
//!   cargo run --bin superintelligence -- --verbose
//!   cargo run --bin superintelligence -- --episodes 15 --tasks 30 --target 95

use anyhow::Result;
use clap::Parser;
use ruvector_benchmarks::intelligence_metrics::IntelligenceCalculator;
use ruvector_benchmarks::superintelligence::{run_pathway, SIConfig};

#[derive(Parser, Debug)]
#[command(name = "superintelligence")]
#[command(about = "Run 5-level superintelligence pathway with IQ tracking")]
struct Args {
    /// Episodes per level
    #[arg(short, long, default_value = "12")]
    episodes: usize,

    /// Tasks per episode
    #[arg(short, long, default_value = "25")]
    tasks: usize,

    /// Random seed
    #[arg(long, default_value = "42")]
    seed: u64,

    /// Noise injection rate (0.0-1.0)
    #[arg(long, default_value = "0.25")]
    noise: f64,

    /// Step budget per episode
    #[arg(long, default_value = "400")]
    step_budget: usize,

    /// Target IQ score
    #[arg(long, default_value = "98.0")]
    target: f64,

    /// Ensemble size for Level 3
    #[arg(long, default_value = "4")]
    ensemble: usize,

    /// Recursive improvement cycles for Level 4
    #[arg(long, default_value = "3")]
    cycles: usize,

    /// Adversarial pressure multiplier for Level 5
    #[arg(long, default_value = "1.5")]
    pressure: f64,

    /// Verbose per-episode output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           SUPERINTELLIGENCE PATHWAY ENGINE                   ║");
    println!("║   5-Level Recursive Intelligence Amplification               ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!(
        "  Config: {} eps/level x {} tasks, noise={:.0}%, target IQ={:.0}",
        args.episodes,
        args.tasks,
        args.noise * 100.0,
        args.target
    );
    println!(
        "  Ensemble={}, Cycles={}, Pressure={:.1}",
        args.ensemble, args.cycles, args.pressure
    );
    println!();

    let config = SIConfig {
        episodes_per_level: args.episodes,
        tasks_per_episode: args.tasks,
        seed: args.seed,
        noise_rate: args.noise,
        step_budget: args.step_budget,
        target_iq: args.target,
        ensemble_size: args.ensemble,
        recursive_cycles: args.cycles,
        adversarial_pressure: args.pressure,
        verbose: args.verbose,
        ..Default::default()
    };

    let result = run_pathway(&config)?;
    result.print();

    // Detailed assessment for peak level
    let calculator = IntelligenceCalculator::default();
    if let Some(peak) = result
        .levels
        .iter()
        .max_by(|a, b| a.iq_score.partial_cmp(&b.iq_score).unwrap())
    {
        println!("  Peak Level ({}) Assessment:", peak.name);
        let assessment = calculator.calculate(&peak.raw_metrics);
        println!(
            "    Reasoning:     coherence={:.2}, efficiency={:.2}, error_rate={:.2}",
            assessment.reasoning.logical_coherence,
            assessment.reasoning.reasoning_efficiency,
            assessment.reasoning.error_rate
        );
        println!(
            "    Learning:      sample_eff={:.2}, regret_sub={:.2}, rate={:.2}",
            assessment.learning.sample_efficiency,
            assessment.learning.regret_sublinearity,
            assessment.learning.learning_rate
        );
        println!(
            "    Capabilities:  pattern={:.1}, planning={:.1}, adaptation={:.1}",
            assessment.capabilities.pattern_recognition,
            assessment.capabilities.planning,
            assessment.capabilities.adaptation
        );
        println!(
            "    Meta-cog:      self_correct={:.2}, strategy_adapt={:.2}",
            assessment.meta_cognition.self_correction_rate,
            assessment.meta_cognition.strategy_adaptation
        );
        println!();
    }

    Ok(())
}
