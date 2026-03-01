//! Benchmarks for the real SubpolynomialMinCut integration
//!
//! Tests the El-Hayek/Henzinger/Li O(n^{o(1)}) algorithm performance.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ruqu::mincut::DynamicMinCutEngine;

/// Benchmark min-cut engine creation
fn bench_engine_creation(c: &mut Criterion) {
    c.bench_function("mincut_engine_creation", |b| {
        b.iter(|| black_box(DynamicMinCutEngine::new()));
    });
}

/// Benchmark edge insertion
fn bench_edge_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_edge_insertion");

    for size in [10, 50, 100, 500] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter_batched(
                || DynamicMinCutEngine::new(),
                |mut engine| {
                    for i in 0..size {
                        engine.insert_edge(i as u32, (i + 1) as u32, 1.0);
                    }
                    black_box(engine)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// Benchmark min-cut query after building a graph
fn bench_mincut_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_query");

    for size in [10, 50, 100, 200] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            // Build a random-ish graph
            let mut engine = DynamicMinCutEngine::new();
            for i in 0..size {
                engine.insert_edge(i as u32, ((i + 1) % size) as u32, 1.0);
                if i > 0 {
                    engine.insert_edge(i as u32, ((i + size / 2) % size) as u32, 0.5);
                }
            }

            b.iter(|| black_box(engine.min_cut_value()));
        });
    }
    group.finish();
}

/// Benchmark dynamic updates (insert + query)
fn bench_dynamic_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_dynamic_updates");

    for size in [50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            // Build initial graph
            let mut engine = DynamicMinCutEngine::new();
            for i in 0..size {
                engine.insert_edge(i as u32, ((i + 1) % size) as u32, 1.0);
            }
            // Query once to prime
            let _ = engine.min_cut_value();

            let mut counter = 0u32;
            b.iter(|| {
                // Insert edge
                engine.insert_edge(counter % size as u32, (counter + 10) % size as u32, 1.5);
                // Query
                let cut = engine.min_cut_value();
                // Delete edge
                engine.delete_edge(counter % size as u32, (counter + 10) % size as u32);
                counter = counter.wrapping_add(1);
                black_box(cut)
            });
        });
    }
    group.finish();
}

/// Benchmark grid graph (surface code-like)
fn bench_surface_code_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_surface_code");

    for distance in [5, 7, 9] {
        let num_qubits = 2 * distance * distance - 2 * distance + 1;
        group.bench_with_input(
            BenchmarkId::new("distance", distance),
            &distance,
            |b, &d| {
                b.iter_batched(
                    || {
                        // Build a grid graph approximating surface code
                        let mut engine = DynamicMinCutEngine::new();
                        for row in 0..d {
                            for col in 0..d {
                                let v = (row * d + col) as u32;
                                // Horizontal edges
                                if col + 1 < d {
                                    engine.insert_edge(v, v + 1, 1.0);
                                }
                                // Vertical edges
                                if row + 1 < d {
                                    engine.insert_edge(v, v + d as u32, 1.0);
                                }
                            }
                        }
                        engine
                    },
                    |mut engine| {
                        // Simulate syndrome updates
                        for i in 0..10 {
                            let v = (i % (d * d)) as u32;
                            engine.insert_edge(v, v + 1, 0.8);
                            let _ = engine.min_cut_value();
                        }
                        black_box(engine)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

/// Benchmark full min-cut result with certificate
fn bench_mincut_certified(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_certified");

    for size in [50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut engine = DynamicMinCutEngine::new();
            for i in 0..size {
                engine.insert_edge(i as u32, ((i + 1) % size) as u32, 1.0);
            }

            b.iter(|| {
                let result = engine.min_cut();
                black_box((result.value, result.is_exact, result.witness_hash))
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_engine_creation,
    bench_edge_insertion,
    bench_mincut_query,
    bench_dynamic_updates,
    bench_surface_code_graph,
    bench_mincut_certified,
);

criterion_main!(benches);
