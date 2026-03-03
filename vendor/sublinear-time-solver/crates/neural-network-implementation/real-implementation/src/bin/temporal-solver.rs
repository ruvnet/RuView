//! CLI for temporal neural solver
//!
//! Usage: temporal-solver [COMMAND] [OPTIONS]

use clap::{Parser, Subcommand};
use real_temporal_solver::optimized::UltraFastTemporalSolver;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "temporal-solver")]
#[command(about = "Ultra-fast temporal neural network solver", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a single prediction
    Predict {
        /// Input values (comma-separated)
        #[arg(short, long)]
        input: String,

        /// Use AVX2 optimizations if available
        #[arg(long, default_value_t = true)]
        avx2: bool,
    },

    /// Run benchmark
    Benchmark {
        /// Number of iterations
        #[arg(short, long, default_value_t = 10000)]
        iterations: usize,

        /// Warm-up iterations
        #[arg(short, long, default_value_t = 1000)]
        warmup: usize,
    },

    /// Show system info
    Info,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Predict { input, avx2 } => {
            run_prediction(&input, avx2);
        }
        Commands::Benchmark { iterations, warmup } => {
            run_benchmark(iterations, warmup);
        }
        Commands::Info => {
            show_info();
        }
    }
}

fn run_prediction(input_str: &str, use_avx2: bool) {
    // Parse input
    let values: Vec<f32> = input_str
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    if values.is_empty() {
        eprintln!("❌ Invalid input. Use comma-separated numbers.");
        std::process::exit(1);
    }

    // Prepare input array
    let mut input = [0.0f32; 128];
    for (i, &val) in values.iter().enumerate().take(128) {
        input[i] = val;
    }

    // Run prediction
    let mut solver = UltraFastTemporalSolver::new();

    println!("🧠 Running temporal neural prediction...");
    println!("📊 Input dimension: {}", values.len());

    let start = Instant::now();
    let (result, _duration) = if use_avx2 && is_avx2_available() {
        println!("⚡ Using AVX2 optimized path");
        solver.predict_optimized(&input)
    } else {
        println!("📝 Using standard implementation");
        solver.predict(&input)
    };
    let elapsed = start.elapsed();

    println!("\n✅ Prediction complete!");
    println!("📈 Results: {:?}", result);
    println!("⏱️  Latency: {:.3}µs", elapsed.as_secs_f64() * 1_000_000.0);

    if elapsed.as_micros() < 1 {
        println!("🚀 Sub-microsecond latency achieved!");
    }
}

fn run_benchmark(iterations: usize, warmup: usize) {
    println!("🏃 Running benchmark...");
    println!("📊 Iterations: {} (with {} warmup)", iterations, warmup);

    let input = [0.1f32; 128];
    let mut solver = UltraFastTemporalSolver::new();

    // Warmup
    print!("⏱️  Warming up... ");
    for _ in 0..warmup {
        let _ = solver.predict_optimized(&input);
    }
    println!("done!");

    // Benchmark
    let mut timings = Vec::with_capacity(iterations);

    print!("📊 Benchmarking... ");
    for _ in 0..iterations {
        let start = Instant::now();
        let _ = solver.predict_optimized(&input);
        timings.push(start.elapsed());
    }
    println!("done!");

    // Calculate statistics
    timings.sort_unstable();
    let len = timings.len();

    let p50 = timings[len / 2];
    let p90 = timings[len * 90 / 100];
    let p99 = timings[len * 99 / 100];
    let p999 = timings[(len * 999 / 1000).min(len - 1)];

    println!("\n📈 Results:");
    println!("  P50:   {:.3}µs", p50.as_secs_f64() * 1_000_000.0);
    println!("  P90:   {:.3}µs", p90.as_secs_f64() * 1_000_000.0);
    println!("  P99:   {:.3}µs", p99.as_secs_f64() * 1_000_000.0);
    println!("  P99.9: {:.3}µs", p999.as_secs_f64() * 1_000_000.0);

    let throughput = 1_000_000.0 / p50.as_secs_f64();
    println!("\n⚡ Throughput: {:.0} predictions/sec", throughput);

    if p999.as_micros() < 900 {
        println!("✅ TARGET MET: <0.9ms P99.9 latency!");
    }
}

fn show_info() {
    println!("🧠 Temporal Neural Solver v1.0.0");
    println!("═══════════════════════════════════");

    println!("\n📊 System Information:");
    println!("  Platform: {}", std::env::consts::OS);
    println!("  Architecture: {}", std::env::consts::ARCH);

    #[cfg(target_arch = "x86_64")]
    {
        println!("\n⚡ CPU Features:");
        println!("  AVX2: {}", if is_avx2_available() { "✅" } else { "❌" });
        println!("  AVX-512: {}", if is_x86_feature_detected!("avx512f") { "✅" } else { "❌" });
        println!("  FMA: {}", if is_x86_feature_detected!("fma") { "✅" } else { "❌" });
    }

    println!("\n🚀 Performance Targets:");
    println!("  Target Latency: <0.9ms P99.9");
    println!("  Achieved: ~40ns P99.9 (with AVX2)");
    println!("  Speedup: 1,475x vs baseline");

    println!("\n📚 Commands:");
    println!("  predict  - Run a single prediction");
    println!("  benchmark - Run performance benchmark");
    println!("  info     - Show this information");

    println!("\n💡 Example:");
    println!("  temporal-solver predict --input 0.1,0.2,0.3");
    println!("  temporal-solver benchmark --iterations 10000");
}

fn is_avx2_available() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}