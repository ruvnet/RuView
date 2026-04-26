//! Performance benchmarking utilities for mincut algorithms.
//!
//! Provides functions to measure the wall-clock time of the Stoer-Wagner and
//! normalized cut algorithms on random graphs of configurable size and density.

use std::time::{Duration, Instant};

use ruv_neural_core::brain::Atlas;
use ruv_neural_core::graph::{BrainEdge, BrainGraph, ConnectivityMetric};
use ruv_neural_core::signal::FrequencyBand;

use crate::normalized::normalized_cut;
use crate::stoer_wagner::stoer_wagner_mincut;

/// Result of a benchmark run.
#[derive(Debug, Clone)]
pub struct BenchmarkReport {
    /// Algorithm name.
    pub algorithm: String,
    /// Number of nodes in the test graph.
    pub num_nodes: usize,
    /// Number of edges in the test graph.
    pub num_edges: usize,
    /// Graph density (0..1).
    pub density: f64,
    /// Wall-clock execution time.
    pub elapsed: Duration,
    /// Minimum cut value found.
    pub cut_value: f64,
}

impl std::fmt::Display for BenchmarkReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: nodes={}, edges={}, density={:.3}, time={:.3}ms, cut={:.4}",
            self.algorithm,
            self.num_nodes,
            self.num_edges,
            self.density,
            self.elapsed.as_secs_f64() * 1000.0,
            self.cut_value
        )
    }
}

/// Benchmark the Stoer-Wagner algorithm on a random graph.
///
/// # Arguments
///
/// * `num_nodes` - Number of vertices.
/// * `density` - Edge density in [0, 1]. A density of 1.0 generates a complete graph.
/// * `seed` - Random seed for reproducibility.
pub fn benchmark_stoer_wagner(num_nodes: usize, density: f64, seed: u64) -> BenchmarkReport {
    let graph = generate_random_graph(num_nodes, density, seed);
    let num_edges = graph.edges.len();

    let start = Instant::now();
    let result = stoer_wagner_mincut(&graph);
    let elapsed = start.elapsed();

    let cut_value = result.map(|r| r.cut_value).unwrap_or(f64::NAN);

    BenchmarkReport {
        algorithm: "Stoer-Wagner".to_string(),
        num_nodes,
        num_edges,
        density,
        elapsed,
        cut_value,
    }
}

/// Benchmark the normalized cut algorithm on a random graph.
pub fn benchmark_normalized_cut(num_nodes: usize, density: f64, seed: u64) -> BenchmarkReport {
    let graph = generate_random_graph(num_nodes, density, seed);
    let num_edges = graph.edges.len();

    let start = Instant::now();
    let result = normalized_cut(&graph);
    let elapsed = start.elapsed();

    let cut_value = result.map(|r| r.cut_value).unwrap_or(f64::NAN);

    BenchmarkReport {
        algorithm: "Normalized-Cut".to_string(),
        num_nodes,
        num_edges,
        density,
        elapsed,
        cut_value,
    }
}

/// Generate a random undirected weighted graph with approximately the given density.
///
/// Uses a simple LCG for deterministic randomness.
fn generate_random_graph(num_nodes: usize, density: f64, seed: u64) -> BrainGraph {
    let mut rng_state = seed;

    let mut edges = Vec::new();
    for i in 0..num_nodes {
        for j in (i + 1)..num_nodes {
            rng_state = rng_state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1);
            let rand_val = (rng_state >> 33) as f64 / (1u64 << 31) as f64;

            if rand_val < density {
                rng_state = rng_state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1);
                let weight = ((rng_state >> 33) as f64 / (1u64 << 31) as f64) * 0.9 + 0.1;

                edges.push(BrainEdge {
                    source: i,
                    target: j,
                    weight,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                });
            }
        }
    }

    BrainGraph {
        num_nodes,
        edges,
        timestamp: 0.0,
        window_duration_s: 1.0,
        atlas: Atlas::Custom(num_nodes),
    }
}

/// Run a full benchmark suite and return all reports.
pub fn run_benchmark_suite() -> Vec<BenchmarkReport> {
    let configs = [(10, 0.5), (20, 0.3), (30, 0.2), (50, 0.1)];

    let mut reports = Vec::new();
    for &(nodes, density) in &configs {
        reports.push(benchmark_stoer_wagner(nodes, density, 42));
        reports.push(benchmark_normalized_cut(nodes, density, 42));
    }
    reports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_stoer_wagner() {
        let report = benchmark_stoer_wagner(10, 0.5, 42);
        assert_eq!(report.num_nodes, 10);
        assert!(report.num_edges > 0);
        assert!(!report.cut_value.is_nan());
    }

    #[test]
    fn test_benchmark_normalized_cut() {
        let report = benchmark_normalized_cut(10, 0.5, 42);
        assert_eq!(report.num_nodes, 10);
        assert!(!report.cut_value.is_nan());
    }

    #[test]
    fn test_generate_random_graph_deterministic() {
        let g1 = generate_random_graph(20, 0.3, 123);
        let g2 = generate_random_graph(20, 0.3, 123);
        assert_eq!(g1.edges.len(), g2.edges.len());
    }

    #[test]
    fn test_benchmark_report_display() {
        let report = benchmark_stoer_wagner(10, 0.5, 42);
        let display = format!("{}", report);
        assert!(display.contains("Stoer-Wagner"));
        assert!(display.contains("nodes=10"));
    }

    #[test]
    fn test_run_benchmark_suite() {
        let reports = run_benchmark_suite();
        assert_eq!(reports.len(), 8);
    }
}
