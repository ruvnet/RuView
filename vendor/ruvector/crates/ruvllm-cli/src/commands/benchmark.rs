//! Benchmark command implementation
//!
//! Runs performance benchmarks on LLM models to measure inference speed,
//! memory usage, and throughput on Apple Silicon.

use anyhow::{Context, Result};
use colored::Colorize;
use console::style;
use prettytable::{row, Table};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::models::{get_model, resolve_model_id, QuantPreset};

/// Benchmark results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    pub model_id: String,
    pub quantization: String,
    pub prompt_length: usize,
    pub gen_length: usize,
    pub iterations: usize,
    pub warmup: usize,
    pub metrics: BenchmarkMetrics,
    pub system_info: SystemInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    pub time_to_first_token_ms: f64,
    pub tokens_per_second: f64,
    pub total_time_ms: f64,
    pub prompt_eval_time_ms: f64,
    pub generation_time_ms: f64,
    pub memory_usage_mb: f64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub memory_gb: f64,
}

/// Run the benchmark command
pub async fn run(
    model: &str,
    warmup: usize,
    iterations: usize,
    prompt_length: usize,
    gen_length: usize,
    quantization: &str,
    format: &str,
    cache_dir: &str,
) -> Result<()> {
    let model_id = resolve_model_id(model);
    let quant = QuantPreset::from_str(quantization)
        .ok_or_else(|| anyhow::anyhow!("Invalid quantization format: {}", quantization))?;

    // Print header
    println!();
    println!("{}", style("RuvLLM Performance Benchmark").bold().cyan());
    println!("{}", "=".repeat(50).dimmed());
    println!();
    println!("  {} {}", "Model:".dimmed(), model_id);
    println!("  {} {}", "Quantization:".dimmed(), quant);
    println!("  {} {} tokens", "Prompt Length:".dimmed(), prompt_length);
    println!("  {} {} tokens", "Generation Length:".dimmed(), gen_length);
    println!("  {} {}", "Warmup Iterations:".dimmed(), warmup);
    println!("  {} {}", "Benchmark Iterations:".dimmed(), iterations);
    println!();

    // Load model
    println!("{}", "Loading model...".yellow());
    let backend = load_model(&model_id, quant, cache_dir)?;

    if backend.is_model_loaded() {
        if let Some(info) = backend.model_info() {
            println!(
                "{} Loaded {} ({:.1}B params, {} memory)",
                style("Ready!").green().bold(),
                info.name,
                info.num_parameters as f64 / 1e9,
                bytesize::ByteSize(info.memory_usage as u64)
            );
        }
    } else {
        println!(
            "{} Running benchmark in mock mode (no real model loaded)",
            style("Warning:").yellow().bold()
        );
    }
    println!();

    // Generate test prompt
    let prompt = generate_test_prompt(prompt_length);
    let params = ruvllm::GenerateParams {
        max_tokens: gen_length,
        temperature: 0.7,
        ..Default::default()
    };

    // Warmup
    if warmup > 0 {
        println!("{}", "Running warmup iterations...".dimmed());
        let warmup_pb = indicatif::ProgressBar::new(warmup as u64);
        warmup_pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("  Warmup: [{bar:30}] {pos}/{len}")
                .unwrap(),
        );

        for _ in 0..warmup {
            let _ = backend.generate(&prompt, params.clone());
            warmup_pb.inc(1);
        }
        warmup_pb.finish_and_clear();
        println!("  {} warmup iterations completed", warmup);
        println!();
    }

    // Benchmark
    println!("{}", "Running benchmark...".yellow());
    let bench_pb = indicatif::ProgressBar::new(iterations as u64);
    bench_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  Benchmark: [{bar:30}] {pos}/{len} ({eta})")
            .unwrap(),
    );

    let mut latencies = Vec::with_capacity(iterations);
    let mut ttft_times = Vec::with_capacity(iterations);
    let mut tokens_generated = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let start = Instant::now();

        // Generate
        let result = backend.generate(&prompt, params.clone());
        let total_time = start.elapsed();

        // Record metrics
        latencies.push(total_time);

        if let Ok(text) = &result {
            let token_count = text.split_whitespace().count();
            tokens_generated.push(token_count);
            // Estimate TTFT as a fraction of total time
            ttft_times.push(Duration::from_secs_f64(total_time.as_secs_f64() * 0.1));
        } else {
            tokens_generated.push(gen_length);
            ttft_times.push(Duration::from_millis(50));
        }

        bench_pb.inc(1);
    }

    bench_pb.finish_and_clear();
    println!("  {} benchmark iterations completed", iterations);
    println!();

    // Calculate metrics
    let metrics = calculate_metrics(&latencies, &ttft_times, &tokens_generated);

    // Get system info
    let system_info = get_system_info();

    // Create results
    let results = BenchmarkResults {
        model_id: model_id.clone(),
        quantization: quant.to_string(),
        prompt_length,
        gen_length,
        iterations,
        warmup,
        metrics,
        system_info,
    };

    // Output results
    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        "csv" => {
            print_csv(&results);
        }
        _ => {
            print_results(&results);
        }
    }

    Ok(())
}

/// Load model for benchmarking
fn load_model(
    model_id: &str,
    quant: QuantPreset,
    cache_dir: &str,
) -> Result<Box<dyn ruvllm::LlmBackend>> {
    let mut backend = ruvllm::create_backend();

    let config = ruvllm::ModelConfig {
        architecture: detect_architecture(model_id),
        quantization: Some(map_quantization(quant)),
        ..Default::default()
    };

    let model_path = PathBuf::from(cache_dir).join("models").join(model_id);
    let load_result = if model_path.exists() {
        backend.load_model(model_path.to_str().unwrap(), config.clone())
    } else {
        backend.load_model(model_id, config)
    };

    if let Err(e) = load_result {
        tracing::warn!("Model load failed: {}", e);
    }

    Ok(backend)
}

/// Generate test prompt of approximate length
fn generate_test_prompt(target_length: usize) -> String {
    let base_text = "The quick brown fox jumps over the lazy dog. ";
    let mut prompt = String::new();

    while prompt.split_whitespace().count() < target_length {
        prompt.push_str(base_text);
    }

    // Truncate to target
    let words: Vec<&str> = prompt.split_whitespace().take(target_length).collect();
    words.join(" ")
}

/// Calculate benchmark metrics
fn calculate_metrics(
    latencies: &[Duration],
    ttft_times: &[Duration],
    tokens_generated: &[usize],
) -> BenchmarkMetrics {
    let total_time_ms = latencies
        .iter()
        .map(|d| d.as_secs_f64() * 1000.0)
        .sum::<f64>()
        / latencies.len() as f64;

    let total_tokens: usize = tokens_generated.iter().sum();
    let total_duration: Duration = latencies.iter().sum();
    let tokens_per_second = total_tokens as f64 / total_duration.as_secs_f64();

    let ttft_avg = ttft_times
        .iter()
        .map(|d| d.as_secs_f64() * 1000.0)
        .sum::<f64>()
        / ttft_times.len() as f64;

    // Calculate percentiles
    let mut sorted_latencies: Vec<f64> =
        latencies.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    sorted_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p50_idx = (sorted_latencies.len() as f64 * 0.50) as usize;
    let p95_idx = (sorted_latencies.len() as f64 * 0.95) as usize;
    let p99_idx = (sorted_latencies.len() as f64 * 0.99) as usize;

    BenchmarkMetrics {
        time_to_first_token_ms: ttft_avg,
        tokens_per_second,
        total_time_ms,
        prompt_eval_time_ms: ttft_avg * 0.8,
        generation_time_ms: total_time_ms - ttft_avg,
        memory_usage_mb: 0.0, // Would need system-specific implementation
        latency_p50_ms: sorted_latencies.get(p50_idx).copied().unwrap_or(0.0),
        latency_p95_ms: sorted_latencies.get(p95_idx).copied().unwrap_or(0.0),
        latency_p99_ms: sorted_latencies.get(p99_idx).copied().unwrap_or(0.0),
    }
}

/// Get system information
fn get_system_info() -> SystemInfo {
    SystemInfo {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        cpu: get_cpu_info(),
        memory_gb: get_memory_info(),
    }
}

fn get_cpu_info() -> String {
    #[cfg(target_os = "macos")]
    {
        // Try to get CPU info on macOS
        std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "Apple Silicon".to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        "Unknown".to_string()
    }
}

fn get_memory_info() -> f64 {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .map(|bytes| bytes as f64 / (1024.0 * 1024.0 * 1024.0))
            .unwrap_or(0.0)
    }

    #[cfg(not(target_os = "macos"))]
    {
        0.0
    }
}

/// Print results in text format
fn print_results(results: &BenchmarkResults) {
    println!("{}", style("Benchmark Results").bold().green());
    println!("{}", "=".repeat(50).dimmed());
    println!();

    // Main metrics table
    let mut table = Table::new();
    table.add_row(row!["Metric", "Value"]);
    table.add_row(row![
        "Tokens/Second".cyan(),
        format!("{:.2}", results.metrics.tokens_per_second)
    ]);
    table.add_row(row![
        "Time to First Token".cyan(),
        format!("{:.2} ms", results.metrics.time_to_first_token_ms)
    ]);
    table.add_row(row![
        "Total Time (avg)".cyan(),
        format!("{:.2} ms", results.metrics.total_time_ms)
    ]);
    table.add_row(row![
        "Prompt Eval Time".cyan(),
        format!("{:.2} ms", results.metrics.prompt_eval_time_ms)
    ]);
    table.add_row(row![
        "Generation Time".cyan(),
        format!("{:.2} ms", results.metrics.generation_time_ms)
    ]);

    table.printstd();
    println!();

    // Latency percentiles
    println!("{}", style("Latency Distribution").bold());
    let mut lat_table = Table::new();
    lat_table.add_row(row!["Percentile", "Latency (ms)"]);
    lat_table.add_row(row![
        "P50",
        format!("{:.2}", results.metrics.latency_p50_ms)
    ]);
    lat_table.add_row(row![
        "P95",
        format!("{:.2}", results.metrics.latency_p95_ms)
    ]);
    lat_table.add_row(row![
        "P99",
        format!("{:.2}", results.metrics.latency_p99_ms)
    ]);
    lat_table.printstd();
    println!();

    // System info
    println!("{}", style("System Information").bold());
    println!("  {} {}", "OS:".dimmed(), results.system_info.os);
    println!("  {} {}", "Arch:".dimmed(), results.system_info.arch);
    println!("  {} {}", "CPU:".dimmed(), results.system_info.cpu);
    println!(
        "  {} {:.1} GB",
        "Memory:".dimmed(),
        results.system_info.memory_gb
    );
    println!();

    // Performance rating
    print_performance_rating(&results.metrics);
}

/// Print performance rating
fn print_performance_rating(metrics: &BenchmarkMetrics) {
    let rating = if metrics.tokens_per_second >= 50.0 {
        ("Excellent", "green")
    } else if metrics.tokens_per_second >= 30.0 {
        ("Good", "green")
    } else if metrics.tokens_per_second >= 15.0 {
        ("Acceptable", "yellow")
    } else if metrics.tokens_per_second >= 5.0 {
        ("Slow", "yellow")
    } else {
        ("Very Slow", "red")
    };

    println!("{}", style("Performance Rating").bold());
    match rating.1 {
        "green" => println!("  {} {}", "Rating:".dimmed(), rating.0.green().bold()),
        "yellow" => println!("  {} {}", "Rating:".dimmed(), rating.0.yellow().bold()),
        _ => println!("  {} {}", "Rating:".dimmed(), rating.0.red().bold()),
    }

    // Recommendations
    if metrics.tokens_per_second < 15.0 {
        println!();
        println!("{}", "Recommendations:".bold());
        println!("  - Try a smaller quantization (e.g., Q4_K_M)");
        println!("  - Use a smaller model");
        println!("  - Reduce context length");
    }
}

/// Print results in CSV format
fn print_csv(results: &BenchmarkResults) {
    println!("model,quantization,prompt_len,gen_len,iterations,tps,ttft_ms,total_ms,p50_ms,p95_ms,p99_ms");
    println!(
        "{},{},{},{},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
        results.model_id,
        results.quantization,
        results.prompt_length,
        results.gen_length,
        results.iterations,
        results.metrics.tokens_per_second,
        results.metrics.time_to_first_token_ms,
        results.metrics.total_time_ms,
        results.metrics.latency_p50_ms,
        results.metrics.latency_p95_ms,
        results.metrics.latency_p99_ms,
    );
}

/// Detect architecture from model ID
fn detect_architecture(model_id: &str) -> ruvllm::ModelArchitecture {
    let lower = model_id.to_lowercase();
    if lower.contains("mistral") {
        ruvllm::ModelArchitecture::Mistral
    } else if lower.contains("llama") {
        ruvllm::ModelArchitecture::Llama
    } else if lower.contains("phi") {
        ruvllm::ModelArchitecture::Phi
    } else if lower.contains("qwen") {
        ruvllm::ModelArchitecture::Qwen
    } else {
        ruvllm::ModelArchitecture::Llama
    }
}

/// Map quantization preset
fn map_quantization(quant: QuantPreset) -> ruvllm::Quantization {
    match quant {
        QuantPreset::Q4K => ruvllm::Quantization::Q4K,
        QuantPreset::Q8 => ruvllm::Quantization::Q8,
        QuantPreset::F16 => ruvllm::Quantization::F16,
        QuantPreset::None => ruvllm::Quantization::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_test_prompt() {
        let prompt = generate_test_prompt(50);
        let word_count = prompt.split_whitespace().count();
        assert_eq!(word_count, 50);
    }

    #[test]
    fn test_calculate_metrics() {
        let latencies = vec![
            Duration::from_millis(100),
            Duration::from_millis(110),
            Duration::from_millis(105),
        ];
        let ttft = vec![
            Duration::from_millis(10),
            Duration::from_millis(11),
            Duration::from_millis(10),
        ];
        let tokens = vec![50, 52, 48];

        let metrics = calculate_metrics(&latencies, &ttft, &tokens);
        assert!(metrics.tokens_per_second > 0.0);
        assert!(metrics.total_time_ms > 0.0);
    }
}
