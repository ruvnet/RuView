//! Benchmarks for Dynamic Minimum Cut Algorithm
//!
//! Measures:
//! - Insert/delete throughput at various graph sizes
//! - Query latency
//! - Scaling behavior (subpolynomial verification)
//! - Comparison with static algorithms

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;
use ruvector_mincut::graph::DynamicGraph;
use std::collections::HashSet;

/// Generate a random graph with n vertices and m edges
fn generate_random_graph(n: usize, m: usize, seed: u64) -> Vec<(u64, u64, f64)> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut edges = Vec::with_capacity(m);
    let mut edge_set = HashSet::new();

    while edges.len() < m {
        let u = rng.gen_range(0..n as u64);
        let v = rng.gen_range(0..n as u64);
        if u != v {
            let key = if u < v { (u, v) } else { (v, u) };
            if edge_set.insert(key) {
                edges.push((u, v, 1.0));
            }
        }
    }

    edges
}

/// Generate a grid graph (good test case with known min cuts)
fn generate_grid_graph(width: usize, height: usize) -> Vec<(u64, u64, f64)> {
    let mut edges = Vec::new();
    for i in 0..height {
        for j in 0..width {
            let v = (i * width + j) as u64;
            if j + 1 < width {
                edges.push((v, v + 1, 1.0));
            }
            if i + 1 < height {
                edges.push((v, v + width as u64, 1.0));
            }
        }
    }
    edges
}

/// Generate a complete graph with n vertices
fn generate_complete_graph(n: usize) -> Vec<(u64, u64, f64)> {
    let mut edges = Vec::new();
    for i in 0..n as u64 {
        for j in (i + 1)..n as u64 {
            edges.push((i, j, 1.0));
        }
    }
    edges
}

/// Generate a sparse graph (average degree ~4)
fn generate_sparse_graph(n: usize, seed: u64) -> Vec<(u64, u64, f64)> {
    let m = n * 2; // Average degree of 4
    generate_random_graph(n, m, seed)
}

/// Generate a dense graph (edge probability ~0.3)
fn generate_dense_graph(n: usize, seed: u64) -> Vec<(u64, u64, f64)> {
    let m = (n * (n - 1)) / 6; // ~30% of possible edges
    generate_random_graph(n, m, seed)
}

/// Benchmark edge insertion throughput
fn bench_insert_edge(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_edge");

    for size in [100, 500, 1000, 5000, 10000].iter() {
        let edges = generate_random_graph(*size, size * 2, 42);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || {
                    let graph = DynamicGraph::with_capacity(size, size * 3);
                    // Pre-populate with initial edges
                    for (u, v, w) in &edges {
                        let _ = graph.insert_edge(*u, *v, *w);
                    }
                    (graph, rand::rngs::StdRng::seed_from_u64(123))
                },
                |(graph, mut rng)| {
                    let u = rng.gen_range(0..size as u64);
                    let v = rng.gen_range(0..size as u64);
                    if u != v && !graph.has_edge(u, v) {
                        let _ = black_box(graph.insert_edge(u, v, 1.0));
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Benchmark edge deletion throughput
fn bench_delete_edge(c: &mut Criterion) {
    let mut group = c.benchmark_group("delete_edge");

    for size in [100, 500, 1000, 5000, 10000].iter() {
        let edges = generate_random_graph(*size, size * 2, 42);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter_batched(
                || {
                    let graph = DynamicGraph::with_capacity(*size, size * 3);
                    for (u, v, w) in &edges {
                        let _ = graph.insert_edge(*u, *v, *w);
                    }
                    graph
                },
                |graph| {
                    let edges_list = graph.edges();
                    if !edges_list.is_empty() {
                        let idx = rand::thread_rng().gen_range(0..edges_list.len());
                        let edge = edges_list[idx];
                        let _ = black_box(graph.delete_edge(edge.source, edge.target));
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Benchmark query operations (connectivity checks)
fn bench_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_connectivity");

    for size in [100, 500, 1000, 5000, 10000].iter() {
        let edges = generate_random_graph(*size, size * 2, 42);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let graph = DynamicGraph::with_capacity(*size, size * 3);
            for (u, v, w) in &edges {
                let _ = graph.insert_edge(*u, *v, *w);
            }

            b.iter(|| black_box(graph.is_connected()));
        });
    }
    group.finish();
}

/// Benchmark degree queries
fn bench_degree_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_degree");

    for size in [100, 500, 1000, 5000, 10000].iter() {
        let edges = generate_random_graph(*size, size * 2, 42);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let graph = DynamicGraph::with_capacity(size, size * 3);
            for (u, v, w) in &edges {
                let _ = graph.insert_edge(*u, *v, *w);
            }

            let mut rng = rand::rngs::StdRng::seed_from_u64(456);
            b.iter(|| {
                let v = rng.gen_range(0..size as u64);
                black_box(graph.degree(v))
            });
        });
    }
    group.finish();
}

/// Benchmark edge existence queries
fn bench_has_edge_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("query_has_edge");

    for size in [100, 500, 1000, 5000, 10000].iter() {
        let edges = generate_random_graph(*size, size * 2, 42);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let graph = DynamicGraph::with_capacity(size, size * 3);
            for (u, v, w) in &edges {
                let _ = graph.insert_edge(*u, *v, *w);
            }

            let mut rng = rand::rngs::StdRng::seed_from_u64(789);
            b.iter(|| {
                let u = rng.gen_range(0..size as u64);
                let v = rng.gen_range(0..size as u64);
                black_box(graph.has_edge(u, v))
            });
        });
    }
    group.finish();
}

/// Benchmark mixed workload: 50% inserts, 30% deletes, 20% queries
fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    for size in [100, 500, 1000, 5000].iter() {
        let initial_edges = generate_random_graph(*size, size * 2, 42);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched(
                || {
                    let graph = DynamicGraph::with_capacity(size, size * 3);
                    for (u, v, w) in &initial_edges {
                        let _ = graph.insert_edge(*u, *v, *w);
                    }
                    (graph, rand::rngs::StdRng::seed_from_u64(999))
                },
                |(graph, mut rng)| {
                    let op = rng.gen_range(0..10);

                    match op {
                        // 50% inserts (0-4)
                        0..=4 => {
                            let u = rng.gen_range(0..size as u64);
                            let v = rng.gen_range(0..size as u64);
                            if u != v && !graph.has_edge(u, v) {
                                let _ = graph.insert_edge(u, v, 1.0);
                            }
                        }
                        // 30% deletes (5-7)
                        5..=7 => {
                            let edges_list = graph.edges();
                            if !edges_list.is_empty() {
                                let idx = rng.gen_range(0..edges_list.len());
                                let edge = edges_list[idx];
                                let _ = graph.delete_edge(edge.source, edge.target);
                            }
                        }
                        // 20% queries (8-9)
                        _ => {
                            let u = rng.gen_range(0..size as u64);
                            let _ = black_box(graph.degree(u));
                        }
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Benchmark scaling behavior - verify subpolynomial update time
fn bench_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_analysis");
    group.sample_size(20); // Reduce sample size for larger graphs

    // Test sizes chosen to show subpolynomial scaling: n^(2/3)
    let sizes = vec![100, 316, 1000, 3162, 10000];

    for size in sizes {
        let edges = generate_random_graph(size, size * 2, 42);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("insert_scaling", size),
            &size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let graph = DynamicGraph::with_capacity(size, size * 3);
                        for (u, v, w) in &edges {
                            let _ = graph.insert_edge(*u, *v, *w);
                        }
                        (graph, rand::rngs::StdRng::seed_from_u64(111))
                    },
                    |(graph, mut rng)| {
                        let u = rng.gen_range(0..size as u64);
                        let v = rng.gen_range(0..size as u64);
                        if u != v && !graph.has_edge(u, v) {
                            let _ = black_box(graph.insert_edge(u, v, 1.0));
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        group.bench_with_input(BenchmarkId::new("delete_scaling", size), &size, |b, _| {
            b.iter_batched(
                || {
                    let graph = DynamicGraph::with_capacity(size, size * 3);
                    for (u, v, w) in &edges {
                        let _ = graph.insert_edge(*u, *v, *w);
                    }
                    graph
                },
                |graph| {
                    let edges_list = graph.edges();
                    if !edges_list.is_empty() {
                        let idx = rand::thread_rng().gen_range(0..edges_list.len());
                        let edge = edges_list[idx];
                        let _ = black_box(graph.delete_edge(edge.source, edge.target));
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark different graph types
fn bench_graph_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_types");
    let size = 1000;

    // Random graph
    let random_edges = generate_random_graph(size, size * 2, 42);
    group.bench_function("random_insert", |b| {
        b.iter_batched(
            || DynamicGraph::with_capacity(size, size * 3),
            |graph| {
                for (u, v, w) in &random_edges {
                    let _ = black_box(graph.insert_edge(*u, *v, *w));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Grid graph (31x32 ≈ 1000 vertices)
    let grid_edges = generate_grid_graph(31, 32);
    group.bench_function("grid_insert", |b| {
        b.iter_batched(
            || DynamicGraph::with_capacity(size, size * 2),
            |graph| {
                for (u, v, w) in &grid_edges {
                    let _ = black_box(graph.insert_edge(*u, *v, *w));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Complete graph (45 vertices ≈ 990 edges)
    let complete_edges = generate_complete_graph(45);
    group.bench_function("complete_insert", |b| {
        b.iter_batched(
            || DynamicGraph::with_capacity(45, 1000),
            |graph| {
                for (u, v, w) in &complete_edges {
                    let _ = black_box(graph.insert_edge(*u, *v, *w));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Sparse graph
    let sparse_edges = generate_sparse_graph(size, 42);
    group.bench_function("sparse_insert", |b| {
        b.iter_batched(
            || DynamicGraph::with_capacity(size, size * 3),
            |graph| {
                for (u, v, w) in &sparse_edges {
                    let _ = black_box(graph.insert_edge(*u, *v, *w));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Dense graph
    let dense_edges = generate_dense_graph(size, 42);
    group.bench_function("dense_insert", |b| {
        b.iter_batched(
            || DynamicGraph::with_capacity(size, dense_edges.len() + 100),
            |graph| {
                for (u, v, w) in &dense_edges {
                    let _ = black_box(graph.insert_edge(*u, *v, *w));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark graph statistics computation
fn bench_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_computation");

    for size in [100, 500, 1000, 5000].iter() {
        let edges = generate_random_graph(*size, size * 2, 42);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let graph = DynamicGraph::with_capacity(*size, size * 3);
            for (u, v, w) in &edges {
                let _ = graph.insert_edge(*u, *v, *w);
            }

            b.iter(|| black_box(graph.stats()));
        });
    }

    group.finish();
}

/// Benchmark connected components computation
fn bench_connected_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("connected_components");

    for size in [100, 500, 1000, 5000].iter() {
        // Create a graph with multiple components
        let mut edges = Vec::new();
        let component_size = size / 5; // 5 components

        for comp in 0..5 {
            let offset = comp * component_size;
            for i in 0..component_size - 1 {
                let u = (offset + i) as u64;
                let v = (offset + i + 1) as u64;
                edges.push((u, v, 1.0));
            }
        }

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            let graph = DynamicGraph::with_capacity(*size, edges.len());
            for (u, v, w) in &edges {
                let _ = graph.insert_edge(*u, *v, *w);
            }

            b.iter(|| black_box(graph.connected_components()));
        });
    }

    group.finish();
}

/// Benchmark neighbor iteration
fn bench_neighbors(c: &mut Criterion) {
    let mut group = c.benchmark_group("neighbors_iteration");

    for degree in [10, 50, 100, 500, 1000].iter() {
        // Create a star graph with one high-degree vertex
        let mut edges = Vec::new();
        for i in 1..=*degree {
            edges.push((0, i as u64, 1.0));
        }

        group.bench_with_input(BenchmarkId::from_parameter(degree), degree, |b, _| {
            let graph = DynamicGraph::with_capacity(*degree + 1, *degree);
            for (u, v, w) in &edges {
                let _ = graph.insert_edge(*u, *v, *w);
            }

            b.iter(|| black_box(graph.neighbors(0)));
        });
    }

    group.finish();
}

/// Benchmark batch operations
fn bench_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_operations");

    for batch_size in [10, 50, 100, 500, 1000].iter() {
        let edges = generate_random_graph(5000, *batch_size, 42);

        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_insert", batch_size),
            batch_size,
            |b, _| {
                b.iter_batched(
                    || DynamicGraph::with_capacity(5000, *batch_size + 100),
                    |graph| {
                        for (u, v, w) in &edges {
                            let _ = black_box(graph.insert_edge(*u, *v, *w));
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark memory footprint (using num_edges as proxy)
fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency");

    for size in [1000, 5000, 10000].iter() {
        let edges = generate_random_graph(*size, size * 2, 42);

        group.bench_with_input(BenchmarkId::new("graph_creation", size), size, |b, _| {
            b.iter(|| {
                let graph = DynamicGraph::with_capacity(*size, size * 3);
                for (u, v, w) in &edges {
                    let _ = graph.insert_edge(*u, *v, *w);
                }
                black_box(graph)
            });
        });
    }

    group.finish();
}

criterion_group!(
    graph_ops,
    bench_insert_edge,
    bench_delete_edge,
    bench_degree_query,
    bench_has_edge_query,
);

criterion_group!(
    queries,
    bench_query,
    bench_stats,
    bench_connected_components,
    bench_neighbors,
);

criterion_group!(workloads, bench_mixed_workload, bench_batch_operations,);

criterion_group!(
    scaling,
    bench_scaling,
    bench_graph_types,
    bench_memory_efficiency,
);

criterion_main!(graph_ops, queries, workloads, scaling);
