//! Benchmarks for tropical algebra operations

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;
use ruvector_math::tropical::{MinPlusMatrix, TropicalMatrix};

fn generate_tropical_matrix(n: usize, seed: u64) -> TropicalMatrix {
    let mut rng = StdRng::seed_from_u64(seed);
    let data: Vec<Vec<f64>> = (0..n)
        .map(|_| (0..n).map(|_| rng.gen_range(-10.0..10.0)).collect())
        .collect();
    TropicalMatrix::from_rows(data)
}

fn generate_minplus_matrix(n: usize, seed: u64) -> MinPlusMatrix {
    let mut rng = StdRng::seed_from_u64(seed);
    let adj: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| {
                    if i == j {
                        0.0
                    } else if rng.gen_bool(0.3) {
                        rng.gen_range(1.0..20.0)
                    } else {
                        f64::INFINITY
                    }
                })
                .collect()
        })
        .collect();
    MinPlusMatrix::from_adjacency(adj)
}

fn bench_tropical_matmul(c: &mut Criterion) {
    let mut group = c.benchmark_group("tropical_matmul");

    for n in [32, 64, 128, 256] {
        group.throughput(Throughput::Elements((n * n) as u64));

        let a = generate_tropical_matrix(n, 42);
        let b = generate_tropical_matrix(n, 43);

        group.bench_with_input(BenchmarkId::new("size", n), &(&a, &b), |bench, (a, b)| {
            bench.iter(|| a.mul(black_box(b)));
        });
    }

    group.finish();
}

fn bench_tropical_power(c: &mut Criterion) {
    let mut group = c.benchmark_group("tropical_power");

    for n in [16, 32, 64] {
        let m = generate_tropical_matrix(n, 42);

        for k in [2, 4, 8] {
            group.bench_with_input(
                BenchmarkId::new(format!("n={}_k={}", n, k), n),
                &m,
                |bench, m: &TropicalMatrix| {
                    bench.iter(|| m.pow(black_box(k)));
                },
            );
        }
    }

    group.finish();
}

fn bench_shortest_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("shortest_paths");

    for n in [32, 64, 128, 256] {
        group.throughput(Throughput::Elements((n * n) as u64));

        let m = generate_minplus_matrix(n, 42);

        group.bench_with_input(
            BenchmarkId::new("floyd_warshall", n),
            &m,
            |bench, m: &MinPlusMatrix| {
                bench.iter(|| m.all_pairs_shortest_paths());
            },
        );
    }

    group.finish();
}

fn bench_tropical_eigenvalue(c: &mut Criterion) {
    let mut group = c.benchmark_group("tropical_eigenvalue");

    for n in [16, 32, 64, 128] {
        let m = generate_tropical_matrix(n, 42);

        group.bench_with_input(
            BenchmarkId::new("max_cycle_mean", n),
            &m,
            |bench, m: &TropicalMatrix| {
                bench.iter(|| m.max_cycle_mean());
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_tropical_matmul,
    bench_tropical_power,
    bench_shortest_paths,
    bench_tropical_eigenvalue,
);
criterion_main!(benches);
