use causal_emergence::*;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

/// Generates a random-like transition matrix
fn generate_transition_matrix(n: usize) -> Vec<f32> {
    let mut matrix = vec![0.0; n * n];
    for i in 0..n {
        let mut row_sum = 0.0;
        for j in 0..n {
            let val = ((i * 73 + j * 37) % 100) as f32 / 100.0;
            matrix[i * n + j] = val;
            row_sum += val;
        }
        // Normalize row
        for j in 0..n {
            matrix[i * n + j] /= row_sum;
        }
    }
    matrix
}

/// Generates synthetic time-series data with multi-scale structure
fn generate_time_series(n: usize) -> Vec<f32> {
    (0..n)
        .map(|t| {
            let t_f = t as f32;
            // Three scales: slow, medium, fast oscillations
            0.5 * (t_f * 0.01).sin() + 0.3 * (t_f * 0.05).cos() + 0.2 * (t_f * 0.2).sin()
        })
        .collect()
}

/// Benchmark: Effective Information computation with SIMD
fn bench_effective_information(c: &mut Criterion) {
    let mut group = c.benchmark_group("effective_information");

    for n in [16, 64, 256, 1024].iter() {
        let matrix = generate_transition_matrix(*n);

        group.throughput(Throughput::Elements((n * n) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, &n| {
            b.iter(|| compute_ei_simd(black_box(&matrix), black_box(n)));
        });
    }

    group.finish();
}

/// Benchmark: Entropy computation with SIMD
fn bench_entropy(c: &mut Criterion) {
    let mut group = c.benchmark_group("entropy_simd");

    for n in [16, 64, 256, 1024, 4096].iter() {
        let probs: Vec<f32> = (0..*n)
            .map(|i| (i as f32 + 1.0) / (*n as f32 * (*n as f32 + 1.0) / 2.0))
            .collect();

        group.throughput(Throughput::Elements(*n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, _| {
            b.iter(|| entropy_simd(black_box(&probs)));
        });
    }

    group.finish();
}

/// Benchmark: Hierarchical coarse-graining
fn bench_coarse_graining(c: &mut Criterion) {
    let mut group = c.benchmark_group("coarse_graining");

    for n in [64, 256, 1024].iter() {
        let matrix = generate_transition_matrix(*n);

        group.throughput(Throughput::Elements(*n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, &n| {
            b.iter(|| ScaleHierarchy::build_sequential(black_box(matrix.clone()), black_box(2)));
        });
    }

    group.finish();
}

/// Benchmark: Transfer entropy computation
fn bench_transfer_entropy(c: &mut Criterion) {
    let mut group = c.benchmark_group("transfer_entropy");

    for n in [100, 500, 1000, 5000].iter() {
        let x: Vec<usize> = (0..*n).map(|i| (i * 13 + 7) % 10).collect();
        let y: Vec<usize> = (0..*n).map(|i| (i * 17 + 3) % 10).collect();

        group.throughput(Throughput::Elements(*n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, _| {
            b.iter(|| transfer_entropy(black_box(&x), black_box(&y), black_box(1), black_box(1)));
        });
    }

    group.finish();
}

/// Benchmark: Full consciousness assessment pipeline
fn bench_consciousness_assessment(c: &mut Criterion) {
    let mut group = c.benchmark_group("consciousness_assessment");

    for n in [200, 500, 1000].iter() {
        let data = generate_time_series(*n);

        group.throughput(Throughput::Elements(*n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, _| {
            b.iter(|| {
                assess_consciousness(
                    black_box(&data),
                    black_box(2),
                    black_box(false),
                    black_box(5.0),
                )
            });
        });
    }

    group.finish();
}

/// Benchmark: Emergence detection
fn bench_emergence_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("emergence_detection");

    for n in [200, 500, 1000].iter() {
        let data = generate_time_series(*n);

        group.throughput(Throughput::Elements(*n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, _| {
            b.iter(|| detect_emergence(black_box(&data), black_box(2), black_box(0.5)));
        });
    }

    group.finish();
}

/// Benchmark: Causal hierarchy construction from time series
fn bench_causal_hierarchy(c: &mut Criterion) {
    let mut group = c.benchmark_group("causal_hierarchy");

    for n in [200, 500, 1000].iter() {
        let data = generate_time_series(*n);

        group.throughput(Throughput::Elements(*n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), n, |b, _| {
            b.iter(|| {
                CausalHierarchy::from_time_series(black_box(&data), black_box(2), black_box(false))
            });
        });
    }

    group.finish();
}

/// Benchmark: Real-time monitoring update
fn bench_real_time_monitor(c: &mut Criterion) {
    let mut monitor = ConsciousnessMonitor::new(200, 2, 5.0);

    // Prime the buffer
    for t in 0..200 {
        monitor.update((t as f32 * 0.1).sin());
    }

    c.bench_function("monitor_update", |b| {
        let mut t = 200;
        b.iter(|| {
            let value = (t as f32 * 0.1).sin();
            t += 1;
            monitor.update(black_box(value))
        });
    });
}

/// Benchmark: Multi-scale EI computation
fn bench_multi_scale_ei(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_scale_ei");

    let num_scales = 5;
    let matrices: Vec<Vec<f32>> = (0..num_scales)
        .map(|i| {
            let n = 256 >> i; // 256, 128, 64, 32, 16
            generate_transition_matrix(n)
        })
        .collect();

    let state_counts: Vec<usize> = (0..num_scales).map(|i| 256 >> i).collect();

    group.bench_function("5_scales", |b| {
        b.iter(|| compute_ei_multi_scale(black_box(&matrices), black_box(&state_counts)));
    });

    group.finish();
}

/// Benchmark comparison: Sequential vs Optimal coarse-graining
fn bench_coarse_graining_methods(c: &mut Criterion) {
    let mut group = c.benchmark_group("coarse_graining_methods");

    let n = 256;
    let matrix = generate_transition_matrix(n);

    group.bench_function("sequential", |b| {
        b.iter(|| ScaleHierarchy::build_sequential(black_box(matrix.clone()), black_box(2)));
    });

    group.bench_function("optimal", |b| {
        b.iter(|| ScaleHierarchy::build_optimal(black_box(matrix.clone()), black_box(2)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_effective_information,
    bench_entropy,
    bench_coarse_graining,
    bench_transfer_entropy,
    bench_consciousness_assessment,
    bench_emergence_detection,
    bench_causal_hierarchy,
    bench_real_time_monitor,
    bench_multi_scale_ei,
    bench_coarse_graining_methods,
);

criterion_main!(benches);
