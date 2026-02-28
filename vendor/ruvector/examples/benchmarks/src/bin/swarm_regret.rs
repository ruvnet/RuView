//! Swarm Regret Tracking Runner
//!
//! Track sublinear regret across episodes for swarm controller evaluation.
//!
//! Usage:
//!   cargo run --bin swarm-regret -- --episodes 20 --tasks-per-episode 20

use anyhow::Result;
use clap::Parser;
use ruvector_benchmarks::{
    logging::BenchmarkLogger,
    swarm_regret::SwarmController,
    temporal::TemporalSolver,
    timepuzzles::{PuzzleGenerator, PuzzleGeneratorConfig},
};
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "swarm-regret")]
#[command(about = "Track sublinear regret for swarm controller")]
struct Args {
    /// Number of episodes to run
    #[arg(short, long, default_value = "20")]
    episodes: usize,

    /// Tasks per episode
    #[arg(short, long, default_value = "20")]
    tasks_per_episode: usize,

    /// Enable calendar tool
    #[arg(long, default_value = "true")]
    calendar: bool,

    /// Enable web search tool
    #[arg(long, default_value = "false")]
    web_search: bool,

    /// Maximum steps per task
    #[arg(long, default_value = "100")]
    max_steps: usize,

    /// Random seed
    #[arg(long)]
    seed: Option<u64>,

    /// Output log file
    #[arg(short, long, default_value = "logs/swarm_regret.jsonl")]
    output: String,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║            Swarm Controller Regret Tracking                   ║");
    println!("║          Sublinear Regret for Multi-Agent Control             ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Initialize
    let mut logger = BenchmarkLogger::new(&args.output)?;
    logger.log_system("INFO", "Starting regret tracking", "swarm-regret")?;

    let mut controller = SwarmController::new(args.tasks_per_episode);
    let mut solver = TemporalSolver::with_tools(args.calendar, args.web_search);
    solver.max_steps = args.max_steps;

    let puzzle_config = PuzzleGeneratorConfig {
        min_difficulty: 1,
        max_difficulty: 10,
        constraint_density: 3,
        seed: args.seed,
        ..Default::default()
    };

    println!("🔧 Configuration:");
    println!("   Episodes:         {}", args.episodes);
    println!("   Tasks/episode:    {}", args.tasks_per_episode);
    println!("   Calendar tool:    {}", args.calendar);
    println!("   Web search:       {}", args.web_search);
    println!("   Max steps/task:   {}", args.max_steps);
    println!();

    println!("🏃 Running episodes...");
    println!();
    println!("┌────────┬────────┬─────────┬─────────┬──────────┬───────────┐");
    println!("│Episode │ Acc(%) │  Regret │ Cum.Reg │ Avg.Reg  │ Sublinear │");
    println!("├────────┼────────┼─────────┼─────────┼──────────┼───────────┤");

    let total_start = Instant::now();

    for ep in 0..args.episodes {
        controller.start_episode();

        // Generate puzzles for this episode
        let mut generator = PuzzleGenerator::new(puzzle_config.clone());
        let puzzles = generator.generate_batch(args.tasks_per_episode)?;

        let mut solved = 0;
        let mut correct = 0;
        let mut total_steps = 0;
        let mut total_tool_calls = 0;
        let mut total_latency = 0u64;

        // Solve puzzles
        for puzzle in &puzzles {
            let result = solver.solve(puzzle)?;
            if result.solved {
                solved += 1;
            }
            if result.correct {
                correct += 1;
            }
            total_steps += result.steps;
            total_tool_calls += result.tool_calls;
            total_latency += result.latency_ms;
        }

        // Record episode
        controller.complete_episode(
            solved,
            correct,
            total_steps,
            total_tool_calls,
            total_latency,
        );

        // Get status
        let summary = controller.regret.summary();
        let last_episode = controller.regret.episodes.last().unwrap();

        // Log episode
        logger.log_swarm(
            ep + 1,
            args.tasks_per_episode,
            solved,
            correct,
            last_episode.reward,
            last_episode.oracle_reward,
            summary.total_regret,
            summary.average_regret,
            summary.is_sublinear,
        )?;

        // Print row
        let sublinear = if summary.is_sublinear { "✓" } else { "✗" };
        println!(
            "│ {:6} │ {:5.1}  │ {:7.2} │ {:7.2} │ {:8.4} │     {}     │",
            ep + 1,
            last_episode.accuracy() * 100.0,
            last_episode.regret(),
            summary.total_regret,
            summary.average_regret,
            sublinear
        );
    }

    println!("└────────┴────────┴─────────┴─────────┴──────────┴───────────┘");
    println!();

    let total_time = total_start.elapsed();

    // Final summary
    let summary = controller.regret.summary();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                       Final Summary                           ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("📊 Regret Analysis:");
    println!("   Total episodes:      {}", summary.total_episodes);
    println!("   Cumulative regret:   {:.2}", summary.total_regret);
    println!("   Average regret:      {:.4}", summary.average_regret);
    println!(
        "   Regret trend:        {:.6} ({})",
        summary.regret_trend,
        if summary.regret_trend < 0.0 {
            "decreasing ✓"
        } else {
            "increasing ✗"
        }
    );
    println!(
        "   Sublinear:           {}",
        if summary.is_sublinear {
            "Yes ✓"
        } else {
            "No ✗"
        }
    );
    println!();
    println!("📈 Performance:");
    println!(
        "   Average accuracy:    {:.1}%",
        summary.average_accuracy * 100.0
    );
    println!("   Average reward:      {:.2}", summary.average_reward);
    println!(
        "   Moving avg reward:   {:.2}",
        summary.moving_average_reward
    );
    println!("   Total time:          {:.2}s", total_time.as_secs_f64());
    println!();

    // Regret curve analysis
    if controller.regret.average_regret.len() >= 5 {
        println!("📉 Regret Curve (R_k/k):");
        let regrets = &controller.regret.average_regret;
        let step = regrets.len().max(10) / 10;
        for (i, r) in regrets.iter().enumerate() {
            if i % step == 0 || i == regrets.len() - 1 {
                let bar_len = (r * 50.0).min(50.0) as usize;
                let bar = "█".repeat(bar_len);
                println!("   Episode {:3}: {:.4} {}", i + 1, r, bar);
            }
        }
        println!();
    }

    // Goal check
    println!("🎯 Goal Status:");
    if summary.is_sublinear && summary.regret_trend < 0.0 {
        println!("   ✓ Achieving sublinear regret - average regret trending to zero");
    } else if summary.is_sublinear {
        println!("   ~ Sublinear but trend not clearly decreasing");
    } else {
        println!("   ✗ Not yet achieving sublinear regret");
        println!("   Recommendation: Increase episodes or tune solver parameters");
    }

    // Flush logs
    logger.flush()?;
    println!();
    println!("📝 Results saved to: {}", args.output);

    // Save summary
    let summary_path = args.output.replace(".jsonl", "_summary.json");
    let summary_json = serde_json::to_string_pretty(&summary)?;
    std::fs::write(&summary_path, summary_json)?;
    println!("📝 Summary saved to: {}", summary_path);

    Ok(())
}
