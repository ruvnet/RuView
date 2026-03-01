use clap::Args;
use crate::{config::Config, output::{print_benchmark_results, BenchmarkRow}};
use ruvector_attention::{
    attention::{ScaledDotProductAttention, MultiHeadAttention},
    hyperbolic::HyperbolicAttention,
    sparse::{FlashAttention, LinearAttention},
    moe::MoEAttention,
};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Instant;

#[derive(Args)]
pub struct BenchmarkArgs {
    /// Attention types to benchmark (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    attention_types: Option<Vec<String>>,

    /// Dimensions to test (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    dimensions: Option<Vec<usize>>,

    /// Number of iterations per test
    #[arg(short, long)]
    iterations: Option<usize>,

    /// Number of warmup iterations
    #[arg(short, long)]
    warmup: Option<usize>,

    /// Sequence length
    #[arg(short, long, default_value = "128")]
    seq_length: usize,

    /// Output results to file
    #[arg(short, long)]
    output: Option<std::path::PathBuf>,

    /// Output format (json, csv, table)
    #[arg(short, long, default_value = "table")]
    format: String,
}

pub async fn run(args: BenchmarkArgs, config: &Config) -> anyhow::Result<()> {
    let attention_types = args.attention_types.unwrap_or_else(|| {
        vec![
            "scaled_dot".to_string(),
            "multi_head".to_string(),
            "hyperbolic".to_string(),
            "flash".to_string(),
            "linear".to_string(),
            "moe".to_string(),
        ]
    });

    let dimensions = args.dimensions.unwrap_or_else(|| config.benchmark.dimensions.clone());
    let iterations = args.iterations.unwrap_or(config.benchmark.iterations);
    let warmup = args.warmup.unwrap_or(config.benchmark.warmup);

    println!("Running benchmarks...");
    println!("Attention types: {:?}", attention_types);
    println!("Dimensions: {:?}", dimensions);
    println!("Iterations: {}, Warmup: {}", iterations, warmup);
    println!();

    let total_tests = attention_types.len() * dimensions.len();
    let pb = ProgressBar::new(total_tests as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")?
            .progress_chars("##-")
    );

    let mut results = Vec::new();

    for attention_type in &attention_types {
        for &dim in &dimensions {
            pb.set_message(format!("Testing {} (dim={})", attention_type, dim));

            let timings = benchmark_attention(
                attention_type,
                dim,
                args.seq_length,
                iterations,
                warmup
            )?;

            let mean = timings.iter().sum::<f64>() / timings.len() as f64;
            let variance = timings.iter()
                .map(|&x| (x - mean).powi(2))
                .sum::<f64>() / timings.len() as f64;
            let std_dev = variance.sqrt();
            let throughput = 1000.0 / mean; // operations per second

            results.push(BenchmarkRow {
                attention_type: attention_type.clone(),
                dimension: dim,
                mean_time_ms: mean,
                std_dev_ms: std_dev,
                throughput,
            });

            pb.inc(1);
        }
    }

    pb.finish_with_message("Benchmarks complete!");
    println!();

    match args.format.as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&results)?;
            if let Some(path) = args.output {
                std::fs::write(path, json)?;
            } else {
                println!("{}", json);
            }
        }
        "csv" => {
            let mut csv = String::from("attention_type,dimension,mean_time_ms,std_dev_ms,throughput\n");
            for row in &results {
                csv.push_str(&format!(
                    "{},{},{},{},{}\n",
                    row.attention_type, row.dimension, row.mean_time_ms, row.std_dev_ms, row.throughput
                ));
            }
            if let Some(path) = args.output {
                std::fs::write(path, csv)?;
            } else {
                println!("{}", csv);
            }
        }
        _ => {
            print_benchmark_results(results);
        }
    }

    Ok(())
}

fn benchmark_attention(
    attention_type: &str,
    dim: usize,
    seq_length: usize,
    iterations: usize,
    warmup: usize,
) -> anyhow::Result<Vec<f64>> {
    // Generate random test data
    let query: Vec<Vec<f32>> = (0..seq_length)
        .map(|_| (0..dim).map(|_| rand::random::<f32>()).collect())
        .collect();
    let keys: Vec<Vec<f32>> = (0..seq_length)
        .map(|_| (0..dim).map(|_| rand::random::<f32>()).collect())
        .collect();
    let values: Vec<Vec<f32>> = (0..seq_length)
        .map(|_| (0..dim).map(|_| rand::random::<f32>()).collect())
        .collect();

    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

    // Warmup
    for _ in 0..warmup {
        run_attention(attention_type, dim, &query, &keys_refs, &values_refs)?;
    }

    // Actual benchmark
    let mut timings = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        run_attention(attention_type, dim, &query, &keys_refs, &values_refs)?;
        let elapsed = start.elapsed();
        timings.push(elapsed.as_secs_f64() * 1000.0);
    }

    Ok(timings)
}

fn run_attention(
    attention_type: &str,
    dim: usize,
    query: &[Vec<f32>],
    keys: &[&[f32]],
    values: &[&[f32]],
) -> anyhow::Result<Vec<Vec<f32>>> {
    match attention_type {
        "scaled_dot" => {
            let attention = ScaledDotProductAttention::new(dim, None);
            attention.compute(query, keys, values)
        }
        "multi_head" => {
            let attention = MultiHeadAttention::new(dim, 8)?;
            attention.compute(query, keys, values)
        }
        "hyperbolic" => {
            let attention = HyperbolicAttention::new(dim, 1.0)?;
            attention.compute(query, keys, values)
        }
        "flash" => {
            let attention = FlashAttention::new(dim, 64)?;
            attention.compute(query, keys, values)
        }
        "linear" => {
            let attention = LinearAttention::new(dim)?;
            attention.compute(query, keys, values)
        }
        "moe" => {
            let attention = MoEAttention::new(dim, 4, 2)?;
            attention.compute(query, keys, values)
        }
        _ => Err(anyhow::anyhow!("Unknown attention type: {}", attention_type)),
    }
}
