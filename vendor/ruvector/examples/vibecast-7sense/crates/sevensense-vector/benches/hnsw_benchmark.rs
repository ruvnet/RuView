//! Benchmarks for HNSW index performance.
//!
//! Run with: cargo bench --package sevensense-vector

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use rand::prelude::*;

use sevensense_vector::{
    HnswIndex, HnswConfig, EmbeddingId,
    cosine_distance, euclidean_distance, cosine_similarity, normalize_vector,
};

/// Generate a random vector of given dimension.
fn random_vector(dim: usize, rng: &mut StdRng) -> Vec<f32> {
    (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

/// Generate a normalized random vector.
fn random_unit_vector(dim: usize, rng: &mut StdRng) -> Vec<f32> {
    normalize_vector(&random_vector(dim, rng))
}

fn bench_hnsw_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_search");

    for num_vectors in [1000, 10_000, 100_000] {
        let config = HnswConfig::for_dimension(128)
            .with_max_elements(num_vectors + 1000);
        let mut index = HnswIndex::new(&config);

        let mut rng = StdRng::seed_from_u64(42);

        // Insert vectors
        for _ in 0..num_vectors {
            let id = EmbeddingId::new();
            let vector = random_unit_vector(128, &mut rng);
            let _ = index.insert(id, &vector);
        }
        index.build().unwrap();

        // Query vector
        let query = random_unit_vector(128, &mut rng);

        group.bench_with_input(
            BenchmarkId::new("k10", num_vectors),
            &query,
            |b, q| b.iter(|| black_box(index.search(q, 10))),
        );

        group.bench_with_input(
            BenchmarkId::new("k50", num_vectors),
            &query,
            |b, q| b.iter(|| black_box(index.search(q, 50))),
        );
    }

    group.finish();
}

fn bench_brute_force_vs_hnsw(c: &mut Criterion) {
    let mut group = c.benchmark_group("brute_force_vs_hnsw");

    let num_vectors = 10_000;
    let dim = 128;

    let mut rng = StdRng::seed_from_u64(42);

    // Generate vectors
    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|_| random_unit_vector(dim, &mut rng))
        .collect();

    // HNSW index
    let config = HnswConfig::for_dimension(dim)
        .with_max_elements(num_vectors + 100);
    let mut hnsw_index = HnswIndex::new(&config);

    for (i, vector) in vectors.iter().enumerate() {
        let id = EmbeddingId::from_uuid(uuid::Uuid::from_u128(i as u128));
        let _ = hnsw_index.insert(id, vector);
    }
    hnsw_index.build().unwrap();

    let query = random_unit_vector(dim, &mut rng);
    let k = 10;

    // Brute force search
    group.bench_function("brute_force", |b| {
        b.iter(|| {
            let mut distances: Vec<(usize, f32)> = vectors
                .iter()
                .enumerate()
                .map(|(i, v)| (i, cosine_distance(&query, v)))
                .collect();
            distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            black_box(distances.into_iter().take(k).collect::<Vec<_>>())
        })
    });

    // HNSW search
    group.bench_function("hnsw", |b| {
        b.iter(|| black_box(hnsw_index.search(&query, k)))
    });

    group.finish();
}

fn bench_distance_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("distance_metrics");

    for dim in [128, 384, 768, 1536] {
        let mut rng = StdRng::seed_from_u64(42);
        let a = random_unit_vector(dim, &mut rng);
        let b = random_unit_vector(dim, &mut rng);

        group.bench_with_input(
            BenchmarkId::new("cosine_distance", dim),
            &(&a, &b),
            |bench, (a, b)| bench.iter(|| black_box(cosine_distance(a, b))),
        );

        group.bench_with_input(
            BenchmarkId::new("cosine_similarity", dim),
            &(&a, &b),
            |bench, (a, b)| bench.iter(|| black_box(cosine_similarity(a, b))),
        );

        group.bench_with_input(
            BenchmarkId::new("euclidean_distance", dim),
            &(&a, &b),
            |bench, (a, b)| bench.iter(|| black_box(euclidean_distance(a, b))),
        );
    }

    group.finish();
}

fn bench_batch_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_insert");

    for batch_size in [100, 1000, 10_000] {
        let config = HnswConfig::for_dimension(128)
            .with_max_elements(batch_size + 100);

        let mut rng = StdRng::seed_from_u64(42);
        let vectors: Vec<(EmbeddingId, Vec<f32>)> = (0..batch_size)
            .map(|_| (EmbeddingId::new(), random_unit_vector(128, &mut rng)))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("insert_and_build", batch_size),
            &vectors,
            |b, vecs| {
                b.iter(|| {
                    let mut index = HnswIndex::new(&config);
                    for (id, vector) in vecs {
                        let _ = index.insert(*id, vector);
                    }
                    index.build().unwrap();
                    black_box(index.len())
                })
            },
        );
    }

    group.finish();
}

fn bench_normalization(c: &mut Criterion) {
    let mut group = c.benchmark_group("normalization");

    for dim in [128, 384, 768, 1536] {
        let mut rng = StdRng::seed_from_u64(42);
        let v = random_vector(dim, &mut rng);

        group.bench_with_input(
            BenchmarkId::new("normalize_vector", dim),
            &v,
            |b, v| b.iter(|| black_box(normalize_vector(v))),
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_hnsw_search,
    bench_brute_force_vs_hnsw,
    bench_distance_metrics,
    bench_batch_insert,
    bench_normalization,
);

criterion_main!(benches);
