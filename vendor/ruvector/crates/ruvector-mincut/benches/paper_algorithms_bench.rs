//! Benchmarks for 2025 paper algorithm implementations
//!
//! Tests performance of:
//! - PolylogConnectivity (arXiv:2510.08297)
//! - ApproxMinCut (SODA 2025, arXiv:2412.15069)
//! - CacheOptBFS

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_mincut::connectivity::cache_opt::{BatchProcessor, CacheOptAdjacency, CacheOptBFS};
use ruvector_mincut::{
    ApproxMinCut, ApproxMinCutConfig, DynamicConnectivity, PolylogConnectivity, PolylogStats,
};

/// Generate a random graph with n vertices and m edges
fn generate_graph(n: usize, m: usize, seed: u64) -> Vec<(u64, u64)> {
    let mut edges = Vec::with_capacity(m);
    let mut rng = seed;

    for _ in 0..m {
        // Simple LCG for deterministic "random" numbers
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let u = (rng % n as u64) as u64;
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let v = (rng % n as u64) as u64;
        if u != v {
            edges.push((u, v));
        }
    }
    edges
}

/// Generate weighted graph edges
fn generate_weighted_graph(n: usize, m: usize, seed: u64) -> Vec<(u64, u64, f64)> {
    let mut edges = Vec::with_capacity(m);
    let mut rng = seed;

    for _ in 0..m {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let u = (rng % n as u64) as u64;
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let v = (rng % n as u64) as u64;
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        let w = ((rng % 100) as f64 + 1.0) / 10.0;
        if u != v {
            edges.push((u, v, w));
        }
    }
    edges
}

// ============================================================================
// PolylogConnectivity Benchmarks
// ============================================================================

fn bench_polylog_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("polylog_insert");

    for size in [100, 500, 1000, 5000].iter() {
        let edges = generate_graph(*size, size * 2, 42);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let mut conn = PolylogConnectivity::new();
                for &(u, v) in &edges {
                    conn.insert_edge(u, v);
                }
                black_box(conn.component_count())
            });
        });
    }
    group.finish();
}

fn bench_polylog_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("polylog_delete");

    for size in [100, 500, 1000].iter() {
        let edges = generate_graph(*size, size * 2, 42);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter_batched(
                || {
                    let mut conn = PolylogConnectivity::new();
                    for &(u, v) in &edges {
                        conn.insert_edge(u, v);
                    }
                    conn
                },
                |mut conn| {
                    // Delete half the edges
                    for i in 0..edges.len() / 2 {
                        let (u, v) = edges[i];
                        conn.delete_edge(u, v);
                    }
                    black_box(conn.component_count())
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_polylog_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("polylog_query");

    for size in [100, 500, 1000, 5000].iter() {
        let edges = generate_graph(*size, size * 2, 42);
        let mut conn = PolylogConnectivity::new();
        for &(u, v) in &edges {
            conn.insert_edge(u, v);
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let queries: Vec<(u64, u64)> = (0..100)
                .map(|i| {
                    (
                        (i * 7) as u64 % size as u64,
                        (i * 13 + 1) as u64 % size as u64,
                    )
                })
                .collect();

            b.iter(|| {
                let mut count = 0;
                for &(u, v) in &queries {
                    if conn.connected(u, v) {
                        count += 1;
                    }
                }
                black_box(count)
            });
        });
    }
    group.finish();
}

// ============================================================================
// ApproxMinCut Benchmarks
// ============================================================================

fn bench_approx_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("approx_mincut_insert");

    for size in [100, 500, 1000].iter() {
        let edges = generate_weighted_graph(*size, size * 2, 42);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let mut approx = ApproxMinCut::with_epsilon(0.1);
                for &(u, v, w) in &edges {
                    approx.insert_edge(u, v, w);
                }
                black_box(approx.edge_count())
            });
        });
    }
    group.finish();
}

fn bench_approx_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("approx_mincut_query");

    for size in [50, 100, 200, 500].iter() {
        let edges = generate_weighted_graph(*size, size * 2, 42);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter_batched(
                || {
                    let mut approx = ApproxMinCut::with_epsilon(0.1);
                    for &(u, v, w) in &edges {
                        approx.insert_edge(u, v, w);
                    }
                    approx
                },
                |mut approx| black_box(approx.min_cut_value()),
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_approx_epsilon_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("approx_epsilon");
    let size = 200;
    let edges = generate_weighted_graph(size, size * 2, 42);

    for epsilon in [0.05, 0.1, 0.2, 0.5].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(epsilon), epsilon, |b, &eps| {
            b.iter_batched(
                || {
                    let mut approx = ApproxMinCut::with_epsilon(eps);
                    for &(u, v, w) in &edges {
                        approx.insert_edge(u, v, w);
                    }
                    approx
                },
                |mut approx| black_box(approx.min_cut_value()),
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

// ============================================================================
// CacheOptBFS Benchmarks
// ============================================================================

fn bench_cache_opt_bfs(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_opt_bfs");

    for size in [100, 500, 1000, 5000].iter() {
        let edges: Vec<(u64, u64, f64)> = generate_weighted_graph(*size, size * 3, 42);
        let max_v = edges
            .iter()
            .map(|(u, v, _)| (*u).max(*v))
            .max()
            .unwrap_or(0);
        let adj = CacheOptAdjacency::from_edges(&edges, max_v);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let bfs = CacheOptBFS::new(&adj, 0);
                black_box(bfs.run().len())
            });
        });
    }
    group.finish();
}

fn bench_batch_processor(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_processor");

    for size in [100, 500, 1000, 5000].iter() {
        let edges: Vec<(u64, u64, f64)> = generate_weighted_graph(*size, size * 3, 42);
        let max_v = edges
            .iter()
            .map(|(u, v, _)| (*u).max(*v))
            .max()
            .unwrap_or(0);
        let adj = CacheOptAdjacency::from_edges(&edges, max_v);
        let vertices: Vec<u64> = (0..=max_v).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let processor = BatchProcessor::new();
            b.iter(|| {
                let degrees = processor.compute_degrees(&adj, &vertices);
                black_box(degrees.len())
            });
        });
    }
    group.finish();
}

// ============================================================================
// Comparison: PolylogConnectivity vs DynamicConnectivity
// ============================================================================

fn bench_connectivity_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("connectivity_comparison");

    let size = 500;
    let edges = generate_graph(size, size * 2, 42);

    // Polylog insert
    group.bench_function("polylog_build_500", |b| {
        b.iter(|| {
            let mut conn = PolylogConnectivity::new();
            for &(u, v) in &edges {
                conn.insert_edge(u, v);
            }
            black_box(conn.component_count())
        });
    });

    // Standard DynamicConnectivity insert
    group.bench_function("dynamic_build_500", |b| {
        b.iter(|| {
            let mut conn = DynamicConnectivity::new();
            for &(u, v) in &edges {
                conn.insert_edge(u, v);
            }
            black_box(conn.component_count())
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_polylog_insert,
    bench_polylog_delete,
    bench_polylog_query,
    bench_approx_insert,
    bench_approx_query,
    bench_approx_epsilon_comparison,
    bench_cache_opt_bfs,
    bench_batch_processor,
    bench_connectivity_comparison,
);

criterion_main!(benches);
