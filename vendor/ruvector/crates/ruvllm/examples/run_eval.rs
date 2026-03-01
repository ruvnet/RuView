#![allow(
    clippy::all,
    unused_imports,
    unused_variables,
    dead_code,
    unused_mut,
    unused_assignments,
    non_camel_case_types,
    clippy::approx_constant,
    unexpected_cfgs,
    unused_must_use,
    unused_parens
)]
//! RuvLLM Evaluation CLI
//!
//! Run real LLM evaluations using SWE-Bench tasks with the full RuvLLM stack.
//!
//! ## Usage
//!
//! ```bash
//! # Run evaluation with a GGUF model on sample tasks
//! cargo run -p ruvllm --example run_eval --features candle -- \
//!   --model ./models/llama-7b-q4.gguf \
//!   --tasks sample
//!
//! # Run on SWE-bench-lite (downloads and caches)
//! cargo run -p ruvllm --example run_eval --features candle -- \
//!   --model ./models/llama-7b-q4.gguf \
//!   --tasks swe-bench-lite \
//!   --max-tasks 50
//!
//! # Run with specific ablation modes
//! cargo run -p ruvllm --example run_eval --features candle -- \
//!   --model ./models/llama-7b-q4.gguf \
//!   --tasks sample \
//!   --modes baseline,full
//!
//! # Run on local JSON file
//! cargo run -p ruvllm --example run_eval --features candle -- \
//!   --model ./models/llama-7b-q4.gguf \
//!   --tasks ./my-tasks.json \
//!   --output ./results.json
//! ```
//!
//! ## Environment Variables
//!
//! - `RUVLLM_MODELS_DIR`: Default directory for model files
//! - `RUVLLM_CACHE_DIR`: Cache directory for downloaded datasets

use ruvllm::backends::ModelConfig;
use ruvllm::evaluation::{
    swe_bench::{SweBenchConfig, SweBenchLoader},
    AblationMode, EvalConfig, EvalTask, RealEvaluationHarness, RealInferenceConfig,
};
use std::env;
use std::path::PathBuf;
use std::process;

fn main() {
    // Initialize logging
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        print_help();
        return;
    }

    // Parse arguments
    let config = match parse_args(&args[1..]) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("\nRun with --help for usage information.");
            process::exit(1);
        }
    };

    // Run evaluation
    if let Err(e) = run_evaluation(config) {
        eprintln!("Evaluation failed: {}", e);
        process::exit(1);
    }
}

fn print_help() {
    println!(
        r#"RuvLLM Evaluation CLI

Run real LLM evaluations on SWE-Bench tasks with SONA learning and HNSW routing.

USAGE:
    run_eval [OPTIONS] --model <PATH>

OPTIONS:
    --model <PATH>          Path to GGUF model file (required)
    --tasks <SOURCE>        Task source: sample, swe-bench-lite, swe-bench, or file path
                            (default: sample)
    --max-tasks <N>         Maximum number of tasks to evaluate (default: all)
    --modes <MODES>         Comma-separated ablation modes (default: all)
                            Options: baseline, retrieval, adapters, retrieval+adapters, full
    --seeds <SEEDS>         Comma-separated random seeds (default: 42,123,456)
    --output <PATH>         Output file for results JSON (default: stdout summary)
    --quality-threshold <F> Minimum quality score for acceptance (default: 0.7)
    --cost-target <F>       Target cost per patch in dollars (default: 0.10)
    --no-sona               Disable SONA learning
    --no-hnsw               Disable HNSW routing
    --repo <NAME>           Filter tasks by repository name
    --verbose               Enable verbose output
    -h, --help              Show this help message

EXAMPLES:
    # Quick test with sample tasks
    run_eval --model ./model.gguf --tasks sample

    # Run SWE-bench-lite evaluation
    run_eval --model ./model.gguf --tasks swe-bench-lite --max-tasks 100

    # Compare baseline vs full mode
    run_eval --model ./model.gguf --modes baseline,full --output results.json

    # Run on custom task file
    run_eval --model ./model.gguf --tasks ./my-tasks.json --verbose
"#
    );
}

#[derive(Debug)]
struct CliConfig {
    model_path: PathBuf,
    task_source: TaskSource,
    max_tasks: Option<usize>,
    ablation_modes: Vec<AblationMode>,
    seeds: Vec<u64>,
    output_path: Option<PathBuf>,
    quality_threshold: f64,
    cost_target: f64,
    enable_sona: bool,
    enable_hnsw: bool,
    repo_filter: Option<String>,
    verbose: bool,
}

#[derive(Debug)]
enum TaskSource {
    Sample,
    SweBenchLite,
    SweBenchFull,
    File(PathBuf),
}

fn parse_args(args: &[String]) -> Result<CliConfig, String> {
    let mut model_path: Option<PathBuf> = None;
    let mut task_source = TaskSource::Sample;
    let mut max_tasks = None;
    let mut ablation_modes = Vec::new();
    let mut seeds = vec![42, 123, 456];
    let mut output_path = None;
    let mut quality_threshold = 0.7;
    let mut cost_target = 0.10;
    let mut enable_sona = true;
    let mut enable_hnsw = true;
    let mut repo_filter = None;
    let mut verbose = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--model" => {
                i += 1;
                model_path = Some(PathBuf::from(args.get(i).ok_or("--model requires a path")?));
            }
            "--tasks" => {
                i += 1;
                let source = args.get(i).ok_or("--tasks requires a value")?;
                task_source = match source.as_str() {
                    "sample" => TaskSource::Sample,
                    "swe-bench-lite" => TaskSource::SweBenchLite,
                    "swe-bench" => TaskSource::SweBenchFull,
                    path => TaskSource::File(PathBuf::from(path)),
                };
            }
            "--max-tasks" => {
                i += 1;
                let n: usize = args
                    .get(i)
                    .ok_or("--max-tasks requires a number")?
                    .parse()
                    .map_err(|_| "Invalid number for --max-tasks")?;
                max_tasks = Some(n);
            }
            "--modes" => {
                i += 1;
                let modes_str = args.get(i).ok_or("--modes requires a value")?;
                ablation_modes = parse_modes(modes_str)?;
            }
            "--seeds" => {
                i += 1;
                let seeds_str = args.get(i).ok_or("--seeds requires a value")?;
                seeds = seeds_str
                    .split(',')
                    .map(|s| s.trim().parse().map_err(|_| "Invalid seed"))
                    .collect::<Result<Vec<_>, _>>()?;
            }
            "--output" => {
                i += 1;
                output_path = Some(PathBuf::from(
                    args.get(i).ok_or("--output requires a path")?,
                ));
            }
            "--quality-threshold" => {
                i += 1;
                quality_threshold = args
                    .get(i)
                    .ok_or("--quality-threshold requires a value")?
                    .parse()
                    .map_err(|_| "Invalid quality threshold")?;
            }
            "--cost-target" => {
                i += 1;
                cost_target = args
                    .get(i)
                    .ok_or("--cost-target requires a value")?
                    .parse()
                    .map_err(|_| "Invalid cost target")?;
            }
            "--repo" => {
                i += 1;
                repo_filter = Some(args.get(i).ok_or("--repo requires a value")?.clone());
            }
            "--no-sona" => enable_sona = false,
            "--no-hnsw" => enable_hnsw = false,
            "--verbose" => verbose = true,
            arg => {
                if arg.starts_with('-') {
                    return Err(format!("Unknown option: {}", arg));
                }
            }
        }
        i += 1;
    }

    let model_path = model_path.ok_or("--model is required")?;

    // Default to all modes if none specified
    if ablation_modes.is_empty() {
        ablation_modes = vec![
            AblationMode::Baseline,
            AblationMode::RetrievalOnly,
            AblationMode::AdaptersOnly,
            AblationMode::RetrievalPlusAdapters,
            AblationMode::Full,
        ];
    }

    Ok(CliConfig {
        model_path,
        task_source,
        max_tasks,
        ablation_modes,
        seeds,
        output_path,
        quality_threshold,
        cost_target,
        enable_sona,
        enable_hnsw,
        repo_filter,
        verbose,
    })
}

fn parse_modes(modes_str: &str) -> Result<Vec<AblationMode>, String> {
    modes_str
        .split(',')
        .map(|s| match s.trim().to_lowercase().as_str() {
            "baseline" => Ok(AblationMode::Baseline),
            "retrieval" | "retrieval-only" | "retrieval_only" => Ok(AblationMode::RetrievalOnly),
            "adapters" | "adapters-only" | "adapters_only" => Ok(AblationMode::AdaptersOnly),
            "retrieval+adapters" | "retrieval_plus_adapters" => {
                Ok(AblationMode::RetrievalPlusAdapters)
            }
            "full" => Ok(AblationMode::Full),
            other => Err(format!("Unknown ablation mode: {}", other)),
        })
        .collect()
}

fn run_evaluation(config: CliConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!("RuvLLM Evaluation");
    println!("=================\n");

    // Verify model exists
    if !config.model_path.exists() {
        return Err(format!("Model not found: {}", config.model_path.display()).into());
    }
    println!("Model: {}", config.model_path.display());

    // Load tasks
    println!("\nLoading tasks...");
    let tasks = load_tasks(&config)?;
    println!("Loaded {} tasks", tasks.len());

    if config.verbose {
        for task in tasks.iter().take(5) {
            println!("  - {} ({})", task.id, task.repo);
        }
        if tasks.len() > 5 {
            println!("  ... and {} more", tasks.len() - 5);
        }
    }

    // Configure evaluation
    let eval_config = EvalConfig {
        task_count: config.max_tasks.unwrap_or(tasks.len()),
        seeds: config.seeds.clone(),
        ablation_modes: config.ablation_modes.clone(),
        quality_threshold: config.quality_threshold,
        cost_target: config.cost_target,
        ..Default::default()
    };

    println!("\nConfiguration:");
    println!("  Tasks: {}", eval_config.task_count);
    println!("  Seeds: {:?}", eval_config.seeds);
    println!(
        "  Modes: {:?}",
        eval_config
            .ablation_modes
            .iter()
            .map(|m| m.name())
            .collect::<Vec<_>>()
    );
    println!(
        "  Quality threshold: {:.0}%",
        eval_config.quality_threshold * 100.0
    );
    println!(
        "  SONA: {}",
        if config.enable_sona {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!(
        "  HNSW: {}",
        if config.enable_hnsw {
            "enabled"
        } else {
            "disabled"
        }
    );

    // Configure inference
    let inference_config = RealInferenceConfig {
        model_path: config.model_path.to_string_lossy().to_string(),
        model_config: ModelConfig::default(),
        enable_sona: config.enable_sona,
        enable_hnsw: config.enable_hnsw,
        ..Default::default()
    };

    // Create harness
    println!("\nInitializing evaluation harness...");
    let mut harness = RealEvaluationHarness::with_config(eval_config, inference_config)?;

    // Check if model loaded
    if !harness.is_model_loaded() {
        return Err("Failed to load model".into());
    }
    println!("Model loaded successfully!");

    // Run evaluation
    println!("\nRunning evaluation...");
    println!("This may take a while depending on model size and task count.\n");

    let runtime = tokio::runtime::Runtime::new()?;
    let report = runtime.block_on(harness.run_evaluation(&tasks))?;

    // Output results
    println!("\n{}", "=".repeat(60));
    println!("EVALUATION COMPLETE");
    println!("{}\n", "=".repeat(60));

    // Print summary
    println!("{}", report.summary());
    println!();

    // Print leaderboard
    println!("Leaderboard:");
    println!("{:-<60}", "");
    println!(
        "{:<5} {:<20} {:>10} {:>10} {:>10}",
        "Rank", "Mode", "Success%", "Quality", "$/patch"
    );
    println!("{:-<60}", "");

    for entry in report.to_leaderboard_entries() {
        println!(
            "{:<5} {:<20} {:>9.1}% {:>10.2} {:>10.4}",
            entry.rank,
            entry.mode.name(),
            entry.success_rate * 100.0,
            entry.quality_score,
            entry.cost_per_patch
        );
    }
    println!();

    // Print ablation analysis
    println!("Ablation Analysis vs Baseline:");
    for comparison in report.compare_all_to_baseline() {
        let direction = if comparison.success_delta > 0.0 {
            "+"
        } else {
            ""
        };
        let sig = if comparison.is_significant { "*" } else { "" };
        println!(
            "  {}: {}{:.1}%{} success rate",
            comparison.target.name(),
            direction,
            comparison.success_delta * 100.0,
            sig
        );
    }

    // Save to file if requested
    if let Some(output_path) = config.output_path {
        println!("\nSaving results to {}...", output_path.display());
        let json = report.to_json()?;
        std::fs::write(&output_path, json)?;
        println!("Results saved!");

        // Also save markdown report
        let md_path = output_path.with_extension("md");
        std::fs::write(&md_path, report.to_markdown())?;
        println!("Markdown report saved to {}", md_path.display());
    }

    Ok(())
}

fn load_tasks(config: &CliConfig) -> Result<Vec<EvalTask>, Box<dyn std::error::Error>> {
    let swe_config = SweBenchConfig {
        max_tasks: config.max_tasks,
        repo_filter: config.repo_filter.clone(),
        ..Default::default()
    };

    let loader = SweBenchLoader::new(swe_config);

    let tasks: Vec<EvalTask> = match &config.task_source {
        TaskSource::Sample => {
            println!("Using sample tasks (3 tasks)");
            SweBenchLoader::sample_tasks()
                .into_iter()
                .map(|t| t.into())
                .collect()
        }
        TaskSource::SweBenchLite => {
            println!("Loading SWE-bench-lite dataset...");
            // For now, use sample tasks since we don't have async download in sync context
            // In a real implementation, we'd use tokio::runtime to download
            println!("Note: Using sample tasks. Run with async for full dataset download.");
            SweBenchLoader::sample_tasks()
                .into_iter()
                .map(|t| t.into())
                .collect()
        }
        TaskSource::SweBenchFull => {
            println!("Loading full SWE-bench dataset...");
            println!("Note: Using sample tasks. Run with async for full dataset download.");
            SweBenchLoader::sample_tasks()
                .into_iter()
                .map(|t| t.into())
                .collect()
        }
        TaskSource::File(path) => {
            println!("Loading tasks from {}...", path.display());
            let swe_tasks = if path.extension().map_or(false, |e| e == "jsonl") {
                loader.load_from_jsonl(path)?
            } else {
                loader.load_from_file(path)?
            };

            // Print stats
            let stats = SweBenchLoader::stats(&swe_tasks);
            if config.verbose {
                println!("{}", stats);
            }

            swe_tasks.into_iter().map(|t| t.into()).collect()
        }
    };

    // Apply max_tasks filter
    let tasks = if let Some(max) = config.max_tasks {
        tasks.into_iter().take(max).collect()
    } else {
        tasks
    };

    Ok(tasks)
}
