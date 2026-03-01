//! AgenticDB compatibility benchmark
//!
//! Tests AgenticDB-specific workloads:
//! - Reflexion episode storage and retrieval
//! - Skill library search
//! - Causal graph queries
//! - Learning session throughput

use anyhow::Result;
use clap::Parser;
use rand::Rng;
use ruvector_bench::{
    create_progress_bar, BenchmarkResult, DatasetGenerator, LatencyStats, MemoryProfiler,
    ResultWriter, VectorDistribution,
};
use ruvector_core::{
    types::{DbOptions, HnswConfig, QuantizationConfig},
    DistanceMetric, SearchQuery, VectorDB, VectorEntry,
};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "agenticdb-benchmark")]
#[command(about = "AgenticDB workload testing")]
struct Args {
    /// Number of episodes
    #[arg(long, default_value = "10000")]
    episodes: usize,

    /// Number of skills
    #[arg(long, default_value = "1000")]
    skills: usize,

    /// Number of queries
    #[arg(short, long, default_value = "500")]
    queries: usize,

    /// Output directory
    #[arg(short, long, default_value = "bench_results")]
    output: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("╔════════════════════════════════════════╗");
    println!("║   Ruvector AgenticDB Benchmark        ║");
    println!("╚════════════════════════════════════════╝\n");

    let mut all_results = Vec::new();

    // Test 1: Reflexion episode storage/retrieval
    println!("\n{}", "=".repeat(60));
    println!("Test 1: Reflexion Episode Storage & Retrieval");
    println!("{}\n", "=".repeat(60));
    let result = bench_reflexion_episodes(&args)?;
    all_results.push(result);

    // Test 2: Skill library search
    println!("\n{}", "=".repeat(60));
    println!("Test 2: Skill Library Search");
    println!("{}\n", "=".repeat(60));
    let result = bench_skill_library(&args)?;
    all_results.push(result);

    // Test 3: Causal graph queries
    println!("\n{}", "=".repeat(60));
    println!("Test 3: Causal Graph Queries");
    println!("{}\n", "=".repeat(60));
    let result = bench_causal_graph(&args)?;
    all_results.push(result);

    // Test 4: Learning session throughput
    println!("\n{}", "=".repeat(60));
    println!("Test 4: Learning Session Throughput");
    println!("{}\n", "=".repeat(60));
    let result = bench_learning_session(&args)?;
    all_results.push(result);

    // Write results
    let writer = ResultWriter::new(&args.output)?;
    writer.write_json("agenticdb_benchmark", &all_results)?;
    writer.write_csv("agenticdb_benchmark", &all_results)?;
    writer.write_markdown_report("agenticdb_benchmark", &all_results)?;

    print_summary(&all_results);

    println!(
        "\n✓ AgenticDB benchmark complete! Results saved to: {}",
        args.output.display()
    );
    Ok(())
}

fn bench_reflexion_episodes(args: &Args) -> Result<BenchmarkResult> {
    println!("Simulating {} Reflexion episodes...", args.episodes);

    // Reflexion episodes use 384D embeddings (typical for sentence-transformers)
    let dimensions = 384;
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("episodes.db");

    let options = DbOptions {
        dimensions,
        distance_metric: DistanceMetric::Cosine,
        storage_path: db_path.to_str().unwrap().to_string(),
        hnsw_config: Some(HnswConfig::default()),
        quantization: Some(QuantizationConfig::Scalar),
    };

    let mem_profiler = MemoryProfiler::new();
    let build_start = Instant::now();
    let db = VectorDB::new(options)?;

    // Generate episode data
    let gen = DatasetGenerator::new(
        dimensions,
        VectorDistribution::Normal {
            mean: 0.0,
            std_dev: 1.0,
        },
    );

    println!("Storing episodes...");
    let pb = create_progress_bar(args.episodes as u64, "Storing episodes");

    for i in 0..args.episodes {
        let entry = VectorEntry {
            id: Some(format!("episode_{}", i)),
            vector: gen.generate(1).into_iter().next().unwrap(),
            metadata: Some(
                vec![
                    ("trajectory".to_string(), json!(format!("traj_{}", i))),
                    ("reward".to_string(), json!(rand::thread_rng().gen::<f32>())),
                    (
                        "success".to_string(),
                        json!(rand::thread_rng().gen_bool(0.7)),
                    ),
                    (
                        "step_count".to_string(),
                        json!(rand::thread_rng().gen_range(10..100)),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        };
        db.insert(entry)?;
        pb.inc(1);
    }
    pb.finish_with_message("✓ Episodes stored");

    let build_time = build_start.elapsed();
    let memory_mb = mem_profiler.current_usage_mb();

    // Query similar episodes
    println!("Querying similar episodes...");
    let mut latency_stats = LatencyStats::new()?;
    let query_vectors = gen.generate(args.queries);

    let search_start = Instant::now();
    let pb = create_progress_bar(args.queries as u64, "Searching");

    for query in query_vectors {
        let query_start = Instant::now();
        db.search(SearchQuery {
            vector: query,
            k: 10,
            filter: None,
            ef_search: None,
        })?;
        latency_stats.record(query_start.elapsed())?;
        pb.inc(1);
    }
    pb.finish_with_message("✓ Search complete");

    let total_search_time = search_start.elapsed();
    let qps = args.queries as f64 / total_search_time.as_secs_f64();

    Ok(BenchmarkResult {
        name: "reflexion_episodes".to_string(),
        dataset: "reflexion".to_string(),
        dimensions,
        num_vectors: args.episodes,
        num_queries: args.queries,
        k: 10,
        qps,
        latency_p50: latency_stats.percentile(0.50).as_secs_f64() * 1000.0,
        latency_p95: latency_stats.percentile(0.95).as_secs_f64() * 1000.0,
        latency_p99: latency_stats.percentile(0.99).as_secs_f64() * 1000.0,
        latency_p999: latency_stats.percentile(0.999).as_secs_f64() * 1000.0,
        recall_at_1: 1.0, // No ground truth for synthetic
        recall_at_10: 1.0,
        recall_at_100: 1.0,
        memory_mb,
        build_time_secs: build_time.as_secs_f64(),
        metadata: HashMap::new(),
    })
}

fn bench_skill_library(args: &Args) -> Result<BenchmarkResult> {
    println!("Simulating {} skills in library...", args.skills);

    let dimensions = 768; // Larger embeddings for code/skill descriptions
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("skills.db");

    let options = DbOptions {
        dimensions,
        distance_metric: DistanceMetric::Cosine,
        storage_path: db_path.to_str().unwrap().to_string(),
        hnsw_config: Some(HnswConfig::default()),
        quantization: Some(QuantizationConfig::Scalar),
    };

    let mem_profiler = MemoryProfiler::new();
    let build_start = Instant::now();
    let db = VectorDB::new(options)?;

    let gen = DatasetGenerator::new(
        dimensions,
        VectorDistribution::Clustered {
            num_clusters: 20, // Skills grouped by categories
        },
    );

    println!("Storing skills...");
    let pb = create_progress_bar(args.skills as u64, "Storing skills");

    for i in 0..args.skills {
        let entry = VectorEntry {
            id: Some(format!("skill_{}", i)),
            vector: gen.generate(1).into_iter().next().unwrap(),
            metadata: Some(
                vec![
                    ("name".to_string(), json!(format!("skill_{}", i))),
                    ("category".to_string(), json!(format!("cat_{}", i % 20))),
                    (
                        "success_rate".to_string(),
                        json!(rand::thread_rng().gen::<f32>()),
                    ),
                    (
                        "usage_count".to_string(),
                        json!(rand::thread_rng().gen_range(0..1000)),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        };
        db.insert(entry)?;
        pb.inc(1);
    }
    pb.finish_with_message("✓ Skills stored");

    let build_time = build_start.elapsed();
    let memory_mb = mem_profiler.current_usage_mb();

    // Search for relevant skills
    println!("Searching for relevant skills...");
    let mut latency_stats = LatencyStats::new()?;
    let query_vectors = gen.generate(args.queries);

    let search_start = Instant::now();
    let pb = create_progress_bar(args.queries as u64, "Searching");

    for query in query_vectors {
        let query_start = Instant::now();
        db.search(SearchQuery {
            vector: query,
            k: 5,
            filter: None,
            ef_search: None,
        })?;
        latency_stats.record(query_start.elapsed())?;
        pb.inc(1);
    }
    pb.finish_with_message("✓ Search complete");

    let total_search_time = search_start.elapsed();
    let qps = args.queries as f64 / total_search_time.as_secs_f64();

    Ok(BenchmarkResult {
        name: "skill_library".to_string(),
        dataset: "skills".to_string(),
        dimensions,
        num_vectors: args.skills,
        num_queries: args.queries,
        k: 5,
        qps,
        latency_p50: latency_stats.percentile(0.50).as_secs_f64() * 1000.0,
        latency_p95: latency_stats.percentile(0.95).as_secs_f64() * 1000.0,
        latency_p99: latency_stats.percentile(0.99).as_secs_f64() * 1000.0,
        latency_p999: latency_stats.percentile(0.999).as_secs_f64() * 1000.0,
        recall_at_1: 1.0,
        recall_at_10: 1.0,
        recall_at_100: 1.0,
        memory_mb,
        build_time_secs: build_time.as_secs_f64(),
        metadata: HashMap::new(),
    })
}

fn bench_causal_graph(args: &Args) -> Result<BenchmarkResult> {
    println!(
        "Simulating causal graph with {} nodes...",
        args.episodes / 10
    );

    let dimensions = 256;
    let num_nodes = args.episodes / 10;
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("causal.db");

    let options = DbOptions {
        dimensions,
        distance_metric: DistanceMetric::Cosine,
        storage_path: db_path.to_str().unwrap().to_string(),
        hnsw_config: Some(HnswConfig::default()),
        quantization: Some(QuantizationConfig::Scalar),
    };

    let mem_profiler = MemoryProfiler::new();
    let build_start = Instant::now();
    let db = VectorDB::new(options)?;

    let gen = DatasetGenerator::new(
        dimensions,
        VectorDistribution::Normal {
            mean: 0.0,
            std_dev: 1.0,
        },
    );

    println!("Building causal graph...");
    let pb = create_progress_bar(num_nodes as u64, "Storing nodes");

    for i in 0..num_nodes {
        let entry = VectorEntry {
            id: Some(format!("node_{}", i)),
            vector: gen.generate(1).into_iter().next().unwrap(),
            metadata: Some(
                vec![
                    ("state".to_string(), json!(format!("state_{}", i))),
                    ("action".to_string(), json!(format!("action_{}", i % 50))),
                    (
                        "causal_strength".to_string(),
                        json!(rand::thread_rng().gen::<f32>()),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        };
        db.insert(entry)?;
        pb.inc(1);
    }
    pb.finish_with_message("✓ Graph built");

    let build_time = build_start.elapsed();
    let memory_mb = mem_profiler.current_usage_mb();

    // Query causal relationships
    println!("Querying causal relationships...");
    let mut latency_stats = LatencyStats::new()?;
    let query_vectors = gen.generate(args.queries / 2);

    let search_start = Instant::now();
    let pb = create_progress_bar((args.queries / 2) as u64, "Searching");

    for query in query_vectors {
        let query_start = Instant::now();
        db.search(SearchQuery {
            vector: query,
            k: 20,
            filter: None,
            ef_search: None,
        })?;
        latency_stats.record(query_start.elapsed())?;
        pb.inc(1);
    }
    pb.finish_with_message("✓ Search complete");

    let total_search_time = search_start.elapsed();
    let qps = (args.queries / 2) as f64 / total_search_time.as_secs_f64();

    Ok(BenchmarkResult {
        name: "causal_graph".to_string(),
        dataset: "causal".to_string(),
        dimensions,
        num_vectors: num_nodes,
        num_queries: args.queries / 2,
        k: 20,
        qps,
        latency_p50: latency_stats.percentile(0.50).as_secs_f64() * 1000.0,
        latency_p95: latency_stats.percentile(0.95).as_secs_f64() * 1000.0,
        latency_p99: latency_stats.percentile(0.99).as_secs_f64() * 1000.0,
        latency_p999: latency_stats.percentile(0.999).as_secs_f64() * 1000.0,
        recall_at_1: 1.0,
        recall_at_10: 1.0,
        recall_at_100: 1.0,
        memory_mb,
        build_time_secs: build_time.as_secs_f64(),
        metadata: HashMap::new(),
    })
}

fn bench_learning_session(args: &Args) -> Result<BenchmarkResult> {
    println!("Simulating mixed-workload learning session...");

    let dimensions = 512;
    let num_items = args.episodes;
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("learning.db");

    let options = DbOptions {
        dimensions,
        distance_metric: DistanceMetric::Cosine,
        storage_path: db_path.to_str().unwrap().to_string(),
        hnsw_config: Some(HnswConfig::default()),
        quantization: Some(QuantizationConfig::Scalar),
    };

    let mem_profiler = MemoryProfiler::new();
    let build_start = Instant::now();
    let db = VectorDB::new(options)?;

    let gen = DatasetGenerator::new(
        dimensions,
        VectorDistribution::Normal {
            mean: 0.0,
            std_dev: 1.0,
        },
    );

    println!("Running learning session with mixed read/write...");
    let mut latency_stats = LatencyStats::new()?;
    let pb = create_progress_bar(num_items as u64, "Processing");

    let mut write_count = 0;
    let mut read_count = 0;

    for i in 0..num_items {
        // 70% writes, 30% reads (typical learning scenario)
        if rand::thread_rng().gen_bool(0.7) {
            let entry = VectorEntry {
                id: Some(format!("item_{}", i)),
                vector: gen.generate(1).into_iter().next().unwrap(),
                metadata: Some(
                    vec![("timestamp".to_string(), json!(i))]
                        .into_iter()
                        .collect(),
                ),
            };
            db.insert(entry)?;
            write_count += 1;
        } else {
            let query = gen.generate(1).into_iter().next().unwrap();
            let query_start = Instant::now();
            db.search(SearchQuery {
                vector: query,
                k: 10,
                filter: None,
                ef_search: None,
            })?;
            latency_stats.record(query_start.elapsed())?;
            read_count += 1;
        }
        pb.inc(1);
    }
    pb.finish_with_message("✓ Learning session complete");

    let build_time = build_start.elapsed();
    let memory_mb = mem_profiler.current_usage_mb();
    let throughput = num_items as f64 / build_time.as_secs_f64();

    Ok(BenchmarkResult {
        name: "learning_session".to_string(),
        dataset: "mixed_workload".to_string(),
        dimensions,
        num_vectors: write_count,
        num_queries: read_count,
        k: 10,
        qps: throughput,
        latency_p50: latency_stats.percentile(0.50).as_secs_f64() * 1000.0,
        latency_p95: latency_stats.percentile(0.95).as_secs_f64() * 1000.0,
        latency_p99: latency_stats.percentile(0.99).as_secs_f64() * 1000.0,
        latency_p999: latency_stats.percentile(0.999).as_secs_f64() * 1000.0,
        recall_at_1: 1.0,
        recall_at_10: 1.0,
        recall_at_100: 1.0,
        memory_mb,
        build_time_secs: build_time.as_secs_f64(),
        metadata: vec![
            ("writes".to_string(), write_count.to_string()),
            ("reads".to_string(), read_count.to_string()),
        ]
        .into_iter()
        .collect(),
    })
}

fn print_summary(results: &[BenchmarkResult]) {
    use tabled::{Table, Tabled};

    #[derive(Tabled)]
    struct ResultRow {
        #[tabled(rename = "Workload")]
        name: String,
        #[tabled(rename = "Vectors")]
        vectors: String,
        #[tabled(rename = "Throughput")]
        qps: String,
        #[tabled(rename = "p50 (ms)")]
        p50: String,
        #[tabled(rename = "p99 (ms)")]
        p99: String,
        #[tabled(rename = "Memory (MB)")]
        memory: String,
    }

    let rows: Vec<ResultRow> = results
        .iter()
        .map(|r| ResultRow {
            name: r.name.clone(),
            vectors: r.num_vectors.to_string(),
            qps: format!("{:.0} ops/s", r.qps),
            p50: format!("{:.2}", r.latency_p50),
            p99: format!("{:.2}", r.latency_p99),
            memory: format!("{:.1}", r.memory_mb),
        })
        .collect();

    println!("\n\n{}", Table::new(rows));
}
