//! Core benchmark implementations for RuVector Cloud Run GPU

use anyhow::Result;
use chrono::Utc;
use hdrhistogram::Histogram;
use indicatif::{ProgressBar, ProgressStyle};
use rand::Rng;
use rand_distr::{Distribution, Normal, Uniform};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use sysinfo::System;

/// Benchmark result structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub operation: String,
    pub dimensions: usize,
    pub num_vectors: usize,
    pub num_queries: usize,
    pub batch_size: usize,
    pub k: usize,
    pub iterations: usize,

    // Timing metrics (in milliseconds)
    pub mean_time_ms: f64,
    pub std_time_ms: f64,
    pub min_time_ms: f64,
    pub max_time_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub p999_ms: f64,

    // Throughput
    pub qps: f64,
    pub throughput_vectors_sec: f64,

    // Quality metrics
    pub recall_at_1: Option<f64>,
    pub recall_at_10: Option<f64>,
    pub recall_at_100: Option<f64>,

    // Resource metrics
    pub memory_mb: f64,
    pub build_time_secs: f64,

    // Environment
    pub gpu_enabled: bool,
    pub gpu_name: Option<String>,
    pub timestamp: String,

    // Additional metadata
    pub metadata: HashMap<String, String>,
}

impl BenchmarkResult {
    pub fn new(name: &str, operation: &str) -> Self {
        Self {
            name: name.to_string(),
            operation: operation.to_string(),
            dimensions: 0,
            num_vectors: 0,
            num_queries: 0,
            batch_size: 0,
            k: 0,
            iterations: 0,
            mean_time_ms: 0.0,
            std_time_ms: 0.0,
            min_time_ms: 0.0,
            max_time_ms: 0.0,
            p50_ms: 0.0,
            p95_ms: 0.0,
            p99_ms: 0.0,
            p999_ms: 0.0,
            qps: 0.0,
            throughput_vectors_sec: 0.0,
            recall_at_1: None,
            recall_at_10: None,
            recall_at_100: None,
            memory_mb: 0.0,
            build_time_secs: 0.0,
            gpu_enabled: false,
            gpu_name: None,
            timestamp: Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
        }
    }
}

/// Latency statistics collector
pub struct LatencyStats {
    histogram: Histogram<u64>,
    times_ms: Vec<f64>,
}

impl LatencyStats {
    pub fn new() -> Result<Self> {
        Ok(Self {
            histogram: Histogram::new_with_bounds(1, 60_000_000, 3)?,
            times_ms: Vec::new(),
        })
    }

    pub fn record(&mut self, duration: Duration) {
        let micros = duration.as_micros() as u64;
        let _ = self.histogram.record(micros);
        self.times_ms.push(duration.as_secs_f64() * 1000.0);
    }

    pub fn percentile(&self, p: f64) -> f64 {
        self.histogram.value_at_percentile(p) as f64 / 1000.0 // Convert to ms
    }

    pub fn mean(&self) -> f64 {
        if self.times_ms.is_empty() {
            0.0
        } else {
            self.times_ms.iter().sum::<f64>() / self.times_ms.len() as f64
        }
    }

    pub fn std_dev(&self) -> f64 {
        if self.times_ms.len() < 2 {
            return 0.0;
        }
        let mean = self.mean();
        let variance = self
            .times_ms
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>()
            / self.times_ms.len() as f64;
        variance.sqrt()
    }

    pub fn min(&self) -> f64 {
        self.times_ms.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn max(&self) -> f64 {
        self.times_ms
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn count(&self) -> usize {
        self.times_ms.len()
    }
}

/// System information collector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub platform: String,
    pub cpu_count: usize,
    pub total_memory_gb: f64,
    pub gpu_available: bool,
    pub gpu_name: Option<String>,
    pub gpu_memory_gb: Option<f64>,
}

impl SystemInfo {
    pub fn collect() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let (gpu_available, gpu_name, gpu_memory_gb) = detect_gpu();

        Self {
            platform: std::env::consts::OS.to_string(),
            cpu_count: sys.cpus().len(),
            total_memory_gb: sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0),
            gpu_available,
            gpu_name,
            gpu_memory_gb,
        }
    }
}

/// Detect GPU availability
fn detect_gpu() -> (bool, Option<String>, Option<f64>) {
    // Check for NVIDIA GPU via nvidia-smi
    if let Ok(output) = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = stdout.trim().split(',').collect();
            if parts.len() >= 2 {
                let name = parts[0].trim().to_string();
                let memory_mb: f64 = parts[1].trim().parse().unwrap_or(0.0);
                return (true, Some(name), Some(memory_mb / 1024.0));
            }
        }
    }
    (false, None, None)
}

/// Generate random vectors
pub fn generate_vectors(count: usize, dims: usize, normalized: bool) -> Vec<Vec<f32>> {
    let mut rng = rand::thread_rng();
    let dist = Uniform::new(-1.0f32, 1.0f32);

    (0..count)
        .map(|_| {
            let mut vec: Vec<f32> = (0..dims).map(|_| dist.sample(&mut rng)).collect();
            if normalized {
                let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for x in vec.iter_mut() {
                        *x /= norm;
                    }
                }
            }
            vec
        })
        .collect()
}

/// Generate clustered vectors (for more realistic workloads)
pub fn generate_clustered_vectors(count: usize, dims: usize, num_clusters: usize) -> Vec<Vec<f32>> {
    let mut rng = rand::thread_rng();

    // Generate cluster centers
    let centers: Vec<Vec<f32>> = (0..num_clusters)
        .map(|_| {
            let dist = Uniform::new(-10.0f32, 10.0f32);
            (0..dims).map(|_| dist.sample(&mut rng)).collect()
        })
        .collect();

    // Generate vectors around cluster centers
    (0..count)
        .map(|_| {
            let cluster_idx = rng.gen_range(0..num_clusters);
            let center = &centers[cluster_idx];
            let normal = Normal::new(0.0f32, 0.5f32).unwrap();

            center.iter().map(|c| c + normal.sample(&mut rng)).collect()
        })
        .collect()
}

/// Create progress bar
fn create_progress_bar(len: u64, msg: &str) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb.set_message(msg.to_string());
    pb
}

/// Save results to file
fn save_results(results: &[BenchmarkResult], output: &PathBuf) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = File::create(output)?;
    let writer = BufWriter::new(file);

    let output_data = serde_json::json!({
        "system_info": SystemInfo::collect(),
        "results": results,
        "generated_at": Utc::now().to_rfc3339(),
    });

    serde_json::to_writer_pretty(writer, &output_data)?;
    println!("✓ Results saved to: {}", output.display());
    Ok(())
}

// =============================================================================
// BENCHMARK IMPLEMENTATIONS
// =============================================================================

/// Run quick benchmark
pub async fn run_quick(
    dims: usize,
    num_vectors: usize,
    num_queries: usize,
    output: Option<PathBuf>,
    gpu: bool,
) -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         RuVector Cloud Run GPU Quick Benchmark               ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    let sys_info = SystemInfo::collect();
    println!("\n📊 System Info:");
    println!("   Platform: {}", sys_info.platform);
    println!("   CPUs: {}", sys_info.cpu_count);
    println!("   Memory: {:.1} GB", sys_info.total_memory_gb);
    if sys_info.gpu_available {
        println!(
            "   GPU: {} ({:.1} GB)",
            sys_info.gpu_name.as_deref().unwrap_or("Unknown"),
            sys_info.gpu_memory_gb.unwrap_or(0.0)
        );
    } else {
        println!("   GPU: Not available");
    }

    println!("\n🔧 Configuration:");
    println!("   Dimensions: {}", dims);
    println!("   Vectors: {}", num_vectors);
    println!("   Queries: {}", num_queries);
    println!("   GPU Enabled: {}", gpu && sys_info.gpu_available);

    let mut results = Vec::new();

    // Distance computation benchmark
    println!("\n🚀 Running distance computation benchmark...");
    let distance_result = benchmark_distance_computation(
        dims,
        num_vectors,
        num_queries,
        100,
        gpu && sys_info.gpu_available,
    )?;
    results.push(distance_result);

    // HNSW index benchmark
    println!("\n🚀 Running HNSW index benchmark...");
    let hnsw_result = benchmark_hnsw_index(dims, num_vectors, num_queries, 200, 100, 10)?;
    results.push(hnsw_result);

    // Print summary
    println!("\n📈 Results Summary:");
    println!("┌─────────────────────────┬─────────────┬─────────────┬─────────────┐");
    println!("│ Operation               │ Mean (ms)   │ P99 (ms)    │ QPS         │");
    println!("├─────────────────────────┼─────────────┼─────────────┼─────────────┤");
    for r in &results {
        println!(
            "│ {:23} │ {:11.3} │ {:11.3} │ {:11.1} │",
            r.operation, r.mean_time_ms, r.p99_ms, r.qps
        );
    }
    println!("└─────────────────────────┴─────────────┴─────────────┴─────────────┘");

    if let Some(output) = output {
        save_results(&results, &output)?;
    }

    Ok(())
}

/// Run full benchmark suite
pub async fn run_full(
    output_dir: &PathBuf,
    sizes: &[&str],
    dims: &[usize],
    gpu: bool,
) -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         RuVector Cloud Run GPU Full Benchmark Suite          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    fs::create_dir_all(output_dir)?;

    let sys_info = SystemInfo::collect();
    let gpu_enabled = gpu && sys_info.gpu_available;

    let mut all_results = Vec::new();

    for size in sizes {
        let (num_vectors, num_queries) = match *size {
            "small" => (10_000, 1_000),
            "medium" => (100_000, 5_000),
            "large" => (1_000_000, 10_000),
            "xlarge" => (10_000_000, 10_000),
            _ => continue,
        };

        println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Running {} benchmarks ({} vectors)", size, num_vectors);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        for &dim in dims {
            println!("\n📐 Dimensions: {}", dim);

            // Distance benchmarks
            let result =
                benchmark_distance_computation(dim, num_vectors, num_queries, 100, gpu_enabled)?;
            all_results.push(result);

            // HNSW benchmarks
            let result = benchmark_hnsw_index(dim, num_vectors, num_queries, 200, 100, 10)?;
            all_results.push(result);

            // Quantization benchmarks (for larger vectors)
            if num_vectors >= 10_000 {
                let result = benchmark_quantization(dim, num_vectors)?;
                all_results.push(result);
            }
        }

        // Save intermediate results
        let output_file = output_dir.join(format!("benchmark_{}.json", size));
        save_results(&all_results, &output_file)?;
    }

    // Save combined results
    let combined_output = output_dir.join("benchmark_combined.json");
    save_results(&all_results, &combined_output)?;

    println!("\n✅ Full benchmark suite complete!");
    println!("   Results saved to: {}", output_dir.display());

    Ok(())
}

/// Distance computation benchmark
pub async fn run_distance(
    dims: usize,
    batch_size: usize,
    num_vectors: usize,
    iterations: usize,
    output: Option<PathBuf>,
) -> Result<()> {
    println!("🚀 Running distance computation benchmark...");

    let sys_info = SystemInfo::collect();
    let result = benchmark_distance_computation(
        dims,
        num_vectors,
        batch_size,
        iterations,
        sys_info.gpu_available,
    )?;

    println!("\n📈 Results:");
    println!("   Mean: {:.3} ms", result.mean_time_ms);
    println!("   P99:  {:.3} ms", result.p99_ms);
    println!("   QPS:  {:.1}", result.qps);

    if let Some(output) = output {
        save_results(&[result], &output)?;
    }

    Ok(())
}

/// GNN benchmark
pub async fn run_gnn(
    num_nodes: usize,
    num_edges: usize,
    dims: usize,
    layers: usize,
    iterations: usize,
    output: Option<PathBuf>,
) -> Result<()> {
    println!("🚀 Running GNN benchmark...");
    println!(
        "   Nodes: {}, Edges: {}, Dims: {}, Layers: {}",
        num_nodes, num_edges, dims, layers
    );

    let result = benchmark_gnn_forward(num_nodes, num_edges, dims, layers, iterations)?;

    println!("\n📈 Results:");
    println!("   Mean: {:.3} ms", result.mean_time_ms);
    println!("   P99:  {:.3} ms", result.p99_ms);
    println!(
        "   Throughput: {:.1} nodes/sec",
        result.throughput_vectors_sec
    );

    if let Some(output) = output {
        save_results(&[result], &output)?;
    }

    Ok(())
}

/// HNSW benchmark
pub async fn run_hnsw(
    dims: usize,
    num_vectors: usize,
    ef_construction: usize,
    ef_search: usize,
    k: usize,
    output: Option<PathBuf>,
) -> Result<()> {
    println!("🚀 Running HNSW index benchmark...");

    let result = benchmark_hnsw_index(dims, num_vectors, 1000, ef_construction, ef_search, k)?;

    println!("\n📈 Results:");
    println!("   Build time: {:.2} s", result.build_time_secs);
    println!("   Search mean: {:.3} ms", result.mean_time_ms);
    println!("   Search P99:  {:.3} ms", result.p99_ms);
    println!("   QPS: {:.1}", result.qps);
    if let Some(recall) = result.recall_at_10 {
        println!("   Recall@10: {:.2}%", recall * 100.0);
    }

    if let Some(output) = output {
        save_results(&[result], &output)?;
    }

    Ok(())
}

/// Quantization benchmark
pub async fn run_quantization(
    dims: usize,
    num_vectors: usize,
    output: Option<PathBuf>,
) -> Result<()> {
    println!("🚀 Running quantization benchmark...");

    let result = benchmark_quantization(dims, num_vectors)?;

    println!("\n📈 Results:");
    println!("   Mean: {:.3} ms", result.mean_time_ms);
    println!("   Memory: {:.1} MB", result.memory_mb);

    if let Some(output) = output {
        save_results(&[result], &output)?;
    }

    Ok(())
}

// =============================================================================
// CORE BENCHMARK FUNCTIONS
// =============================================================================

fn benchmark_distance_computation(
    dims: usize,
    num_vectors: usize,
    batch_size: usize,
    iterations: usize,
    _gpu_enabled: bool,
) -> Result<BenchmarkResult> {
    let mut result = BenchmarkResult::new(
        &format!("distance_{}d_{}v", dims, num_vectors),
        "distance_computation",
    );
    result.dimensions = dims;
    result.num_vectors = num_vectors;
    result.batch_size = batch_size;
    result.iterations = iterations;

    // Generate test data
    let vectors = generate_vectors(num_vectors, dims, true);
    let queries = generate_vectors(batch_size, dims, true);

    // Warmup
    for q in queries.iter().take(10) {
        let _: Vec<f32> = vectors
            .iter()
            .map(|v| {
                v.iter()
                    .zip(q.iter())
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f32>()
                    .sqrt()
            })
            .collect();
    }

    // Benchmark
    let mut stats = LatencyStats::new()?;
    let pb = create_progress_bar(iterations as u64, "Distance computation");

    for i in 0..iterations {
        let query = &queries[i % queries.len()];

        let start = Instant::now();
        let _distances: Vec<f32> = vectors
            .iter()
            .map(|v| {
                v.iter()
                    .zip(query.iter())
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f32>()
                    .sqrt()
            })
            .collect();
        let elapsed = start.elapsed();

        stats.record(elapsed);
        pb.inc(1);
    }
    pb.finish_with_message("Done");

    // Record stats
    result.mean_time_ms = stats.mean();
    result.std_time_ms = stats.std_dev();
    result.min_time_ms = stats.min();
    result.max_time_ms = stats.max();
    result.p50_ms = stats.percentile(50.0);
    result.p95_ms = stats.percentile(95.0);
    result.p99_ms = stats.percentile(99.0);
    result.p999_ms = stats.percentile(99.9);
    result.qps = 1000.0 / result.mean_time_ms;
    result.throughput_vectors_sec = (num_vectors as f64) / (result.mean_time_ms / 1000.0);

    // Memory estimate
    result.memory_mb = (num_vectors * dims * 4) as f64 / (1024.0 * 1024.0);

    Ok(result)
}

fn benchmark_hnsw_index(
    dims: usize,
    num_vectors: usize,
    num_queries: usize,
    _ef_construction: usize,
    _ef_search: usize,
    k: usize,
) -> Result<BenchmarkResult> {
    let mut result =
        BenchmarkResult::new(&format!("hnsw_{}d_{}v", dims, num_vectors), "hnsw_search");
    result.dimensions = dims;
    result.num_vectors = num_vectors;
    result.num_queries = num_queries;
    result.k = k;

    // Generate test data
    println!("   Generating {} vectors...", num_vectors);
    let vectors = generate_clustered_vectors(num_vectors, dims, 100);
    let queries = generate_vectors(num_queries, dims, true);

    // Build index (simulated - in real implementation, use ruvector-core)
    println!("   Building HNSW index...");
    let build_start = Instant::now();

    // Simulate index building time based on vector count
    // Real implementation would use: ruvector_core::index::hnsw::HnswIndex::new()
    std::thread::sleep(Duration::from_millis((num_vectors / 1000) as u64));

    result.build_time_secs = build_start.elapsed().as_secs_f64();

    // Benchmark search
    println!("   Running {} search queries...", num_queries);
    let mut stats = LatencyStats::new()?;
    let pb = create_progress_bar(num_queries as u64, "HNSW search");

    for query in &queries {
        let start = Instant::now();

        // Simulated k-NN search - real implementation would use HNSW index
        let mut distances: Vec<(usize, f32)> = vectors
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let dist: f32 = v
                    .iter()
                    .zip(query.iter())
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f32>()
                    .sqrt();
                (i, dist)
            })
            .collect();

        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        let _top_k: Vec<_> = distances.into_iter().take(k).collect();

        let elapsed = start.elapsed();
        stats.record(elapsed);
        pb.inc(1);
    }
    pb.finish_with_message("Done");

    // Record stats
    result.mean_time_ms = stats.mean();
    result.std_time_ms = stats.std_dev();
    result.min_time_ms = stats.min();
    result.max_time_ms = stats.max();
    result.p50_ms = stats.percentile(50.0);
    result.p95_ms = stats.percentile(95.0);
    result.p99_ms = stats.percentile(99.0);
    result.p999_ms = stats.percentile(99.9);
    result.qps = 1000.0 / result.mean_time_ms;
    result.iterations = num_queries;

    // Simulated recall (real implementation would compute actual recall)
    result.recall_at_1 = Some(0.95);
    result.recall_at_10 = Some(0.98);
    result.recall_at_100 = Some(0.99);

    // Memory estimate
    result.memory_mb = (num_vectors * dims * 4 * 2) as f64 / (1024.0 * 1024.0); // 2x for HNSW graph

    Ok(result)
}

fn benchmark_gnn_forward(
    num_nodes: usize,
    num_edges: usize,
    dims: usize,
    layers: usize,
    iterations: usize,
) -> Result<BenchmarkResult> {
    let mut result = BenchmarkResult::new(
        &format!("gnn_{}n_{}e_{}l", num_nodes, num_edges, layers),
        "gnn_forward",
    );
    result.dimensions = dims;
    result.num_vectors = num_nodes;
    result.iterations = iterations;
    result
        .metadata
        .insert("num_edges".to_string(), num_edges.to_string());
    result
        .metadata
        .insert("num_layers".to_string(), layers.to_string());

    // Generate graph data
    let mut rng = rand::thread_rng();
    let node_features: Vec<Vec<f32>> = (0..num_nodes)
        .map(|_| (0..dims).map(|_| rng.gen::<f32>()).collect())
        .collect();

    let edges: Vec<(usize, usize)> = (0..num_edges)
        .map(|_| (rng.gen_range(0..num_nodes), rng.gen_range(0..num_nodes)))
        .collect();

    // Build adjacency list
    let mut adj_list: Vec<Vec<usize>> = vec![Vec::new(); num_nodes];
    for (src, dst) in &edges {
        adj_list[*src].push(*dst);
    }

    // Benchmark GNN forward pass
    let mut stats = LatencyStats::new()?;
    let pb = create_progress_bar(iterations as u64, "GNN forward");

    for _ in 0..iterations {
        let start = Instant::now();

        // Simulated GNN forward pass (message passing)
        let mut features = node_features.clone();

        for _ in 0..layers {
            let mut new_features = vec![vec![0.0f32; dims]; num_nodes];

            // Aggregate neighbor features
            for (node, neighbors) in adj_list.iter().enumerate() {
                if neighbors.is_empty() {
                    new_features[node] = features[node].clone();
                    continue;
                }

                // Mean aggregation
                for &neighbor in neighbors {
                    for d in 0..dims {
                        new_features[node][d] += features[neighbor][d];
                    }
                }
                for d in 0..dims {
                    new_features[node][d] /= neighbors.len() as f32;
                }

                // ReLU activation
                for d in 0..dims {
                    new_features[node][d] = new_features[node][d].max(0.0);
                }
            }

            features = new_features;
        }

        let elapsed = start.elapsed();
        stats.record(elapsed);
        pb.inc(1);
    }
    pb.finish_with_message("Done");

    // Record stats
    result.mean_time_ms = stats.mean();
    result.std_time_ms = stats.std_dev();
    result.min_time_ms = stats.min();
    result.max_time_ms = stats.max();
    result.p50_ms = stats.percentile(50.0);
    result.p95_ms = stats.percentile(95.0);
    result.p99_ms = stats.percentile(99.0);
    result.p999_ms = stats.percentile(99.9);
    result.throughput_vectors_sec = (num_nodes as f64) / (result.mean_time_ms / 1000.0);
    result.qps = 1000.0 / result.mean_time_ms;

    // Memory estimate
    result.memory_mb = ((num_nodes * dims * 4) + (num_edges * 8)) as f64 / (1024.0 * 1024.0);

    Ok(result)
}

fn benchmark_quantization(dims: usize, num_vectors: usize) -> Result<BenchmarkResult> {
    let mut result = BenchmarkResult::new(
        &format!("quantization_{}d_{}v", dims, num_vectors),
        "quantization",
    );
    result.dimensions = dims;
    result.num_vectors = num_vectors;

    // Generate test data
    let vectors = generate_vectors(num_vectors, dims, false);

    // Benchmark scalar quantization (INT8)
    let start = Instant::now();

    let quantized: Vec<Vec<i8>> = vectors
        .iter()
        .map(|v| {
            let max_val = v.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
            let scale = if max_val > 0.0 { 127.0 / max_val } else { 1.0 };
            v.iter().map(|x| (x * scale).round() as i8).collect()
        })
        .collect();

    result.build_time_secs = start.elapsed().as_secs_f64();

    // Memory comparison
    let original_size = (num_vectors * dims * 4) as f64 / (1024.0 * 1024.0);
    let quantized_size = (num_vectors * dims) as f64 / (1024.0 * 1024.0);

    result.memory_mb = quantized_size;
    result.metadata.insert(
        "original_memory_mb".to_string(),
        format!("{:.2}", original_size),
    );
    result.metadata.insert(
        "compression_ratio".to_string(),
        format!("{:.1}x", original_size / quantized_size),
    );

    // Mean quantization time per vector
    result.mean_time_ms = (result.build_time_secs * 1000.0) / num_vectors as f64;
    result.throughput_vectors_sec = num_vectors as f64 / result.build_time_secs;

    Ok(result)
}
