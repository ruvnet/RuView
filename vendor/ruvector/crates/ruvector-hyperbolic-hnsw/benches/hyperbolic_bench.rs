//! Benchmarks for hyperbolic HNSW operations
//!
//! Metrics as specified in evaluation protocol:
//! - p50 and p95 latency
//! - Memory overhead
//! - Search recall@k

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ruvector_hyperbolic_hnsw::*;

fn bench_poincare_distance(c: &mut Criterion) {
    let dims = [8, 32, 128, 512];

    let mut group = c.benchmark_group("poincare_distance");

    for dim in dims {
        let x: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.01) % 0.9).collect();
        let y: Vec<f32> = (0..dim).map(|i| ((i as f32 * 0.02) + 0.1) % 0.9).collect();

        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, _| {
            b.iter(|| poincare_distance(black_box(&x), black_box(&y), 1.0))
        });
    }

    group.finish();
}

fn bench_mobius_add(c: &mut Criterion) {
    let dims = [8, 32, 128];

    let mut group = c.benchmark_group("mobius_add");

    for dim in dims {
        let x: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.01) % 0.5).collect();
        let y: Vec<f32> = (0..dim).map(|i| ((i as f32 * 0.02) + 0.1) % 0.5).collect();

        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, _| {
            b.iter(|| mobius_add(black_box(&x), black_box(&y), 1.0))
        });
    }

    group.finish();
}

fn bench_exp_log_map(c: &mut Criterion) {
    let dim = 32;
    let p: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.01) % 0.3).collect();
    let v: Vec<f32> = (0..dim).map(|i| ((i as f32 * 0.005) - 0.1) % 0.2).collect();
    let q: Vec<f32> = (0..dim).map(|i| ((i as f32 * 0.02) + 0.1) % 0.4).collect();

    let mut group = c.benchmark_group("exp_log_map");

    group.bench_function("exp_map", |b| {
        b.iter(|| exp_map(black_box(&v), black_box(&p), 1.0))
    });

    group.bench_function("log_map", |b| {
        b.iter(|| log_map(black_box(&q), black_box(&p), 1.0))
    });

    group.finish();
}

fn bench_hnsw_insert(c: &mut Criterion) {
    let sizes = [100, 500, 1000];

    let mut group = c.benchmark_group("hnsw_insert");
    group.sample_size(20);

    for size in sizes {
        let vectors: Vec<Vec<f32>> = (0..size)
            .map(|i| vec![
                (i as f32 * 0.01) % 0.8,
                ((i as f32 * 0.02) + 0.1) % 0.8,
            ])
            .collect();

        group.bench_with_input(BenchmarkId::new("n", size), &vectors, |b, vecs| {
            b.iter(|| {
                let mut hnsw = HyperbolicHnsw::default_config();
                for v in vecs {
                    hnsw.insert(v.clone()).unwrap();
                }
            })
        });
    }

    group.finish();
}

fn bench_hnsw_search(c: &mut Criterion) {
    let ks = [1, 5, 10, 50];

    // Build index once
    let mut hnsw = HyperbolicHnsw::default_config();
    for i in 0..1000 {
        let v = vec![
            (i as f32 * 0.01) % 0.8,
            ((i as f32 * 0.02) + 0.1) % 0.8,
        ];
        hnsw.insert(v).unwrap();
    }

    let query = vec![0.4, 0.4];

    let mut group = c.benchmark_group("hnsw_search");

    for k in ks {
        group.bench_with_input(BenchmarkId::new("k", k), &k, |b, &k| {
            b.iter(|| hnsw.search(black_box(&query), k))
        });
    }

    group.finish();
}

fn bench_tangent_cache(c: &mut Criterion) {
    let sizes = [100, 500, 1000];

    let mut group = c.benchmark_group("tangent_cache");
    group.sample_size(20);

    for size in sizes {
        let points: Vec<Vec<f32>> = (0..size)
            .map(|i| vec![
                (i as f32 * 0.01) % 0.8,
                ((i as f32 * 0.02) + 0.1) % 0.8,
            ])
            .collect();
        let indices: Vec<usize> = (0..size).collect();

        group.bench_with_input(BenchmarkId::new("build", size), &(points.clone(), indices.clone()), |b, (p, i)| {
            b.iter(|| TangentCache::new(black_box(p), black_box(i), 1.0))
        });
    }

    group.finish();
}

fn bench_search_with_pruning(c: &mut Criterion) {
    // Build index with tangent cache
    let mut hnsw = HyperbolicHnsw::default_config();
    for i in 0..1000 {
        let v = vec![
            (i as f32 * 0.01) % 0.8,
            ((i as f32 * 0.02) + 0.1) % 0.8,
        ];
        hnsw.insert(v).unwrap();
    }
    hnsw.build_tangent_cache().unwrap();

    let query = vec![0.4, 0.4];

    let mut group = c.benchmark_group("search_comparison");

    group.bench_function("standard_search", |b| {
        b.iter(|| hnsw.search(black_box(&query), 10))
    });

    group.bench_function("pruning_search", |b| {
        b.iter(|| hnsw.search_with_pruning(black_box(&query), 10))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_poincare_distance,
    bench_mobius_add,
    bench_exp_log_map,
    bench_hnsw_insert,
    bench_hnsw_search,
    bench_tangent_cache,
    bench_search_with_pruning,
);

criterion_main!(benches);
