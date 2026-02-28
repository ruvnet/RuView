use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_core::distance::*;
use ruvector_core::types::DistanceMetric;

fn bench_euclidean(c: &mut Criterion) {
    let mut group = c.benchmark_group("euclidean_distance");

    for size in [128, 384, 768, 1536].iter() {
        let a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..*size).map(|i| (i + 1) as f32).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, _| {
            bench.iter(|| euclidean_distance(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

fn bench_cosine(c: &mut Criterion) {
    let mut group = c.benchmark_group("cosine_distance");

    for size in [128, 384, 768, 1536].iter() {
        let a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..*size).map(|i| (i + 1) as f32).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, _| {
            bench.iter(|| cosine_distance(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

fn bench_dot_product(c: &mut Criterion) {
    let mut group = c.benchmark_group("dot_product_distance");

    for size in [128, 384, 768, 1536].iter() {
        let a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..*size).map(|i| (i + 1) as f32).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, _| {
            bench.iter(|| dot_product_distance(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

fn bench_batch_distances(c: &mut Criterion) {
    let query: Vec<f32> = (0..384).map(|i| i as f32).collect();
    let vectors: Vec<Vec<f32>> = (0..1000)
        .map(|_| (0..384).map(|i| (i as f32) * 1.1).collect())
        .collect();

    c.bench_function("batch_distances_1000x384", |b| {
        b.iter(|| {
            batch_distances(
                black_box(&query),
                black_box(&vectors),
                DistanceMetric::Cosine,
            )
        });
    });
}

criterion_group!(
    benches,
    bench_euclidean,
    bench_cosine,
    bench_dot_product,
    bench_batch_distances
);
criterion_main!(benches);
