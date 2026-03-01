//! Real Benchmarks for RuVector Core
//!
//! These are ACTUAL performance measurements, not simulations.
//! Run with: cargo bench -p ruvector-core --bench real_benchmark

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ruvector_core::types::{DbOptions, HnswConfig};
use ruvector_core::{DistanceMetric, SearchQuery, VectorDB, VectorEntry};
use tempfile::tempdir;

/// Generate random vectors for benchmarking
fn generate_vectors(count: usize, dim: usize) -> Vec<Vec<f32>> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    (0..count)
        .map(|i| {
            (0..dim)
                .map(|j| {
                    let mut hasher = DefaultHasher::new();
                    (i * dim + j).hash(&mut hasher);
                    let h = hasher.finish();
                    ((h % 2000) as f32 / 1000.0) - 1.0 // Range [-1, 1]
                })
                .collect()
        })
        .collect()
}

/// Benchmark: Vector insertion (single)
fn bench_insert_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_single");

    for dim in [64, 128, 256, 512].iter() {
        let vectors = generate_vectors(1000, *dim);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("dimensions", dim), dim, |b, &dim| {
            let dir = tempdir().unwrap();
            let options = DbOptions {
                storage_path: dir.path().join("bench.db").to_string_lossy().to_string(),
                dimensions: dim,
                distance_metric: DistanceMetric::Cosine,
                hnsw_config: Some(HnswConfig::default()),
                quantization: None,
            };
            let db = VectorDB::new(options).unwrap();
            let mut idx = 0;

            b.iter(|| {
                let entry = VectorEntry {
                    id: None,
                    vector: vectors[idx % vectors.len()].clone(),
                    metadata: None,
                };
                let _ = black_box(db.insert(entry));
                idx += 1;
            });
        });
    }
    group.finish();
}

/// Benchmark: Vector insertion (batch)
fn bench_insert_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_batch");

    for batch_size in [100, 500, 1000].iter() {
        let vectors = generate_vectors(*batch_size, 128);

        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_size", batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    let dir = tempdir().unwrap();
                    let options = DbOptions {
                        storage_path: dir.path().join("bench.db").to_string_lossy().to_string(),
                        dimensions: 128,
                        distance_metric: DistanceMetric::Cosine,
                        hnsw_config: Some(HnswConfig::default()),
                        quantization: None,
                    };
                    let db = VectorDB::new(options).unwrap();

                    let entries: Vec<VectorEntry> = vectors
                        .iter()
                        .map(|v| VectorEntry {
                            id: None,
                            vector: v.clone(),
                            metadata: None,
                        })
                        .collect();

                    black_box(db.insert_batch(entries).unwrap())
                });
            },
        );
    }
    group.finish();
}

/// Benchmark: Search (k-NN)
fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");

    // Pre-populate database
    let dir = tempdir().unwrap();
    let options = DbOptions {
        storage_path: dir.path().join("bench.db").to_string_lossy().to_string(),
        dimensions: 128,
        distance_metric: DistanceMetric::Cosine,
        hnsw_config: Some(HnswConfig {
            m: 16,
            ef_construction: 100,
            ef_search: 50,
            max_elements: 100000,
        }),
        quantization: None,
    };
    let db = VectorDB::new(options).unwrap();

    // Insert 10k vectors
    let vectors = generate_vectors(10000, 128);
    let entries: Vec<VectorEntry> = vectors
        .iter()
        .map(|v| VectorEntry {
            id: None,
            vector: v.clone(),
            metadata: None,
        })
        .collect();
    db.insert_batch(entries).unwrap();

    // Generate query vectors
    let queries = generate_vectors(100, 128);

    for k in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("top_k", k), k, |b, &k| {
            let mut query_idx = 0;
            b.iter(|| {
                let query = &queries[query_idx % queries.len()];
                let search_query = SearchQuery {
                    vector: query.clone(),
                    k,
                    filter: None,
                    ef_search: None,
                };
                let results = black_box(db.search(search_query));
                query_idx += 1;
                results
            });
        });
    }
    group.finish();
}

/// Benchmark: Distance computation (raw)
fn bench_distance(c: &mut Criterion) {
    use ruvector_core::distance::{cosine_distance, dot_product_distance, euclidean_distance};

    let mut group = c.benchmark_group("distance");

    for dim in [64, 128, 256, 512, 1024].iter() {
        let v1: Vec<f32> = (0..*dim).map(|i| (i as f32 * 0.01).sin()).collect();
        let v2: Vec<f32> = (0..*dim).map(|i| (i as f32 * 0.02).cos()).collect();

        group.throughput(Throughput::Elements(1));

        group.bench_with_input(BenchmarkId::new("cosine", dim), dim, |b, _| {
            b.iter(|| black_box(cosine_distance(&v1, &v2)));
        });

        group.bench_with_input(BenchmarkId::new("euclidean", dim), dim, |b, _| {
            b.iter(|| black_box(euclidean_distance(&v1, &v2)));
        });

        group.bench_with_input(BenchmarkId::new("dot_product", dim), dim, |b, _| {
            b.iter(|| black_box(dot_product_distance(&v1, &v2)));
        });
    }
    group.finish();
}

/// Benchmark: Quantization
fn bench_quantization(c: &mut Criterion) {
    use ruvector_core::quantization::{QuantizedVector, ScalarQuantized};

    let mut group = c.benchmark_group("quantization");

    for dim in [128, 256, 512].iter() {
        let vector: Vec<f32> = (0..*dim).map(|i| (i as f32 * 0.01).sin()).collect();

        group.bench_with_input(BenchmarkId::new("scalar_quantize", dim), dim, |b, _| {
            b.iter(|| black_box(ScalarQuantized::quantize(&vector)));
        });

        let quantized = ScalarQuantized::quantize(&vector);
        group.bench_with_input(BenchmarkId::new("scalar_distance", dim), dim, |b, _| {
            b.iter(|| black_box(quantized.distance(&quantized)));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_distance,
    bench_quantization,
    bench_insert_single,
    bench_insert_batch,
    bench_search,
);

criterion_main!(benches);
