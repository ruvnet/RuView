use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_core::types::{DistanceMetric, SearchQuery};
use ruvector_core::{DbOptions, VectorDB, VectorEntry};
use tempfile::tempdir;

fn bench_batch_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_insert");

    for batch_size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |bench, &size| {
                bench.iter_batched(
                    || {
                        // Setup: Create DB and vectors
                        let dir = tempdir().unwrap();
                        let mut options = DbOptions::default();
                        options.storage_path =
                            dir.path().join("bench.db").to_string_lossy().to_string();
                        options.dimensions = 128;
                        options.hnsw_config = None; // Use flat index for faster insertion

                        let db = VectorDB::new(options).unwrap();

                        let vectors: Vec<VectorEntry> = (0..size)
                            .map(|i| VectorEntry {
                                id: Some(format!("vec_{}", i)),
                                vector: (0..128).map(|j| ((i + j) as f32) * 0.01).collect(),
                                metadata: None,
                            })
                            .collect();

                        (db, vectors, dir)
                    },
                    |(db, vectors, _dir)| {
                        // Benchmark: Batch insert
                        db.insert_batch(black_box(vectors)).unwrap()
                    },
                    criterion::BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_individual_insert_vs_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("individual_vs_batch_insert");
    let size = 1000;

    // Individual inserts
    group.bench_function("individual_1000", |bench| {
        bench.iter_batched(
            || {
                let dir = tempdir().unwrap();
                let mut options = DbOptions::default();
                options.storage_path = dir.path().join("bench.db").to_string_lossy().to_string();
                options.dimensions = 64;
                options.hnsw_config = None;

                let db = VectorDB::new(options).unwrap();
                let vectors: Vec<VectorEntry> = (0..size)
                    .map(|i| VectorEntry {
                        id: Some(format!("vec_{}", i)),
                        vector: vec![i as f32; 64],
                        metadata: None,
                    })
                    .collect();

                (db, vectors, dir)
            },
            |(db, vectors, _dir)| {
                for vector in vectors {
                    db.insert(black_box(vector)).unwrap();
                }
            },
            criterion::BatchSize::LargeInput,
        );
    });

    // Batch insert
    group.bench_function("batch_1000", |bench| {
        bench.iter_batched(
            || {
                let dir = tempdir().unwrap();
                let mut options = DbOptions::default();
                options.storage_path = dir.path().join("bench.db").to_string_lossy().to_string();
                options.dimensions = 64;
                options.hnsw_config = None;

                let db = VectorDB::new(options).unwrap();
                let vectors: Vec<VectorEntry> = (0..size)
                    .map(|i| VectorEntry {
                        id: Some(format!("vec_{}", i)),
                        vector: vec![i as f32; 64],
                        metadata: None,
                    })
                    .collect();

                (db, vectors, dir)
            },
            |(db, vectors, _dir)| db.insert_batch(black_box(vectors)).unwrap(),
            criterion::BatchSize::LargeInput,
        );
    });

    group.finish();
}

fn bench_parallel_searches(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir
        .path()
        .join("search_bench.db")
        .to_string_lossy()
        .to_string();
    options.dimensions = 128;
    options.distance_metric = DistanceMetric::Cosine;
    options.hnsw_config = None;

    let db = VectorDB::new(options).unwrap();

    // Insert test data
    let vectors: Vec<VectorEntry> = (0..1000)
        .map(|i| VectorEntry {
            id: Some(format!("vec_{}", i)),
            vector: (0..128).map(|j| ((i + j) as f32) * 0.01).collect(),
            metadata: None,
        })
        .collect();

    db.insert_batch(vectors).unwrap();

    // Benchmark multiple sequential searches
    c.bench_function("sequential_searches_100", |bench| {
        bench.iter(|| {
            for i in 0..100 {
                let query: Vec<f32> = (0..128).map(|j| ((i + j) as f32) * 0.01).collect();
                let _ = db
                    .search(SearchQuery {
                        vector: black_box(query),
                        k: 10,
                        filter: None,
                        ef_search: None,
                    })
                    .unwrap();
            }
        });
    });
}

fn bench_batch_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_delete");

    for size in [100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, &size| {
            bench.iter_batched(
                || {
                    // Setup: Create DB with vectors
                    let dir = tempdir().unwrap();
                    let mut options = DbOptions::default();
                    options.storage_path =
                        dir.path().join("bench.db").to_string_lossy().to_string();
                    options.dimensions = 32;
                    options.hnsw_config = None;

                    let db = VectorDB::new(options).unwrap();

                    let vectors: Vec<VectorEntry> = (0..size)
                        .map(|i| VectorEntry {
                            id: Some(format!("vec_{}", i)),
                            vector: vec![i as f32; 32],
                            metadata: None,
                        })
                        .collect();

                    let ids = db.insert_batch(vectors).unwrap();
                    (db, ids, dir)
                },
                |(db, ids, _dir)| {
                    // Benchmark: Delete all
                    for id in ids {
                        db.delete(black_box(&id)).unwrap();
                    }
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_batch_insert,
    bench_individual_insert_vs_batch,
    bench_parallel_searches,
    bench_batch_delete
);
criterion_main!(benches);
