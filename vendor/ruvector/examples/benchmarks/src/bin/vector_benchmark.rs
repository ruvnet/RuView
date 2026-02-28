//! Vector Index Benchmark Runner
//!
//! Benchmark vector operations with IVF and coherence gating.
//!
//! Usage:
//!   cargo run --bin vector-benchmark -- --dim 128 --vectors 10000

use anyhow::Result;
use clap::Parser;
use ruvector_benchmarks::{
    logging::BenchmarkLogger,
    vector_index::{CoherenceGate, DenseVec, IvfConfig, VectorIndex},
};
use std::time::Instant;

#[derive(Parser, Debug)]
#[command(name = "vector-benchmark")]
#[command(about = "Benchmark vector index operations")]
struct Args {
    /// Vector dimensionality
    #[arg(short, long, default_value = "128")]
    dim: usize,

    /// Number of vectors to insert
    #[arg(short = 'n', long, default_value = "10000")]
    vectors: usize,

    /// Number of queries to run
    #[arg(short, long, default_value = "1000")]
    queries: usize,

    /// Top-k results per query
    #[arg(short, long, default_value = "10")]
    top_k: usize,

    /// Enable IVF indexing
    #[arg(long, default_value = "true")]
    ivf: bool,

    /// Number of IVF clusters
    #[arg(long, default_value = "64")]
    clusters: usize,

    /// Number of clusters to probe
    #[arg(long, default_value = "4")]
    probes: usize,

    /// Enable coherence gate
    #[arg(long)]
    gate: bool,

    /// Coherence gate threshold
    #[arg(long, default_value = "0.5")]
    gate_threshold: f32,

    /// Output log file
    #[arg(short, long, default_value = "logs/vector_benchmark.jsonl")]
    output: String,

    /// Verbose output
    #[arg(short = 'V', long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              Vector Index Benchmark Runner                    ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Initialize logger
    let mut logger = BenchmarkLogger::new(&args.output)?;
    logger.log_system("INFO", "Starting vector benchmark", "vector-benchmark")?;

    // Create index
    println!("🔧 Configuration:");
    println!("   Dimensions:  {}", args.dim);
    println!("   Vectors:     {}", args.vectors);
    println!("   Queries:     {}", args.queries);
    println!("   Top-K:       {}", args.top_k);
    println!("   IVF:         {}", args.ivf);
    if args.ivf {
        println!("   Clusters:    {}", args.clusters);
        println!("   Probes:      {}", args.probes);
    }
    println!("   Gate:        {}", args.gate);
    if args.gate {
        println!("   Threshold:   {}", args.gate_threshold);
    }
    println!();

    let mut index = VectorIndex::new(args.dim);

    if args.gate {
        index = index.with_gate(CoherenceGate::new(args.gate_threshold));
    }

    if args.ivf {
        index = index.with_ivf(IvfConfig::new(args.clusters, args.probes));
    }

    // Insert vectors
    println!("📥 Inserting {} vectors...", args.vectors);
    let insert_start = Instant::now();

    for i in 0..args.vectors {
        index.insert(DenseVec::random(args.dim))?;
        if args.verbose && (i + 1) % 1000 == 0 {
            println!("   Inserted {} vectors", i + 1);
        }
    }

    let insert_time = insert_start.elapsed();
    println!(
        "✓ Insert complete ({:.2}s, {:.0} vec/s)",
        insert_time.as_secs_f64(),
        args.vectors as f64 / insert_time.as_secs_f64()
    );
    println!();

    // Build IVF if enabled
    if args.ivf {
        println!("🏗️  Building IVF index...");
        let build_start = Instant::now();
        index.rebuild_ivf()?;
        let build_time = build_start.elapsed();
        println!("✓ IVF build complete ({:.2}s)", build_time.as_secs_f64());
        println!();
    }

    // Print index stats
    let stats = index.stats();
    println!("📊 Index Statistics:");
    println!("   Active vectors:  {}", stats.active_vectors);
    println!("   IVF clusters:    {}", stats.ivf_clusters);
    println!();

    // Run queries
    println!("🔍 Running {} queries...", args.queries);
    let query_start = Instant::now();

    let mut latencies: Vec<u64> = Vec::with_capacity(args.queries);
    let mut total_results = 0usize;

    for i in 0..args.queries {
        let q = DenseVec::random(args.dim);
        let coherence = if args.gate {
            rand::random::<f32>()
        } else {
            1.0
        };

        let start = Instant::now();
        let results = index.search(&q, args.top_k, coherence)?;
        let latency_us = start.elapsed().as_micros() as u64;

        latencies.push(latency_us);
        total_results += results.len();

        // Log query
        logger.log_vector(
            "search",
            args.dim,
            stats.active_vectors,
            1,
            args.top_k,
            args.ivf,
            coherence,
            latency_us,
            results.len(),
        )?;

        if args.verbose && (i + 1) % 100 == 0 {
            println!("   Completed {} queries", i + 1);
        }
    }

    let query_time = query_start.elapsed();
    println!(
        "✓ Queries complete ({:.2}s, {:.0} q/s)",
        query_time.as_secs_f64(),
        args.queries as f64 / query_time.as_secs_f64()
    );
    println!();

    // Compute statistics
    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[latencies.len() * 95 / 100];
    let p99 = latencies[latencies.len() * 99 / 100];
    let avg = latencies.iter().sum::<u64>() / latencies.len() as u64;
    let max = *latencies.last().unwrap();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                     Benchmark Results                         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("⏱️  Latency (microseconds):");
    println!("   Average: {}µs", avg);
    println!("   P50:     {}µs", p50);
    println!("   P95:     {}µs", p95);
    println!("   P99:     {}µs", p99);
    println!("   Max:     {}µs", max);
    println!();
    println!("📈 Throughput:");
    println!(
        "   Queries/sec:    {:.0}",
        args.queries as f64 / query_time.as_secs_f64()
    );
    println!(
        "   Insert/sec:     {:.0}",
        args.vectors as f64 / insert_time.as_secs_f64()
    );
    println!();
    println!("📊 Results:");
    println!("   Total results:  {}", total_results);
    println!(
        "   Avg results:    {:.2}",
        total_results as f64 / args.queries as f64
    );

    if args.gate {
        let gated = latencies
            .iter()
            .enumerate()
            .filter(|(_, &l)| l < 10)
            .count();
        println!(
            "   Gated queries:  {:.1}%",
            gated as f64 / args.queries as f64 * 100.0
        );
    }

    // Save index
    println!();
    let index_path = "data/vector_index.bin";
    std::fs::create_dir_all("data")?;
    index.save_to_file(index_path)?;
    println!("💾 Index saved to: {}", index_path);

    // Flush logs
    logger.flush()?;
    println!("📝 Results saved to: {}", args.output);

    Ok(())
}
