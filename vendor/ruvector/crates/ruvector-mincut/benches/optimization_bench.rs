//! Benchmark suite for j-Tree + BMSSP optimizations
//!
//! Measures before/after performance for each optimization:
//! - DSpar: 5.9x target
//! - Cache: 10x target
//! - SIMD: 2-4x target
//! - Pool: 50-75% memory reduction
//! - Parallel: Near-linear scaling
//! - WASM Batch: 10x FFI reduction
//!
//! Target: Combined 10x speedup

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_mincut::graph::DynamicGraph;
use ruvector_mincut::optimization::{
    BatchConfig, BenchmarkSuite, CacheConfig, DegreePresparse, DistanceArray, LevelData, LevelPool,
    LevelUpdateResult, ParallelConfig, ParallelLevelUpdater, PathDistanceCache, PoolConfig,
    PresparseConfig, SimdDistanceOps, WasmBatchOps,
};
use std::collections::HashSet;

/// Create test graph with specified size
fn create_test_graph(vertices: usize, edges: usize) -> DynamicGraph {
    let graph = DynamicGraph::new();

    for i in 0..vertices {
        graph.add_vertex(i as u64);
    }

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

/// Benchmark DSpar sparsification
fn bench_dspar(c: &mut Criterion) {
    let mut group = c.benchmark_group("DSpar");

    for size in [100, 1000, 5000].iter() {
        let graph = create_test_graph(*size, size * 5);

        group.bench_with_input(BenchmarkId::new("baseline", size), size, |b, _| {
            b.iter(|| {
                let edges: Vec<_> = graph.edges().collect();
                black_box(edges.len())
            })
        });

        let mut dspar = DegreePresparse::with_config(PresparseConfig {
            target_sparsity: 0.1,
            ..Default::default()
        });

        group.bench_with_input(BenchmarkId::new("optimized", size), size, |b, _| {
            b.iter(|| {
                let result = dspar.presparse(&graph);
                black_box(result.edges.len())
            })
        });
    }

    group.finish();
}

/// Benchmark path distance cache
fn bench_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("PathCache");

    for size in [100, 1000, 5000].iter() {
        group.bench_with_input(
            BenchmarkId::new("baseline_no_cache", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut total = 0.0;
                    for i in 0..size {
                        total += (i as f64 * 1.414).sqrt();
                    }
                    black_box(total)
                })
            },
        );

        let cache = PathDistanceCache::with_config(CacheConfig {
            max_entries: *size,
            ..Default::default()
        });

        // Pre-populate cache
        for i in 0..*size {
            cache.insert(i as u64, (i + 1) as u64, (i as f64).sqrt());
        }

        group.bench_with_input(
            BenchmarkId::new("optimized_with_cache", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut total = 0.0;
                    for i in 0..size {
                        if let Some(d) = cache.get(i as u64, (i + 1) as u64) {
                            total += d;
                        }
                    }
                    black_box(total)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark SIMD distance operations
fn bench_simd(c: &mut Criterion) {
    let mut group = c.benchmark_group("SIMD");

    for size in [100, 1000, 10000].iter() {
        let mut arr = DistanceArray::new(*size);
        for i in 0..*size {
            arr.set(i as u64, (i as f64) * 0.5 + 1.0);
        }
        arr.set((*size / 2) as u64, 0.1);

        group.bench_with_input(BenchmarkId::new("find_min_naive", size), &arr, |b, arr| {
            b.iter(|| {
                let data = arr.as_slice();
                let mut min_val = f64::INFINITY;
                let mut min_idx = 0;
                for (i, &d) in data.iter().enumerate() {
                    if d < min_val {
                        min_val = d;
                        min_idx = i;
                    }
                }
                black_box((min_val, min_idx))
            })
        });

        group.bench_with_input(BenchmarkId::new("find_min_simd", size), &arr, |b, arr| {
            b.iter(|| black_box(SimdDistanceOps::find_min(arr)))
        });

        let neighbors: Vec<_> = (0..(size / 10).min(100))
            .map(|i| ((i * 10) as u64, 1.0))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("relax_batch_naive", size),
            size,
            |b, &size| {
                let mut arr = DistanceArray::new(size);
                b.iter(|| {
                    let data = arr.as_mut_slice();
                    for &(idx, weight) in &neighbors {
                        let idx = idx as usize;
                        if idx < data.len() {
                            let new_dist = 0.0 + weight;
                            if new_dist < data[idx] {
                                data[idx] = new_dist;
                            }
                        }
                    }
                    black_box(())
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("relax_batch_simd", size),
            size,
            |b, &size| {
                let mut arr = DistanceArray::new(size);
                b.iter(|| black_box(SimdDistanceOps::relax_batch(&mut arr, 0.0, &neighbors)))
            },
        );
    }

    group.finish();
}

/// Benchmark pool allocation
fn bench_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("Pool");

    for size in [100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("baseline_alloc_dealloc", size),
            size,
            |b, &size| {
                b.iter(|| {
                    let mut levels = Vec::new();
                    for i in 0..10 {
                        levels.push(LevelData::new(i, size));
                    }
                    black_box(levels.len())
                })
            },
        );

        let pool = LevelPool::with_config(PoolConfig {
            max_materialized_levels: 5,
            lazy_dealloc: true,
            ..Default::default()
        });

        group.bench_with_input(
            BenchmarkId::new("optimized_pool", size),
            size,
            |b, &size| {
                b.iter(|| {
                    for i in 0..10 {
                        let level = pool.allocate_level(i, size);
                        pool.materialize(i, level);
                    }
                    black_box(pool.stats().materialized_levels)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark parallel processing
fn bench_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("Parallel");

    let levels: Vec<usize> = (0..100).collect();

    for work_size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("sequential", work_size),
            work_size,
            |b, &work_size| {
                b.iter(|| {
                    let _results: Vec<_> = levels
                        .iter()
                        .map(|&level| {
                            let mut sum = 0.0;
                            for i in 0..work_size {
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
                    black_box(())
                })
            },
        );

        let updater = ParallelLevelUpdater::with_config(ParallelConfig {
            min_parallel_size: 10,
            ..Default::default()
        });

        group.bench_with_input(
            BenchmarkId::new("parallel_rayon", work_size),
            work_size,
            |b, &work_size| {
                b.iter(|| {
                    let _results = updater.process_parallel(&levels, |level| {
                        let mut sum = 0.0;
                        for i in 0..work_size {
                            sum += (i as f64).sqrt();
                        }
                        LevelUpdateResult {
                            level,
                            cut_value: sum,
                            partition: HashSet::new(),
                            time_us: 0,
                        }
                    });
                    black_box(())
                })
            },
        );
    }

    group.finish();
}

/// Benchmark WASM batch operations
fn bench_wasm_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("WASM_Batch");

    for size in [100, 1000, 5000].iter() {
        let edges: Vec<_> = (0..*size)
            .map(|i| (i as u64, (i + 1) as u64, 1.0))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("individual_ops", size),
            &edges,
            |b, edges| {
                b.iter(|| {
                    for edge in edges {
                        black_box(edge);
                    }
                })
            },
        );

        let mut batch = WasmBatchOps::with_config(BatchConfig {
            max_batch_size: 1024,
            ..Default::default()
        });

        group.bench_with_input(BenchmarkId::new("batched_ops", size), &edges, |b, edges| {
            b.iter(|| {
                batch.queue_insert_edges(edges.clone());
                let results = batch.execute_batch();
                black_box(results.len())
            })
        });
    }

    group.finish();
}

/// Run complete benchmark suite
fn bench_complete_suite(c: &mut Criterion) {
    let mut group = c.benchmark_group("Complete_Suite");

    group.bench_function("full_optimization_suite", |b| {
        b.iter(|| {
            let mut suite = BenchmarkSuite::new()
                .with_sizes(vec![100])
                .with_iterations(1);

            let results = suite.run_all();
            let combined = suite.combined_speedup();
            black_box((results.len(), combined))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_dspar,
    bench_cache,
    bench_simd,
    bench_pool,
    bench_parallel,
    bench_wasm_batch,
    bench_complete_suite,
);

criterion_main!(benches);
