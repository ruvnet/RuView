use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use router_core::{DistanceMetric, SearchQuery, VectorDB, VectorEntry};
use std::collections::HashMap;
use tempfile::tempdir;

fn bench_insert(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bench.db");

    let db = VectorDB::builder()
        .dimensions(384)
        .storage_path(&path)
        .build()
        .unwrap();

    let mut group = c.benchmark_group("insert");

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let entries: Vec<VectorEntry> = (0..size)
                .map(|i| VectorEntry {
                    id: format!("vec_{}", i),
                    vector: vec![0.1; 384],
                    metadata: HashMap::new(),
                    timestamp: 0,
                })
                .collect();

            b.iter(|| {
                for entry in &entries {
                    db.insert(entry.clone()).unwrap();
                }
            });
        });
    }

    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bench.db");

    let db = VectorDB::builder()
        .dimensions(384)
        .storage_path(&path)
        .build()
        .unwrap();

    // Insert test vectors
    let entries: Vec<VectorEntry> = (0..1000)
        .map(|i| VectorEntry {
            id: format!("vec_{}", i),
            vector: vec![i as f32 * 0.001; 384],
            metadata: HashMap::new(),
            timestamp: 0,
        })
        .collect();

    db.insert_batch(entries).unwrap();

    let mut group = c.benchmark_group("search");

    for k in [1, 10, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(k), k, |b, &k| {
            let query = SearchQuery {
                vector: vec![0.5; 384],
                k,
                filters: None,
                threshold: None,
                ef_search: None,
            };

            b.iter(|| {
                black_box(db.search(query.clone()).unwrap());
            });
        });
    }

    group.finish();
}

fn bench_distance_calculations(c: &mut Criterion) {
    use router_core::distance::*;

    let a = vec![0.5; 384];
    let b = vec![0.6; 384];

    c.bench_function("euclidean_distance", |bencher| {
        bencher.iter(|| {
            black_box(euclidean_distance(black_box(&a), black_box(&b)));
        });
    });

    c.bench_function("cosine_similarity", |bencher| {
        bencher.iter(|| {
            black_box(cosine_similarity(black_box(&a), black_box(&b)));
        });
    });

    c.bench_function("dot_product", |bencher| {
        bencher.iter(|| {
            black_box(dot_product(black_box(&a), black_box(&b)));
        });
    });
}

criterion_group!(
    benches,
    bench_insert,
    bench_search,
    bench_distance_calculations
);
criterion_main!(benches);
