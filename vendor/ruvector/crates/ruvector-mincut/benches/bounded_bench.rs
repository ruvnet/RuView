//! Benchmarks for bounded-range dynamic minimum cut

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use ruvector_mincut::prelude::*;
use ruvector_mincut::wrapper::MinCutWrapper;
use std::sync::Arc;

/// Generate a random graph with n vertices and m edges
fn generate_random_edges(n: usize, m: usize, seed: u64) -> Vec<(u64, u64)> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut edges = Vec::with_capacity(m);
    let mut seen = std::collections::HashSet::new();

    while edges.len() < m {
        let u = rng.gen_range(0..n as u64);
        let v = rng.gen_range(0..n as u64);
        if u != v {
            let key = if u < v { (u, v) } else { (v, u) };
            if seen.insert(key) {
                edges.push((u, v));
            }
        }
    }

    edges
}

/// Generate path graph edges
fn generate_path_edges(n: usize) -> Vec<(u64, u64)> {
    (0..n as u64 - 1).map(|i| (i, i + 1)).collect()
}

/// Generate cycle graph edges
fn generate_cycle_edges(n: usize) -> Vec<(u64, u64)> {
    let mut edges: Vec<_> = (0..n as u64 - 1).map(|i| (i, i + 1)).collect();
    edges.push((n as u64 - 1, 0));
    edges
}

/// Generate complete graph edges
fn generate_complete_edges(n: usize) -> Vec<(u64, u64)> {
    let mut edges = Vec::new();
    for i in 0..n as u64 {
        for j in (i + 1)..n as u64 {
            edges.push((i, j));
        }
    }
    edges
}

/// Generate grid graph edges
fn generate_grid_edges(width: usize, height: usize) -> Vec<(u64, u64)> {
    let mut edges = Vec::new();
    for i in 0..height {
        for j in 0..width {
            let v = (i * width + j) as u64;
            if j + 1 < width {
                edges.push((v, v + 1));
            }
            if i + 1 < height {
                edges.push((v, v + (width as u64)));
            }
        }
    }
    edges
}

fn benchmark_insert_edge(c: &mut Criterion) {
    let mut group = c.benchmark_group("bounded_insert_edge");

    for &size in &[100, 500, 1000, 5000] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter_batched(
                || {
                    // Setup: create graph and wrapper with existing edges
                    let graph = Arc::new(DynamicGraph::new());
                    let edges = generate_path_edges(size);

                    for (i, (u, v)) in edges.iter().enumerate() {
                        graph.insert_edge(*u, *v, 1.0).unwrap();
                    }

                    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
                    for (i, (u, v)) in edges.iter().enumerate() {
                        wrapper.insert_edge(i as u64, *u, *v);
                    }

                    // Prepare new edge to insert
                    let new_u = 0;
                    let new_v = size as u64 / 2;

                    (graph, wrapper, new_u, new_v, size as u64)
                },
                |(graph, mut wrapper, u, v, edge_id)| {
                    // Benchmark: single insert operation
                    if graph.insert_edge(u, v, 1.0).is_ok() {
                        wrapper.insert_edge(edge_id, u, v);
                    }
                    black_box(wrapper)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn benchmark_delete_edge(c: &mut Criterion) {
    let mut group = c.benchmark_group("bounded_delete_edge");

    for &size in &[100, 500, 1000] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter_batched(
                || {
                    // Setup: create graph with cycle so deletion doesn't disconnect
                    let graph = Arc::new(DynamicGraph::new());
                    let edges = generate_cycle_edges(size);

                    for (u, v) in &edges {
                        graph.insert_edge(*u, *v, 1.0).unwrap();
                    }

                    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
                    for (i, (u, v)) in edges.iter().enumerate() {
                        wrapper.insert_edge(i as u64, *u, *v);
                    }

                    // Edge to delete
                    let del_u = 0;
                    let del_v = 1;

                    (graph, wrapper, del_u, del_v)
                },
                |(graph, mut wrapper, u, v)| {
                    // Benchmark: single delete operation
                    wrapper.delete_edge(0, u, v);
                    black_box(wrapper)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn benchmark_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("bounded_query");

    for &size in &[100, 500, 1000, 5000] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter_batched(
                || {
                    // Setup: build path graph
                    let graph = Arc::new(DynamicGraph::new());
                    let edges = generate_path_edges(size);

                    for (u, v) in &edges {
                        graph.insert_edge(*u, *v, 1.0).unwrap();
                    }

                    let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
                    for (i, (u, v)) in edges.iter().enumerate() {
                        wrapper.insert_edge(i as u64, *u, *v);
                    }

                    wrapper
                },
                |mut wrapper| {
                    // Benchmark: query operation
                    let result = wrapper.query();
                    black_box(result)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn benchmark_query_after_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("bounded_query_after_updates");

    for &num_updates in &[10, 50, 100, 500] {
        group.throughput(Throughput::Elements(num_updates as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_updates),
            &num_updates,
            |b, &num_updates| {
                b.iter_batched(
                    || {
                        // Setup: build base graph
                        let graph = Arc::new(DynamicGraph::new());
                        let base_size = 500;
                        let edges = generate_path_edges(base_size);

                        for (u, v) in &edges {
                            graph.insert_edge(*u, *v, 1.0).unwrap();
                        }

                        let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
                        for (i, (u, v)) in edges.iter().enumerate() {
                            wrapper.insert_edge(i as u64, *u, *v);
                        }

                        // Add buffered updates
                        let mut rng = StdRng::seed_from_u64(42);
                        let mut edge_id = base_size as u64;

                        for _ in 0..num_updates {
                            let u = rng.gen_range(0..base_size as u64);
                            let v = rng.gen_range(0..base_size as u64);
                            if u != v && graph.insert_edge(u, v, 1.0).is_ok() {
                                wrapper.insert_edge(edge_id, u, v);
                                edge_id += 1;
                            }
                        }

                        wrapper
                    },
                    |mut wrapper| {
                        // Benchmark: query with buffered updates
                        let result = wrapper.query();
                        black_box(result)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn benchmark_different_topologies(c: &mut Criterion) {
    let mut group = c.benchmark_group("bounded_topologies");
    let size = 500;

    // Path graph
    group.bench_function("path", |b| {
        b.iter_batched(
            || {
                let graph = Arc::new(DynamicGraph::new());
                let edges = generate_path_edges(size);

                for (u, v) in &edges {
                    graph.insert_edge(*u, *v, 1.0).unwrap();
                }

                let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
                for (i, (u, v)) in edges.iter().enumerate() {
                    wrapper.insert_edge(i as u64, *u, *v);
                }

                wrapper
            },
            |mut wrapper| {
                let result = wrapper.query();
                black_box(result)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Cycle graph
    group.bench_function("cycle", |b| {
        b.iter_batched(
            || {
                let graph = Arc::new(DynamicGraph::new());
                let edges = generate_cycle_edges(size);

                for (u, v) in &edges {
                    graph.insert_edge(*u, *v, 1.0).unwrap();
                }

                let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
                for (i, (u, v)) in edges.iter().enumerate() {
                    wrapper.insert_edge(i as u64, *u, *v);
                }

                wrapper
            },
            |mut wrapper| {
                let result = wrapper.query();
                black_box(result)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Grid graph
    let grid_size = 22; // ~484 vertices
    group.bench_function("grid", |b| {
        b.iter_batched(
            || {
                let graph = Arc::new(DynamicGraph::new());
                let edges = generate_grid_edges(grid_size, grid_size);

                for (u, v) in &edges {
                    graph.insert_edge(*u, *v, 1.0).unwrap();
                }

                let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
                for (i, (u, v)) in edges.iter().enumerate() {
                    wrapper.insert_edge(i as u64, *u, *v);
                }

                wrapper
            },
            |mut wrapper| {
                let result = wrapper.query();
                black_box(result)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Complete graph (small due to O(n^2) edges)
    let complete_size = 30;
    group.bench_function("complete", |b| {
        b.iter_batched(
            || {
                let graph = Arc::new(DynamicGraph::new());
                let edges = generate_complete_edges(complete_size);

                for (u, v) in &edges {
                    graph.insert_edge(*u, *v, 1.0).unwrap();
                }

                let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
                for (i, (u, v)) in edges.iter().enumerate() {
                    wrapper.insert_edge(i as u64, *u, *v);
                }

                wrapper
            },
            |mut wrapper| {
                let result = wrapper.query();
                black_box(result)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn benchmark_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("bounded_mixed_workload");

    group.bench_function("realistic_workload", |b| {
        b.iter_batched(
            || {
                let graph = Arc::new(DynamicGraph::new());
                let base_size = 1000;
                let edges = generate_random_edges(base_size, base_size * 2, 42);

                for (u, v) in &edges {
                    graph.insert_edge(*u, *v, 1.0).unwrap();
                }

                let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
                for (i, (u, v)) in edges.iter().enumerate() {
                    wrapper.insert_edge(i as u64, *u, *v);
                }

                (graph, wrapper)
            },
            |(graph, mut wrapper)| {
                let mut rng = StdRng::seed_from_u64(12345);
                let mut edge_id = 2000u64;

                // Simulate realistic workload:
                // 70% queries, 20% inserts, 10% deletes
                for _ in 0..100 {
                    let op = rng.gen_range(0..10);

                    if op < 7 {
                        // Query
                        let _ = black_box(wrapper.query());
                    } else if op < 9 {
                        // Insert
                        let u = rng.gen_range(0..1000);
                        let v = rng.gen_range(0..1000);
                        if u != v && graph.insert_edge(u, v, 1.0).is_ok() {
                            wrapper.insert_edge(edge_id, u, v);
                            edge_id += 1;
                        }
                    } else {
                        // Delete (try to delete a random edge)
                        let u = rng.gen_range(0..1000);
                        let v = rng.gen_range(0..1000);
                        if graph.delete_edge(u, v).is_ok() {
                            wrapper.delete_edge(edge_id, u, v);
                            edge_id += 1;
                        }
                    }
                }

                black_box((graph, wrapper))
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn benchmark_lazy_instantiation(c: &mut Criterion) {
    let mut group = c.benchmark_group("bounded_lazy_instantiation");

    group.bench_function("first_query", |b| {
        b.iter_batched(
            || {
                // Setup: wrapper with no instances created yet
                let graph = Arc::new(DynamicGraph::new());
                let edges = generate_path_edges(500);

                for (u, v) in &edges {
                    graph.insert_edge(*u, *v, 1.0).unwrap();
                }

                let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
                for (i, (u, v)) in edges.iter().enumerate() {
                    wrapper.insert_edge(i as u64, *u, *v);
                }

                assert_eq!(wrapper.num_instances(), 0, "No instances before query");

                wrapper
            },
            |mut wrapper| {
                // Benchmark: first query triggers instantiation
                let result = wrapper.query();
                black_box(result)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("subsequent_query", |b| {
        b.iter_batched(
            || {
                // Setup: wrapper with instances already created
                let graph = Arc::new(DynamicGraph::new());
                let edges = generate_path_edges(500);

                for (u, v) in &edges {
                    graph.insert_edge(*u, *v, 1.0).unwrap();
                }

                let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
                for (i, (u, v)) in edges.iter().enumerate() {
                    wrapper.insert_edge(i as u64, *u, *v);
                }

                // Trigger initial instantiation
                let _ = wrapper.query();
                assert!(
                    wrapper.num_instances() > 0,
                    "Instances created after first query"
                );

                wrapper
            },
            |mut wrapper| {
                // Benchmark: subsequent query (instances already exist)
                let result = wrapper.query();
                black_box(result)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_insert_edge,
    benchmark_delete_edge,
    benchmark_query,
    benchmark_query_after_updates,
    benchmark_different_topologies,
    benchmark_mixed_workload,
    benchmark_lazy_instantiation,
);

criterion_main!(benches);
