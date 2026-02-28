//! Benchmarks for optimal transport algorithms

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;
use rand_distr::StandardNormal;
use ruvector_math::optimal_transport::{OptimalTransport, SinkhornSolver, SlicedWasserstein};

fn generate_points(n: usize, dim: usize, seed: u64) -> Vec<Vec<f64>> {
    let mut rng = StdRng::seed_from_u64(seed);
    (0..n)
        .map(|_| (0..dim).map(|_| rng.sample(StandardNormal)).collect())
        .collect()
}

fn bench_sliced_wasserstein(c: &mut Criterion) {
    let mut group = c.benchmark_group("sliced_wasserstein");

    for n in [100, 500, 1000, 5000] {
        group.throughput(Throughput::Elements(n as u64));

        let source = generate_points(n, 128, 42);
        let target = generate_points(n, 128, 43);

        // Vary number of projections
        for projections in [50, 100, 200] {
            let sw = SlicedWasserstein::new(projections).with_seed(42);

            group.bench_with_input(
                BenchmarkId::new(format!("n={}_proj={}", n, projections), n),
                &(&source, &target),
                |b, (s, t)| {
                    b.iter(|| sw.distance(black_box(s), black_box(t)));
                },
            );
        }
    }

    group.finish();
}

fn bench_sinkhorn(c: &mut Criterion) {
    let mut group = c.benchmark_group("sinkhorn");

    for n in [50, 100, 200, 500] {
        group.throughput(Throughput::Elements((n * n) as u64));

        let source = generate_points(n, 32, 42);
        let target = generate_points(n, 32, 43);

        for reg in [0.01, 0.05, 0.1] {
            let solver = SinkhornSolver::new(reg, 100);

            group.bench_with_input(
                BenchmarkId::new(format!("n={}_reg={}", n, reg), n),
                &(&source, &target),
                |b, (s, t)| {
                    b.iter(|| solver.distance(black_box(s), black_box(t)));
                },
            );
        }
    }

    group.finish();
}

fn bench_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling");

    // Test how Sliced Wasserstein scales with dimension
    let n = 500;
    for dim in [32, 64, 128, 256, 512] {
        let source = generate_points(n, dim, 42);
        let target = generate_points(n, dim, 43);
        let sw = SlicedWasserstein::new(100).with_seed(42);

        group.bench_with_input(
            BenchmarkId::new("sw_dim_scaling", dim),
            &(&source, &target),
            |b, (s, t)| {
                b.iter(|| sw.distance(black_box(s), black_box(t)));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_sliced_wasserstein,
    bench_sinkhorn,
    bench_scaling,
);
criterion_main!(benches);
