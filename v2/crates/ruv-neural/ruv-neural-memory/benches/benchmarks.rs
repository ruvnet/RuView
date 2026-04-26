//! Criterion benchmarks for ruv-neural-memory.
//!
//! Benchmarks the performance-critical vector search operations:
//! - HNSW insert (building the index)
//! - HNSW search (approximate nearest neighbor queries)
//! - Brute-force nearest neighbor (baseline comparison)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::Rng;

use ruv_neural_memory::HnswIndex;

const DIM: usize = 64;

/// Generate a set of random embeddings.
fn generate_embeddings(count: usize, dim: usize) -> Vec<Vec<f64>> {
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|_| (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect())
        .collect()
}

/// Build an HNSW index from a set of embeddings.
fn build_hnsw(embeddings: &[Vec<f64>]) -> HnswIndex {
    let mut index = HnswIndex::new(16, 200);
    for emb in embeddings {
        index.insert(emb);
    }
    index
}

/// Euclidean distance between two vectors.
fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

/// Brute-force k-nearest-neighbor search.
fn brute_force_knn(
    embeddings: &[Vec<f64>],
    query: &[f64],
    k: usize,
) -> Vec<(usize, f64)> {
    let mut distances: Vec<(usize, f64)> = embeddings
        .iter()
        .enumerate()
        .map(|(i, v)| (i, euclidean_distance(query, v)))
        .collect();
    distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    distances.truncate(k);
    distances
}

fn bench_hnsw_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_insert");
    group.sample_size(10);

    for &count in &[1_000, 10_000] {
        let embeddings = generate_embeddings(count, DIM);
        group.bench_with_input(
            BenchmarkId::new("embeddings", count),
            &embeddings,
            |b, embeddings| {
                b.iter(|| {
                    let mut index = HnswIndex::new(16, 200);
                    for emb in embeddings.iter() {
                        index.insert(black_box(emb));
                    }
                    index
                })
            },
        );
    }

    group.finish();
}

fn bench_hnsw_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_search");

    for &count in &[1_000, 10_000] {
        let embeddings = generate_embeddings(count, DIM);
        let index = build_hnsw(&embeddings);
        let mut rng = rand::thread_rng();
        let query: Vec<f64> = (0..DIM).map(|_| rng.gen_range(-1.0..1.0)).collect();

        group.bench_with_input(
            BenchmarkId::new("k10_embeddings", count),
            &(index, query),
            |b, (index, query)| {
                b.iter(|| index.search(black_box(query), black_box(10), black_box(50)))
            },
        );
    }

    group.finish();
}

fn bench_brute_force_nn(c: &mut Criterion) {
    let mut group = c.benchmark_group("brute_force_nn");

    for &count in &[1_000, 10_000] {
        let embeddings = generate_embeddings(count, DIM);
        let mut rng = rand::thread_rng();
        let query: Vec<f64> = (0..DIM).map(|_| rng.gen_range(-1.0..1.0)).collect();

        group.bench_with_input(
            BenchmarkId::new("k10_embeddings", count),
            &(embeddings, query),
            |b, (embeddings, query)| {
                b.iter(|| brute_force_knn(black_box(embeddings), black_box(query), black_box(10)))
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_hnsw_insert,
    bench_hnsw_search,
    bench_brute_force_nn,
);
criterion_main!(benches);
