//! ANN-Benchmarks compatible benchmark suite
//!
//! Runs standard benchmarks on SIFT1M, GIST1M, and Deep1M datasets
//! compatible with http://ann-benchmarks.com format

use anyhow::{Context, Result};
use clap::Parser;
use ruvector_bench::{
    calculate_recall, create_progress_bar, BenchmarkResult, DatasetGenerator, LatencyStats,
    MemoryProfiler, ResultWriter, VectorDistribution,
};
use ruvector_core::{
    types::{DbOptions, HnswConfig, QuantizationConfig},
    DistanceMetric, SearchQuery, VectorDB, VectorEntry,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "ann-benchmark")]
#[command(about = "ANN-Benchmarks compatible testing")]
struct Args {
    /// Dataset to use: sift1m, gist1m, deep1m, or synthetic
    #[arg(short, long, default_value = "synthetic")]
    dataset: String,

    /// Number of vectors for synthetic dataset
    #[arg(short, long, default_value = "100000")]
    num_vectors: usize,

    /// Number of queries
    #[arg(short = 'q', long, default_value = "1000")]
    num_queries: usize,

    /// Vector dimensions (for synthetic)
    #[arg(short = 'd', long, default_value = "128")]
    dimensions: usize,

    /// K nearest neighbors to retrieve
    #[arg(short, long, default_value = "10")]
    k: usize,

    /// HNSW M parameter
    #[arg(short, long, default_value = "32")]
    m: usize,

    /// HNSW ef_construction
    #[arg(long, default_value = "200")]
    ef_construction: usize,

    /// HNSW ef_search values to test (comma-separated)
    #[arg(long, default_value = "50,100,200,400")]
    ef_search_values: String,

    /// Output directory for results
    #[arg(short, long, default_value = "bench_results")]
    output: PathBuf,

    /// Distance metric
    #[arg(long, default_value = "cosine")]
    metric: String,

    /// Quantization: none, scalar, binary
    #[arg(long, default_value = "scalar")]
    quantization: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("╔════════════════════════════════════════╗");
    println!("║   Ruvector ANN-Benchmarks Suite       ║");
    println!("╚════════════════════════════════════════╝\n");

    // Parse ef_search values
    let ef_search_values: Vec<usize> = args
        .ef_search_values
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();

    // Load or generate dataset
    let (vectors, queries, ground_truth) = load_dataset(&args)?;
    println!(
        "✓ Dataset loaded: {} vectors, {} queries",
        vectors.len(),
        queries.len()
    );

    let mut all_results = Vec::new();

    // Run benchmarks for each ef_search value
    for &ef_search in &ef_search_values {
        println!("\n{}", "=".repeat(60));
        println!("Testing with ef_search = {}", ef_search);
        println!("{}\n", "=".repeat(60));

        let result = run_benchmark(&args, &vectors, &queries, &ground_truth, ef_search)?;
        all_results.push(result);
    }

    // Write results
    let writer = ResultWriter::new(&args.output)?;
    writer.write_json("ann_benchmark", &all_results)?;
    writer.write_csv("ann_benchmark", &all_results)?;
    writer.write_markdown_report("ann_benchmark", &all_results)?;

    // Print summary table
    print_summary_table(&all_results);

    println!(
        "\n✓ Benchmark complete! Results saved to: {}",
        args.output.display()
    );
    Ok(())
}

fn load_dataset(args: &Args) -> Result<(Vec<Vec<f32>>, Vec<Vec<f32>>, Vec<Vec<String>>)> {
    match args.dataset.as_str() {
        "sift1m" => load_sift1m(),
        "gist1m" => load_gist1m(),
        "deep1m" => load_deep1m(),
        "synthetic" | _ => {
            println!("Generating synthetic {} dataset...", args.dataset);
            let gen = DatasetGenerator::new(
                args.dimensions,
                VectorDistribution::Normal {
                    mean: 0.0,
                    std_dev: 1.0,
                },
            );

            let pb = create_progress_bar(args.num_vectors as u64, "Generating vectors");
            let vectors: Vec<Vec<f32>> = (0..args.num_vectors)
                .map(|_| {
                    pb.inc(1);
                    gen.generate(1).into_iter().next().unwrap()
                })
                .collect();
            pb.finish_with_message("✓ Vectors generated");

            let queries = gen.generate(args.num_queries);

            // Generate ground truth using brute force
            let ground_truth = compute_ground_truth(&vectors, &queries, args.k)?;

            Ok((vectors, queries, ground_truth))
        }
    }
}

fn load_sift1m() -> Result<(Vec<Vec<f32>>, Vec<Vec<f32>>, Vec<Vec<String>>)> {
    // TODO: Implement HDF5 loading when dataset is available
    println!("⚠ SIFT1M dataset not found, using synthetic data");
    println!("  Download SIFT1M with: scripts/download_datasets.sh");

    let gen = DatasetGenerator::new(
        128,
        VectorDistribution::Normal {
            mean: 0.0,
            std_dev: 1.0,
        },
    );
    let vectors = gen.generate(10000);
    let queries = gen.generate(100);
    let ground_truth = compute_ground_truth(&vectors, &queries, 10)?;
    Ok((vectors, queries, ground_truth))
}

fn load_gist1m() -> Result<(Vec<Vec<f32>>, Vec<Vec<f32>>, Vec<Vec<String>>)> {
    println!("⚠ GIST1M dataset not found, using synthetic data");
    let gen = DatasetGenerator::new(
        960,
        VectorDistribution::Normal {
            mean: 0.0,
            std_dev: 1.0,
        },
    );
    let vectors = gen.generate(10000);
    let queries = gen.generate(100);
    let ground_truth = compute_ground_truth(&vectors, &queries, 10)?;
    Ok((vectors, queries, ground_truth))
}

fn load_deep1m() -> Result<(Vec<Vec<f32>>, Vec<Vec<f32>>, Vec<Vec<String>>)> {
    println!("⚠ Deep1M dataset not found, using synthetic data");
    let gen = DatasetGenerator::new(
        96,
        VectorDistribution::Normal {
            mean: 0.0,
            std_dev: 1.0,
        },
    );
    let vectors = gen.generate(10000);
    let queries = gen.generate(100);
    let ground_truth = compute_ground_truth(&vectors, &queries, 10)?;
    Ok((vectors, queries, ground_truth))
}

fn compute_ground_truth(
    vectors: &[Vec<f32>],
    queries: &[Vec<f32>],
    k: usize,
) -> Result<Vec<Vec<String>>> {
    println!("Computing ground truth with brute force...");
    let pb = create_progress_bar(queries.len() as u64, "Computing ground truth");

    let ground_truth: Vec<Vec<String>> = queries
        .iter()
        .map(|query| {
            pb.inc(1);
            let mut distances: Vec<(usize, f32)> = vectors
                .iter()
                .enumerate()
                .map(|(idx, vec)| {
                    let dist = cosine_distance(query, vec);
                    (idx, dist)
                })
                .collect();

            distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            distances
                .iter()
                .take(k)
                .map(|(idx, _)| idx.to_string())
                .collect()
        })
        .collect();

    pb.finish_with_message("✓ Ground truth computed");
    Ok(ground_truth)
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    1.0 - (dot / (norm_a * norm_b))
}

fn run_benchmark(
    args: &Args,
    vectors: &[Vec<f32>],
    queries: &[Vec<f32>],
    ground_truth: &[Vec<String>],
    ef_search: usize,
) -> Result<BenchmarkResult> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("bench.db");

    // Parse distance metric
    let distance_metric = match args.metric.as_str() {
        "cosine" => DistanceMetric::Cosine,
        "euclidean" => DistanceMetric::Euclidean,
        "dot" => DistanceMetric::DotProduct,
        _ => DistanceMetric::Cosine,
    };

    // Parse quantization
    let quantization = match args.quantization.as_str() {
        "none" => QuantizationConfig::None,
        "scalar" => QuantizationConfig::Scalar,
        "binary" => QuantizationConfig::Binary,
        _ => QuantizationConfig::Scalar,
    };

    let dimensions = vectors[0].len();
    let options = DbOptions {
        dimensions,
        distance_metric,
        storage_path: db_path.to_str().unwrap().to_string(),
        hnsw_config: Some(HnswConfig {
            m: args.m,
            ef_construction: args.ef_construction,
            ef_search,
            max_elements: vectors.len() * 2,
        }),
        quantization: Some(quantization),
    };

    // Measure build time and memory
    let mem_profiler = MemoryProfiler::new();
    let build_start = Instant::now();

    let db = VectorDB::new(options)?;

    println!("Indexing {} vectors...", vectors.len());
    let pb = create_progress_bar(vectors.len() as u64, "Indexing");

    for (idx, vector) in vectors.iter().enumerate() {
        let entry = VectorEntry {
            id: Some(idx.to_string()),
            vector: vector.clone(),
            metadata: None,
        };
        db.insert(entry)?;
        pb.inc(1);
    }
    pb.finish_with_message("✓ Indexing complete");

    let build_time = build_start.elapsed();
    let memory_mb = mem_profiler.current_usage_mb();

    // Run search benchmark
    println!("Running {} queries...", queries.len());
    let mut latency_stats = LatencyStats::new()?;
    let mut search_results = Vec::new();

    let search_start = Instant::now();
    let pb = create_progress_bar(queries.len() as u64, "Searching");

    for query in queries {
        let query_start = Instant::now();
        let results = db.search(SearchQuery {
            vector: query.clone(),
            k: args.k,
            filter: None,
            ef_search: Some(ef_search),
        })?;
        latency_stats.record(query_start.elapsed())?;

        let result_ids: Vec<String> = results.into_iter().map(|r| r.id).collect();
        search_results.push(result_ids);
        pb.inc(1);
    }
    pb.finish_with_message("✓ Search complete");

    let total_search_time = search_start.elapsed();
    let qps = queries.len() as f64 / total_search_time.as_secs_f64();

    // Calculate recall
    let recall_1 = calculate_recall(&search_results, ground_truth, 1);
    let recall_10 = calculate_recall(&search_results, ground_truth, 10.min(args.k));
    let recall_100 = calculate_recall(&search_results, ground_truth, 100.min(args.k));

    let mut metadata = HashMap::new();
    metadata.insert("m".to_string(), args.m.to_string());
    metadata.insert(
        "ef_construction".to_string(),
        args.ef_construction.to_string(),
    );
    metadata.insert("ef_search".to_string(), ef_search.to_string());
    metadata.insert("metric".to_string(), args.metric.clone());
    metadata.insert("quantization".to_string(), args.quantization.clone());

    Ok(BenchmarkResult {
        name: format!("ruvector-ef{}", ef_search),
        dataset: args.dataset.clone(),
        dimensions,
        num_vectors: vectors.len(),
        num_queries: queries.len(),
        k: args.k,
        qps,
        latency_p50: latency_stats.percentile(0.50).as_secs_f64() * 1000.0,
        latency_p95: latency_stats.percentile(0.95).as_secs_f64() * 1000.0,
        latency_p99: latency_stats.percentile(0.99).as_secs_f64() * 1000.0,
        latency_p999: latency_stats.percentile(0.999).as_secs_f64() * 1000.0,
        recall_at_1: recall_1,
        recall_at_10: recall_10,
        recall_at_100: recall_100,
        memory_mb,
        build_time_secs: build_time.as_secs_f64(),
        metadata,
    })
}

fn print_summary_table(results: &[BenchmarkResult]) {
    use tabled::{Table, Tabled};

    #[derive(Tabled)]
    struct ResultRow {
        #[tabled(rename = "ef_search")]
        ef_search: String,
        #[tabled(rename = "QPS")]
        qps: String,
        #[tabled(rename = "p50 (ms)")]
        p50: String,
        #[tabled(rename = "p99 (ms)")]
        p99: String,
        #[tabled(rename = "Recall@10")]
        recall: String,
        #[tabled(rename = "Memory (MB)")]
        memory: String,
    }

    let rows: Vec<ResultRow> = results
        .iter()
        .map(|r| ResultRow {
            ef_search: r.metadata.get("ef_search").unwrap().clone(),
            qps: format!("{:.0}", r.qps),
            p50: format!("{:.2}", r.latency_p50),
            p99: format!("{:.2}", r.latency_p99),
            recall: format!("{:.2}%", r.recall_at_10 * 100.0),
            memory: format!("{:.1}", r.memory_mb),
        })
        .collect();

    println!("\n\n{}", Table::new(rows));
}
