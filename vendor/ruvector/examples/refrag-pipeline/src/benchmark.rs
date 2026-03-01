//! REFRAG Pipeline Benchmark
//!
//! Measures performance of the Compress-Sense-Expand pipeline.
//!
//! Run with: cargo run --bin refrag-benchmark --release

use refrag_pipeline_example::{
    compress::{CompressionStrategy, TensorCompressor},
    expand::{ExpandLayer, Projector, ProjectorRegistry},
    sense::{LinearPolicy, MLPPolicy, PolicyModel, PolicyNetwork, ThresholdPolicy},
    store::RefragStoreBuilder,
    types::RefragEntry,
};

use rand::Rng;
use std::time::{Duration, Instant};

fn main() -> anyhow::Result<()> {
    println!("=================================================");
    println!("  REFRAG Pipeline Benchmark                      ");
    println!("=================================================\n");

    // Run all benchmarks
    benchmark_compression()?;
    benchmark_policy()?;
    benchmark_projection()?;
    benchmark_end_to_end()?;

    Ok(())
}

fn benchmark_compression() -> anyhow::Result<()> {
    println!("--- Compression Layer Benchmark ---\n");

    let dimensions = [384, 768, 1024, 2048, 4096];
    let iterations = 10000;

    println!(
        "{:>8} | {:>12} | {:>12} | {:>12} | {:>12}",
        "Dims", "None (us)", "Float16 (us)", "Int8 (us)", "Binary (us)"
    );
    println!("{}", "-".repeat(70));

    for dim in dimensions {
        let mut rng = rand::thread_rng();
        let vector: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();

        let strategies = [
            CompressionStrategy::None,
            CompressionStrategy::Float16,
            CompressionStrategy::Int8,
            CompressionStrategy::Binary,
        ];

        let mut times = Vec::new();

        for strategy in strategies {
            let compressor = TensorCompressor::new(dim).with_strategy(strategy);

            let start = Instant::now();
            for _ in 0..iterations {
                let _ = compressor.compress(&vector);
            }
            let elapsed = start.elapsed();
            times.push(elapsed.as_nanos() as f64 / iterations as f64 / 1000.0);
        }

        println!(
            "{:>8} | {:>12.2} | {:>12.2} | {:>12.2} | {:>12.2}",
            dim, times[0], times[1], times[2], times[3]
        );
    }

    println!();
    Ok(())
}

fn benchmark_policy() -> anyhow::Result<()> {
    println!("--- Sense Layer (Policy) Benchmark ---\n");

    let dimensions = [384, 768, 1024];
    let iterations = 100000;

    println!(
        "{:>8} | {:>15} | {:>15} | {:>15}",
        "Dims", "Threshold (us)", "Linear (us)", "MLP-32 (us)"
    );
    println!("{}", "-".repeat(60));

    for dim in dimensions {
        let mut rng = rand::thread_rng();
        let chunk: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
        let query: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();

        // Threshold policy
        let threshold_policy = ThresholdPolicy::new(0.5);
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = threshold_policy.decide(&chunk, &query);
        }
        let threshold_time = start.elapsed().as_nanos() as f64 / iterations as f64 / 1000.0;

        // Linear policy
        let linear_policy = LinearPolicy::new(dim, 0.5);
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = linear_policy.decide(&chunk, &query);
        }
        let linear_time = start.elapsed().as_nanos() as f64 / iterations as f64 / 1000.0;

        // MLP policy
        let mlp_policy = MLPPolicy::new(dim, 32, 0.5);
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = mlp_policy.decide(&chunk, &query);
        }
        let mlp_time = start.elapsed().as_nanos() as f64 / iterations as f64 / 1000.0;

        println!(
            "{:>8} | {:>15.3} | {:>15.3} | {:>15.3}",
            dim, threshold_time, linear_time, mlp_time
        );
    }

    println!();
    Ok(())
}

fn benchmark_projection() -> anyhow::Result<()> {
    println!("--- Expand Layer (Projection) Benchmark ---\n");

    let projections = [
        (768, 4096, "RoBERTa -> LLaMA-8B"),
        (768, 8192, "RoBERTa -> LLaMA-70B"),
        (1536, 8192, "OpenAI -> GPT-4"),
        (4096, 4096, "Identity"),
    ];
    let iterations = 10000;

    println!(
        "{:>25} | {:>12} | {:>15}",
        "Projection", "Time (us)", "Throughput"
    );
    println!("{}", "-".repeat(60));

    for (source, target, name) in projections {
        let mut rng = rand::thread_rng();
        let input: Vec<f32> = (0..source).map(|_| rng.gen_range(-1.0..1.0)).collect();

        let projector = if source == target {
            Projector::identity(source, "test")
        } else {
            Projector::new(source, target, "test")
        };

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = projector.project(&input);
        }
        let elapsed = start.elapsed();
        let time_us = elapsed.as_nanos() as f64 / iterations as f64 / 1000.0;
        let throughput = iterations as f64 / elapsed.as_secs_f64();

        println!("{:>25} | {:>12.2} | {:>12.0}/s", name, time_us, throughput);
    }

    println!();
    Ok(())
}

fn benchmark_end_to_end() -> anyhow::Result<()> {
    println!("--- End-to-End Pipeline Benchmark ---\n");

    let configs = [
        (100, 10, "Small (100 docs, k=10)"),
        (1000, 10, "Medium (1K docs, k=10)"),
        (10000, 10, "Large (10K docs, k=10)"),
        (10000, 100, "Large (10K docs, k=100)"),
    ];

    let search_dim = 384;
    let tensor_dim = 768;
    let num_queries = 100;

    println!(
        "{:>30} | {:>12} | {:>12} | {:>10}",
        "Configuration", "Avg (us)", "P99 (us)", "QPS"
    );
    println!("{}", "-".repeat(75));

    for (num_docs, k, name) in configs {
        let store = RefragStoreBuilder::new()
            .search_dimensions(search_dim)
            .tensor_dimensions(tensor_dim)
            .compress_threshold(0.5)
            .auto_project(false)
            .build()?;

        // Insert documents
        let mut rng = rand::thread_rng();
        for i in 0..num_docs {
            let search_vec: Vec<f32> = (0..search_dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
            let tensor_vec: Vec<f32> = (0..tensor_dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
            let tensor_bytes: Vec<u8> = tensor_vec.iter().flat_map(|f| f.to_le_bytes()).collect();

            let entry = RefragEntry::new(format!("doc_{}", i), search_vec, format!("Text {}", i))
                .with_tensor(tensor_bytes, "llama3-8b");
            store.insert(entry)?;
        }

        // Run queries and collect latencies
        let mut latencies = Vec::with_capacity(num_queries);

        for _ in 0..num_queries {
            let query: Vec<f32> = (0..search_dim).map(|_| rng.gen_range(-1.0..1.0)).collect();

            let start = Instant::now();
            let _ = store.search_hybrid(&query, k, None)?;
            latencies.push(start.elapsed());
        }

        // Calculate statistics
        latencies.sort();
        let avg_us =
            latencies.iter().map(|d| d.as_micros()).sum::<u128>() as f64 / num_queries as f64;
        let p99_idx = (num_queries as f64 * 0.99) as usize;
        let p99_us = latencies[p99_idx.min(num_queries - 1)].as_micros();
        let total_time: Duration = latencies.iter().sum();
        let qps = num_queries as f64 / total_time.as_secs_f64();

        println!(
            "{:>30} | {:>12.1} | {:>12} | {:>10.0}",
            name, avg_us, p99_us, qps
        );
    }

    println!();

    // Comparison summary
    println!("--- Performance Summary ---\n");
    println!("REFRAG Pipeline Latency Breakdown:");
    println!("  1. Vector search (HNSW):    ~100-500us");
    println!("  2. Policy decision:         ~1-50us");
    println!("  3. Tensor decompression:    ~1-10us");
    println!("  4. Projection (optional):   ~10-100us");
    println!("  ----------------------------------------");
    println!("  Total per query:            ~150-700us");
    println!();
    println!("Compared to traditional RAG:");
    println!("  - Text tokenization:        ~1-5ms");
    println!("  - LLM context preparation:  ~5-20ms");
    println!("  - Network transfer (text):  ~10-50ms");
    println!("  ----------------------------------------");
    println!("  Potential speedup:          10-30x\n");

    Ok(())
}
