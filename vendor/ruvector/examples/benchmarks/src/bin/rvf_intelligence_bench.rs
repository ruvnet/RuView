//! RVF Intelligence Benchmark Runner
//!
//! Runs head-to-head comparison across 6 intelligence verticals:
//! Baseline (no learning) vs. RVF-Learning (full pipeline).
//!
//! Usage:
//!   cargo run --bin rvf-intelligence-bench -- --episodes 15 --tasks 25 --verbose
//!   cargo run --bin rvf-intelligence-bench -- --noise 0.4 --step-budget 300

use anyhow::Result;
use clap::Parser;
use ruvector_benchmarks::intelligence_metrics::IntelligenceCalculator;
use ruvector_benchmarks::rvf_intelligence_bench::{run_comparison, BenchmarkConfig};

#[derive(Parser, Debug)]
#[command(name = "rvf-intelligence-bench")]
#[command(about = "Benchmark intelligence with and without RVF learning across 6 verticals")]
struct Args {
    /// Number of episodes per mode
    #[arg(short, long, default_value = "10")]
    episodes: usize,

    /// Tasks per episode
    #[arg(short, long, default_value = "20")]
    tasks: usize,

    /// Minimum difficulty (1-10)
    #[arg(long, default_value = "1")]
    min_diff: u8,

    /// Maximum difficulty (1-10)
    #[arg(long, default_value = "10")]
    max_diff: u8,

    /// Random seed for reproducibility
    #[arg(long, default_value = "42")]
    seed: u64,

    /// Noise probability (0.0-1.0)
    #[arg(long, default_value = "0.25")]
    noise: f64,

    /// Step budget per episode
    #[arg(long, default_value = "400")]
    step_budget: usize,

    /// Max retries for error recovery (RVF only)
    #[arg(long, default_value = "2")]
    max_retries: usize,

    /// Retention fraction (0.0-1.0)
    #[arg(long, default_value = "0.15")]
    retention: f64,

    /// Token budget per episode (RVF mode)
    #[arg(long, default_value = "200000")]
    token_budget: u32,

    /// Tool call budget per episode (RVF mode)
    #[arg(long, default_value = "50")]
    tool_budget: u16,

    /// Verbose per-episode output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!();
    println!("================================================================");
    println!("  RVF Intelligence Benchmark v2 — Six Verticals");
    println!("  Baseline vs. RVF-Learning (noise + step limits + retry + transfer)");
    println!("================================================================");
    println!();
    println!("  Configuration:");
    println!("    Episodes:       {}", args.episodes);
    println!("    Tasks/episode:  {}", args.tasks);
    println!("    Difficulty:     {}-{}", args.min_diff, args.max_diff);
    println!("    Seed:           {}", args.seed);
    println!("    Noise prob:     {:.0}%", args.noise * 100.0);
    println!("    Step budget/ep: {}", args.step_budget);
    println!("    Max retries:    {}", args.max_retries);
    println!("    Retention:      {:.0}%", args.retention * 100.0);
    println!();

    let config = BenchmarkConfig {
        episodes: args.episodes,
        tasks_per_episode: args.tasks,
        min_difficulty: args.min_diff,
        max_difficulty: args.max_diff,
        seed: Some(args.seed),
        token_budget: args.token_budget,
        tool_call_budget: args.tool_budget,
        verbose: args.verbose,
        noise_probability: args.noise,
        step_budget_per_episode: args.step_budget,
        max_retries: args.max_retries,
        retention_fraction: args.retention,
        ..Default::default()
    };

    println!("  Phase 1/2: Running baseline (no learning)...");
    let report = run_comparison(&config)?;

    // Print comparison report
    report.print();

    // Full IQ assessment
    let calculator = IntelligenceCalculator::default();

    println!("----------------------------------------------------------------");
    println!("  Detailed Intelligence Assessment: Baseline");
    println!("----------------------------------------------------------------");
    let base_assessment = calculator.calculate(&report.baseline.raw_metrics);
    print_compact_assessment(&base_assessment);

    println!();
    println!("----------------------------------------------------------------");
    println!("  Detailed Intelligence Assessment: RVF-Learning");
    println!("----------------------------------------------------------------");
    let rvf_assessment = calculator.calculate(&report.rvf_learning.raw_metrics);
    print_compact_assessment(&rvf_assessment);

    // Final IQ comparison
    println!();
    println!("================================================================");
    println!("  Intelligence Score Comparison");
    println!("================================================================");
    println!(
        "  Baseline IQ Score:     {:.1}/100",
        base_assessment.overall_score
    );
    println!(
        "  RVF-Learning IQ Score: {:.1}/100",
        rvf_assessment.overall_score
    );
    let iq_delta = rvf_assessment.overall_score - base_assessment.overall_score;
    println!("  Delta:                 {:+.1}", iq_delta);
    println!();

    if iq_delta > 10.0 {
        println!("  >> RVF learning loop provides a DRAMATIC intelligence boost.");
    } else if iq_delta > 5.0 {
        println!("  >> RVF learning loop provides a SIGNIFICANT intelligence boost.");
    } else if iq_delta > 1.0 {
        println!("  >> RVF learning loop provides a MEASURABLE intelligence improvement.");
    } else if iq_delta > 0.0 {
        println!("  >> RVF learning loop provides a MARGINAL intelligence gain.");
    } else {
        println!("  >> Performance is comparable. Increase noise or reduce step budget.");
    }
    println!();

    Ok(())
}

fn print_compact_assessment(a: &ruvector_benchmarks::intelligence_metrics::IntelligenceAssessment) {
    println!("  Overall Score: {:.1}/100", a.overall_score);
    println!(
        "  Reasoning:     coherence={:.2}, efficiency={:.2}, error_rate={:.2}",
        a.reasoning.logical_coherence, a.reasoning.reasoning_efficiency, a.reasoning.error_rate,
    );
    println!(
        "  Learning:      sample_eff={:.2}, regret_sub={:.2}, rate={:.2}, gen={:.2}",
        a.learning.sample_efficiency,
        a.learning.regret_sublinearity,
        a.learning.learning_rate,
        a.learning.generalization,
    );
    println!(
        "  Capabilities:  pattern={:.1}, planning={:.1}, adaptation={:.1}",
        a.capabilities.pattern_recognition, a.capabilities.planning, a.capabilities.adaptation,
    );
    println!(
        "  Meta-cog:      self_correct={:.2}, strategy_adapt={:.2}",
        a.meta_cognition.self_correction_rate, a.meta_cognition.strategy_adaptation,
    );
}
