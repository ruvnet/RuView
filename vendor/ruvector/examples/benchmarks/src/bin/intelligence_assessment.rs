//! Intelligence Assessment Runner
//!
//! Runs comprehensive intelligence assessment across all benchmark types.
//!
//! Usage:
//!   cargo run --bin intelligence-assessment -- --episodes 10 --puzzles 50

use anyhow::Result;
use clap::Parser;
use ruvector_benchmarks::{
    intelligence_metrics::{
        print_intelligence_report, DifficultyStats, EpisodeMetrics, IntelligenceCalculator,
        RawMetrics,
    },
    swarm_regret::SwarmController,
    temporal::{AdaptiveSolver, TemporalSolver},
    timepuzzles::{PuzzleGenerator, PuzzleGeneratorConfig},
};

#[derive(Parser, Debug)]
#[command(name = "intelligence-assessment")]
#[command(about = "Run comprehensive intelligence assessment")]
struct Args {
    /// Number of episodes for regret tracking
    #[arg(short, long, default_value = "10")]
    episodes: usize,

    /// Tasks per episode
    #[arg(short, long, default_value = "10")]
    tasks_per_episode: usize,

    /// Enable calendar tool
    #[arg(long, default_value = "true")]
    calendar: bool,

    /// Enable adaptive learning (ReasoningBank)
    #[arg(long, default_value = "true")]
    adaptive: bool,

    /// Random seed
    #[arg(long)]
    seed: Option<u64>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         Comprehensive Intelligence Assessment                 ║");
    println!("║      Measuring Reasoning, Learning & Cognitive Abilities      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Initialize metrics collector
    let mut raw_metrics = RawMetrics::default();

    // Initialize components
    let mut controller = SwarmController::new(args.tasks_per_episode);

    // Choose solver based on adaptive flag
    let mut adaptive_solver = if args.adaptive {
        Some(AdaptiveSolver::new())
    } else {
        None
    };
    let mut basic_solver = if !args.adaptive {
        let mut s = TemporalSolver::with_tools(args.calendar, false);
        s.max_steps = 100;
        Some(s)
    } else {
        None
    };

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
    println!("   Adaptive learning:{}", args.adaptive);
    println!();

    println!("🏃 Running assessment...");
    println!();

    // Run episodes
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

        // Solve puzzles and collect metrics
        for puzzle in &puzzles {
            raw_metrics.tasks_attempted += 1;

            // Use adaptive or basic solver
            let result = if let Some(ref mut solver) = adaptive_solver {
                solver.solve(puzzle)?
            } else if let Some(ref mut solver) = basic_solver {
                solver.solve(puzzle)?
            } else {
                unreachable!()
            };

            if result.solved {
                solved += 1;
                raw_metrics.tasks_completed += 1;
            }
            if result.correct {
                correct += 1;
                raw_metrics.tasks_correct += 1;
            }

            total_steps += result.steps;
            total_tool_calls += result.tool_calls;
            total_latency += result.latency_ms;

            raw_metrics.total_steps += result.steps;
            raw_metrics.total_tool_calls += result.tool_calls;
            raw_metrics.total_latency_ms += result.latency_ms;

            // Track by difficulty
            let entry = raw_metrics
                .by_difficulty
                .entry(puzzle.difficulty)
                .or_insert(DifficultyStats {
                    attempted: 0,
                    completed: 0,
                    correct: 0,
                    avg_steps: 0.0,
                });
            entry.attempted += 1;
            if result.solved {
                entry.completed += 1;
            }
            if result.correct {
                entry.correct += 1;
            }
        }

        // Record episode for swarm controller
        controller.complete_episode(
            solved,
            correct,
            total_steps,
            total_tool_calls,
            total_latency,
        );

        // Record episode metrics
        let episode_accuracy = if args.tasks_per_episode > 0 {
            correct as f64 / args.tasks_per_episode as f64
        } else {
            0.0
        };

        let last_ep = controller.regret.episodes.last().unwrap();
        raw_metrics.episodes.push(EpisodeMetrics {
            episode: ep + 1,
            accuracy: episode_accuracy,
            reward: last_ep.reward,
            regret: last_ep.regret(),
            cumulative_regret: controller.regret.current_cumulative_regret(),
        });

        if args.verbose {
            println!(
                "  Episode {:2}: Accuracy {:.1}%, Regret {:.2}",
                ep + 1,
                episode_accuracy * 100.0,
                last_ep.regret()
            );
        } else {
            print!(".");
            use std::io::Write;
            std::io::stdout().flush()?;
        }
    }

    if !args.verbose {
        println!();
    }
    println!();

    // Update difficulty stats with average steps
    for (_, stats) in raw_metrics.by_difficulty.iter_mut() {
        if stats.attempted > 0 {
            // This is a simplification - we'd need to track this properly
            stats.avg_steps = raw_metrics.total_steps as f64 / raw_metrics.tasks_attempted as f64;
        }
    }

    // Calculate intelligence assessment
    let calculator = IntelligenceCalculator::default();
    let assessment = calculator.calculate(&raw_metrics);

    // Print report
    print_intelligence_report(&assessment);

    // Additional insights
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    Performance Summary                        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    println!("📊 Task Performance:");
    println!("   Tasks Attempted:   {}", raw_metrics.tasks_attempted);
    println!("   Tasks Completed:   {}", raw_metrics.tasks_completed);
    println!("   Tasks Correct:     {}", raw_metrics.tasks_correct);
    println!(
        "   Overall Accuracy:  {:.1}%",
        raw_metrics.tasks_correct as f64 / raw_metrics.tasks_attempted as f64 * 100.0
    );
    println!();

    println!("📈 Learning Progress:");
    let regret_summary = controller.regret.summary();
    println!("   Cumulative Regret: {:.2}", regret_summary.total_regret);
    println!("   Average Regret:    {:.4}", regret_summary.average_regret);
    println!(
        "   Sublinear:         {}",
        if regret_summary.is_sublinear {
            "Yes ✓"
        } else {
            "No ✗"
        }
    );
    println!(
        "   Regret Trend:      {:.4} ({})",
        regret_summary.regret_trend,
        if regret_summary.regret_trend < 0.0 {
            "decreasing ✓"
        } else {
            "increasing ✗"
        }
    );
    println!();

    // Grade the overall performance
    let grade = if assessment.overall_score >= 90.0 {
        "A+ (Excellent)"
    } else if assessment.overall_score >= 80.0 {
        "A (Very Good)"
    } else if assessment.overall_score >= 70.0 {
        "B (Good)"
    } else if assessment.overall_score >= 60.0 {
        "C (Adequate)"
    } else if assessment.overall_score >= 50.0 {
        "D (Below Average)"
    } else {
        "F (Needs Improvement)"
    };

    println!("🎯 Final Grade: {}", grade);
    println!();

    // Recommendations
    println!("💡 Recommendations:");
    if assessment.capabilities.temporal_reasoning < 70.0 {
        println!("   • Improve temporal reasoning with more constraint examples");
    }
    if assessment.learning.regret_sublinearity < 0.5 {
        println!("   • Increase episodes to achieve sublinear regret");
    }
    if assessment.tool_use.utilization_effectiveness < 0.7 {
        println!("   • Better tool selection needed for complex tasks");
    }
    if assessment.meta_cognition.strategy_adaptation < 0.5 {
        println!("   • Enable adaptive strategy switching");
    }
    if assessment.overall_score >= 70.0 {
        println!("   • Good performance! Consider harder difficulty levels");
    }

    // Show adaptive learning progress if enabled
    if let Some(ref solver) = adaptive_solver {
        println!();
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║                 Adaptive Learning Progress                    ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!();

        let progress = solver.learning_progress();
        println!("🧠 ReasoningBank Statistics:");
        println!("   Total trajectories: {}", progress.total_trajectories);
        println!(
            "   Success rate:       {:.1}%",
            progress.success_rate * 100.0
        );
        println!("   Improvement rate:   {:.4}", progress.improvement_rate);
        println!("   Patterns learned:   {}", progress.patterns_learned);
        println!("   Strategies tried:   {}", progress.strategies_tried);
        println!(
            "   Is improving:       {}",
            if progress.is_improving {
                "Yes ✓"
            } else {
                "No ✗"
            }
        );

        // Show learned patterns
        if !solver.reasoning_bank.patterns.is_empty() {
            println!();
            println!("📚 Learned Patterns:");
            for (constraint_type, patterns) in &solver.reasoning_bank.patterns {
                for p in patterns.iter().filter(|p| p.observations >= 3) {
                    println!(
                        "   • {}: {} strategy ({:.0}% success, {} obs)",
                        constraint_type,
                        p.best_strategy,
                        p.success_rate * 100.0,
                        p.observations
                    );
                }
            }
        }

        // Show strategy stats
        if !solver.reasoning_bank.strategy_stats.is_empty() {
            println!();
            println!("📊 Strategy Performance:");
            for (strategy, stats) in &solver.reasoning_bank.strategy_stats {
                println!(
                    "   • {}: {:.1}% success ({} attempts, {:.1} avg steps)",
                    strategy,
                    stats.success_rate() * 100.0,
                    stats.attempts,
                    stats.avg_steps()
                );
            }
        }
    }

    Ok(())
}
