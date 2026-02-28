//! Comprehensive distance function benchmarks
//!
//! Compare SIMD vs scalar implementations across different vector sizes
//! and distance metrics (L2, cosine, inner product, Manhattan).
//!
//! Dimensions tested: 128, 384, 768, 1536, 3072
//! This covers common embedding sizes:
//! - 128: SBERT MiniLM
//! - 384: all-MiniLM-L6-v2
//! - 768: BERT base, RoBERTa
//! - 1536: OpenAI text-embedding-ada-002
//! - 3072: OpenAI text-embedding-3-large

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;

// ============================================================================
// Distance Implementations
// ============================================================================

mod distance_impl {
    /// Scalar Euclidean distance
    pub fn euclidean_scalar(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| {
                let diff = x - y;
                diff * diff
            })
            .sum::<f32>()
            .sqrt()
    }

    /// Scalar cosine distance
    pub fn cosine_scalar(a: &[f32], b: &[f32]) -> f32 {
        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for (x, y) in a.iter().zip(b.iter()) {
            dot += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }

        let denominator = (norm_a * norm_b).sqrt();
        if denominator == 0.0 {
            return 1.0;
        }

        1.0 - (dot / denominator)
    }

    /// Scalar inner product distance (negative)
    pub fn inner_product_scalar(a: &[f32], b: &[f32]) -> f32 {
        -a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>()
    }

    /// Scalar Manhattan distance
    pub fn manhattan_scalar(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).abs())
            .sum::<f32>()
    }

    /// AVX2 Euclidean distance squared (L2^2)
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2", enable = "fma")]
    pub unsafe fn euclidean_avx2(a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let n = a.len();
        let mut sum = _mm256_setzero_ps();

        let chunks = n / 8;
        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm256_loadu_ps(b.as_ptr().add(offset));
            let diff = _mm256_sub_ps(va, vb);
            sum = _mm256_fmadd_ps(diff, diff, sum);
        }

        // Horizontal sum
        let sum_high = _mm256_extractf128_ps(sum, 1);
        let sum_low = _mm256_castps256_ps128(sum);
        let sum128 = _mm_add_ps(sum_high, sum_low);
        let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
        let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));

        let mut result = _mm_cvtss_f32(sum32);

        // Handle remainder
        for i in (chunks * 8)..n {
            let diff = a[i] - b[i];
            result += diff * diff;
        }

        result.sqrt()
    }

    /// AVX2 cosine distance
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2", enable = "fma")]
    pub unsafe fn cosine_avx2(a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let n = a.len();
        let mut dot_sum = _mm256_setzero_ps();
        let mut norm_a_sum = _mm256_setzero_ps();
        let mut norm_b_sum = _mm256_setzero_ps();

        let chunks = n / 8;
        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm256_loadu_ps(b.as_ptr().add(offset));

            dot_sum = _mm256_fmadd_ps(va, vb, dot_sum);
            norm_a_sum = _mm256_fmadd_ps(va, va, norm_a_sum);
            norm_b_sum = _mm256_fmadd_ps(vb, vb, norm_b_sum);
        }

        // Horizontal sums
        let h_dot = horizontal_sum_avx2(dot_sum);
        let h_norm_a = horizontal_sum_avx2(norm_a_sum);
        let h_norm_b = horizontal_sum_avx2(norm_b_sum);

        // Handle remainder
        let mut dot = h_dot;
        let mut norm_a = h_norm_a;
        let mut norm_b = h_norm_b;
        for i in (chunks * 8)..n {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }

        let denom = (norm_a * norm_b).sqrt();
        if denom == 0.0 {
            return 1.0;
        }
        1.0 - (dot / denom)
    }

    /// AVX2 inner product
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "avx2", enable = "fma")]
    pub unsafe fn inner_product_avx2(a: &[f32], b: &[f32]) -> f32 {
        use std::arch::x86_64::*;

        let n = a.len();
        let mut sum = _mm256_setzero_ps();

        let chunks = n / 8;
        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_ps(a.as_ptr().add(offset));
            let vb = _mm256_loadu_ps(b.as_ptr().add(offset));
            sum = _mm256_fmadd_ps(va, vb, sum);
        }

        let mut result = horizontal_sum_avx2(sum);

        // Handle remainder
        for i in (chunks * 8)..n {
            result += a[i] * b[i];
        }

        -result
    }

    #[cfg(target_arch = "x86_64")]
    #[inline]
    unsafe fn horizontal_sum_avx2(v: std::arch::x86_64::__m256) -> f32 {
        use std::arch::x86_64::*;
        let sum_high = _mm256_extractf128_ps(v, 1);
        let sum_low = _mm256_castps256_ps128(v);
        let sum128 = _mm_add_ps(sum_high, sum_low);
        let sum64 = _mm_add_ps(sum128, _mm_movehl_ps(sum128, sum128));
        let sum32 = _mm_add_ss(sum64, _mm_shuffle_ps(sum64, sum64, 1));
        _mm_cvtss_f32(sum32)
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub unsafe fn euclidean_avx2(a: &[f32], b: &[f32]) -> f32 {
        euclidean_scalar(a, b)
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub unsafe fn cosine_avx2(a: &[f32], b: &[f32]) -> f32 {
        cosine_scalar(a, b)
    }

    #[cfg(not(target_arch = "x86_64"))]
    pub unsafe fn inner_product_avx2(a: &[f32], b: &[f32]) -> f32 {
        inner_product_scalar(a, b)
    }
}

// ============================================================================
// Test Data Generation
// ============================================================================

fn generate_vectors(dims: usize, seed: u64) -> (Vec<f32>, Vec<f32>) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let a: Vec<f32> = (0..dims).map(|_| rng.gen_range(-1.0..1.0)).collect();
    let b: Vec<f32> = (0..dims).map(|_| rng.gen_range(-1.0..1.0)).collect();
    (a, b)
}

fn generate_normalized_vectors(dims: usize, seed: u64) -> (Vec<f32>, Vec<f32>) {
    let (mut a, mut b) = generate_vectors(dims, seed);

    // Normalize vectors
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    for x in &mut a {
        *x /= norm_a;
    }
    for x in &mut b {
        *x /= norm_b;
    }

    (a, b)
}

fn generate_vector_dataset(n: usize, dims: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..n)
        .map(|_| (0..dims).map(|_| rng.gen_range(-1.0..1.0)).collect())
        .collect()
}

// ============================================================================
// Euclidean Distance Benchmarks
// ============================================================================

const DIMENSIONS: [usize; 5] = [128, 384, 768, 1536, 3072];

fn bench_euclidean(c: &mut Criterion) {
    let mut group = c.benchmark_group("Euclidean Distance");

    for dims in DIMENSIONS.iter() {
        let (a, b) = generate_vectors(*dims, 42);

        group.throughput(Throughput::Elements(*dims as u64));

        group.bench_with_input(BenchmarkId::new("scalar", dims), dims, |bench, _| {
            bench.iter(|| distance_impl::euclidean_scalar(black_box(&a), black_box(&b)))
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            group.bench_with_input(BenchmarkId::new("avx2", dims), dims, |bench, _| {
                bench
                    .iter(|| unsafe { distance_impl::euclidean_avx2(black_box(&a), black_box(&b)) })
            });
        }
    }

    group.finish();
}

// ============================================================================
// Cosine Distance Benchmarks
// ============================================================================

fn bench_cosine(c: &mut Criterion) {
    let mut group = c.benchmark_group("Cosine Distance");

    for dims in DIMENSIONS.iter() {
        let (a, b) = generate_vectors(*dims, 42);

        group.throughput(Throughput::Elements(*dims as u64));

        group.bench_with_input(BenchmarkId::new("scalar", dims), dims, |bench, _| {
            bench.iter(|| distance_impl::cosine_scalar(black_box(&a), black_box(&b)))
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            group.bench_with_input(BenchmarkId::new("avx2", dims), dims, |bench, _| {
                bench.iter(|| unsafe { distance_impl::cosine_avx2(black_box(&a), black_box(&b)) })
            });
        }
    }

    group.finish();
}

// ============================================================================
// Cosine Distance for Pre-Normalized Vectors
// ============================================================================

fn bench_cosine_normalized(c: &mut Criterion) {
    let mut group = c.benchmark_group("Cosine Distance (Normalized)");

    for dims in DIMENSIONS.iter() {
        let (a, b) = generate_normalized_vectors(*dims, 42);

        group.throughput(Throughput::Elements(*dims as u64));

        // For normalized vectors, cosine = 1 - dot product
        group.bench_with_input(BenchmarkId::new("scalar_dot", dims), dims, |bench, _| {
            bench.iter(|| {
                let dot: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
                1.0 - black_box(dot)
            })
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            group.bench_with_input(BenchmarkId::new("avx2_dot", dims), dims, |bench, _| {
                bench.iter(|| unsafe {
                    1.0 + distance_impl::inner_product_avx2(black_box(&a), black_box(&b))
                })
            });
        }
    }

    group.finish();
}

// ============================================================================
// Inner Product Benchmarks
// ============================================================================

fn bench_inner_product(c: &mut Criterion) {
    let mut group = c.benchmark_group("Inner Product");

    for dims in DIMENSIONS.iter() {
        let (a, b) = generate_vectors(*dims, 42);

        group.throughput(Throughput::Elements(*dims as u64));

        group.bench_with_input(BenchmarkId::new("scalar", dims), dims, |bench, _| {
            bench.iter(|| distance_impl::inner_product_scalar(black_box(&a), black_box(&b)))
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            group.bench_with_input(BenchmarkId::new("avx2", dims), dims, |bench, _| {
                bench.iter(|| unsafe {
                    distance_impl::inner_product_avx2(black_box(&a), black_box(&b))
                })
            });
        }
    }

    group.finish();
}

// ============================================================================
// Manhattan Distance Benchmarks
// ============================================================================

fn bench_manhattan(c: &mut Criterion) {
    let mut group = c.benchmark_group("Manhattan Distance");

    for dims in DIMENSIONS.iter() {
        let (a, b) = generate_vectors(*dims, 42);

        group.throughput(Throughput::Elements(*dims as u64));

        group.bench_with_input(BenchmarkId::new("scalar", dims), dims, |bench, _| {
            bench.iter(|| distance_impl::manhattan_scalar(black_box(&a), black_box(&b)))
        });
    }

    group.finish();
}

// ============================================================================
// Batch Distance Benchmarks (1000 vectors)
// ============================================================================

fn bench_batch_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("Batch Distance (Sequential, 1000 vectors)");

    for dims in [128, 384, 1536].iter() {
        let query = generate_vectors(*dims, 42).0;
        let vectors = generate_vector_dataset(1000, *dims, 123);

        group.throughput(Throughput::Elements(1000));

        group.bench_with_input(BenchmarkId::new("euclidean", dims), dims, |bench, _| {
            bench.iter(|| {
                vectors
                    .iter()
                    .map(|v| distance_impl::euclidean_scalar(black_box(&query), black_box(v)))
                    .collect::<Vec<_>>()
            })
        });

        group.bench_with_input(BenchmarkId::new("cosine", dims), dims, |bench, _| {
            bench.iter(|| {
                vectors
                    .iter()
                    .map(|v| distance_impl::cosine_scalar(black_box(&query), black_box(v)))
                    .collect::<Vec<_>>()
            })
        });

        group.bench_with_input(BenchmarkId::new("inner_product", dims), dims, |bench, _| {
            bench.iter(|| {
                vectors
                    .iter()
                    .map(|v| distance_impl::inner_product_scalar(black_box(&query), black_box(v)))
                    .collect::<Vec<_>>()
            })
        });
    }

    group.finish();
}

fn bench_batch_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("Batch Distance (Parallel, 1000 vectors)");

    for dims in [128, 384, 1536].iter() {
        let query = generate_vectors(*dims, 42).0;
        let vectors = generate_vector_dataset(1000, *dims, 123);

        group.throughput(Throughput::Elements(1000));

        group.bench_with_input(
            BenchmarkId::new("euclidean_rayon", dims),
            dims,
            |bench, _| {
                bench.iter(|| {
                    vectors
                        .par_iter()
                        .map(|v| distance_impl::euclidean_scalar(black_box(&query), black_box(v)))
                        .collect::<Vec<_>>()
                })
            },
        );

        group.bench_with_input(BenchmarkId::new("cosine_rayon", dims), dims, |bench, _| {
            bench.iter(|| {
                vectors
                    .par_iter()
                    .map(|v| distance_impl::cosine_scalar(black_box(&query), black_box(v)))
                    .collect::<Vec<_>>()
            })
        });
    }

    group.finish();
}

// ============================================================================
// Large Batch Benchmarks (10K vectors)
// ============================================================================

fn bench_large_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("Large Batch Distance (10K vectors)");
    group.sample_size(10);

    for dims in [384, 768, 1536].iter() {
        let query = generate_vectors(*dims, 42).0;
        let vectors = generate_vector_dataset(10_000, *dims, 123);

        group.throughput(Throughput::Elements(10_000));

        group.bench_with_input(BenchmarkId::new("sequential", dims), dims, |bench, _| {
            bench.iter(|| {
                vectors
                    .iter()
                    .map(|v| distance_impl::euclidean_scalar(black_box(&query), black_box(v)))
                    .collect::<Vec<_>>()
            })
        });

        group.bench_with_input(BenchmarkId::new("parallel", dims), dims, |bench, _| {
            bench.iter(|| {
                vectors
                    .par_iter()
                    .map(|v| distance_impl::euclidean_scalar(black_box(&query), black_box(v)))
                    .collect::<Vec<_>>()
            })
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            group.bench_with_input(BenchmarkId::new("parallel_avx2", dims), dims, |bench, _| {
                bench.iter(|| {
                    vectors
                        .par_iter()
                        .map(|v| unsafe {
                            distance_impl::euclidean_avx2(black_box(&query), black_box(v))
                        })
                        .collect::<Vec<_>>()
                })
            });
        }
    }

    group.finish();
}

// ============================================================================
// SIMD Speedup Comparison
// ============================================================================

fn bench_simd_speedup(c: &mut Criterion) {
    let mut group = c.benchmark_group("SIMD Speedup Analysis");

    #[cfg(target_arch = "x86_64")]
    if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
        for dims in DIMENSIONS.iter() {
            let (a, b) = generate_vectors(*dims, 42);

            // Euclidean
            group.bench_with_input(
                BenchmarkId::new("euclidean_scalar", dims),
                dims,
                |bench, _| {
                    bench.iter(|| distance_impl::euclidean_scalar(black_box(&a), black_box(&b)))
                },
            );

            group.bench_with_input(
                BenchmarkId::new("euclidean_avx2", dims),
                dims,
                |bench, _| {
                    bench.iter(|| unsafe {
                        distance_impl::euclidean_avx2(black_box(&a), black_box(&b))
                    })
                },
            );

            // Cosine
            group.bench_with_input(BenchmarkId::new("cosine_scalar", dims), dims, |bench, _| {
                bench.iter(|| distance_impl::cosine_scalar(black_box(&a), black_box(&b)))
            });

            group.bench_with_input(BenchmarkId::new("cosine_avx2", dims), dims, |bench, _| {
                bench.iter(|| unsafe { distance_impl::cosine_avx2(black_box(&a), black_box(&b)) })
            });
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_euclidean,
    bench_cosine,
    bench_cosine_normalized,
    bench_inner_product,
    bench_manhattan,
    bench_batch_sequential,
    bench_batch_parallel,
    bench_large_batch,
    bench_simd_speedup,
);

criterion_main!(benches);
