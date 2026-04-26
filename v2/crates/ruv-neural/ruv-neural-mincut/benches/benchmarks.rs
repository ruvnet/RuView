//! Criterion benchmarks for ruv-neural-mincut.
//!
//! Benchmarks the performance-critical graph cut algorithms:
//! - Stoer-Wagner global minimum cut (O(V^3))
//! - Spectral bisection via Fiedler vector
//! - Cheeger constant (exact enumeration for small graphs)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::Rng;

use ruv_neural_core::brain::Atlas;
use ruv_neural_core::graph::{BrainEdge, BrainGraph, ConnectivityMetric};
use ruv_neural_core::signal::FrequencyBand;
use ruv_neural_mincut::{cheeger_constant, spectral_bisection, stoer_wagner_mincut};

/// Build a random weighted graph with the given number of nodes.
///
/// Creates a connected graph by first building a spanning path, then adding
/// random edges with density ~30% to ensure non-trivial structure.
fn random_graph(num_nodes: usize) -> BrainGraph {
    let mut rng = rand::thread_rng();
    let mut edges = Vec::new();

    // Spanning path to guarantee connectivity
    for i in 0..(num_nodes - 1) {
        edges.push(BrainEdge {
            source: i,
            target: i + 1,
            weight: rng.gen_range(0.1..2.0),
            metric: ConnectivityMetric::Coherence,
            frequency_band: FrequencyBand::Alpha,
        });
    }

    // Additional random edges (~30% density)
    for i in 0..num_nodes {
        for j in (i + 2)..num_nodes {
            if rng.gen_bool(0.3) {
                edges.push(BrainEdge {
                    source: i,
                    target: j,
                    weight: rng.gen_range(0.1..2.0),
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

fn bench_stoer_wagner(c: &mut Criterion) {
    let mut group = c.benchmark_group("stoer_wagner");

    for &n in &[10, 20, 50, 68] {
        let graph = random_graph(n);
        group.bench_with_input(BenchmarkId::new("nodes", n), &graph, |b, graph| {
            b.iter(|| stoer_wagner_mincut(black_box(graph)))
        });
    }

    group.finish();
}

fn bench_spectral_bisection(c: &mut Criterion) {
    let mut group = c.benchmark_group("spectral_bisection");

    for &n in &[10, 20, 50, 68] {
        let graph = random_graph(n);
        group.bench_with_input(BenchmarkId::new("nodes", n), &graph, |b, graph| {
            b.iter(|| spectral_bisection(black_box(graph)))
        });
    }

    group.finish();
}

fn bench_cheeger_constant(c: &mut Criterion) {
    let mut group = c.benchmark_group("cheeger_constant");

    // Cheeger uses exact enumeration for n <= 16, so test within that range
    for &n in &[8, 12, 16] {
        let graph = random_graph(n);
        group.bench_with_input(BenchmarkId::new("nodes", n), &graph, |b, graph| {
            b.iter(|| cheeger_constant(black_box(graph)))
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_stoer_wagner,
    bench_spectral_bisection,
    bench_cheeger_constant,
);
criterion_main!(benches);
