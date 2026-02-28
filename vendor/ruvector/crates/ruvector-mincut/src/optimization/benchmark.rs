//! Comprehensive Benchmark Suite for j-Tree + BMSSP Optimizations
//!
//! Measures before/after performance for each optimization:
//! - DSpar: 5.9x target speedup
//! - Cache: 10x target for repeated queries
//! - SIMD: 2-4x target for distance operations
//! - Pool: 50-75% memory reduction
//! - Parallel: Near-linear scaling
//! - WASM Batch: 10x FFI overhead reduction
//!
//! Target: Combined 10x speedup over naive implementation

use super::cache::{CacheConfig, PathDistanceCache};
use super::dspar::{DegreePresparse, PresparseConfig};
use super::parallel::{LevelUpdateResult, ParallelConfig, ParallelLevelUpdater, WorkItem};
use super::pool::{LevelData, LevelPool, PoolConfig};
use super::simd_distance::{DistanceArray, SimdDistanceOps};
use super::wasm_batch::{BatchConfig, WasmBatchOps};
use crate::graph::DynamicGraph;
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Single benchmark result
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Name of the benchmark
    pub name: String,
    /// Baseline time (naive implementation)
    pub baseline_us: u64,
    /// Optimized time
    pub optimized_us: u64,
    /// Speedup factor (baseline / optimized)
    pub speedup: f64,
    /// Target speedup
    pub target_speedup: f64,
    /// Whether target was achieved
    pub target_achieved: bool,
    /// Memory usage baseline (bytes)
    pub baseline_memory: usize,
    /// Memory usage optimized (bytes)
    pub optimized_memory: usize,
    /// Memory reduction percentage
    pub memory_reduction_percent: f64,
    /// Additional metrics
    pub metrics: Vec<(String, f64)>,
}

impl BenchmarkResult {
    /// Create new result
    pub fn new(name: &str, baseline_us: u64, optimized_us: u64, target_speedup: f64) -> Self {
        let speedup = if optimized_us > 0 {
            baseline_us as f64 / optimized_us as f64
        } else {
            f64::INFINITY
        };

        Self {
            name: name.to_string(),
            baseline_us,
            optimized_us,
            speedup,
            target_speedup,
            target_achieved: speedup >= target_speedup,
            baseline_memory: 0,
            optimized_memory: 0,
            memory_reduction_percent: 0.0,
            metrics: Vec::new(),
        }
    }

    /// Set memory metrics
    pub fn with_memory(mut self, baseline: usize, optimized: usize) -> Self {
        self.baseline_memory = baseline;
        self.optimized_memory = optimized;
        self.memory_reduction_percent = if baseline > 0 {
            100.0 * (1.0 - (optimized as f64 / baseline as f64))
        } else {
            0.0
        };
        self
    }

    /// Add custom metric
    pub fn add_metric(&mut self, name: &str, value: f64) {
        self.metrics.push((name.to_string(), value));
    }
}

/// Individual optimization benchmark
#[derive(Debug, Clone)]
pub struct OptimizationBenchmark {
    /// Optimization name
    pub name: String,
    /// Results for different workloads
    pub results: Vec<BenchmarkResult>,
    /// Overall assessment
    pub summary: BenchmarkSummary,
}

/// Summary of benchmark results
#[derive(Debug, Clone, Default)]
pub struct BenchmarkSummary {
    /// Average speedup achieved
    pub avg_speedup: f64,
    /// Minimum speedup
    pub min_speedup: f64,
    /// Maximum speedup
    pub max_speedup: f64,
    /// Percentage of targets achieved
    pub targets_achieved_percent: f64,
    /// Overall memory reduction
    pub avg_memory_reduction: f64,
}

/// Comprehensive benchmark suite
pub struct BenchmarkSuite {
    /// Test graph sizes
    sizes: Vec<usize>,
    /// Number of iterations per test
    iterations: usize,
    /// Results
    results: Vec<OptimizationBenchmark>,
}

impl BenchmarkSuite {
    /// Create new benchmark suite
    pub fn new() -> Self {
        Self {
            sizes: vec![100, 1000, 10000],
            iterations: 10,
            results: Vec::new(),
        }
    }

    /// Set test sizes
    pub fn with_sizes(mut self, sizes: Vec<usize>) -> Self {
        self.sizes = sizes;
        self
    }

    /// Set iterations
    pub fn with_iterations(mut self, iterations: usize) -> Self {
        self.iterations = iterations;
        self
    }

    /// Run all benchmarks
    pub fn run_all(&mut self) -> &Vec<OptimizationBenchmark> {
        self.results.clear();

        self.results.push(self.benchmark_dspar());
        self.results.push(self.benchmark_cache());
        self.results.push(self.benchmark_simd());
        self.results.push(self.benchmark_pool());
        self.results.push(self.benchmark_parallel());
        self.results.push(self.benchmark_wasm_batch());

        &self.results
    }

    /// Get combined speedup estimate
    pub fn combined_speedup(&self) -> f64 {
        if self.results.is_empty() {
            return 1.0;
        }

        // Estimate combined speedup (conservative: product of square roots)
        // Skip results with zero or negative speedup to avoid NaN
        let mut combined = 1.0;
        let mut count = 0;
        for result in &self.results {
            let speedup = result.summary.avg_speedup;
            if speedup > 0.0 && speedup.is_finite() {
                combined *= speedup.sqrt();
                count += 1;
            }
        }

        if count == 0 {
            return 1.0;
        }

        combined
    }

    /// Benchmark DSpar (Degree-based presparse)
    fn benchmark_dspar(&self) -> OptimizationBenchmark {
        let mut results = Vec::new();

        for &size in &self.sizes {
            let graph = create_test_graph(size, size * 5);

            // Baseline: process all edges
            let baseline_start = Instant::now();
            for _ in 0..self.iterations {
                let edges = graph.edges();
                let _count = edges.len();
            }
            let baseline_us = baseline_start.elapsed().as_micros() as u64 / self.iterations as u64;

            // Optimized: DSpar filtering
            let mut dspar = DegreePresparse::with_config(PresparseConfig {
                target_sparsity: 0.1,
                ..Default::default()
            });

            let opt_start = Instant::now();
            for _ in 0..self.iterations {
                let _ = dspar.presparse(&graph);
            }
            let opt_us = opt_start.elapsed().as_micros() as u64 / self.iterations as u64;

            let mut result = BenchmarkResult::new(
                &format!("DSpar n={}", size),
                baseline_us,
                opt_us,
                5.9, // Target speedup
            );

            // Get sparsification stats
            let sparse_result = dspar.presparse(&graph);
            result.add_metric("sparsity_ratio", sparse_result.stats.sparsity_ratio);
            result.add_metric(
                "edges_reduced",
                (sparse_result.stats.original_edges - sparse_result.stats.sparse_edges) as f64,
            );

            results.push(result);
        }

        compute_summary("DSpar", results)
    }

    /// Benchmark cache performance
    fn benchmark_cache(&self) -> OptimizationBenchmark {
        let mut results = Vec::new();

        for &size in &self.sizes {
            // Baseline: no caching (compute every time)
            let baseline_start = Instant::now();
            let mut total = 0.0;
            for _ in 0..self.iterations {
                for i in 0..size {
                    // Simulate distance computation
                    total += (i as f64 * 1.414).sqrt();
                }
            }
            let baseline_us = baseline_start.elapsed().as_micros() as u64 / self.iterations as u64;
            let _ = total; // Prevent optimization

            // Optimized: with caching
            let cache = PathDistanceCache::with_config(CacheConfig {
                max_entries: size,
                ..Default::default()
            });

            // Warm up cache
            for i in 0..(size / 2) {
                cache.insert(i as u64, (i + 1) as u64, (i as f64).sqrt());
            }

            let opt_start = Instant::now();
            for _ in 0..self.iterations {
                for i in 0..size {
                    if cache.get(i as u64, (i + 1) as u64).is_none() {
                        cache.insert(i as u64, (i + 1) as u64, (i as f64).sqrt());
                    }
                }
            }
            let opt_us = opt_start.elapsed().as_micros() as u64 / self.iterations as u64;

            let mut result = BenchmarkResult::new(
                &format!("Cache n={}", size),
                baseline_us,
                opt_us,
                10.0, // Target speedup for cached hits
            );

            let stats = cache.stats();
            result.add_metric("hit_rate", stats.hit_rate());
            result.add_metric("cache_size", stats.size as f64);

            results.push(result);
        }

        compute_summary("Cache", results)
    }

    /// Benchmark SIMD operations
    fn benchmark_simd(&self) -> OptimizationBenchmark {
        let mut results = Vec::new();

        for &size in &self.sizes {
            let mut arr = DistanceArray::new(size);

            // Initialize with test data
            for i in 0..size {
                arr.set(i as u64, (i as f64) * 0.5 + 1.0);
            }
            arr.set((size / 2) as u64, 0.1); // Min value

            // Baseline: naive find_min
            let baseline_start = Instant::now();
            for _ in 0..self.iterations {
                let data = arr.as_slice();
                let mut min_val = f64::INFINITY;
                let mut min_idx = 0;
                for (i, &d) in data.iter().enumerate() {
                    if d < min_val {
                        min_val = d;
                        min_idx = i;
                    }
                }
                let _ = (min_val, min_idx);
            }
            let baseline_us = baseline_start.elapsed().as_micros() as u64 / self.iterations as u64;

            // Optimized: SIMD find_min
            let opt_start = Instant::now();
            for _ in 0..self.iterations {
                let _ = SimdDistanceOps::find_min(&arr);
            }
            let opt_us = opt_start.elapsed().as_micros() as u64 / self.iterations as u64;

            let result = BenchmarkResult::new(
                &format!("SIMD find_min n={}", size),
                baseline_us,
                opt_us.max(1), // Avoid divide by zero
                2.0,           // Target speedup
            );

            results.push(result);

            // Also benchmark relax_batch
            let neighbors: Vec<_> = (0..(size / 10).min(100))
                .map(|i| ((i * 10) as u64, 1.0))
                .collect();

            let baseline_start = Instant::now();
            let mut arr_baseline = DistanceArray::new(size);
            for _ in 0..self.iterations {
                let data = arr_baseline.as_mut_slice();
                for &(idx, weight) in &neighbors {
                    let idx = idx as usize;
                    if idx < data.len() {
                        let new_dist = 0.0 + weight;
                        if new_dist < data[idx] {
                            data[idx] = new_dist;
                        }
                    }
                }
            }
            let baseline_us = baseline_start.elapsed().as_micros() as u64 / self.iterations as u64;

            let mut arr_opt = DistanceArray::new(size);
            let opt_start = Instant::now();
            for _ in 0..self.iterations {
                SimdDistanceOps::relax_batch(&mut arr_opt, 0.0, &neighbors);
            }
            let opt_us = opt_start.elapsed().as_micros() as u64 / self.iterations as u64;

            let result = BenchmarkResult::new(
                &format!("SIMD relax_batch n={}", size),
                baseline_us,
                opt_us.max(1),
                2.0,
            );

            results.push(result);
        }

        compute_summary("SIMD", results)
    }

    /// Benchmark pool allocation
    fn benchmark_pool(&self) -> OptimizationBenchmark {
        let mut results = Vec::new();

        for &size in &self.sizes {
            // Baseline: allocate/deallocate each time
            let baseline_start = Instant::now();
            let mut baseline_memory = 0usize;
            for _ in 0..self.iterations {
                let mut levels = Vec::new();
                for i in 0..10 {
                    let level = LevelData::new(i, size);
                    baseline_memory = baseline_memory.max(std::mem::size_of_val(&level));
                    levels.push(level);
                }
                // Drop all
                drop(levels);
            }
            let baseline_us = baseline_start.elapsed().as_micros() as u64 / self.iterations as u64;

            // Optimized: pool allocation with lazy deallocation
            let pool = LevelPool::with_config(PoolConfig {
                max_materialized_levels: 5,
                lazy_dealloc: true,
                ..Default::default()
            });

            let opt_start = Instant::now();
            for _ in 0..self.iterations {
                for i in 0..10 {
                    let level = pool.allocate_level(i, size);
                    pool.materialize(i, level);
                }
                // Some evictions happen automatically
            }
            let opt_us = opt_start.elapsed().as_micros() as u64 / self.iterations as u64;

            let stats = pool.stats();

            let mut result =
                BenchmarkResult::new(&format!("Pool n={}", size), baseline_us, opt_us.max(1), 2.0);

            result = result.with_memory(
                baseline_memory * 10,  // Baseline: all levels materialized
                stats.pool_size_bytes, // Optimized: only max_materialized
            );

            result.add_metric("evictions", stats.evictions as f64);
            result.add_metric("materialized_levels", stats.materialized_levels as f64);

            results.push(result);
        }

        compute_summary("Pool", results)
    }

    /// Benchmark parallel processing
    fn benchmark_parallel(&self) -> OptimizationBenchmark {
        let mut results = Vec::new();

        for &size in &self.sizes {
            let levels: Vec<usize> = (0..100).collect();

            // Baseline: sequential processing
            let baseline_start = Instant::now();
            for _ in 0..self.iterations {
                let _results: Vec<_> = levels
                    .iter()
                    .map(|&level| {
                        // Simulate work
                        let mut sum = 0.0;
                        for i in 0..(size / 100).max(1) {
                            sum += (i as f64).sqrt();
                        }
                        LevelUpdateResult {
                            level,
                            cut_value: sum,
                            partition: HashSet::new(),
                            time_us: 0,
                        }
                    })
                    .collect();
            }
            let baseline_us = baseline_start.elapsed().as_micros() as u64 / self.iterations as u64;

            // Optimized: parallel processing
            let updater = ParallelLevelUpdater::with_config(ParallelConfig {
                min_parallel_size: 10,
                ..Default::default()
            });

            let opt_start = Instant::now();
            for _ in 0..self.iterations {
                let _results = updater.process_parallel(&levels, |level| {
                    let mut sum = 0.0;
                    for i in 0..(size / 100).max(1) {
                        sum += (i as f64).sqrt();
                    }
                    LevelUpdateResult {
                        level,
                        cut_value: sum,
                        partition: HashSet::new(),
                        time_us: 0,
                    }
                });
            }
            let opt_us = opt_start.elapsed().as_micros() as u64 / self.iterations as u64;

            let result = BenchmarkResult::new(
                &format!("Parallel n={}", size),
                baseline_us,
                opt_us.max(1),
                2.0, // Conservative target (depends on core count)
            );

            results.push(result);
        }

        compute_summary("Parallel", results)
    }

    /// Benchmark WASM batch operations
    fn benchmark_wasm_batch(&self) -> OptimizationBenchmark {
        let mut results = Vec::new();

        for &size in &self.sizes {
            let edges: Vec<_> = (0..size).map(|i| (i as u64, (i + 1) as u64, 1.0)).collect();

            // Baseline: individual operations
            let baseline_start = Instant::now();
            for _ in 0..self.iterations {
                // Simulate individual FFI calls
                for edge in &edges {
                    let _ = edge; // FFI overhead simulation
                    std::hint::black_box(edge);
                }
            }
            let baseline_us = baseline_start.elapsed().as_micros() as u64 / self.iterations as u64;

            // Optimized: batch operations
            let mut batch = WasmBatchOps::with_config(BatchConfig {
                max_batch_size: 1024,
                ..Default::default()
            });

            let opt_start = Instant::now();
            for _ in 0..self.iterations {
                batch.queue_insert_edges(edges.clone());
                let _ = batch.execute_batch();
            }
            let opt_us = opt_start.elapsed().as_micros() as u64 / self.iterations as u64;

            let stats = batch.stats();

            let mut result = BenchmarkResult::new(
                &format!("WASM Batch n={}", size),
                baseline_us,
                opt_us.max(1),
                10.0,
            );

            result.add_metric("avg_items_per_op", stats.avg_items_per_op);

            results.push(result);
        }

        compute_summary("WASM Batch", results)
    }

    /// Get results
    pub fn results(&self) -> &Vec<OptimizationBenchmark> {
        &self.results
    }

    /// Generate report string
    pub fn report(&self) -> String {
        let mut report = String::new();

        report.push_str("=== j-Tree + BMSSP Optimization Benchmark Report ===\n\n");

        for opt in &self.results {
            report.push_str(&format!("## {} Optimization\n", opt.name));
            report.push_str(&format!(
                "   Average Speedup: {:.2}x\n",
                opt.summary.avg_speedup
            ));
            report.push_str(&format!(
                "   Min/Max: {:.2}x / {:.2}x\n",
                opt.summary.min_speedup, opt.summary.max_speedup
            ));
            report.push_str(&format!(
                "   Targets Achieved: {:.0}%\n",
                opt.summary.targets_achieved_percent
            ));

            if opt.summary.avg_memory_reduction > 0.0 {
                report.push_str(&format!(
                    "   Memory Reduction: {:.1}%\n",
                    opt.summary.avg_memory_reduction
                ));
            }

            report.push_str("\n   Details:\n");
            for result in &opt.results {
                report.push_str(&format!(
                    "   - {}: {:.2}x (target: {:.2}x) {}\n",
                    result.name,
                    result.speedup,
                    result.target_speedup,
                    if result.target_achieved {
                        "[OK]"
                    } else {
                        "[MISS]"
                    }
                ));
            }
            report.push_str("\n");
        }

        let combined = self.combined_speedup();
        report.push_str(&format!("## Combined Speedup Estimate: {:.2}x\n", combined));
        report.push_str(&format!("   Target: 10x\n"));
        report.push_str(&format!(
            "   Status: {}\n",
            if combined >= 10.0 {
                "TARGET ACHIEVED"
            } else {
                "In Progress"
            }
        ));

        report
    }
}

impl Default for BenchmarkSuite {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create test graph
fn create_test_graph(vertices: usize, edges: usize) -> DynamicGraph {
    let graph = DynamicGraph::new();

    // Create vertices
    for i in 0..vertices {
        graph.add_vertex(i as u64);
    }

    // Create random-ish edges
    let mut edge_count = 0;
    for i in 0..vertices {
        for j in (i + 1)..vertices {
            if edge_count >= edges {
                break;
            }
            let _ = graph.insert_edge(i as u64, j as u64, 1.0);
            edge_count += 1;
        }
        if edge_count >= edges {
            break;
        }
    }

    graph
}

/// Compute summary from results
fn compute_summary(name: &str, results: Vec<BenchmarkResult>) -> OptimizationBenchmark {
    if results.is_empty() {
        return OptimizationBenchmark {
            name: name.to_string(),
            results: Vec::new(),
            summary: BenchmarkSummary::default(),
        };
    }

    let speedups: Vec<f64> = results.iter().map(|r| r.speedup).collect();
    let achieved: Vec<bool> = results.iter().map(|r| r.target_achieved).collect();
    let memory_reductions: Vec<f64> = results
        .iter()
        .filter(|r| r.baseline_memory > 0)
        .map(|r| r.memory_reduction_percent)
        .collect();

    let avg_speedup = speedups.iter().sum::<f64>() / speedups.len() as f64;
    let min_speedup = speedups.iter().copied().fold(f64::INFINITY, f64::min);
    let max_speedup = speedups.iter().copied().fold(0.0, f64::max);
    let achieved_count = achieved.iter().filter(|&&a| a).count();
    let targets_achieved_percent = 100.0 * achieved_count as f64 / achieved.len() as f64;

    let avg_memory_reduction = if memory_reductions.is_empty() {
        0.0
    } else {
        memory_reductions.iter().sum::<f64>() / memory_reductions.len() as f64
    };

    OptimizationBenchmark {
        name: name.to_string(),
        results,
        summary: BenchmarkSummary {
            avg_speedup,
            min_speedup,
            max_speedup,
            targets_achieved_percent,
            avg_memory_reduction,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_result() {
        let result = BenchmarkResult::new("test", 1000, 100, 5.0);

        assert_eq!(result.speedup, 10.0);
        assert!(result.target_achieved);
    }

    #[test]
    fn test_benchmark_result_memory() {
        let result = BenchmarkResult::new("test", 100, 50, 1.0).with_memory(1000, 250);

        assert_eq!(result.memory_reduction_percent, 75.0);
    }

    #[test]
    fn test_create_test_graph() {
        let graph = create_test_graph(10, 20);

        assert_eq!(graph.num_vertices(), 10);
        assert!(graph.num_edges() <= 20);
    }

    #[test]
    fn test_benchmark_suite_small() {
        let mut suite = BenchmarkSuite::new()
            .with_sizes(vec![10])
            .with_iterations(1);

        let results = suite.run_all();

        assert!(!results.is_empty());
    }

    #[test]
    fn test_combined_speedup() {
        let mut suite = BenchmarkSuite::new()
            .with_sizes(vec![10])
            .with_iterations(1);

        suite.run_all();
        let combined = suite.combined_speedup();

        // For very small inputs, overhead may exceed benefit
        // Just verify we get a valid positive result
        assert!(
            combined > 0.0 && combined.is_finite(),
            "Combined speedup {} should be positive and finite",
            combined
        );
    }

    #[test]
    fn test_report_generation() {
        let mut suite = BenchmarkSuite::new()
            .with_sizes(vec![10])
            .with_iterations(1);

        suite.run_all();
        let report = suite.report();

        assert!(report.contains("Benchmark Report"));
        assert!(report.contains("DSpar"));
        assert!(report.contains("Combined Speedup"));
    }
}
