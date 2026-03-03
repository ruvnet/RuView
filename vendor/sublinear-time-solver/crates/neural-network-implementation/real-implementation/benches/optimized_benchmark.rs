//! Comprehensive benchmark comparing all implementations
//!
//! Run with: cargo bench --features benchmark

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use real_temporal_solver::{
    TemporalSolver,
    optimized::{OptimizedNeuralNetwork, DiagonalKalmanFilter, GaussSeidelSolver, UltraFastTemporalSolver},
    fully_optimized::FullyOptimizedSolver,
};
use ndarray::Array1;
use std::time::Duration;

/// Benchmark the baseline implementation
fn bench_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("baseline");
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(2));

    let input = Array1::from_vec(vec![0.1f32; 128]);
    let mut solver = TemporalSolver::new(128, 32, 4);

    group.throughput(Throughput::Elements(1));
    group.bench_function("predict", |b| {
        b.iter(|| {
            let (pred, _cert, _duration) = solver.predict(&input).unwrap();
            black_box(pred)
        })
    });

    group.finish();
}

/// Benchmark the optimized implementation
fn bench_optimized(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimized");
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(2));

    let input = [0.1f32; 128];
    let mut solver = UltraFastTemporalSolver::new();

    group.throughput(Throughput::Elements(1));
    group.bench_function("predict", |b| {
        b.iter(|| {
            let (pred, _duration) = solver.predict(&input);
            black_box(pred)
        })
    });

    group.bench_function("predict_optimized", |b| {
        b.iter(|| {
            let (pred, _duration) = solver.predict_optimized(&input);
            black_box(pred)
        })
    });

    group.finish();
}

/// Benchmark the fully optimized INT8 + AVX2 implementation
fn bench_fully_optimized(c: &mut Criterion) {
    let mut group = c.benchmark_group("fully_optimized");
    group.measurement_time(Duration::from_secs(10));
    group.warm_up_time(Duration::from_secs(2));

    unsafe {
        let input = [0.1f32; 128];
        let mut solver = FullyOptimizedSolver::new();

        group.throughput(Throughput::Elements(1));
        group.bench_function("predict_avx2_int8", |b| {
            b.iter(|| {
                let pred = solver.predict(&input);
                black_box(pred)
            })
        });

        // Test batch processing
        let batch = vec![[0.1f32; 128]; 32];
        group.throughput(Throughput::Elements(32));
        group.bench_function("batch_32", |b| {
            b.iter(|| {
                for input in &batch {
                    let pred = solver.predict(input);
                    black_box(pred);
                }
            })
        });
    }

    group.finish();
}

/// Benchmark different matrix sizes
fn bench_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling");

    for size in [32, 64, 128, 256].iter() {
        unsafe {
            let input = vec![0.1f32; *size];
            let mut solver = FullyOptimizedSolver::new();

            group.throughput(Throughput::Elements(1));
            group.bench_with_input(
                BenchmarkId::from_parameter(size),
                size,
                |b, _| {
                    b.iter(|| {
                        // Adjust for different sizes - use first 128 elements
                        let mut input_128 = [0.0f32; 128];
                        for i in 0..(*size).min(128) {
                            input_128[i] = input[i];
                        }
                        let pred = solver.predict(&input_128);
                        black_box(pred)
                    })
                },
            );
        }
    }

    group.finish();
}

/// Compare all implementations side by side
fn bench_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("comparison");
    group.measurement_time(Duration::from_secs(10));

    // Baseline
    {
        let input = Array1::from_vec(vec![0.1f32; 128]);
        let mut solver = TemporalSolver::new(128, 32, 4);

        group.bench_function("baseline", |b| {
            b.iter(|| {
                let (pred, _cert, _duration) = solver.predict(&input).unwrap();
                black_box(pred)
            })
        });
    }

    // Optimized
    {
        let input = [0.1f32; 128];
        let mut solver = UltraFastTemporalSolver::new();

        group.bench_function("optimized", |b| {
            b.iter(|| {
                let (pred, _duration) = solver.predict_optimized(&input);
                black_box(pred)
            })
        });
    }

    // Fully Optimized
    unsafe {
        let input = [0.1f32; 128];
        let mut solver = FullyOptimizedSolver::new();

        group.bench_function("fully_optimized_avx2_int8", |b| {
            b.iter(|| {
                let pred = solver.predict(&input);
                black_box(pred)
            })
        });
    }

    group.finish();
}

/// Benchmark latency distribution
fn bench_latency_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency");
    group.sample_size(10000);
    group.measurement_time(Duration::from_secs(20));

    unsafe {
        let input = [0.1f32; 128];
        let mut solver = FullyOptimizedSolver::new();

        group.bench_function("p50_p99", |b| {
            b.iter(|| {
                let pred = solver.predict(&input);
                black_box(pred)
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_baseline,
    bench_optimized,
    bench_fully_optimized,
    bench_scaling,
    bench_comparison,
    bench_latency_distribution
);
criterion_main!(benches);