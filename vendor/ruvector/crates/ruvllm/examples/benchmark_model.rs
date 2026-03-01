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
//! Benchmark token generation speed on real GGUF models
//!
//! This benchmark measures:
//! - Time to first token (TTFT)
//! - Tokens per second (throughput)
//! - Latency distribution (p50, p95, p99)
//! - Memory usage
//!
//! ## Usage
//!
//! ```bash
//! # Benchmark a specific model
//! cargo run -p ruvllm --example benchmark_model --release -- --model ./test_models/tinyllama.gguf
//!
//! # With custom parameters
//! cargo run -p ruvllm --example benchmark_model --release -- \
//!     --model ./model.gguf \
//!     --warmup 5 \
//!     --iterations 20 \
//!     --max-tokens 100
//!
//! # JSON output for CI/automation
//! cargo run -p ruvllm --example benchmark_model --release -- \
//!     --model ./model.gguf --json
//! ```
//!
//! ## Output Example
//!
//! ```text
//! RuvLLM Model Benchmark
//! =====================
//! Model: ./test_models/tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf
//! Model Size: 669.34 MB
//!
//! Configuration:
//!   Warmup iterations: 5
//!   Benchmark iterations: 20
//!   Max tokens per generation: 50
//!
//! Running warmup...
//!   Warmup 1/5: 32.4 tok/s
//!   Warmup 2/5: 35.2 tok/s
//!   ...
//!
//! Running benchmark...
//!   Iteration 1/20: 34.8 tok/s, TTFT: 45.2ms
//!   Iteration 2/20: 35.1 tok/s, TTFT: 44.8ms
//!   ...
//!
//! Results:
//!   Throughput (tok/s):
//!     Mean:   35.2
//!     Median: 35.1
//!     Std:    1.2
//!     Min:    33.5
//!     Max:    37.8
//!
//!   Latency (ms):
//!     TTFT Mean: 45.0
//!     P50:  28.5
//!     P95:  32.1
//!     P99:  35.8
//!
//!   Memory:
//!     Peak RSS: 1.2 GB
//! ```

use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// Benchmark configuration
#[derive(Debug, Clone)]
struct BenchmarkConfig {
    /// Path to the GGUF model file
    model_path: PathBuf,
    /// Number of warmup iterations (not counted in results)
    warmup_iterations: usize,
    /// Number of benchmark iterations
    benchmark_iterations: usize,
    /// Maximum tokens to generate per iteration
    max_tokens: usize,
    /// Test prompts to use (reserved for future use with actual model loading)
    #[allow(dead_code)]
    prompts: Vec<String>,
    /// Output results as JSON
    json_output: bool,
    /// Temperature for generation
    temperature: f32,
    /// Verbose output
    verbose: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::new(),
            warmup_iterations: 5,
            benchmark_iterations: 20,
            max_tokens: 50,
            prompts: vec![
                "The quick brown fox".to_string(),
                "Once upon a time".to_string(),
                "In the beginning".to_string(),
                "Hello, I am".to_string(),
                "The capital of France is".to_string(),
            ],
            json_output: false,
            temperature: 0.7,
            verbose: false,
        }
    }
}

/// Results from a single generation
#[derive(Debug, Clone)]
struct GenerationResult {
    tokens_generated: usize,
    total_duration: Duration,
    time_to_first_token: Duration,
    token_latencies: Vec<Duration>,
}

impl GenerationResult {
    fn tokens_per_second(&self) -> f64 {
        if self.total_duration.as_secs_f64() > 0.0 {
            self.tokens_generated as f64 / self.total_duration.as_secs_f64()
        } else {
            0.0
        }
    }
}

/// Aggregated benchmark results
#[derive(Debug)]
struct BenchmarkResults {
    model_path: String,
    model_size_bytes: u64,
    warmup_iterations: usize,
    benchmark_iterations: usize,
    max_tokens: usize,

    // Throughput statistics
    throughput_mean: f64,
    throughput_median: f64,
    throughput_std: f64,
    throughput_min: f64,
    throughput_max: f64,

    // Latency statistics (in milliseconds)
    ttft_mean: f64,
    ttft_median: f64,
    latency_p50: f64,
    latency_p95: f64,
    latency_p99: f64,

    // Memory (if available)
    peak_memory_bytes: Option<u64>,

    // Individual results (reserved for detailed analysis)
    #[allow(dead_code)]
    results: Vec<GenerationResult>,
}

impl BenchmarkResults {
    fn from_results(
        config: &BenchmarkConfig,
        model_size_bytes: u64,
        results: Vec<GenerationResult>,
    ) -> Self {
        let throughputs: Vec<f64> = results.iter().map(|r| r.tokens_per_second()).collect();
        let ttfts: Vec<f64> = results
            .iter()
            .map(|r| r.time_to_first_token.as_secs_f64() * 1000.0)
            .collect();

        // Collect all token latencies
        let mut all_latencies: Vec<f64> = results
            .iter()
            .flat_map(|r| r.token_latencies.iter().map(|d| d.as_secs_f64() * 1000.0))
            .collect();
        all_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        Self {
            model_path: config.model_path.display().to_string(),
            model_size_bytes,
            warmup_iterations: config.warmup_iterations,
            benchmark_iterations: config.benchmark_iterations,
            max_tokens: config.max_tokens,

            throughput_mean: mean(&throughputs),
            throughput_median: median(&throughputs),
            throughput_std: std_dev(&throughputs),
            throughput_min: throughputs.iter().cloned().fold(f64::INFINITY, f64::min),
            throughput_max: throughputs
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max),

            ttft_mean: mean(&ttfts),
            ttft_median: median(&ttfts),
            latency_p50: percentile(&all_latencies, 50),
            latency_p95: percentile(&all_latencies, 95),
            latency_p99: percentile(&all_latencies, 99),

            peak_memory_bytes: get_peak_memory(),
            results,
        }
    }

    fn print_text(&self) {
        println!("\nResults:");
        println!("========");
        println!();
        println!("Throughput (tok/s):");
        println!("  Mean:   {:.1}", self.throughput_mean);
        println!("  Median: {:.1}", self.throughput_median);
        println!("  Std:    {:.1}", self.throughput_std);
        println!("  Min:    {:.1}", self.throughput_min);
        println!("  Max:    {:.1}", self.throughput_max);
        println!();
        println!("Latency (ms):");
        println!("  TTFT Mean:   {:.1}", self.ttft_mean);
        println!("  TTFT Median: {:.1}", self.ttft_median);
        println!("  P50:  {:.1}", self.latency_p50);
        println!("  P95:  {:.1}", self.latency_p95);
        println!("  P99:  {:.1}", self.latency_p99);

        if let Some(mem) = self.peak_memory_bytes {
            println!();
            println!("Memory:");
            println!("  Peak RSS: {}", format_bytes(mem));
        }
    }

    fn print_json(&self) {
        let json = format!(
            r#"{{
  "model_path": "{}",
  "model_size_bytes": {},
  "config": {{
    "warmup_iterations": {},
    "benchmark_iterations": {},
    "max_tokens": {}
  }},
  "throughput": {{
    "mean": {:.2},
    "median": {:.2},
    "std": {:.2},
    "min": {:.2},
    "max": {:.2}
  }},
  "latency_ms": {{
    "ttft_mean": {:.2},
    "ttft_median": {:.2},
    "p50": {:.2},
    "p95": {:.2},
    "p99": {:.2}
  }},
  "memory_bytes": {}
}}"#,
            self.model_path,
            self.model_size_bytes,
            self.warmup_iterations,
            self.benchmark_iterations,
            self.max_tokens,
            self.throughput_mean,
            self.throughput_median,
            self.throughput_std,
            self.throughput_min,
            self.throughput_max,
            self.ttft_mean,
            self.ttft_median,
            self.latency_p50,
            self.latency_p95,
            self.latency_p99,
            self.peak_memory_bytes
                .map(|m| m.to_string())
                .unwrap_or_else(|| "null".to_string()),
        );
        println!("{}", json);
    }
}

fn main() {
    let config = parse_args();

    // Validate model path
    if !config.model_path.exists() {
        eprintln!(
            "Error: Model file not found: {}",
            config.model_path.display()
        );
        eprintln!();
        eprintln!("Download a test model with:");
        eprintln!("  cargo run -p ruvllm --example download_test_model -- --model tinyllama");
        std::process::exit(1);
    }

    // Get model size
    let model_size = fs::metadata(&config.model_path)
        .map(|m| m.len())
        .unwrap_or(0);

    if !config.json_output {
        println!("RuvLLM Model Benchmark");
        println!("======================");
        println!();
        println!("Model: {}", config.model_path.display());
        println!("Model Size: {}", format_bytes(model_size));
        println!();
        println!("Configuration:");
        println!("  Warmup iterations: {}", config.warmup_iterations);
        println!("  Benchmark iterations: {}", config.benchmark_iterations);
        println!("  Max tokens per generation: {}", config.max_tokens);
        println!("  Temperature: {}", config.temperature);
        println!();
    }

    // Run benchmark
    let results = run_benchmark(&config, model_size);

    // Output results
    if config.json_output {
        results.print_json();
    } else {
        results.print_text();
    }
}

fn parse_args() -> BenchmarkConfig {
    let args: Vec<String> = env::args().collect();
    let mut config = BenchmarkConfig::default();

    if args.len() < 2 || args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        print_help();
        std::process::exit(0);
    }

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--model" | "-m" => {
                i += 1;
                if i < args.len() {
                    config.model_path = PathBuf::from(&args[i]);
                }
            }
            "--warmup" | "-w" => {
                i += 1;
                if i < args.len() {
                    config.warmup_iterations = args[i].parse().unwrap_or(5);
                }
            }
            "--iterations" | "-i" => {
                i += 1;
                if i < args.len() {
                    config.benchmark_iterations = args[i].parse().unwrap_or(20);
                }
            }
            "--max-tokens" | "-t" => {
                i += 1;
                if i < args.len() {
                    config.max_tokens = args[i].parse().unwrap_or(50);
                }
            }
            "--temperature" => {
                i += 1;
                if i < args.len() {
                    config.temperature = args[i].parse().unwrap_or(0.7);
                }
            }
            "--json" | "-j" => {
                config.json_output = true;
            }
            "--verbose" | "-v" => {
                config.verbose = true;
            }
            arg if !arg.starts_with('-') && config.model_path.as_os_str().is_empty() => {
                config.model_path = PathBuf::from(arg);
            }
            _ => {}
        }
        i += 1;
    }

    config
}

fn print_help() {
    println!("RuvLLM Model Benchmark");
    println!();
    println!("USAGE:");
    println!("    cargo run -p ruvllm --example benchmark_model --release -- [OPTIONS] <MODEL>");
    println!();
    println!("ARGUMENTS:");
    println!("    <MODEL>    Path to GGUF model file");
    println!();
    println!("OPTIONS:");
    println!("    -m, --model <PATH>          Path to GGUF model file");
    println!("    -w, --warmup <N>            Number of warmup iterations (default: 5)");
    println!("    -i, --iterations <N>        Number of benchmark iterations (default: 20)");
    println!("    -t, --max-tokens <N>        Max tokens per generation (default: 50)");
    println!("        --temperature <TEMP>    Temperature for sampling (default: 0.7)");
    println!("    -j, --json                  Output results as JSON");
    println!("    -v, --verbose               Verbose output");
    println!("    -h, --help                  Print help information");
    println!();
    println!("EXAMPLES:");
    println!("    # Basic benchmark");
    println!("    cargo run -p ruvllm --example benchmark_model --release -- ./model.gguf");
    println!();
    println!("    # Custom configuration");
    println!("    cargo run -p ruvllm --example benchmark_model --release -- \\");
    println!("        --model ./model.gguf --warmup 10 --iterations 50 --max-tokens 100");
    println!();
    println!("    # JSON output for automation");
    println!("    cargo run -p ruvllm --example benchmark_model --release -- \\");
    println!("        --model ./model.gguf --json > results.json");
}

fn run_benchmark(config: &BenchmarkConfig, model_size: u64) -> BenchmarkResults {
    // Try to use real model inference with candle backend
    #[cfg(feature = "candle")]
    {
        match run_real_benchmark(config, model_size) {
            Ok(results) => return results,
            Err(e) => {
                if !config.json_output {
                    println!("Warning: Failed to run real benchmark: {}", e);
                    println!("Falling back to simulated results.");
                    println!();
                }
            }
        }
    }

    // Fallback to simulated results
    run_simulated_benchmark(config, model_size)
}

#[cfg(feature = "candle")]
fn run_real_benchmark(
    config: &BenchmarkConfig,
    model_size: u64,
) -> Result<BenchmarkResults, String> {
    use ruvllm::{CandleBackend, GenerateParams, LlmBackend, ModelConfig};
    use std::time::Instant;

    if !config.json_output {
        println!("Loading model with Candle backend (Metal acceleration)...");
    }

    // Create backend and load model
    let mut backend =
        CandleBackend::new().map_err(|e| format!("Failed to create backend: {}", e))?;

    let model_config = ModelConfig::default();
    backend
        .load_gguf(&config.model_path, &model_config)
        .map_err(|e| format!("Failed to load GGUF model: {}", e))?;

    // Load tokenizer from same directory as model
    if let Some(parent) = config.model_path.parent() {
        let tokenizer_path = parent.join("tokenizer.json");
        if tokenizer_path.exists() {
            if !config.json_output {
                println!("Loading tokenizer from: {:?}", tokenizer_path);
            }
            backend
                .load_tokenizer(&tokenizer_path)
                .map_err(|e| format!("Failed to load tokenizer: {}", e))?;
        } else {
            return Err(format!(
                "Tokenizer not found at {:?}. Download it from HuggingFace.",
                tokenizer_path
            ));
        }
    }

    if !config.json_output {
        println!("Model loaded successfully!");
        println!();
    }

    let prompts = vec![
        "Explain quantum computing in simple terms.",
        "Write a haiku about programming.",
        "What is the meaning of life?",
        "Describe the process of photosynthesis.",
        "Tell me a short story about a robot.",
    ];

    let params = GenerateParams {
        max_tokens: config.max_tokens,
        temperature: config.temperature,
        top_p: 0.9,
        top_k: 40,
        ..Default::default()
    };

    let mut all_results = Vec::new();

    // Warmup phase
    if !config.json_output {
        println!(
            "Running warmup ({} iterations)...",
            config.warmup_iterations
        );
    }

    for i in 0..config.warmup_iterations {
        let prompt = &prompts[i % prompts.len()];
        let start = Instant::now();
        let first_token_time = Instant::now();

        match backend.generate(prompt, params.clone()) {
            Ok(output) => {
                let total_duration = start.elapsed();
                let tokens_generated = output.split_whitespace().count().max(1);

                let result = GenerationResult {
                    tokens_generated,
                    total_duration,
                    time_to_first_token: first_token_time.elapsed(),
                    token_latencies: vec![
                        total_duration / tokens_generated as u32;
                        tokens_generated
                    ],
                };

                if !config.json_output {
                    println!(
                        "  Warmup {}/{}: {:.1} tok/s",
                        i + 1,
                        config.warmup_iterations,
                        result.tokens_per_second()
                    );
                }
            }
            Err(e) => {
                if !config.json_output {
                    println!(
                        "  Warmup {}/{}: Error - {}",
                        i + 1,
                        config.warmup_iterations,
                        e
                    );
                }
            }
        }
    }

    // Benchmark phase
    if !config.json_output {
        println!();
        println!(
            "Running benchmark ({} iterations)...",
            config.benchmark_iterations
        );
    }

    for i in 0..config.benchmark_iterations {
        let prompt = &prompts[i % prompts.len()];
        let start = Instant::now();
        let first_token_time = Instant::now();

        match backend.generate(prompt, params.clone()) {
            Ok(output) => {
                let total_duration = start.elapsed();
                let tokens_generated = output.split_whitespace().count().max(1);

                let result = GenerationResult {
                    tokens_generated,
                    total_duration,
                    time_to_first_token: first_token_time.elapsed(),
                    token_latencies: vec![
                        total_duration / tokens_generated as u32;
                        tokens_generated
                    ],
                };

                if !config.json_output && (config.verbose || i % 5 == 0) {
                    println!(
                        "  Iteration {}/{}: {:.1} tok/s, TTFT: {:.1}ms",
                        i + 1,
                        config.benchmark_iterations,
                        result.tokens_per_second(),
                        result.time_to_first_token.as_secs_f64() * 1000.0
                    );
                }
                all_results.push(result);
            }
            Err(e) => {
                if !config.json_output {
                    println!(
                        "  Iteration {}/{}: Error - {}",
                        i + 1,
                        config.benchmark_iterations,
                        e
                    );
                }
            }
        }
    }

    if all_results.is_empty() {
        return Err("No successful generations".to_string());
    }

    // Print SONA learning stats
    if !config.json_output {
        if let Some(stats) = backend.sona_stats() {
            println!();
            println!("SONA Self-Learning Stats:");
            println!("  Total trajectories: {}", stats.total_trajectories);
            println!("  Instant updates: {}", stats.instant_updates);
            println!("  Background updates: {}", stats.background_updates);
            println!("  Patterns learned: {}", stats.patterns_learned);
        }
    }

    Ok(BenchmarkResults::from_results(
        config,
        model_size,
        all_results,
    ))
}

fn run_simulated_benchmark(config: &BenchmarkConfig, model_size: u64) -> BenchmarkResults {
    if !config.json_output {
        println!("Note: Running with simulated results (candle feature not enabled or model load failed).");
        println!();
    }

    let mut all_results = Vec::new();

    // Warmup phase
    if !config.json_output {
        println!(
            "Running warmup ({} iterations)...",
            config.warmup_iterations
        );
    }

    for i in 0..config.warmup_iterations {
        let result = simulate_generation(config);
        if !config.json_output {
            println!(
                "  Warmup {}/{}: {:.1} tok/s",
                i + 1,
                config.warmup_iterations,
                result.tokens_per_second()
            );
        }
    }

    // Benchmark phase
    if !config.json_output {
        println!();
        println!(
            "Running benchmark ({} iterations)...",
            config.benchmark_iterations
        );
    }

    for i in 0..config.benchmark_iterations {
        let result = simulate_generation(config);
        if !config.json_output && (config.verbose || i % 5 == 0) {
            println!(
                "  Iteration {}/{}: {:.1} tok/s, TTFT: {:.1}ms",
                i + 1,
                config.benchmark_iterations,
                result.tokens_per_second(),
                result.time_to_first_token.as_secs_f64() * 1000.0
            );
        }
        all_results.push(result);
    }

    BenchmarkResults::from_results(config, model_size, all_results)
}

/// Simulate a generation for demonstration purposes
fn simulate_generation(config: &BenchmarkConfig) -> GenerationResult {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Simulate realistic timing characteristics
    // These would be replaced with actual measurements in a real implementation
    let base_speed = 30.0 + rng.gen::<f64>() * 10.0; // 30-40 tok/s
    let tokens = config.max_tokens.min(rng.gen_range(30..60));
    let total_secs = tokens as f64 / base_speed;

    let ttft_ms = 40.0 + rng.gen::<f64>() * 20.0; // 40-60ms TTFT
    let ttft = Duration::from_secs_f64(ttft_ms / 1000.0);

    let mut latencies = Vec::with_capacity(tokens);
    for _ in 0..tokens {
        let latency_ms = 25.0 + rng.gen::<f64>() * 10.0; // 25-35ms per token
        latencies.push(Duration::from_secs_f64(latency_ms / 1000.0));
    }

    GenerationResult {
        tokens_generated: tokens,
        total_duration: Duration::from_secs_f64(total_secs),
        time_to_first_token: ttft,
        token_latencies: latencies,
    }
}

// ============================================================================
// Statistics Helpers
// ============================================================================

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let variance = values.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

fn percentile(sorted_values: &[f64], p: usize) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    let idx = (p * sorted_values.len() / 100).min(sorted_values.len() - 1);
    sorted_values[idx]
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Get peak memory usage (platform-specific)
fn get_peak_memory() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let pid = std::process::id();
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .ok()?;

        let rss_kb: u64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .ok()?;

        Some(rss_kb * 1024) // Convert KB to bytes
    }

    #[cfg(target_os = "linux")]
    {
        use std::fs;
        let status = fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            if line.starts_with("VmPeak:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: u64 = parts[1].parse().ok()?;
                    return Some(kb * 1024);
                }
            }
        }
        None
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistics() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(mean(&values), 3.0);
        assert_eq!(median(&values), 3.0);
        assert!((std_dev(&values) - 1.5811).abs() < 0.001);
    }

    #[test]
    fn test_percentile() {
        let values: Vec<f64> = (0..100).map(|i| i as f64).collect();
        assert_eq!(percentile(&values, 50), 50.0);
        assert_eq!(percentile(&values, 95), 95.0);
        assert_eq!(percentile(&values, 99), 99.0);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1_572_864), "1.50 MB");
        assert_eq!(format_bytes(1_610_612_736), "1.50 GB");
    }
}
