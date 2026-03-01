//! SIMD-Specific Benchmarks for Prime-Radiant Coherence Engine
//!
//! This benchmark suite compares naive/scalar implementations against
//! SIMD-optimized versions for core coherence operations.
//!
//! ## Benchmark Categories
//! 1. Dense Matrix Multiply - naive vs SIMD
//! 2. Vector Norm Computation - naive vs SIMD
//! 3. Batch Residual Computation - naive vs SIMD
//! 4. Dot Products and Reductions
//!
//! ## Architecture Notes
//! - x86_64: AVX2 (256-bit, f32x8) or AVX-512 (512-bit, f32x16)
//! - aarch64: NEON (128-bit, f32x4)
//! - WASM: SIMD128 (128-bit)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// ============================================================================
// TEST DATA GENERATION
// ============================================================================

fn generate_vec(len: usize, seed: u64) -> Vec<f32> {
    (0..len)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            (seed, i).hash(&mut hasher);
            (hasher.finish() % 1000) as f32 / 1000.0 - 0.5
        })
        .collect()
}

fn generate_matrix(rows: usize, cols: usize, seed: u64) -> Vec<f32> {
    (0..rows * cols)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            (seed, i).hash(&mut hasher);
            (hasher.finish() % 1000) as f32 / 1000.0 - 0.5
        })
        .collect()
}

// ============================================================================
// NAIVE IMPLEMENTATIONS (BASELINE)
// ============================================================================

/// Naive matrix-vector multiply: y = Ax
#[inline(never)]
fn matmul_naive(matrix: &[f32], x: &[f32], y: &mut [f32], rows: usize, cols: usize) {
    for i in 0..rows {
        let mut sum = 0.0f32;
        let row_start = i * cols;
        for j in 0..cols {
            sum += matrix[row_start + j] * x[j];
        }
        y[i] = sum;
    }
}

/// Naive squared norm: |v|^2
#[inline(never)]
fn norm_sq_naive(v: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for &x in v {
        sum += x * x;
    }
    sum
}

/// Naive dot product: a . b
#[inline(never)]
fn dot_naive(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for i in 0..a.len() {
        sum += a[i] * b[i];
    }
    sum
}

/// Naive residual norm: |a - b|^2
#[inline(never)]
fn residual_norm_naive(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for i in 0..a.len() {
        let diff = a[i] - b[i];
        sum += diff * diff;
    }
    sum
}

/// Naive batch residual computation
#[inline(never)]
fn batch_residual_naive(sources: &[Vec<f32>], targets: &[Vec<f32>]) -> f32 {
    let mut total = 0.0f32;
    for (src, tgt) in sources.iter().zip(targets.iter()) {
        total += residual_norm_naive(src, tgt);
    }
    total
}

// ============================================================================
// SIMD-FRIENDLY IMPLEMENTATIONS
// ============================================================================

/// Unrolled matrix-vector multiply (auto-vectorization friendly)
#[inline(never)]
fn matmul_unrolled(matrix: &[f32], x: &[f32], y: &mut [f32], rows: usize, cols: usize) {
    for i in 0..rows {
        let row_start = i * cols;

        // Process in chunks of 8
        let chunks = cols / 8;
        let mut acc0 = 0.0f32;
        let mut acc1 = 0.0f32;
        let mut acc2 = 0.0f32;
        let mut acc3 = 0.0f32;
        let mut acc4 = 0.0f32;
        let mut acc5 = 0.0f32;
        let mut acc6 = 0.0f32;
        let mut acc7 = 0.0f32;

        for c in 0..chunks {
            let base = row_start + c * 8;
            acc0 += matrix[base] * x[c * 8];
            acc1 += matrix[base + 1] * x[c * 8 + 1];
            acc2 += matrix[base + 2] * x[c * 8 + 2];
            acc3 += matrix[base + 3] * x[c * 8 + 3];
            acc4 += matrix[base + 4] * x[c * 8 + 4];
            acc5 += matrix[base + 5] * x[c * 8 + 5];
            acc6 += matrix[base + 6] * x[c * 8 + 6];
            acc7 += matrix[base + 7] * x[c * 8 + 7];
        }

        let mut sum = acc0 + acc1 + acc2 + acc3 + acc4 + acc5 + acc6 + acc7;

        // Handle remainder
        for j in (chunks * 8)..cols {
            sum += matrix[row_start + j] * x[j];
        }

        y[i] = sum;
    }
}

/// Unrolled squared norm with 4 accumulators
#[inline(never)]
fn norm_sq_unrolled(v: &[f32]) -> f32 {
    let chunks = v.chunks_exact(4);
    let remainder = chunks.remainder();

    let mut acc0 = 0.0f32;
    let mut acc1 = 0.0f32;
    let mut acc2 = 0.0f32;
    let mut acc3 = 0.0f32;

    for chunk in chunks {
        acc0 += chunk[0] * chunk[0];
        acc1 += chunk[1] * chunk[1];
        acc2 += chunk[2] * chunk[2];
        acc3 += chunk[3] * chunk[3];
    }

    let mut sum = acc0 + acc1 + acc2 + acc3;
    for &x in remainder {
        sum += x * x;
    }
    sum
}

/// Unrolled squared norm with 8 accumulators (better for wider SIMD)
#[inline(never)]
fn norm_sq_unrolled_8(v: &[f32]) -> f32 {
    let chunks = v.chunks_exact(8);
    let remainder = chunks.remainder();

    let mut acc = [0.0f32; 8];

    for chunk in chunks {
        acc[0] += chunk[0] * chunk[0];
        acc[1] += chunk[1] * chunk[1];
        acc[2] += chunk[2] * chunk[2];
        acc[3] += chunk[3] * chunk[3];
        acc[4] += chunk[4] * chunk[4];
        acc[5] += chunk[5] * chunk[5];
        acc[6] += chunk[6] * chunk[6];
        acc[7] += chunk[7] * chunk[7];
    }

    let mut sum: f32 = acc.iter().sum();
    for &x in remainder {
        sum += x * x;
    }
    sum
}

/// Iterator-based squared norm (relies on auto-vectorization)
#[inline(never)]
fn norm_sq_iter(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum()
}

/// Unrolled dot product
#[inline(never)]
fn dot_unrolled(a: &[f32], b: &[f32]) -> f32 {
    let chunks_a = a.chunks_exact(4);
    let chunks_b = b.chunks_exact(4);
    let rem_a = chunks_a.remainder();
    let rem_b = chunks_b.remainder();

    let mut acc0 = 0.0f32;
    let mut acc1 = 0.0f32;
    let mut acc2 = 0.0f32;
    let mut acc3 = 0.0f32;

    for (ca, cb) in chunks_a.zip(chunks_b) {
        acc0 += ca[0] * cb[0];
        acc1 += ca[1] * cb[1];
        acc2 += ca[2] * cb[2];
        acc3 += ca[3] * cb[3];
    }

    let mut sum = acc0 + acc1 + acc2 + acc3;
    for (&a, &b) in rem_a.iter().zip(rem_b.iter()) {
        sum += a * b;
    }
    sum
}

/// Unrolled residual norm
#[inline(never)]
fn residual_norm_unrolled(a: &[f32], b: &[f32]) -> f32 {
    let chunks_a = a.chunks_exact(4);
    let chunks_b = b.chunks_exact(4);
    let rem_a = chunks_a.remainder();
    let rem_b = chunks_b.remainder();

    let mut acc0 = 0.0f32;
    let mut acc1 = 0.0f32;
    let mut acc2 = 0.0f32;
    let mut acc3 = 0.0f32;

    for (ca, cb) in chunks_a.zip(chunks_b) {
        let d0 = ca[0] - cb[0];
        let d1 = ca[1] - cb[1];
        let d2 = ca[2] - cb[2];
        let d3 = ca[3] - cb[3];
        acc0 += d0 * d0;
        acc1 += d1 * d1;
        acc2 += d2 * d2;
        acc3 += d3 * d3;
    }

    let mut sum = acc0 + acc1 + acc2 + acc3;
    for (&a, &b) in rem_a.iter().zip(rem_b.iter()) {
        let d = a - b;
        sum += d * d;
    }
    sum
}

/// Batch residual with unrolled inner loop
#[inline(never)]
fn batch_residual_unrolled(sources: &[Vec<f32>], targets: &[Vec<f32>]) -> f32 {
    let mut total = 0.0f32;
    for (src, tgt) in sources.iter().zip(targets.iter()) {
        total += residual_norm_unrolled(src, tgt);
    }
    total
}

// ============================================================================
// EXPLICIT SIMD (when wide crate is available)
// ============================================================================

#[cfg(feature = "simd")]
mod simd_impl {
    use wide::f32x8;

    /// SIMD squared norm using f32x8
    #[inline(never)]
    pub fn norm_sq_simd(v: &[f32]) -> f32 {
        let chunks = v.chunks_exact(8);
        let remainder = chunks.remainder();

        let mut acc = f32x8::ZERO;

        for chunk in chunks {
            let vals = f32x8::from(<[f32; 8]>::try_from(chunk).unwrap());
            acc += vals * vals;
        }

        let mut sum: f32 = acc.reduce_add();
        for &x in remainder {
            sum += x * x;
        }
        sum
    }

    /// SIMD dot product using f32x8
    #[inline(never)]
    pub fn dot_simd(a: &[f32], b: &[f32]) -> f32 {
        let chunks_a = a.chunks_exact(8);
        let chunks_b = b.chunks_exact(8);
        let rem_a = chunks_a.remainder();
        let rem_b = chunks_b.remainder();

        let mut acc = f32x8::ZERO;

        for (ca, cb) in chunks_a.zip(chunks_b) {
            let va = f32x8::from(<[f32; 8]>::try_from(ca).unwrap());
            let vb = f32x8::from(<[f32; 8]>::try_from(cb).unwrap());
            acc += va * vb;
        }

        let mut sum: f32 = acc.reduce_add();
        for (&a, &b) in rem_a.iter().zip(rem_b.iter()) {
            sum += a * b;
        }
        sum
    }

    /// SIMD residual norm using f32x8
    #[inline(never)]
    pub fn residual_norm_simd(a: &[f32], b: &[f32]) -> f32 {
        let chunks_a = a.chunks_exact(8);
        let chunks_b = b.chunks_exact(8);
        let rem_a = chunks_a.remainder();
        let rem_b = chunks_b.remainder();

        let mut acc = f32x8::ZERO;

        for (ca, cb) in chunks_a.zip(chunks_b) {
            let va = f32x8::from(<[f32; 8]>::try_from(ca).unwrap());
            let vb = f32x8::from(<[f32; 8]>::try_from(cb).unwrap());
            let diff = va - vb;
            acc += diff * diff;
        }

        let mut sum: f32 = acc.reduce_add();
        for (&a, &b) in rem_a.iter().zip(rem_b.iter()) {
            let d = a - b;
            sum += d * d;
        }
        sum
    }

    /// SIMD matrix-vector multiply
    #[inline(never)]
    pub fn matmul_simd(matrix: &[f32], x: &[f32], y: &mut [f32], rows: usize, cols: usize) {
        for i in 0..rows {
            let row_start = i * cols;
            let row = &matrix[row_start..row_start + cols];

            let chunks_m = row.chunks_exact(8);
            let chunks_x = x.chunks_exact(8);
            let rem_m = chunks_m.remainder();
            let rem_x = chunks_x.remainder();

            let mut acc = f32x8::ZERO;

            for (cm, cx) in chunks_m.zip(chunks_x) {
                let vm = f32x8::from(<[f32; 8]>::try_from(cm).unwrap());
                let vx = f32x8::from(<[f32; 8]>::try_from(cx).unwrap());
                acc += vm * vx;
            }

            let mut sum: f32 = acc.reduce_add();
            for (&m, &xv) in rem_m.iter().zip(rem_x.iter()) {
                sum += m * xv;
            }

            y[i] = sum;
        }
    }

    /// SIMD batch residual
    #[inline(never)]
    pub fn batch_residual_simd(sources: &[Vec<f32>], targets: &[Vec<f32>]) -> f32 {
        let mut total = 0.0f32;
        for (src, tgt) in sources.iter().zip(targets.iter()) {
            total += residual_norm_simd(src, tgt);
        }
        total
    }
}

// ============================================================================
// DENSE MATRIX MULTIPLY BENCHMARKS
// ============================================================================

fn bench_dense_matmul(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_matmul");

    // Test matrix sizes: 64x64, 128x128, 256x256
    for size in [64, 128, 256] {
        let matrix = generate_matrix(size, size, 42);
        let x = generate_vec(size, 123);
        let mut y = vec![0.0f32; size];

        group.throughput(Throughput::Elements((size * size) as u64));

        group.bench_with_input(BenchmarkId::new("naive", size), &size, |b, _| {
            b.iter(|| {
                matmul_naive(black_box(&matrix), black_box(&x), &mut y, size, size);
                black_box(y[0])
            })
        });

        group.bench_with_input(BenchmarkId::new("unrolled", size), &size, |b, _| {
            b.iter(|| {
                matmul_unrolled(black_box(&matrix), black_box(&x), &mut y, size, size);
                black_box(y[0])
            })
        });

        #[cfg(feature = "simd")]
        group.bench_with_input(BenchmarkId::new("simd", size), &size, |b, _| {
            b.iter(|| {
                simd_impl::matmul_simd(black_box(&matrix), black_box(&x), &mut y, size, size);
                black_box(y[0])
            })
        });
    }

    group.finish();
}

/// Benchmark non-square matrix multiply (projection)
fn bench_projection_matmul(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_matmul_projection");

    // Common projection sizes in coherence: 64->32, 128->64, 256->128
    for (in_dim, out_dim) in [(64, 32), (128, 64), (256, 128)] {
        let matrix = generate_matrix(out_dim, in_dim, 42);
        let x = generate_vec(in_dim, 123);
        let mut y = vec![0.0f32; out_dim];

        group.throughput(Throughput::Elements((out_dim * in_dim) as u64));

        group.bench_with_input(
            BenchmarkId::new("naive", format!("{}x{}", in_dim, out_dim)),
            &(in_dim, out_dim),
            |b, _| {
                b.iter(|| {
                    matmul_naive(black_box(&matrix), black_box(&x), &mut y, out_dim, in_dim);
                    black_box(y[0])
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("unrolled", format!("{}x{}", in_dim, out_dim)),
            &(in_dim, out_dim),
            |b, _| {
                b.iter(|| {
                    matmul_unrolled(black_box(&matrix), black_box(&x), &mut y, out_dim, in_dim);
                    black_box(y[0])
                })
            },
        );

        #[cfg(feature = "simd")]
        group.bench_with_input(
            BenchmarkId::new("simd", format!("{}x{}", in_dim, out_dim)),
            &(in_dim, out_dim),
            |b, _| {
                b.iter(|| {
                    simd_impl::matmul_simd(
                        black_box(&matrix),
                        black_box(&x),
                        &mut y,
                        out_dim,
                        in_dim,
                    );
                    black_box(y[0])
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// NORM COMPUTATION BENCHMARKS
// ============================================================================

fn bench_norm_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_norm");

    // Test dimensions aligned for SIMD
    for dim in [64, 128, 256, 512, 1024] {
        let v = generate_vec(dim, 42);

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(BenchmarkId::new("naive", dim), &dim, |b, _| {
            b.iter(|| black_box(norm_sq_naive(black_box(&v))))
        });

        group.bench_with_input(BenchmarkId::new("iter", dim), &dim, |b, _| {
            b.iter(|| black_box(norm_sq_iter(black_box(&v))))
        });

        group.bench_with_input(BenchmarkId::new("unrolled_4", dim), &dim, |b, _| {
            b.iter(|| black_box(norm_sq_unrolled(black_box(&v))))
        });

        group.bench_with_input(BenchmarkId::new("unrolled_8", dim), &dim, |b, _| {
            b.iter(|| black_box(norm_sq_unrolled_8(black_box(&v))))
        });

        #[cfg(feature = "simd")]
        group.bench_with_input(BenchmarkId::new("simd_f32x8", dim), &dim, |b, _| {
            b.iter(|| black_box(simd_impl::norm_sq_simd(black_box(&v))))
        });
    }

    group.finish();
}

// ============================================================================
// DOT PRODUCT BENCHMARKS
// ============================================================================

fn bench_dot_product(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_dot");

    for dim in [64, 256, 1024] {
        let a = generate_vec(dim, 42);
        let b = generate_vec(dim, 123);

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(BenchmarkId::new("naive", dim), &dim, |b_iter, _| {
            b_iter.iter(|| black_box(dot_naive(black_box(&a), black_box(&b))))
        });

        group.bench_with_input(BenchmarkId::new("unrolled", dim), &dim, |b_iter, _| {
            b_iter.iter(|| black_box(dot_unrolled(black_box(&a), black_box(&b))))
        });

        #[cfg(feature = "simd")]
        group.bench_with_input(BenchmarkId::new("simd", dim), &dim, |b_iter, _| {
            b_iter.iter(|| black_box(simd_impl::dot_simd(black_box(&a), black_box(&b))))
        });
    }

    group.finish();
}

// ============================================================================
// RESIDUAL NORM BENCHMARKS (CORE COHERENCE OPERATION)
// ============================================================================

fn bench_residual_norm(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_residual_norm");

    for dim in [64, 256, 1024] {
        let a = generate_vec(dim, 42);
        let b = generate_vec(dim, 123);

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(BenchmarkId::new("naive", dim), &dim, |b_iter, _| {
            b_iter.iter(|| black_box(residual_norm_naive(black_box(&a), black_box(&b))))
        });

        group.bench_with_input(BenchmarkId::new("unrolled", dim), &dim, |b_iter, _| {
            b_iter.iter(|| black_box(residual_norm_unrolled(black_box(&a), black_box(&b))))
        });

        #[cfg(feature = "simd")]
        group.bench_with_input(BenchmarkId::new("simd", dim), &dim, |b_iter, _| {
            b_iter.iter(|| black_box(simd_impl::residual_norm_simd(black_box(&a), black_box(&b))))
        });
    }

    group.finish();
}

// ============================================================================
// BATCH RESIDUAL BENCHMARKS
// ============================================================================

fn bench_batch_residual(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_batch_residual");

    let dim = 64;

    for batch_size in [100, 1000, 10000] {
        let sources: Vec<Vec<f32>> = (0..batch_size)
            .map(|i| generate_vec(dim, i as u64))
            .collect();
        let targets: Vec<Vec<f32>> = (0..batch_size)
            .map(|i| generate_vec(dim, i as u64 + 10000))
            .collect();

        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("naive", batch_size),
            &batch_size,
            |b, _| {
                b.iter(|| {
                    black_box(batch_residual_naive(
                        black_box(&sources),
                        black_box(&targets),
                    ))
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("unrolled", batch_size),
            &batch_size,
            |b, _| {
                b.iter(|| {
                    black_box(batch_residual_unrolled(
                        black_box(&sources),
                        black_box(&targets),
                    ))
                })
            },
        );

        #[cfg(feature = "simd")]
        group.bench_with_input(BenchmarkId::new("simd", batch_size), &batch_size, |b, _| {
            b.iter(|| {
                black_box(simd_impl::batch_residual_simd(
                    black_box(&sources),
                    black_box(&targets),
                ))
            })
        });
    }

    group.finish();
}

// ============================================================================
// MEMORY ALIGNMENT BENCHMARKS
// ============================================================================

fn bench_alignment_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_alignment");

    let dim = 256;

    // Aligned (multiple of 8)
    {
        let v = generate_vec(dim, 42);
        group.bench_function("aligned_256", |b| {
            b.iter(|| black_box(norm_sq_unrolled_8(black_box(&v))))
        });
    }

    // Misaligned (not multiple of 8)
    {
        let v = generate_vec(dim + 3, 42);
        group.bench_function("misaligned_259", |b| {
            b.iter(|| black_box(norm_sq_unrolled_8(black_box(&v))))
        });
    }

    // Small vector (below SIMD threshold)
    {
        let v = generate_vec(7, 42);
        group.bench_function("small_7", |b| {
            b.iter(|| black_box(norm_sq_unrolled_8(black_box(&v))))
        });
    }

    group.finish();
}

// ============================================================================
// THROUGHPUT SCALING BENCHMARKS
// ============================================================================

fn bench_throughput_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_throughput_scaling");

    // Test how throughput scales with vector size
    let sizes = [16, 32, 64, 128, 256, 512, 1024, 2048, 4096];

    for &size in &sizes {
        let a = generate_vec(size, 42);
        let b = generate_vec(size, 123);

        group.throughput(Throughput::Bytes((size * 4 * 2) as u64)); // 2 vectors, 4 bytes each

        group.bench_with_input(
            BenchmarkId::new("residual_unrolled", size),
            &size,
            |bench, _| {
                bench.iter(|| black_box(residual_norm_unrolled(black_box(&a), black_box(&b))))
            },
        );

        #[cfg(feature = "simd")]
        group.bench_with_input(
            BenchmarkId::new("residual_simd", size),
            &size,
            |bench, _| {
                bench
                    .iter(|| black_box(simd_impl::residual_norm_simd(black_box(&a), black_box(&b))))
            },
        );
    }

    group.finish();
}

// ============================================================================
// COHERENCE-SPECIFIC SIMD PATTERNS
// ============================================================================

/// Fused multiply-add pattern for coherence energy
fn bench_fma_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_fma_pattern");

    let dim = 256;
    let a = generate_vec(dim, 42);
    let b = generate_vec(dim, 123);
    let weight = 1.5f32;

    // Without FMA (separate multiply and add)
    group.bench_function("separate_ops", |bench| {
        bench.iter(|| {
            let mut sum = 0.0f32;
            for i in 0..dim {
                let diff = a[i] - b[i];
                let sq = diff * diff;
                sum += sq;
            }
            black_box(weight * sum)
        })
    });

    // With potential FMA (compiler may optimize)
    group.bench_function("fma_friendly", |bench| {
        bench.iter(|| {
            let mut acc0 = 0.0f32;
            let mut acc1 = 0.0f32;
            let mut acc2 = 0.0f32;
            let mut acc3 = 0.0f32;

            let chunks = dim / 4;
            for c in 0..chunks {
                let base = c * 4;
                let d0 = a[base] - b[base];
                let d1 = a[base + 1] - b[base + 1];
                let d2 = a[base + 2] - b[base + 2];
                let d3 = a[base + 3] - b[base + 3];

                // These can become FMA operations
                acc0 = d0.mul_add(d0, acc0);
                acc1 = d1.mul_add(d1, acc1);
                acc2 = d2.mul_add(d2, acc2);
                acc3 = d3.mul_add(d3, acc3);
            }

            black_box(weight * (acc0 + acc1 + acc2 + acc3))
        })
    });

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(matmul_benches, bench_dense_matmul, bench_projection_matmul,);

criterion_group!(
    vector_ops_benches,
    bench_norm_computation,
    bench_dot_product,
    bench_residual_norm,
);

criterion_group!(batch_benches, bench_batch_residual,);

criterion_group!(
    optimization_benches,
    bench_alignment_impact,
    bench_throughput_scaling,
    bench_fma_pattern,
);

criterion_main!(
    matmul_benches,
    vector_ops_benches,
    batch_benches,
    optimization_benches
);
