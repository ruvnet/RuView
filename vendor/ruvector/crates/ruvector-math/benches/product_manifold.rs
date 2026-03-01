//! Benchmarks for product manifold operations

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;
use rand_distr::StandardNormal;
use ruvector_math::product_manifold::ProductManifold;
use ruvector_math::spherical::SphericalSpace;

fn generate_point(dim: usize, rng: &mut impl Rng) -> Vec<f64> {
    (0..dim).map(|_| rng.sample(StandardNormal)).collect()
}

fn bench_product_manifold_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("product_manifold_distance");

    // Various configurations
    let configs = [
        (32, 0, 0, "euclidean_only"),
        (0, 16, 0, "hyperbolic_only"),
        (0, 0, 8, "spherical_only"),
        (32, 16, 8, "mixed_small"),
        (64, 32, 16, "mixed_medium"),
        (128, 64, 32, "mixed_large"),
    ];

    for (e, h, s, name) in configs.iter() {
        let manifold = ProductManifold::new(*e, *h, *s);
        let dim = manifold.dim();

        let mut rng = StdRng::seed_from_u64(42);
        let x = manifold.project(&generate_point(dim, &mut rng)).unwrap();
        let y = manifold.project(&generate_point(dim, &mut rng)).unwrap();

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(BenchmarkId::new(*name, dim), &(&x, &y), |b, (px, py)| {
            b.iter(|| manifold.distance(black_box(px), black_box(py)));
        });
    }

    group.finish();
}

fn bench_product_manifold_exp_log(c: &mut Criterion) {
    let mut group = c.benchmark_group("product_manifold_exp_log");

    let manifold = ProductManifold::new(64, 32, 16);
    let dim = manifold.dim();

    let mut rng = StdRng::seed_from_u64(42);
    let x = manifold.project(&generate_point(dim, &mut rng)).unwrap();
    let y = manifold.project(&generate_point(dim, &mut rng)).unwrap();
    let v = manifold.log_map(&x, &y).unwrap();

    group.throughput(Throughput::Elements(dim as u64));

    group.bench_function("exp_map", |b| {
        b.iter(|| manifold.exp_map(black_box(&x), black_box(&v)));
    });

    group.bench_function("log_map", |b| {
        b.iter(|| manifold.log_map(black_box(&x), black_box(&y)));
    });

    group.bench_function("geodesic", |b| {
        b.iter(|| manifold.geodesic(black_box(&x), black_box(&y), 0.5));
    });

    group.finish();
}

fn bench_frechet_mean(c: &mut Criterion) {
    let mut group = c.benchmark_group("frechet_mean");

    for n in [10, 50, 100, 200] {
        let manifold = ProductManifold::new(32, 16, 8);
        let dim = manifold.dim();

        let mut rng = StdRng::seed_from_u64(42);
        let points: Vec<Vec<f64>> = (0..n)
            .map(|_| manifold.project(&generate_point(dim, &mut rng)).unwrap())
            .collect();

        group.throughput(Throughput::Elements((n * dim) as u64));

        group.bench_with_input(
            BenchmarkId::new("product_manifold", n),
            &points,
            |b, pts| {
                b.iter(|| manifold.frechet_mean(black_box(pts), None));
            },
        );
    }

    group.finish();
}

fn bench_spherical_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("spherical");

    for dim in [8, 16, 32, 64, 128] {
        let sphere = SphericalSpace::new(dim);

        let mut rng = StdRng::seed_from_u64(42);
        let x = sphere.project(&generate_point(dim, &mut rng)).unwrap();
        let y = sphere.project(&generate_point(dim, &mut rng)).unwrap();

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(
            BenchmarkId::new("distance", dim),
            &(&x, &y),
            |b, (px, py)| {
                b.iter(|| sphere.distance(black_box(px), black_box(py)));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("exp_map", dim),
            &(&x, &y),
            |b, (px, py)| {
                if let Ok(v) = sphere.log_map(px, py) {
                    b.iter(|| sphere.exp_map(black_box(px), black_box(&v)));
                }
            },
        );
    }

    group.finish();
}

fn bench_knn(c: &mut Criterion) {
    let mut group = c.benchmark_group("knn");

    let manifold = ProductManifold::new(64, 32, 16);
    let dim = manifold.dim();

    for n in [100, 500, 1000] {
        let mut rng = StdRng::seed_from_u64(42);
        let points: Vec<Vec<f64>> = (0..n)
            .map(|_| manifold.project(&generate_point(dim, &mut rng)).unwrap())
            .collect();
        let query = manifold.project(&generate_point(dim, &mut rng)).unwrap();

        group.throughput(Throughput::Elements(n as u64));

        for k in [5, 10, 20] {
            group.bench_with_input(
                BenchmarkId::new(format!("n={}_k={}", n, k), n),
                &(&query, &points),
                |b, (q, pts)| {
                    b.iter(|| manifold.knn(black_box(q), black_box(pts), k));
                },
            );
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_product_manifold_distance,
    bench_product_manifold_exp_log,
    bench_frechet_mean,
    bench_spherical_operations,
    bench_knn,
);
criterion_main!(benches);
