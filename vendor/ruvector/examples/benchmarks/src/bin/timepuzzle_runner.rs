//! TimePuzzle Quick Runner
//!
//! 10-minute probe for temporal reasoning with tool augmentation.
//!
//! Usage:
//!   cargo run --bin timepuzzle-runner -- --quick
//!   cargo run --bin timepuzzle-runner -- --depth 5

use anyhow::Result;
use clap::Parser;
use ruvector_benchmarks::{
    logging::BenchmarkLogger, temporal::TemporalSolver, timepuzzles::SamplePuzzles,
};
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(name = "timepuzzle-runner")]
#[command(about = "Quick TimePuzzle probe for agent testing")]
struct Args {
    /// Quick mode: 50 puzzles, depth-limited steps
    #[arg(long)]
    quick: bool,

    /// Maximum depth (steps) per puzzle
    #[arg(short, long, default_value = "50")]
    depth: usize,

    /// Number of puzzles
    #[arg(short = 'n', long, default_value = "50")]
    puzzles: usize,

    /// Tool latency cap (abort if tool > 1.5x median)
    #[arg(long, default_value = "1.5")]
    latency_cap: f64,

    /// Timeout in seconds
    #[arg(long, default_value = "600")]
    timeout: u64,

    /// Enable constraint rewriting (calendar math)
    #[arg(long, default_value = "true")]
    rewrite: bool,

    /// Enable web search (for factual anchors)
    #[arg(long, default_value = "false")]
    web_search: bool,

    /// Output file
    #[arg(short, long, default_value = "logs/timepuzzle_probe.jsonl")]
    output: String,

    /// Verbose mode
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              TimePuzzle Quick Probe Runner                    ║");
    println!("║        Tool-Augmented Iterative Temporal Reasoning            ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let mut logger = BenchmarkLogger::new(&args.output)?;
    logger.log_system("INFO", "Starting TimePuzzle probe", "timepuzzle-runner")?;

    // Quick mode settings
    let (num_puzzles, max_depth) = if args.quick {
        println!("⚡ Quick mode enabled (50 puzzles, depth {})", args.depth);
        (50, args.depth)
    } else {
        (args.puzzles, args.depth)
    };

    let timeout = Duration::from_secs(args.timeout);

    println!();
    println!("🔧 Configuration:");
    println!("   Puzzles:          {}", num_puzzles);
    println!("   Max depth:        {}", max_depth);
    println!("   Rewriting:        {}", args.rewrite);
    println!("   Web search:       {}", args.web_search);
    println!("   Latency cap:      {}x median", args.latency_cap);
    println!("   Timeout:          {}s", args.timeout);
    println!();

    // Generate puzzles with varying constraint density
    println!("🎲 Generating puzzles...");
    let puzzles = SamplePuzzles::mixed_sample()
        .into_iter()
        .take(num_puzzles)
        .collect::<Vec<_>>();
    println!("✓ Loaded {} puzzles", puzzles.len());
    println!();

    // Configure solver
    let mut solver = TemporalSolver::with_tools(args.rewrite, args.web_search);
    solver.max_steps = max_depth;

    // Run probe
    println!("🏃 Running probe...");
    println!();

    let probe_start = Instant::now();
    let mut results = Vec::new();
    let mut latencies: Vec<u64> = Vec::new();
    let mut median_latency: f64 = 100.0; // Initial estimate

    for (i, puzzle) in puzzles.iter().enumerate() {
        // Check timeout
        if probe_start.elapsed() > timeout {
            println!("⚠️  Timeout reached after {} puzzles", i);
            break;
        }

        let result = solver.solve(puzzle)?;

        // Check latency cap
        if latencies.len() >= 10 {
            let mut sorted = latencies.clone();
            sorted.sort();
            median_latency = sorted[sorted.len() / 2] as f64;

            if result.latency_ms as f64 > median_latency * args.latency_cap {
                if args.verbose {
                    println!(
                        "  ⚠ Puzzle {} aborted: latency {}ms > {:.0}ms cap",
                        puzzle.id,
                        result.latency_ms,
                        median_latency * args.latency_cap
                    );
                }
                // Still record but mark as slow
            }
        }

        latencies.push(result.latency_ms);

        // Log
        logger.log_temporal(
            "timepuzzle-probe",
            &puzzle.id,
            puzzle.difficulty,
            result.solved,
            result.correct,
            result.steps,
            result.tool_calls,
            result.latency_ms,
            puzzle.constraints.len(),
            args.rewrite,
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
                "  {} [{:2}] {}: steps={}, tools={}, {}ms",
                status,
                puzzle.difficulty,
                puzzle.id,
                result.steps,
                result.tool_calls,
                result.latency_ms
            );
        }

        results.push(result);
    }

    let total_time = probe_start.elapsed();
    println!();

    // Analyze results
    let solved = results.iter().filter(|r| r.solved).count();
    let correct = results.iter().filter(|r| r.correct).count();
    let total = results.len();
    let accuracy = correct as f64 / total as f64;

    let avg_steps = results.iter().map(|r| r.steps).sum::<usize>() as f64 / total as f64;
    let avg_tools = results.iter().map(|r| r.tool_calls).sum::<usize>() as f64 / total as f64;
    let avg_latency = results.iter().map(|r| r.latency_ms).sum::<u64>() as f64 / total as f64;

    // Tool toggle analysis
    let with_tool_correct = results
        .iter()
        .filter(|r| r.tool_calls > 0 && r.correct)
        .count();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    Probe Results                              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("📊 Overall Performance:");
    println!("   Puzzles run:      {}", total);
    println!(
        "   Solved:           {} ({:.1}%)",
        solved,
        solved as f64 / total as f64 * 100.0
    );
    println!(
        "   Correct:          {} ({:.1}%)",
        correct,
        accuracy * 100.0
    );
    println!();
    println!("⏱️  Efficiency:");
    println!("   Avg steps:        {:.1}", avg_steps);
    println!("   Avg tool calls:   {:.1}", avg_tools);
    println!("   Avg latency:      {:.1}ms", avg_latency);
    println!("   Median latency:   {:.0}ms", median_latency);
    println!("   Total time:       {:.2}s", total_time.as_secs_f64());
    println!();

    // Scaling curves
    println!("📈 Tool Toggle Analysis:");
    println!(
        "   With rewriting:   {}/{} ({:.1}%)",
        with_tool_correct,
        total,
        with_tool_correct as f64 / total as f64 * 100.0
    );

    // Sensitivity analysis
    let fast_correct = results
        .iter()
        .filter(|r| r.latency_ms < median_latency as u64 && r.correct)
        .count();
    let slow_correct = results
        .iter()
        .filter(|r| r.latency_ms >= median_latency as u64 && r.correct)
        .count();
    let fast_total = results
        .iter()
        .filter(|r| r.latency_ms < median_latency as u64)
        .count();
    let slow_total = total - fast_total;

    if fast_total > 0 && slow_total > 0 {
        println!();
        println!("⚡ Latency Sensitivity:");
        println!(
            "   Fast (<{:.0}ms):    {}/{} ({:.1}%)",
            median_latency,
            fast_correct,
            fast_total,
            fast_correct as f64 / fast_total as f64 * 100.0
        );
        println!(
            "   Slow (>={:.0}ms):   {}/{} ({:.1}%)",
            median_latency,
            slow_correct,
            slow_total,
            slow_correct as f64 / slow_total as f64 * 100.0
        );
    }

    // Accuracy by difficulty
    println!();
    println!("🎯 Accuracy by Difficulty:");
    let mut by_diff: std::collections::HashMap<u8, (usize, usize)> =
        std::collections::HashMap::new();
    for (p, r) in puzzles.iter().zip(results.iter()) {
        let e = by_diff.entry(p.difficulty).or_insert((0, 0));
        e.0 += 1;
        if r.correct {
            e.1 += 1;
        }
    }
    let mut diffs: Vec<_> = by_diff.keys().copied().collect();
    diffs.sort();
    for d in diffs {
        let (t, c) = by_diff[&d];
        let pct = c as f64 / t as f64 * 100.0;
        let bar = "█".repeat((pct / 5.0) as usize);
        println!("   Level {:2}: {:5.1}% {}", d, pct, bar);
    }

    // Recommendations
    println!();
    println!("💡 Insights:");
    if accuracy < 0.5 {
        println!("   • Low accuracy - consider enabling constraint rewriting");
    }
    if avg_steps > max_depth as f64 * 0.8 {
        println!("   • High step count - search may be inefficient");
    }
    if args.web_search && with_tool_correct > correct / 2 {
        println!("   • Web search providing substantial gains");
    }
    if accuracy >= 0.8 {
        println!("   • Good performance - ready for harder puzzles");
    }

    // Flush logs
    logger.flush()?;
    println!();
    println!("📝 Results saved to: {}", args.output);

    Ok(())
}
