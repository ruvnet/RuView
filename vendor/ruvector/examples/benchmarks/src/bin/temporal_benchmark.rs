//! Temporal Benchmark Runner
//!
//! Run temporal reasoning benchmarks based on TimePuzzles methodology.
//!
//! Usage:
//!   cargo run --bin temporal-benchmark -- --puzzles 50 --calendar --web-search

use anyhow::Result;
use clap::Parser;
use ruvector_benchmarks::{
    logging::BenchmarkLogger,
    temporal::{BenchmarkConfig, BenchmarkResults, TemporalSolver},
    timepuzzles::{PuzzleGenerator, PuzzleGeneratorConfig, SamplePuzzles},
};
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "temporal-benchmark")]
#[command(about = "Run temporal reasoning benchmarks")]
struct Args {
    /// Number of puzzles to run
    #[arg(short = 'n', long, default_value = "50")]
    puzzles: usize,

    /// Minimum difficulty (1-10)
    #[arg(long, default_value = "1")]
    min_difficulty: u8,

    /// Maximum difficulty (1-10)
    #[arg(long, default_value = "10")]
    max_difficulty: u8,

    /// Enable calendar math tool
    #[arg(long, default_value = "true")]
    calendar: bool,

    /// Enable web search tool
    #[arg(long, default_value = "false")]
    web_search: bool,

    /// Maximum steps per puzzle
    #[arg(long, default_value = "100")]
    max_steps: usize,

    /// Constraint density (1-5)
    #[arg(long, default_value = "3")]
    constraint_density: u8,

    /// Random seed for reproducibility
    #[arg(long)]
    seed: Option<u64>,

    /// Output log file
    #[arg(short, long, default_value = "logs/temporal_benchmark.jsonl")]
    output: String,

    /// Use sample puzzles instead of generating
    #[arg(long)]
    use_samples: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           Temporal Reasoning Benchmark Runner                 ║");
    println!("║         Based on TimePuzzles (arXiv:2601.07148)              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Initialize logger
    let mut logger = BenchmarkLogger::new(&args.output)?;
    logger.log_system("INFO", "Starting benchmark run", "temporal-benchmark")?;

    // Generate or load puzzles
    let puzzles = if args.use_samples {
        println!("📚 Using sample puzzle set (50 puzzles)...");
        SamplePuzzles::mixed_sample()
    } else {
        println!(
            "🎲 Generating {} puzzles (difficulty {}-{})...",
            args.puzzles, args.min_difficulty, args.max_difficulty
        );

        let config = PuzzleGeneratorConfig {
            min_difficulty: args.min_difficulty,
            max_difficulty: args.max_difficulty,
            constraint_density: args.constraint_density,
            cross_cultural: true,
            relative_constraints: true,
            year_range: (2000, 2030),
            seed: args.seed,
        };

        let mut generator = PuzzleGenerator::new(config);
        generator.generate_batch(args.puzzles)?
    };

    println!("✓ Loaded {} puzzles", puzzles.len());
    println!();

    // Configure solver
    let mut solver = TemporalSolver::with_tools(args.calendar, args.web_search);
    solver.max_steps = args.max_steps;

    println!("🔧 Solver configuration:");
    println!("   Calendar tool: {}", args.calendar);
    println!("   Web search:    {}", args.web_search);
    println!("   Max steps:     {}", args.max_steps);
    println!();

    // Run benchmarks
    println!("🏃 Running benchmarks...");
    println!();

    let benchmark_id = format!(
        "bench-{}-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S"),
        args.seed.unwrap_or(0)
    );

    let mut results = Vec::new();
    let start = Instant::now();

    for (i, puzzle) in puzzles.iter().enumerate() {
        let result = solver.solve(puzzle)?;

        // Log result
        logger.log_temporal(
            &benchmark_id,
            &puzzle.id,
            puzzle.difficulty,
            result.solved,
            result.correct,
            result.steps,
            result.tool_calls,
            result.latency_ms,
            puzzle.constraints.len(),
            args.calendar,
            args.web_search,
        )?;

        if args.verbose {
            let status = if result.correct {
                "✓"
            } else if result.solved {
                "~"
            } else {
                "✗"
            };
            println!(
                "  {} Puzzle {:3}: {} (steps: {}, latency: {}ms)",
                status,
                i + 1,
                puzzle.id,
                result.steps,
                result.latency_ms
            );
        } else if (i + 1) % 10 == 0 {
            print!(".");
            use std::io::Write;
            std::io::stdout().flush()?;
        }

        results.push(result);
    }

    let total_time = start.elapsed();

    if !args.verbose {
        println!();
    }
    println!();

    // Compute aggregate results
    let config = BenchmarkConfig {
        num_puzzles: puzzles.len(),
        difficulty_range: (args.min_difficulty, args.max_difficulty),
        calendar_tool: args.calendar,
        web_search_tool: args.web_search,
        max_steps: args.max_steps,
        constraint_density: args.constraint_density,
    };

    let benchmark_results = BenchmarkResults::from_results(config, results);

    // Print results
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                     Benchmark Results                         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("📊 Summary:");
    println!("   Total puzzles:  {}", benchmark_results.total_puzzles);
    println!("   Solved:         {}", benchmark_results.solved_count);
    println!("   Correct:        {}", benchmark_results.correct_count);
    println!(
        "   Accuracy:       {:.1}%",
        benchmark_results.accuracy * 100.0
    );
    println!();
    println!("⏱️  Performance:");
    println!("   Avg steps:      {:.1}", benchmark_results.avg_steps);
    println!("   Avg tool calls: {:.1}", benchmark_results.avg_tool_calls);
    println!(
        "   Avg latency:    {:.1}ms",
        benchmark_results.avg_latency_ms
    );
    println!("   Total time:     {:.2}s", total_time.as_secs_f64());
    println!();

    // Compute accuracy by difficulty
    let mut by_difficulty: std::collections::HashMap<u8, (usize, usize)> =
        std::collections::HashMap::new();
    for (puzzle, result) in puzzles.iter().zip(benchmark_results.results.iter()) {
        let entry = by_difficulty.entry(puzzle.difficulty).or_insert((0, 0));
        entry.0 += 1;
        if result.correct {
            entry.1 += 1;
        }
    }

    println!("📈 Accuracy by Difficulty:");
    let mut difficulties: Vec<_> = by_difficulty.keys().copied().collect();
    difficulties.sort();
    for d in difficulties {
        let (total, correct) = by_difficulty[&d];
        let acc = correct as f64 / total as f64 * 100.0;
        println!("   Difficulty {}: {:5.1}% ({}/{})", d, acc, correct, total);
    }
    println!();

    // Tool usage analysis
    if args.calendar {
        let with_rewriting = benchmark_results
            .results
            .iter()
            .filter(|r| r.tool_calls > 0 && r.correct)
            .count();
        println!("🔧 Tool Analysis:");
        println!(
            "   Calendar rewriting success: {}/{}",
            with_rewriting, benchmark_results.total_puzzles
        );
    }

    // Flush logs
    logger.flush()?;
    println!();
    println!("📝 Results saved to: {}", args.output);

    // Save full results as JSON
    let results_path = args.output.replace(".jsonl", "_summary.json");
    let results_json = serde_json::to_string_pretty(&benchmark_results)?;
    std::fs::write(&results_path, results_json)?;
    println!("📝 Summary saved to: {}", results_path);

    Ok(())
}
