//! Memory usage profiling benchmark
//!
//! Measures memory consumption at various scales and configurations:
//! - Memory usage at 10K, 100K, 1M vectors
//! - Effect of quantization on memory
//! - Index overhead measurement

use anyhow::Result;
use clap::Parser;
use ruvector_bench::{
    create_progress_bar, BenchmarkResult, DatasetGenerator, MemoryProfiler, ResultWriter,
    VectorDistribution,
};
use ruvector_core::{
    types::{DbOptions, HnswConfig, QuantizationConfig},
    DistanceMetric, VectorDB, VectorEntry,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "memory-benchmark")]
#[command(about = "Memory usage profiling")]
struct Args {
    /// Vector dimensions
    #[arg(short, long, default_value = "384")]
    dimensions: usize,

    /// Scales to test (comma-separated)
    #[arg(short, long, default_value = "1000,10000,100000")]
    scales: String,

    /// Output directory
    #[arg(short, long, default_value = "bench_results")]
    output: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("╔════════════════════════════════════════╗");
    println!("║   Ruvector Memory Profiling           ║");
    println!("╚════════════════════════════════════════╝\n");

    let mut all_results = Vec::new();

    // Parse scales
    let scales: Vec<usize> = args
        .scales
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();

    // Test 1: Memory usage at different scales
    for &scale in &scales {
        println!("\n{}", "=".repeat(60));
        println!("Test: Memory at {} vectors", scale);
        println!("{}\n", "=".repeat(60));
        let result = bench_memory_scale(&args, scale)?;
        all_results.push(result);
    }

    // Test 2: Effect of quantization on memory
    println!("\n{}", "=".repeat(60));
    println!("Test: Effect of Quantization on Memory");
    println!("{}\n", "=".repeat(60));
    let results = bench_quantization_memory(&args)?;
    all_results.extend(results);

    // Test 3: Index overhead analysis
    println!("\n{}", "=".repeat(60));
    println!("Test: Index Overhead Analysis");
    println!("{}\n", "=".repeat(60));
    let result = bench_index_overhead(&args)?;
    all_results.push(result);

    // Write results
    let writer = ResultWriter::new(&args.output)?;
    writer.write_json("memory_benchmark", &all_results)?;
    writer.write_csv("memory_benchmark", &all_results)?;
    writer.write_markdown_report("memory_benchmark", &all_results)?;

    print_summary(&all_results);

    println!(
        "\n✓ Memory benchmark complete! Results saved to: {}",
        args.output.display()
    );
    Ok(())
}

fn bench_memory_scale(args: &Args, num_vectors: usize) -> Result<BenchmarkResult> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("memory_scale.db");

    let options = DbOptions {
        dimensions: args.dimensions,
        distance_metric: DistanceMetric::Cosine,
        storage_path: db_path.to_str().unwrap().to_string(),
        hnsw_config: Some(HnswConfig::default()),
        quantization: Some(QuantizationConfig::Scalar),
    };

    let mem_profiler = MemoryProfiler::new();
    let initial_mb = mem_profiler.current_usage_mb();

    println!("Initial memory: {:.2} MB", initial_mb);
    println!("Indexing {} vectors...", num_vectors);

    let build_start = Instant::now();
    let db = VectorDB::new(options)?;

    let gen = DatasetGenerator::new(
        args.dimensions,
        VectorDistribution::Normal {
            mean: 0.0,
            std_dev: 1.0,
        },
    );

    let pb = create_progress_bar(num_vectors as u64, "Indexing");

    for i in 0..num_vectors {
        let entry = VectorEntry {
            id: Some(i.to_string()),
            vector: gen.generate(1).into_iter().next().unwrap(),
            metadata: None,
        };
        db.insert(entry)?;

        // Sample memory every 10%
        if i % (num_vectors / 10).max(1) == 0 {
            let current_mb = mem_profiler.current_usage_mb();
            println!(
                "  Progress: {}%, Memory: {:.2} MB",
                (i * 100) / num_vectors,
                current_mb
            );
        }
        pb.inc(1);
    }
    pb.finish_with_message("✓ Indexing complete");

    let build_time = build_start.elapsed();
    let final_mb = mem_profiler.current_usage_mb();
    let memory_per_vector_kb = (final_mb - initial_mb) * 1024.0 / num_vectors as f64;

    println!("Final memory: {:.2} MB", final_mb);
    println!("Memory per vector: {:.2} KB", memory_per_vector_kb);

    // Calculate theoretical minimum
    let vector_size_bytes = args.dimensions * 4; // 4 bytes per f32
    let theoretical_mb = (num_vectors * vector_size_bytes) as f64 / 1_048_576.0;
    let overhead_ratio = final_mb / theoretical_mb;

    println!("Theoretical minimum: {:.2} MB", theoretical_mb);
    println!("Overhead ratio: {:.2}x", overhead_ratio);

    Ok(BenchmarkResult {
        name: format!("memory_scale_{}", num_vectors),
        dataset: "synthetic".to_string(),
        dimensions: args.dimensions,
        num_vectors,
        num_queries: 0,
        k: 0,
        qps: 0.0,
        latency_p50: 0.0,
        latency_p95: 0.0,
        latency_p99: 0.0,
        latency_p999: 0.0,
        recall_at_1: 0.0,
        recall_at_10: 0.0,
        recall_at_100: 0.0,
        memory_mb: final_mb,
        build_time_secs: build_time.as_secs_f64(),
        metadata: vec![
            (
                "memory_per_vector_kb".to_string(),
                format!("{:.2}", memory_per_vector_kb),
            ),
            (
                "theoretical_mb".to_string(),
                format!("{:.2}", theoretical_mb),
            ),
            (
                "overhead_ratio".to_string(),
                format!("{:.2}", overhead_ratio),
            ),
        ]
        .into_iter()
        .collect(),
    })
}

fn bench_quantization_memory(args: &Args) -> Result<Vec<BenchmarkResult>> {
    let quantizations = vec![
        ("none", QuantizationConfig::None),
        ("scalar", QuantizationConfig::Scalar),
        ("binary", QuantizationConfig::Binary),
    ];

    let num_vectors = 50_000;
    let mut results = Vec::new();

    for (name, quant_config) in quantizations {
        println!("Testing quantization: {}...", name);

        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("quant_memory.db");

        let options = DbOptions {
            dimensions: args.dimensions,
            distance_metric: DistanceMetric::Cosine,
            storage_path: db_path.to_str().unwrap().to_string(),
            hnsw_config: Some(HnswConfig::default()),
            quantization: Some(quant_config),
        };

        let mem_profiler = MemoryProfiler::new();
        let build_start = Instant::now();
        let db = VectorDB::new(options)?;

        let gen = DatasetGenerator::new(
            args.dimensions,
            VectorDistribution::Normal {
                mean: 0.0,
                std_dev: 1.0,
            },
        );

        let pb = create_progress_bar(num_vectors as u64, &format!("quant={}", name));

        for i in 0..num_vectors {
            let entry = VectorEntry {
                id: Some(i.to_string()),
                vector: gen.generate(1).into_iter().next().unwrap(),
                metadata: None,
            };
            db.insert(entry)?;
            pb.inc(1);
        }
        pb.finish_with_message(format!("✓ {} complete", name));

        let build_time = build_start.elapsed();
        let memory_mb = mem_profiler.current_usage_mb();

        let vector_size_bytes = args.dimensions * 4;
        let theoretical_mb = (num_vectors * vector_size_bytes) as f64 / 1_048_576.0;
        let compression_ratio = theoretical_mb / memory_mb;

        println!(
            "  Memory: {:.2} MB, Compression: {:.2}x",
            memory_mb, compression_ratio
        );

        results.push(BenchmarkResult {
            name: format!("quantization_{}", name),
            dataset: "synthetic".to_string(),
            dimensions: args.dimensions,
            num_vectors,
            num_queries: 0,
            k: 0,
            qps: 0.0,
            latency_p50: 0.0,
            latency_p95: 0.0,
            latency_p99: 0.0,
            latency_p999: 0.0,
            recall_at_1: 0.0,
            recall_at_10: 0.0,
            recall_at_100: 0.0,
            memory_mb,
            build_time_secs: build_time.as_secs_f64(),
            metadata: vec![
                ("quantization".to_string(), name.to_string()),
                (
                    "compression_ratio".to_string(),
                    format!("{:.2}", compression_ratio),
                ),
                (
                    "theoretical_mb".to_string(),
                    format!("{:.2}", theoretical_mb),
                ),
            ]
            .into_iter()
            .collect(),
        });
    }

    Ok(results)
}

fn bench_index_overhead(args: &Args) -> Result<BenchmarkResult> {
    let num_vectors = 100_000;

    println!("Analyzing index overhead for {} vectors...", num_vectors);

    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("overhead.db");

    let options = DbOptions {
        dimensions: args.dimensions,
        distance_metric: DistanceMetric::Cosine,
        storage_path: db_path.to_str().unwrap().to_string(),
        hnsw_config: Some(HnswConfig {
            m: 32,
            ef_construction: 200,
            ef_search: 100,
            max_elements: num_vectors * 2,
        }),
        quantization: Some(QuantizationConfig::None), // No quantization for overhead analysis
    };

    let mem_profiler = MemoryProfiler::new();
    let build_start = Instant::now();
    let db = VectorDB::new(options)?;

    let gen = DatasetGenerator::new(
        args.dimensions,
        VectorDistribution::Normal {
            mean: 0.0,
            std_dev: 1.0,
        },
    );

    let pb = create_progress_bar(num_vectors as u64, "Building index");

    for i in 0..num_vectors {
        let entry = VectorEntry {
            id: Some(i.to_string()),
            vector: gen.generate(1).into_iter().next().unwrap(),
            metadata: None,
        };
        db.insert(entry)?;
        pb.inc(1);
    }
    pb.finish_with_message("✓ Index built");

    let build_time = build_start.elapsed();
    let total_memory_mb = mem_profiler.current_usage_mb();

    // Calculate components
    let vector_data_mb = (num_vectors * args.dimensions * 4) as f64 / 1_048_576.0;
    let index_overhead_mb = total_memory_mb - vector_data_mb;
    let overhead_percentage = (index_overhead_mb / vector_data_mb) * 100.0;

    println!("\nMemory Breakdown:");
    println!("  Vector data: {:.2} MB", vector_data_mb);
    println!(
        "  Index overhead: {:.2} MB ({:.1}%)",
        index_overhead_mb, overhead_percentage
    );
    println!("  Total: {:.2} MB", total_memory_mb);

    Ok(BenchmarkResult {
        name: "index_overhead".to_string(),
        dataset: "synthetic".to_string(),
        dimensions: args.dimensions,
        num_vectors,
        num_queries: 0,
        k: 0,
        qps: 0.0,
        latency_p50: 0.0,
        latency_p95: 0.0,
        latency_p99: 0.0,
        latency_p999: 0.0,
        recall_at_1: 0.0,
        recall_at_10: 0.0,
        recall_at_100: 0.0,
        memory_mb: total_memory_mb,
        build_time_secs: build_time.as_secs_f64(),
        metadata: vec![
            (
                "vector_data_mb".to_string(),
                format!("{:.2}", vector_data_mb),
            ),
            (
                "index_overhead_mb".to_string(),
                format!("{:.2}", index_overhead_mb),
            ),
            (
                "overhead_percentage".to_string(),
                format!("{:.1}", overhead_percentage),
            ),
        ]
        .into_iter()
        .collect(),
    })
}

fn print_summary(results: &[BenchmarkResult]) {
    use tabled::{Table, Tabled};

    #[derive(Tabled)]
    struct ResultRow {
        #[tabled(rename = "Configuration")]
        name: String,
        #[tabled(rename = "Vectors")]
        vectors: String,
        #[tabled(rename = "Memory (MB)")]
        memory: String,
        #[tabled(rename = "Per Vector")]
        per_vector: String,
        #[tabled(rename = "Build Time (s)")]
        build_time: String,
    }

    let rows: Vec<ResultRow> = results
        .iter()
        .map(|r| {
            let per_vector = if r.num_vectors > 0 {
                format!("{:.2} KB", (r.memory_mb * 1024.0) / r.num_vectors as f64)
            } else {
                "N/A".to_string()
            };

            ResultRow {
                name: r.name.clone(),
                vectors: if r.num_vectors > 0 {
                    r.num_vectors.to_string()
                } else {
                    "N/A".to_string()
                },
                memory: format!("{:.2}", r.memory_mb),
                per_vector,
                build_time: format!("{:.2}", r.build_time_secs),
            }
        })
        .collect();

    println!("\n\n{}", Table::new(rows));
}
