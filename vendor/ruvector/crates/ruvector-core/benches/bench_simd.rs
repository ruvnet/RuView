//! SIMD Performance Benchmarks
//!
//! This module benchmarks SIMD-optimized distance calculations
//! across various vector dimensions and operation types.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ruvector_core::simd_intrinsics::*;

// ============================================================================
// Helper Functions
// ============================================================================

fn generate_vectors(dim: usize) -> (Vec<f32>, Vec<f32>) {
    let a: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();
    let b: Vec<f32> = (0..dim).map(|i| ((i + 100) as f32) * 0.01).collect();
    (a, b)
}

fn generate_batch_vectors(dim: usize, count: usize) -> (Vec<f32>, Vec<Vec<f32>>) {
    let query: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();
    let vectors: Vec<Vec<f32>> = (0..count)
        .map(|j| (0..dim).map(|i| ((i + j * 10) as f32) * 0.01).collect())
        .collect();
    (query, vectors)
}

// ============================================================================
// Euclidean Distance Benchmarks
// ============================================================================

fn bench_euclidean_by_dimension(c: &mut Criterion) {
    let mut group = c.benchmark_group("euclidean_by_dimension");

    for dim in [32, 64, 128, 256, 384, 512, 768, 1024, 1536, 2048] {
        let (a, b) = generate_vectors(dim);

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(BenchmarkId::new("simd", dim), &dim, |bench, _| {
            bench.iter(|| euclidean_distance_simd(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

fn bench_euclidean_small_vectors(c: &mut Criterion) {
    let mut group = c.benchmark_group("euclidean_small_vectors");

    // Test small vector sizes that may not benefit from SIMD
    for dim in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 16] {
        let (a, b) = generate_vectors(dim);

        group.bench_with_input(BenchmarkId::new("simd", dim), &dim, |bench, _| {
            bench.iter(|| euclidean_distance_simd(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

fn bench_euclidean_non_aligned(c: &mut Criterion) {
    let mut group = c.benchmark_group("euclidean_non_aligned");

    // Test non-SIMD-aligned sizes
    for dim in [31, 33, 63, 65, 127, 129, 255, 257, 383, 385, 511, 513] {
        let (a, b) = generate_vectors(dim);

        group.bench_with_input(BenchmarkId::new("simd", dim), &dim, |bench, _| {
            bench.iter(|| euclidean_distance_simd(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

// ============================================================================
// Dot Product Benchmarks
// ============================================================================

fn bench_dot_product_by_dimension(c: &mut Criterion) {
    let mut group = c.benchmark_group("dot_product_by_dimension");

    for dim in [32, 64, 128, 256, 384, 512, 768, 1024, 1536, 2048] {
        let (a, b) = generate_vectors(dim);

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(BenchmarkId::new("simd", dim), &dim, |bench, _| {
            bench.iter(|| dot_product_simd(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

fn bench_dot_product_common_embeddings(c: &mut Criterion) {
    let mut group = c.benchmark_group("dot_product_common_embeddings");

    // Common embedding model dimensions
    let dims = [
        (128, "small"),
        (384, "all-MiniLM-L6"),
        (512, "e5-small"),
        (768, "all-mpnet-base"),
        (1024, "e5-large"),
        (1536, "text-embedding-ada-002"),
        (2048, "llama-7b-hidden"),
    ];

    for (dim, name) in dims {
        let (a, b) = generate_vectors(dim);

        group.bench_with_input(BenchmarkId::new(name, dim), &dim, |bench, _| {
            bench.iter(|| dot_product_simd(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

// ============================================================================
// Cosine Similarity Benchmarks
// ============================================================================

fn bench_cosine_by_dimension(c: &mut Criterion) {
    let mut group = c.benchmark_group("cosine_by_dimension");

    for dim in [32, 64, 128, 256, 384, 512, 768, 1024, 1536, 2048] {
        let (a, b) = generate_vectors(dim);

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(BenchmarkId::new("simd", dim), &dim, |bench, _| {
            bench.iter(|| cosine_similarity_simd(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

// ============================================================================
// Manhattan Distance Benchmarks
// ============================================================================

fn bench_manhattan_by_dimension(c: &mut Criterion) {
    let mut group = c.benchmark_group("manhattan_by_dimension");

    for dim in [32, 64, 128, 256, 384, 512, 768, 1024, 1536, 2048] {
        let (a, b) = generate_vectors(dim);

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(BenchmarkId::new("simd", dim), &dim, |bench, _| {
            bench.iter(|| manhattan_distance_simd(black_box(&a), black_box(&b)));
        });
    }

    group.finish();
}

// ============================================================================
// Batch Operations Benchmarks
// ============================================================================

fn bench_batch_euclidean(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_euclidean");

    for count in [10, 100, 1000, 10000] {
        let (query, vectors) = generate_batch_vectors(384, count);

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(BenchmarkId::new("384d", count), &count, |bench, _| {
            bench.iter(|| {
                for v in &vectors {
                    euclidean_distance_simd(black_box(&query), black_box(v));
                }
            });
        });
    }

    group.finish();
}

fn bench_batch_dot_product(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_dot_product");

    for count in [10, 100, 1000, 10000] {
        let (query, vectors) = generate_batch_vectors(768, count);

        group.throughput(Throughput::Elements(count as u64));

        group.bench_with_input(BenchmarkId::new("768d", count), &count, |bench, _| {
            bench.iter(|| {
                for v in &vectors {
                    dot_product_simd(black_box(&query), black_box(v));
                }
            });
        });
    }

    group.finish();
}

// ============================================================================
// Comparison Benchmarks (All Metrics)
// ============================================================================

fn bench_all_metrics_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("metrics_comparison");

    let dim = 384; // Common embedding dimension
    let (a, b) = generate_vectors(dim);

    group.bench_function("euclidean", |bench| {
        bench.iter(|| euclidean_distance_simd(black_box(&a), black_box(&b)));
    });

    group.bench_function("dot_product", |bench| {
        bench.iter(|| dot_product_simd(black_box(&a), black_box(&b)));
    });

    group.bench_function("cosine", |bench| {
        bench.iter(|| cosine_similarity_simd(black_box(&a), black_box(&b)));
    });

    group.bench_function("manhattan", |bench| {
        bench.iter(|| manhattan_distance_simd(black_box(&a), black_box(&b)));
    });

    group.finish();
}

// ============================================================================
// Memory Access Pattern Benchmarks
// ============================================================================

fn bench_sequential_vs_random_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("access_patterns");

    let dim = 512;
    let count = 1000;

    // Generate vectors
    let vectors: Vec<Vec<f32>> = (0..count)
        .map(|j| (0..dim).map(|i| ((i + j * 10) as f32) * 0.01).collect())
        .collect();
    let query: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();

    // Sequential access indices
    let sequential_indices: Vec<usize> = (0..count).collect();

    // Random-ish access indices
    let random_indices: Vec<usize> = (0..count)
        .map(|i| (i * 37 + 13) % count) // Pseudo-random
        .collect();

    group.bench_function("sequential", |bench| {
        bench.iter(|| {
            for &idx in &sequential_indices {
                euclidean_distance_simd(black_box(&query), black_box(&vectors[idx]));
            }
        });
    });

    group.bench_function("random", |bench| {
        bench.iter(|| {
            for &idx in &random_indices {
                euclidean_distance_simd(black_box(&query), black_box(&vectors[idx]));
            }
        });
    });

    group.finish();
}

// ============================================================================
// Throughput Measurement
// ============================================================================

fn bench_throughput_ops_per_second(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");
    group.sample_size(50);

    for dim in [128, 384, 768, 1536] {
        let (a, b) = generate_vectors(dim);

        // Report throughput in operations/second
        group.bench_with_input(BenchmarkId::new("euclidean_ops", dim), &dim, |bench, _| {
            bench.iter(|| {
                // Perform 100 operations per iteration
                for _ in 0..100 {
                    euclidean_distance_simd(black_box(&a), black_box(&b));
                }
            });
        });

        group.bench_with_input(
            BenchmarkId::new("dot_product_ops", dim),
            &dim,
            |bench, _| {
                bench.iter(|| {
                    for _ in 0..100 {
                        dot_product_simd(black_box(&a), black_box(&b));
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    benches,
    bench_euclidean_by_dimension,
    bench_euclidean_small_vectors,
    bench_euclidean_non_aligned,
    bench_dot_product_by_dimension,
    bench_dot_product_common_embeddings,
    bench_cosine_by_dimension,
    bench_manhattan_by_dimension,
    bench_batch_euclidean,
    bench_batch_dot_product,
    bench_all_metrics_comparison,
    bench_sequential_vs_random_access,
    bench_throughput_ops_per_second,
);

criterion_main!(benches);
