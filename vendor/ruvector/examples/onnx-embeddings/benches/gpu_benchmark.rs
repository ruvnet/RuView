//! GPU Acceleration Benchmarks
//!
//! Benchmarks comparing CPU vs GPU performance for:
//! - Similarity computations
//! - Pooling operations
//! - Vector operations

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};

#[cfg(feature = "gpu")]
use ruvector_onnx_embeddings::gpu::{
    GpuAccelerator, GpuConfig, GpuPooler, GpuSimilarity, GpuVectorOps,
    batch_cosine_similarity_gpu, batch_dot_product_gpu, batch_euclidean_gpu,
};

/// CPU baseline implementations for comparison
mod cpu_baseline {
    use rayon::prelude::*;

    pub fn batch_cosine_similarity(query: &[f32], candidates: &[Vec<f32>]) -> Vec<f32> {
        candidates
            .par_iter()
            .map(|c| cosine_similarity(query, c))
            .collect()
    }

    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a > 1e-12 && norm_b > 1e-12 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }

    pub fn mean_pool(
        tokens: &[f32],
        mask: &[i64],
        batch_size: usize,
        seq_length: usize,
        hidden_size: usize,
    ) -> Vec<f32> {
        let mut output = vec![0.0f32; batch_size * hidden_size];

        for batch_idx in 0..batch_size {
            let tokens_base = batch_idx * seq_length * hidden_size;
            let mask_base = batch_idx * seq_length;
            let out_base = batch_idx * hidden_size;

            let mut count = 0.0f32;

            for seq_idx in 0..seq_length {
                if mask[mask_base + seq_idx] == 1 {
                    let start = tokens_base + seq_idx * hidden_size;
                    for j in 0..hidden_size {
                        output[out_base + j] += tokens[start + j];
                    }
                    count += 1.0;
                }
            }

            if count > 0.0 {
                for j in 0..hidden_size {
                    output[out_base + j] /= count;
                }
            }
        }

        output
    }

    pub fn normalize_batch(vectors: &mut [f32], dimension: usize) {
        for chunk in vectors.chunks_mut(dimension) {
            let norm: f32 = chunk.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 1e-12 {
                for val in chunk.iter_mut() {
                    *val /= norm;
                }
            }
        }
    }
}

// ==================== Similarity Benchmarks ====================

fn similarity_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("similarity");

    // Test different dimensions
    for dimension in [128, 384, 768, 1536].iter() {
        let query: Vec<f32> = (0..*dimension).map(|i| (i as f32) * 0.001).collect();

        // Test different candidate counts
        for num_candidates in [100, 1000, 10000].iter() {
            let candidates: Vec<Vec<f32>> = (0..*num_candidates)
                .map(|i| {
                    (0..*dimension)
                        .map(|j| ((i + j) as f32) * 0.0001)
                        .collect()
                })
                .collect();

            let id = format!("dim{}_n{}", dimension, num_candidates);

            group.throughput(Throughput::Elements(*num_candidates as u64));

            // CPU baseline
            group.bench_with_input(
                BenchmarkId::new("cpu_cosine", &id),
                &(&query, &candidates),
                |b, (q, c)| {
                    b.iter(|| cpu_baseline::batch_cosine_similarity(black_box(q), black_box(c)))
                },
            );

            // GPU implementation (uses rayon parallel CPU when GPU unavailable)
            #[cfg(feature = "gpu")]
            {
                let refs: Vec<&[f32]> = candidates.iter().map(|v| v.as_slice()).collect();
                group.bench_with_input(
                    BenchmarkId::new("gpu_cosine", &id),
                    &(&query, &refs),
                    |b, (q, c)| {
                        b.iter(|| batch_cosine_similarity_gpu(black_box(q), black_box(c)))
                    },
                );
            }
        }
    }

    group.finish();
}

// ==================== Pooling Benchmarks ====================

fn pooling_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("gpu_pooling");

    // Test different batch sizes and sequence lengths
    for (batch_size, seq_length, hidden_size) in [
        (1, 128, 384),
        (8, 128, 384),
        (32, 128, 384),
        (64, 256, 768),
        (128, 512, 384),
    ] {
        let tokens: Vec<f32> = (0..batch_size * seq_length * hidden_size)
            .map(|i| (i as f32) * 0.0001)
            .collect();

        let mask: Vec<i64> = (0..batch_size * seq_length)
            .map(|i| if i % seq_length < seq_length - 10 { 1 } else { 0 })
            .collect();

        let id = format!("b{}_s{}_h{}", batch_size, seq_length, hidden_size);

        group.throughput(Throughput::Elements(batch_size as u64));

        // CPU baseline
        group.bench_with_input(
            BenchmarkId::new("cpu_mean_pool", &id),
            &(&tokens, &mask, batch_size, seq_length, hidden_size),
            |b, (t, m, bs, sl, hs)| {
                b.iter(|| {
                    cpu_baseline::mean_pool(black_box(t), black_box(m), *bs, *sl, *hs)
                })
            },
        );

        // Note: GPU pooling would be benchmarked here when full GPU backend is implemented
    }

    group.finish();
}

// ==================== Vector Operations Benchmarks ====================

fn vector_ops_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_ops");

    // Test normalization at different scales
    for (num_vectors, dimension) in [
        (100, 384),
        (1000, 384),
        (10000, 384),
        (1000, 768),
        (1000, 1536),
    ] {
        let mut vectors: Vec<f32> = (0..num_vectors * dimension)
            .map(|i| (i as f32) * 0.001)
            .collect();

        let id = format!("n{}_d{}", num_vectors, dimension);

        group.throughput(Throughput::Elements(num_vectors as u64));

        // CPU baseline
        group.bench_with_input(
            BenchmarkId::new("cpu_normalize", &id),
            &(dimension,),
            |b, (dim,)| {
                let mut v = vectors.clone();
                b.iter(|| {
                    cpu_baseline::normalize_batch(black_box(&mut v), *dim)
                })
            },
        );
    }

    group.finish();
}

// ==================== End-to-End Benchmarks ====================

fn e2e_similarity_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e_search");

    // Realistic similarity search scenario
    let dimension = 384;
    let num_candidates = 10000;
    let top_k = 10;

    let query: Vec<f32> = (0..dimension).map(|i| (i as f32) * 0.001).collect();
    let candidates: Vec<Vec<f32>> = (0..num_candidates)
        .map(|i| {
            (0..dimension)
                .map(|j| ((i * j) as f32).sin() * 0.1)
                .collect()
        })
        .collect();

    group.throughput(Throughput::Elements(num_candidates as u64));

    // CPU: compute similarities and find top-k
    group.bench_function("cpu_top_k", |b| {
        b.iter(|| {
            let sims = cpu_baseline::batch_cosine_similarity(black_box(&query), black_box(&candidates));
            let mut indexed: Vec<(usize, f32)> = sims.into_iter().enumerate().collect();
            indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            indexed.truncate(top_k);
            indexed
        })
    });

    // GPU path
    #[cfg(feature = "gpu")]
    {
        let refs: Vec<&[f32]> = candidates.iter().map(|v| v.as_slice()).collect();
        group.bench_function("gpu_top_k", |b| {
            b.iter(|| {
                let sims = batch_cosine_similarity_gpu(black_box(&query), black_box(&refs));
                let mut indexed: Vec<(usize, f32)> = sims.into_iter().enumerate().collect();
                indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                indexed.truncate(top_k);
                indexed
            })
        });
    }

    group.finish();
}

// ==================== Memory Throughput Benchmarks ====================

fn memory_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_throughput");

    // Measure memory bandwidth with different sizes
    for size_mb in [1, 10, 100].iter() {
        let size = size_mb * 1024 * 1024 / 4; // Convert MB to f32 count
        let data: Vec<f32> = (0..size).map(|i| i as f32).collect();

        group.throughput(Throughput::Bytes((*size_mb * 1024 * 1024) as u64));

        // Simple copy benchmark
        group.bench_with_input(
            BenchmarkId::new("copy", format!("{}MB", size_mb)),
            &data,
            |b, d| {
                b.iter(|| {
                    let _copy: Vec<f32> = black_box(d).iter().copied().collect();
                })
            },
        );

        // Sum reduction benchmark
        group.bench_with_input(
            BenchmarkId::new("sum", format!("{}MB", size_mb)),
            &data,
            |b, d| {
                b.iter(|| {
                    let sum: f32 = black_box(d).iter().sum();
                    sum
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    similarity_benchmarks,
    pooling_benchmarks,
    vector_ops_benchmarks,
    e2e_similarity_search,
    memory_throughput,
);

criterion_main!(benches);
