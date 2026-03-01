//! SOTA (State-of-the-Art) Benchmarks
//!
//! Benchmarks for the advanced optimizations implemented from the December 2025 paper:
//! - Cut size scaling behavior
//! - Graph density impact
//! - Batch operation efficiency
//! - Memory pool performance
//! - Lazy witness benefits
//! - Replacement edge lookup

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use ruvector_mincut::algorithm::ReplacementEdgeIndex;
use ruvector_mincut::instance::witness::LazyWitness;
use ruvector_mincut::pool::BfsPool;
use ruvector_mincut::prelude::*;
use ruvector_mincut::wrapper::MinCutWrapper;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// ============================================================================
// Graph Generators
// ============================================================================

/// Generate a graph with a specific minimum cut value
/// Creates two cliques connected by k edges (min cut = k)
fn generate_known_mincut(n_per_side: usize, mincut_value: usize, seed: u64) -> Vec<(u64, u64)> {
    let mut edges = Vec::new();
    let mut rng = StdRng::seed_from_u64(seed);

    // First clique: vertices 0 to n_per_side-1
    for i in 0..n_per_side as u64 {
        for j in (i + 1)..n_per_side as u64 {
            edges.push((i, j));
        }
    }

    // Second clique: vertices n_per_side to 2*n_per_side-1
    let offset = n_per_side as u64;
    for i in 0..n_per_side as u64 {
        for j in (i + 1)..n_per_side as u64 {
            edges.push((offset + i, offset + j));
        }
    }

    // Connect with exactly mincut_value edges
    for k in 0..mincut_value {
        let u = rng.gen_range(0..n_per_side as u64);
        let v = offset + rng.gen_range(0..n_per_side as u64);
        edges.push((u, v));
    }

    edges
}

/// Generate graph with specified density (0.0 = sparse, 1.0 = complete)
fn generate_density_graph(n: usize, density: f64, seed: u64) -> Vec<(u64, u64)> {
    let mut rng = StdRng::seed_from_u64(seed);
    let max_edges = n * (n - 1) / 2;
    let target_edges = ((max_edges as f64) * density) as usize;

    let mut edges = Vec::with_capacity(target_edges);
    let mut seen = HashSet::new();

    while edges.len() < target_edges {
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

/// Generate path graph
fn generate_path(n: usize) -> Vec<(u64, u64)> {
    (0..n as u64 - 1).map(|i| (i, i + 1)).collect()
}

// ============================================================================
// Cut Size Scaling Benchmarks
// ============================================================================

/// Benchmark how performance scales with minimum cut value
fn bench_cut_size_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("sota_cut_size_scaling");
    group.sample_size(50);

    let n_per_side = 50; // 100 vertices total

    // Test different min-cut values: 1, 2, 4, 8, 16, 32
    for mincut in [1, 2, 4, 8, 16, 32] {
        group.bench_with_input(
            BenchmarkId::new("query_mincut", mincut),
            &mincut,
            |b, &mincut| {
                b.iter_batched(
                    || {
                        let graph = Arc::new(DynamicGraph::new());
                        let edges = generate_known_mincut(n_per_side, mincut, 42);

                        for (u, v) in &edges {
                            let _ = graph.insert_edge(*u, *v, 1.0);
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
            },
        );
    }

    group.finish();
}

// ============================================================================
// Density Impact Benchmarks
// ============================================================================

/// Benchmark how graph density affects performance
fn bench_density_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("sota_density_impact");
    group.sample_size(30);

    let n = 200;

    // Test different densities: 0.01, 0.05, 0.1, 0.2, 0.3
    for density_pct in [1, 5, 10, 20, 30] {
        let density = density_pct as f64 / 100.0;

        group.bench_with_input(
            BenchmarkId::new("query_density", format!("{}pct", density_pct)),
            &density,
            |b, &density| {
                b.iter_batched(
                    || {
                        let graph = Arc::new(DynamicGraph::new());
                        let edges = generate_density_graph(n, density, 42);

                        for (u, v) in &edges {
                            let _ = graph.insert_edge(*u, *v, 1.0);
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
            },
        );
    }

    group.finish();
}

// ============================================================================
// Batch Operation Benchmarks
// ============================================================================

/// Benchmark batch insert vs sequential insert
fn bench_batch_vs_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("sota_batch_operations");

    for batch_size in [10, 50, 100, 500] {
        // Sequential inserts
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("sequential_insert", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter_batched(
                    || {
                        let graph = Arc::new(DynamicGraph::new());
                        // Base graph
                        let base_edges = generate_path(500);
                        for (u, v) in &base_edges {
                            let _ = graph.insert_edge(*u, *v, 1.0);
                        }

                        let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
                        for (i, (u, v)) in base_edges.iter().enumerate() {
                            wrapper.insert_edge(i as u64, *u, *v);
                        }

                        // New edges to insert
                        let mut rng = StdRng::seed_from_u64(123);
                        let new_edges: Vec<_> = (0..batch_size)
                            .filter_map(|i| {
                                let u = rng.gen_range(0..500);
                                let v = rng.gen_range(0..500);
                                if u != v && graph.insert_edge(u, v, 1.0).is_ok() {
                                    Some((1000 + i as u64, u, v))
                                } else {
                                    None
                                }
                            })
                            .collect();

                        (wrapper, new_edges)
                    },
                    |(mut wrapper, edges)| {
                        for (id, u, v) in edges {
                            wrapper.insert_edge(id, u, v);
                        }
                        black_box(wrapper)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        // Batch inserts
        group.bench_with_input(
            BenchmarkId::new("batch_insert", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter_batched(
                    || {
                        let graph = Arc::new(DynamicGraph::new());
                        let base_edges = generate_path(500);
                        for (u, v) in &base_edges {
                            let _ = graph.insert_edge(*u, *v, 1.0);
                        }

                        let mut wrapper = MinCutWrapper::new(Arc::clone(&graph));
                        for (i, (u, v)) in base_edges.iter().enumerate() {
                            wrapper.insert_edge(i as u64, *u, *v);
                        }

                        let mut rng = StdRng::seed_from_u64(123);
                        let new_edges: Vec<_> = (0..batch_size)
                            .filter_map(|i| {
                                let u = rng.gen_range(0..500);
                                let v = rng.gen_range(0..500);
                                if u != v && graph.insert_edge(u, v, 1.0).is_ok() {
                                    Some((1000 + i as u64, u, v))
                                } else {
                                    None
                                }
                            })
                            .collect();

                        (wrapper, new_edges)
                    },
                    |(mut wrapper, edges)| {
                        wrapper.batch_insert_edges(&edges);
                        black_box(wrapper)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

// ============================================================================
// Memory Pool Benchmarks
// ============================================================================

/// Benchmark BFS with and without pool
fn bench_memory_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("sota_memory_pool");

    for size in [100, 500, 1000, 5000] {
        // Without pool - allocate fresh each time
        group.bench_with_input(BenchmarkId::new("bfs_no_pool", size), &size, |b, &size| {
            b.iter(|| {
                let mut queue = std::collections::VecDeque::with_capacity(size);
                let mut visited = HashSet::with_capacity(size);
                let mut results = Vec::with_capacity(size);

                // Simulate BFS work
                queue.push_back(0u64);
                visited.insert(0);
                while let Some(v) = queue.pop_front() {
                    results.push(v);
                    if results.len() >= size {
                        break;
                    }
                    // Simulate adding neighbors
                    for next in v + 1..v + 4 {
                        if visited.insert(next) {
                            queue.push_back(next);
                        }
                    }
                }

                black_box(results)
            });
        });

        // With pool - reuse allocations
        group.bench_with_input(
            BenchmarkId::new("bfs_with_pool", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let mut res = BfsPool::acquire(size);

                    // Simulate BFS work
                    res.queue.push_back(0);
                    res.visited.insert(0);
                    while let Some(v) = res.queue.pop_front() {
                        res.results.push(v);
                        if res.results.len() >= size {
                            break;
                        }
                        for next in v + 1..v + 4 {
                            if res.visited.insert(next) {
                                res.queue.push_back(next);
                            }
                        }
                    }

                    black_box(res.results.len())
                    // Resources returned on drop
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Lazy Witness Benchmarks
// ============================================================================

/// Benchmark lazy vs eager witness materialization
fn bench_lazy_witness(c: &mut Criterion) {
    let mut group = c.benchmark_group("sota_lazy_witness");

    // Build adjacency for materialization
    let adjacency = |v: VertexId| -> Vec<VertexId> {
        // Simple linear graph for benchmarking
        if v == 0 {
            vec![1]
        } else if v < 999 {
            vec![v - 1, v + 1]
        } else {
            vec![v - 1]
        }
    };

    // Benchmark storing witnesses without materialization
    group.bench_function("lazy_store_100", |b| {
        b.iter(|| {
            let witnesses: Vec<_> = (0..100)
                .map(|i| LazyWitness::new(i as u64, 10, i as u64))
                .collect();
            black_box(witnesses)
        });
    });

    // Benchmark materializing just the best witness
    group.bench_function("lazy_materialize_best", |b| {
        b.iter_batched(
            || {
                // Create 100 lazy witnesses
                (0..100)
                    .map(|i| LazyWitness::new(i as u64, 10, i as u64))
                    .collect::<Vec<_>>()
            },
            |witnesses| {
                // Find and materialize only the best (smallest boundary)
                let best = witnesses.iter().min_by_key(|w| w.boundary_size()).unwrap();
                let handle = best.materialize(&adjacency);
                black_box(handle)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Benchmark materializing all witnesses
    group.bench_function("eager_materialize_all", |b| {
        b.iter_batched(
            || {
                (0..100)
                    .map(|i| LazyWitness::new(i as u64, 10, i as u64))
                    .collect::<Vec<_>>()
            },
            |witnesses| {
                let handles: Vec<_> = witnesses
                    .iter()
                    .map(|w| w.materialize(&adjacency))
                    .collect();
                black_box(handles)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================================
// Replacement Edge Benchmarks
// ============================================================================

/// Benchmark replacement edge lookup
fn bench_replacement_edge(c: &mut Criterion) {
    let mut group = c.benchmark_group("sota_replacement_edge");

    for n in [100, 500, 1000, 5000] {
        // Build tree adjacency
        let mut tree_adj: HashMap<VertexId, HashSet<VertexId>> = HashMap::new();
        for i in 0..n as u64 - 1 {
            tree_adj.entry(i).or_default().insert(i + 1);
            tree_adj.entry(i + 1).or_default().insert(i);
        }

        // Add non-tree edges
        let mut rng = StdRng::seed_from_u64(42);
        let mut idx = ReplacementEdgeIndex::new(n);

        // Add tree edges
        for i in 0..n as u64 - 1 {
            idx.add_tree_edge(i, i + 1);
        }

        // Add non-tree edges (skip connections)
        for _ in 0..n / 5 {
            let u = rng.gen_range(0..n as u64);
            let v = rng.gen_range(0..n as u64);
            if u != v && (u as i64 - v as i64).abs() > 1 {
                idx.add_non_tree_edge(u, v);
            }
        }

        group.bench_with_input(BenchmarkId::new("find_replacement", n), &n, |b, _| {
            b.iter_batched(
                || idx.clone(),
                |mut idx| {
                    // Find replacement for middle edge
                    let u = (n / 2) as u64;
                    let v = u + 1;
                    let result = idx.find_replacement(u, v, &tree_adj);
                    black_box(result)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// ============================================================================
// Binary Search Instance Lookup Benchmarks
// ============================================================================

/// Benchmark binary search vs linear instance lookup
fn bench_instance_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("sota_instance_lookup");

    for num_instances in [10, 50, 100, 500] {
        // Simulate instance bounds
        let instances: Vec<(u64, u64)> = (0..num_instances)
            .map(|i| {
                let min = ((1.2f64).powi(i as i32)).floor() as u64;
                let max = ((1.2f64).powi((i + 1) as i32)).floor() as u64;
                (min.max(1), max.max(1))
            })
            .collect();

        // Linear search
        group.bench_with_input(
            BenchmarkId::new("linear_search", num_instances),
            &num_instances,
            |b, _| {
                b.iter(|| {
                    let target = 50u64;
                    let found = instances
                        .iter()
                        .position(|(min, max)| target >= *min && target <= *max);
                    black_box(found)
                });
            },
        );

        // Binary search
        group.bench_with_input(
            BenchmarkId::new("binary_search", num_instances),
            &num_instances,
            |b, _| {
                b.iter(|| {
                    let target = 50u64;
                    let found = instances.binary_search_by(|(min, max)| {
                        if target < *min {
                            std::cmp::Ordering::Greater
                        } else if target > *max {
                            std::cmp::Ordering::Less
                        } else {
                            std::cmp::Ordering::Equal
                        }
                    });
                    black_box(found)
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(cut_scaling, bench_cut_size_scaling,);

criterion_group!(density, bench_density_impact,);

criterion_group!(batch_ops, bench_batch_vs_sequential,);

criterion_group!(memory, bench_memory_pool, bench_lazy_witness,);

criterion_group!(lookup, bench_replacement_edge, bench_instance_lookup,);

criterion_main!(cut_scaling, density, batch_ops, memory, lookup);
