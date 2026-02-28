#![allow(
    clippy::all,
    unused_imports,
    unused_variables,
    dead_code,
    unused_mut,
    unused_assignments,
    non_camel_case_types,
    clippy::approx_constant,
    unexpected_cfgs,
    unused_must_use,
    unused_parens
)]
//! Matrix Multiplication Benchmarks for M4 Pro
//!
//! Benchmarks for GEMV, GEMM, and batched GEMM implementations.
//!
//! ## Running Benchmarks
//!
//! Single-threaded baseline:
//! ```bash
//! cargo bench -p ruvllm --features candle --bench matmul_bench -- gemm/512
//! ```
//!
//! Parallel (with rayon):
//! ```bash
//! cargo bench -p ruvllm --features candle,parallel --bench matmul_bench -- gemm/512
//! ```
//!
//! ## Performance Targets for M4 Pro
//!
//! | Operation | Size | Single-thread | Parallel (10 cores) |
//! |-----------|------|---------------|---------------------|
//! | GEMV | 4096x4096 | <500us | <150us |
//! | GEMM | 1024x1024 | <2ms | <500us |
//! | GEMM | 2048x2048 | <15ms | <3ms |
//! | Batched | 32x128x128 | <2ms | <500us |
//!
//! Target speedup: 4-6x on 10-core M4 Pro for large matrices.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;

const NEON_LANE_WIDTH: usize = 4;
const UNROLL_FACTOR: usize = 4;

const TILE_M: usize = 64;
const TILE_N: usize = 64;
const TILE_K: usize = 64;
const MR: usize = 4;

/// General Matrix-Vector multiplication with NEON
#[inline(always)]
fn gemv_neon(a: &[f32], x: &[f32], y: &mut [f32], m: usize, n: usize) {
    debug_assert_eq!(a.len(), m * n);
    debug_assert_eq!(x.len(), n);
    debug_assert_eq!(y.len(), m);

    #[cfg(target_arch = "aarch64")]
    unsafe {
        gemv_neon_impl(a, x, y, m, n);
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        gemv_scalar(a, x, y, m, n);
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn gemv_neon_impl(a: &[f32], x: &[f32], y: &mut [f32], m: usize, n: usize) {
    use std::arch::aarch64::*;

    let a_ptr = a.as_ptr();
    let x_ptr = x.as_ptr();
    let y_ptr = y.as_mut_ptr();

    let row_chunks = m / MR;

    for rc in 0..row_chunks {
        let row_base = rc * MR;

        let mut sum0 = vdupq_n_f32(0.0);
        let mut sum1 = vdupq_n_f32(0.0);
        let mut sum2 = vdupq_n_f32(0.0);
        let mut sum3 = vdupq_n_f32(0.0);

        let col_chunks = n / NEON_LANE_WIDTH;
        let mut col = 0usize;

        for _ in 0..col_chunks {
            let x_v = vld1q_f32(x_ptr.add(col));

            let a0 = vld1q_f32(a_ptr.add((row_base + 0) * n + col));
            sum0 = vfmaq_f32(sum0, a0, x_v);

            let a1 = vld1q_f32(a_ptr.add((row_base + 1) * n + col));
            sum1 = vfmaq_f32(sum1, a1, x_v);

            let a2 = vld1q_f32(a_ptr.add((row_base + 2) * n + col));
            sum2 = vfmaq_f32(sum2, a2, x_v);

            let a3 = vld1q_f32(a_ptr.add((row_base + 3) * n + col));
            sum3 = vfmaq_f32(sum3, a3, x_v);

            col += 4;
        }

        let mut y0 = vaddvq_f32(sum0);
        let mut y1 = vaddvq_f32(sum1);
        let mut y2 = vaddvq_f32(sum2);
        let mut y3 = vaddvq_f32(sum3);

        for c in col..n {
            let x_val = *x_ptr.add(c);
            y0 += *a_ptr.add((row_base + 0) * n + c) * x_val;
            y1 += *a_ptr.add((row_base + 1) * n + c) * x_val;
            y2 += *a_ptr.add((row_base + 2) * n + c) * x_val;
            y3 += *a_ptr.add((row_base + 3) * n + c) * x_val;
        }

        *y_ptr.add(row_base + 0) = y0;
        *y_ptr.add(row_base + 1) = y1;
        *y_ptr.add(row_base + 2) = y2;
        *y_ptr.add(row_base + 3) = y3;
    }

    for row in (row_chunks * MR)..m {
        let mut sum = vdupq_n_f32(0.0);
        let col_chunks = n / NEON_LANE_WIDTH;
        let mut col = 0usize;

        for _ in 0..col_chunks {
            let x_v = vld1q_f32(x_ptr.add(col));
            let a_v = vld1q_f32(a_ptr.add(row * n + col));
            sum = vfmaq_f32(sum, a_v, x_v);
            col += 4;
        }

        let mut y_val = vaddvq_f32(sum);
        for c in col..n {
            y_val += *a_ptr.add(row * n + c) * *x_ptr.add(c);
        }
        *y_ptr.add(row) = y_val;
    }
}

#[allow(dead_code)]
fn gemv_scalar(a: &[f32], x: &[f32], y: &mut [f32], m: usize, n: usize) {
    for row in 0..m {
        let mut sum = 0.0f32;
        for col in 0..n {
            sum += a[row * n + col] * x[col];
        }
        y[row] = sum;
    }
}

/// General Matrix-Matrix multiplication with NEON
#[inline(always)]
fn gemm_neon(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    debug_assert_eq!(a.len(), m * k);
    debug_assert_eq!(b.len(), k * n);
    debug_assert_eq!(c.len(), m * n);

    c.fill(0.0);

    #[cfg(target_arch = "aarch64")]
    unsafe {
        gemm_neon_impl(a, b, c, m, k, n);
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        gemm_scalar(a, b, c, m, k, n);
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn gemm_neon_impl(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    use std::arch::aarch64::*;

    let a_ptr = a.as_ptr();
    let b_ptr = b.as_ptr();
    let c_ptr = c.as_mut_ptr();

    let mut i = 0usize;
    while i < m {
        let i_end = (i + TILE_M).min(m);

        let mut j = 0usize;
        while j < n {
            let j_end = (j + TILE_N).min(n);

            let mut kk = 0usize;
            while kk < k {
                let kk_end = (kk + TILE_K).min(k);

                for ii in i..i_end {
                    for jj in (j..j_end).step_by(NEON_LANE_WIDTH) {
                        let j_remaining = (j_end - jj).min(NEON_LANE_WIDTH);

                        if j_remaining == NEON_LANE_WIDTH {
                            let mut acc = vld1q_f32(c_ptr.add(ii * n + jj));

                            for kkk in kk..kk_end {
                                let a_val = vdupq_n_f32(*a_ptr.add(ii * k + kkk));
                                let b_v = vld1q_f32(b_ptr.add(kkk * n + jj));
                                acc = vfmaq_f32(acc, a_val, b_v);
                            }

                            vst1q_f32(c_ptr.add(ii * n + jj), acc);
                        } else {
                            for jjj in jj..j_end {
                                let mut sum = *c_ptr.add(ii * n + jjj);
                                for kkk in kk..kk_end {
                                    sum += *a_ptr.add(ii * k + kkk) * *b_ptr.add(kkk * n + jjj);
                                }
                                *c_ptr.add(ii * n + jjj) = sum;
                            }
                        }
                    }
                }

                kk = kk_end;
            }

            j = j_end;
        }

        i = i_end;
    }
}

#[allow(dead_code)]
fn gemm_scalar(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    for i in 0..m {
        for j in 0..n {
            let mut sum = 0.0f32;
            for kk in 0..k {
                sum += a[i * k + kk] * b[kk * n + j];
            }
            c[i * n + j] = sum;
        }
    }
}

/// Batched GEMM for attention computation
#[inline(always)]
fn batched_gemm_neon(
    a: &[f32],
    b: &[f32],
    c: &mut [f32],
    batch_size: usize,
    m: usize,
    k: usize,
    n: usize,
) {
    debug_assert_eq!(a.len(), batch_size * m * k);
    debug_assert_eq!(b.len(), batch_size * k * n);
    debug_assert_eq!(c.len(), batch_size * m * n);

    let a_batch_stride = m * k;
    let b_batch_stride = k * n;
    let c_batch_stride = m * n;

    for batch in 0..batch_size {
        let a_offset = batch * a_batch_stride;
        let b_offset = batch * b_batch_stride;
        let c_offset = batch * c_batch_stride;

        gemm_neon(
            &a[a_offset..a_offset + a_batch_stride],
            &b[b_offset..b_offset + b_batch_stride],
            &mut c[c_offset..c_offset + c_batch_stride],
            m,
            k,
            n,
        );
    }
}

/// GEMM with transposed B matrix (for Q * K^T)
fn gemm_nt_neon(a: &[f32], b_t: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    debug_assert_eq!(a.len(), m * k);
    debug_assert_eq!(b_t.len(), n * k);
    debug_assert_eq!(c.len(), m * n);

    c.fill(0.0);

    #[cfg(target_arch = "aarch64")]
    unsafe {
        gemm_nt_neon_impl(a, b_t, c, m, k, n);
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        gemm_nt_scalar(a, b_t, c, m, k, n);
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn gemm_nt_neon_impl(a: &[f32], b_t: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    use std::arch::aarch64::*;

    let a_ptr = a.as_ptr();
    let b_ptr = b_t.as_ptr();
    let c_ptr = c.as_mut_ptr();

    for i in 0..m {
        let n_chunks = n / NEON_LANE_WIDTH;

        for nc in 0..n_chunks {
            let j_base = nc * NEON_LANE_WIDTH;

            let mut acc0 = 0.0f32;
            let mut acc1 = 0.0f32;
            let mut acc2 = 0.0f32;
            let mut acc3 = 0.0f32;

            let k_chunks = k / NEON_LANE_WIDTH;
            let mut kk = 0usize;

            for _ in 0..k_chunks {
                let a_v = vld1q_f32(a_ptr.add(i * k + kk));

                let b0 = vld1q_f32(b_ptr.add((j_base + 0) * k + kk));
                let b1 = vld1q_f32(b_ptr.add((j_base + 1) * k + kk));
                let b2 = vld1q_f32(b_ptr.add((j_base + 2) * k + kk));
                let b3 = vld1q_f32(b_ptr.add((j_base + 3) * k + kk));

                acc0 += vaddvq_f32(vmulq_f32(a_v, b0));
                acc1 += vaddvq_f32(vmulq_f32(a_v, b1));
                acc2 += vaddvq_f32(vmulq_f32(a_v, b2));
                acc3 += vaddvq_f32(vmulq_f32(a_v, b3));

                kk += 4;
            }

            for kkk in kk..k {
                let a_val = *a_ptr.add(i * k + kkk);
                acc0 += a_val * *b_ptr.add((j_base + 0) * k + kkk);
                acc1 += a_val * *b_ptr.add((j_base + 1) * k + kkk);
                acc2 += a_val * *b_ptr.add((j_base + 2) * k + kkk);
                acc3 += a_val * *b_ptr.add((j_base + 3) * k + kkk);
            }

            *c_ptr.add(i * n + j_base + 0) = acc0;
            *c_ptr.add(i * n + j_base + 1) = acc1;
            *c_ptr.add(i * n + j_base + 2) = acc2;
            *c_ptr.add(i * n + j_base + 3) = acc3;
        }

        for j in (n_chunks * NEON_LANE_WIDTH)..n {
            let mut acc = vdupq_n_f32(0.0);
            let k_chunks = k / NEON_LANE_WIDTH;
            let mut kk = 0usize;

            for _ in 0..k_chunks {
                let a_v = vld1q_f32(a_ptr.add(i * k + kk));
                let b_v = vld1q_f32(b_ptr.add(j * k + kk));
                acc = vfmaq_f32(acc, a_v, b_v);
                kk += 4;
            }

            let mut sum = vaddvq_f32(acc);
            for kkk in kk..k {
                sum += *a_ptr.add(i * k + kkk) * *b_ptr.add(j * k + kkk);
            }
            *c_ptr.add(i * n + j) = sum;
        }
    }
}

#[allow(dead_code)]
fn gemm_nt_scalar(a: &[f32], b_t: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    for i in 0..m {
        for j in 0..n {
            let mut sum = 0.0f32;
            for kk in 0..k {
                sum += a[i * k + kk] * b_t[j * k + kk];
            }
            c[i * n + j] = sum;
        }
    }
}

/// Dot product of two vectors
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn dot_product_neon(a: &[f32], b: &[f32]) -> f32 {
    use std::arch::aarch64::*;

    debug_assert_eq!(a.len(), b.len());

    let len = a.len();
    let a_ptr = a.as_ptr();
    let b_ptr = b.as_ptr();

    let mut sum0 = vdupq_n_f32(0.0);
    let mut sum1 = vdupq_n_f32(0.0);
    let mut sum2 = vdupq_n_f32(0.0);
    let mut sum3 = vdupq_n_f32(0.0);

    let chunks = len / (NEON_LANE_WIDTH * UNROLL_FACTOR);
    let mut idx = 0usize;

    for _ in 0..chunks {
        let a0 = vld1q_f32(a_ptr.add(idx));
        let b0 = vld1q_f32(b_ptr.add(idx));
        sum0 = vfmaq_f32(sum0, a0, b0);

        let a1 = vld1q_f32(a_ptr.add(idx + 4));
        let b1 = vld1q_f32(b_ptr.add(idx + 4));
        sum1 = vfmaq_f32(sum1, a1, b1);

        let a2 = vld1q_f32(a_ptr.add(idx + 8));
        let b2 = vld1q_f32(b_ptr.add(idx + 8));
        sum2 = vfmaq_f32(sum2, a2, b2);

        let a3 = vld1q_f32(a_ptr.add(idx + 12));
        let b3 = vld1q_f32(b_ptr.add(idx + 12));
        sum3 = vfmaq_f32(sum3, a3, b3);

        idx += 16;
    }

    let sum01 = vaddq_f32(sum0, sum1);
    let sum23 = vaddq_f32(sum2, sum3);
    let sum = vaddq_f32(sum01, sum23);

    let remaining = (len - idx) / NEON_LANE_WIDTH;
    let mut final_sum = sum;
    for _ in 0..remaining {
        let a_v = vld1q_f32(a_ptr.add(idx));
        let b_v = vld1q_f32(b_ptr.add(idx));
        final_sum = vfmaq_f32(final_sum, a_v, b_v);
        idx += 4;
    }

    let mut result = vaddvq_f32(final_sum);

    for i in idx..len {
        result += *a_ptr.add(i) * *b_ptr.add(i);
    }

    result
}

// Helper function to generate random tensor data
fn random_tensor(size: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

// === Benchmark Functions ===

fn bench_gemv(c: &mut Criterion) {
    let mut group = c.benchmark_group("gemv");
    group.sample_size(50);

    for (m, n) in [
        (256, 256),
        (512, 512),
        (1024, 1024),
        (2048, 2048),
        (4096, 4096),
    ] {
        let a = random_tensor(m * n);
        let x = random_tensor(n);
        let mut y = vec![0.0; m];

        let flops = 2 * m * n; // multiply + add per element

        let id = BenchmarkId::new(format!("{}x{}", m, n), m * n);

        group.throughput(Throughput::Elements(flops as u64));
        group.bench_function(id, |b| {
            b.iter(|| {
                gemv_neon(black_box(&a), black_box(&x), black_box(&mut y), m, n);
            })
        });
    }

    group.finish();
}

fn bench_gemm(c: &mut Criterion) {
    let mut group = c.benchmark_group("gemm");
    group.sample_size(30);

    for size in [128, 256, 512, 1024, 2048] {
        let m = size;
        let k = size;
        let n = size;

        let mat_a = random_tensor(m * k);
        let mat_b = random_tensor(k * n);
        let mut c_out = vec![0.0; m * n];

        let flops = 2 * m * k * n; // multiply + add per output element

        let id = BenchmarkId::new(format!("{}x{}x{}", m, k, n), m * k * n);

        group.throughput(Throughput::Elements(flops as u64));
        group.bench_function(id, |bencher| {
            bencher.iter(|| {
                gemm_neon(
                    black_box(&mat_a),
                    black_box(&mat_b),
                    black_box(&mut c_out),
                    m,
                    k,
                    n,
                );
            })
        });
    }

    group.finish();
}

fn bench_gemm_non_square(c: &mut Criterion) {
    let mut group = c.benchmark_group("gemm_non_square");
    group.sample_size(30);

    // Common shapes in LLM inference
    let shapes = [
        (1, 4096, 4096),   // Single token projection
        (32, 4096, 4096),  // Batch projection
        (128, 4096, 4096), // Larger batch
        (1, 4096, 11008),  // MLP up projection (Llama2 7B)
        (1, 11008, 4096),  // MLP down projection
        (32, 128, 4096),   // Attention output
    ];

    for (m, k, n) in shapes {
        let mat_a = random_tensor(m * k);
        let mat_b = random_tensor(k * n);
        let mut c_out = vec![0.0; m * n];

        let flops = 2 * m * k * n;

        let id = BenchmarkId::new(format!("{}x{}x{}", m, k, n), m);

        group.throughput(Throughput::Elements(flops as u64));
        group.bench_function(id, |bencher| {
            bencher.iter(|| {
                gemm_neon(
                    black_box(&mat_a),
                    black_box(&mat_b),
                    black_box(&mut c_out),
                    m,
                    k,
                    n,
                );
            })
        });
    }

    group.finish();
}

fn bench_batched_gemm(c: &mut Criterion) {
    let mut group = c.benchmark_group("batched_gemm");
    group.sample_size(30);

    for batch_size in [1, 8, 16, 32] {
        for (m, k, n) in [(64, 64, 64), (128, 128, 128), (256, 256, 256)] {
            let mat_a = random_tensor(batch_size * m * k);
            let mat_b = random_tensor(batch_size * k * n);
            let mut c_out = vec![0.0; batch_size * m * n];

            let flops = 2 * batch_size * m * k * n;

            let id = BenchmarkId::new(
                format!("batch_{}_{}x{}x{}", batch_size, m, k, n),
                batch_size,
            );

            group.throughput(Throughput::Elements(flops as u64));
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    batched_gemm_neon(
                        black_box(&mat_a),
                        black_box(&mat_b),
                        black_box(&mut c_out),
                        batch_size,
                        m,
                        k,
                        n,
                    );
                })
            });
        }
    }

    group.finish();
}

fn bench_gemm_nt(c: &mut Criterion) {
    let mut group = c.benchmark_group("gemm_nt");
    group.sample_size(30);

    // Q * K^T shapes in attention
    let shapes = [
        (128, 128, 128),   // seq=128
        (256, 128, 256),   // seq=256
        (512, 128, 512),   // seq=512
        (1024, 128, 1024), // seq=1024
    ];

    for (m, k, n) in shapes {
        let a = random_tensor(m * k);
        let b_t = random_tensor(n * k); // Transposed
        let mut c_out = vec![0.0; m * n];

        let flops = 2 * m * k * n;

        let id = BenchmarkId::new(format!("{}x{}x{}", m, k, n), m * n);

        group.throughput(Throughput::Elements(flops as u64));
        group.bench_function(id, |b| {
            b.iter(|| {
                gemm_nt_neon(
                    black_box(&a),
                    black_box(&b_t),
                    black_box(&mut c_out),
                    m,
                    k,
                    n,
                );
            })
        });
    }

    group.finish();
}

#[cfg(target_arch = "aarch64")]
fn bench_dot_product(c: &mut Criterion) {
    let mut group = c.benchmark_group("dot_product");
    group.sample_size(100);

    for size in [64, 128, 256, 512, 1024, 2048, 4096] {
        let a = random_tensor(size);
        let b = random_tensor(size);

        let id = BenchmarkId::new(format!("dim_{}", size), size);

        group.throughput(Throughput::Elements((2 * size) as u64)); // multiply + add
        group.bench_function(id, |b_iter| {
            b_iter.iter(|| unsafe { dot_product_neon(black_box(&a), black_box(&b)) })
        });
    }

    group.finish();
}

fn bench_tiling_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("tiling_efficiency");
    group.sample_size(20);

    // Test how well tiling works at various sizes
    for size in [63, 64, 65, 127, 128, 129, 255, 256, 257] {
        let mat_a = random_tensor(size * size);
        let mat_b = random_tensor(size * size);
        let mut c_out = vec![0.0; size * size];

        let flops = 2 * size * size * size;

        let id = BenchmarkId::new(format!("size_{}", size), size);

        group.throughput(Throughput::Elements(flops as u64));
        group.bench_function(id, |bencher| {
            bencher.iter(|| {
                gemm_neon(
                    black_box(&mat_a),
                    black_box(&mat_b),
                    black_box(&mut c_out),
                    size,
                    size,
                    size,
                );
            })
        });
    }

    group.finish();
}

fn bench_memory_bandwidth(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_bandwidth");
    group.sample_size(30);

    // Test memory-bound vs compute-bound behavior
    for (m, k, n) in [
        (1, 4096, 4096),   // Very memory bound (GEMV-like)
        (32, 4096, 4096),  // More compute
        (128, 4096, 4096), // Compute bound
    ] {
        let mat_a = random_tensor(m * k);
        let mat_b = random_tensor(k * n);
        let mut c_out = vec![0.0; m * n];

        // Memory: A (m*k*4), B (k*n*4), C (m*n*4)
        let memory_bytes = ((m * k) + (k * n) + (m * n)) * 4;
        let flops = 2 * m * k * n;

        let id = BenchmarkId::new(
            format!(
                "{}x{}x{}_ratio_{:.2}",
                m,
                k,
                n,
                flops as f64 / memory_bytes as f64
            ),
            m,
        );

        group.throughput(Throughput::Bytes(memory_bytes as u64));
        group.bench_function(id, |bencher| {
            bencher.iter(|| {
                gemm_neon(
                    black_box(&mat_a),
                    black_box(&mat_b),
                    black_box(&mut c_out),
                    m,
                    k,
                    n,
                );
            })
        });
    }

    group.finish();
}

fn bench_llm_projection_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("llm_projections");
    group.sample_size(20);

    // Real LLM projection sizes (single token)
    let configs = [
        ("llama2_7b_qkv", 1, 4096, 4096),
        ("llama2_7b_mlp_up", 1, 4096, 11008),
        ("llama2_7b_mlp_down", 1, 11008, 4096),
        ("llama2_13b_qkv", 1, 5120, 5120),
        ("llama2_70b_qkv", 1, 8192, 8192),
        ("mistral_7b_qkv", 1, 4096, 4096),
    ];

    for (name, m, k, n) in configs {
        let mat_a = random_tensor(m * k);
        let mat_b = random_tensor(k * n);
        let mut c_out = vec![0.0; m * n];

        let flops = 2 * m * k * n;

        let id = BenchmarkId::new(name, flops);

        group.throughput(Throughput::Elements(flops as u64));
        group.bench_function(id, |bencher| {
            bencher.iter(|| {
                gemm_neon(
                    black_box(&mat_a),
                    black_box(&mat_b),
                    black_box(&mut c_out),
                    m,
                    k,
                    n,
                );
            })
        });
    }

    group.finish();
}

// ============================================================================
// Parallel benchmarks (enabled with `parallel` feature)
// ============================================================================

#[cfg(feature = "parallel")]
mod parallel_benches {
    use super::*;

    /// Get physical core count
    fn get_physical_cores() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    }

    /// Configure thread pool once at start
    fn init_thread_pool() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(get_physical_cores())
                .thread_name(|i| format!("bench-gemm-{}", i))
                .build_global()
                .ok();
        });
    }

    // ========================================================================
    // Parallel GEMM implementations (mirrors single-threaded versions)
    // ========================================================================

    fn gemm_parallel(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
        use rayon::prelude::*;

        const MIN_ROWS_PER_THREAD: usize = 32;
        const PARALLEL_THRESHOLD: usize = 128;

        if m < PARALLEL_THRESHOLD || (m * k * n) < 1_000_000 {
            return gemm_neon(a, b, c, m, k, n);
        }

        c.fill(0.0);

        let num_threads = get_physical_cores();
        let chunk_size = (m / num_threads).max(MIN_ROWS_PER_THREAD);

        c.par_chunks_mut(chunk_size * n)
            .enumerate()
            .for_each(|(chunk_idx, c_chunk)| {
                let row_start = chunk_idx * chunk_size;
                let actual_rows = c_chunk.len() / n;
                let row_end = row_start + actual_rows;

                let a_start = row_start * k;
                let a_end = row_end * k;
                let a_chunk = &a[a_start..a_end];

                gemm_chunk(a_chunk, b, c_chunk, actual_rows, k, n);
            });
    }

    fn gemm_chunk(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
        #[cfg(target_arch = "aarch64")]
        unsafe {
            gemm_chunk_neon(a, b, c, m, k, n);
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            for i in 0..m {
                for j in 0..n {
                    let mut sum = 0.0f32;
                    for kk in 0..k {
                        sum += a[i * k + kk] * b[kk * n + j];
                    }
                    c[i * n + j] = sum;
                }
            }
        }
    }

    #[cfg(target_arch = "aarch64")]
    unsafe fn gemm_chunk_neon(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
        use std::arch::aarch64::*;

        let a_ptr = a.as_ptr();
        let b_ptr = b.as_ptr();
        let c_ptr = c.as_mut_ptr();

        let mut i = 0usize;
        while i + 4 <= m {
            let mut j = 0usize;
            while j + 8 <= n {
                let mut c00 = vdupq_n_f32(0.0);
                let mut c01 = vdupq_n_f32(0.0);
                let mut c10 = vdupq_n_f32(0.0);
                let mut c11 = vdupq_n_f32(0.0);
                let mut c20 = vdupq_n_f32(0.0);
                let mut c21 = vdupq_n_f32(0.0);
                let mut c30 = vdupq_n_f32(0.0);
                let mut c31 = vdupq_n_f32(0.0);

                for kk in 0..k {
                    let b0 = vld1q_f32(b_ptr.add(kk * n + j));
                    let b1 = vld1q_f32(b_ptr.add(kk * n + j + 4));

                    let a0 = vdupq_n_f32(*a_ptr.add(i * k + kk));
                    let a1 = vdupq_n_f32(*a_ptr.add((i + 1) * k + kk));
                    let a2 = vdupq_n_f32(*a_ptr.add((i + 2) * k + kk));
                    let a3 = vdupq_n_f32(*a_ptr.add((i + 3) * k + kk));

                    c00 = vfmaq_f32(c00, a0, b0);
                    c01 = vfmaq_f32(c01, a0, b1);
                    c10 = vfmaq_f32(c10, a1, b0);
                    c11 = vfmaq_f32(c11, a1, b1);
                    c20 = vfmaq_f32(c20, a2, b0);
                    c21 = vfmaq_f32(c21, a2, b1);
                    c30 = vfmaq_f32(c30, a3, b0);
                    c31 = vfmaq_f32(c31, a3, b1);
                }

                vst1q_f32(c_ptr.add(i * n + j), c00);
                vst1q_f32(c_ptr.add(i * n + j + 4), c01);
                vst1q_f32(c_ptr.add((i + 1) * n + j), c10);
                vst1q_f32(c_ptr.add((i + 1) * n + j + 4), c11);
                vst1q_f32(c_ptr.add((i + 2) * n + j), c20);
                vst1q_f32(c_ptr.add((i + 2) * n + j + 4), c21);
                vst1q_f32(c_ptr.add((i + 3) * n + j), c30);
                vst1q_f32(c_ptr.add((i + 3) * n + j + 4), c31);

                j += 8;
            }

            while j + 4 <= n {
                let mut c0 = vdupq_n_f32(0.0);
                let mut c1 = vdupq_n_f32(0.0);
                let mut c2 = vdupq_n_f32(0.0);
                let mut c3 = vdupq_n_f32(0.0);

                for kk in 0..k {
                    let b_v = vld1q_f32(b_ptr.add(kk * n + j));
                    c0 = vfmaq_f32(c0, vdupq_n_f32(*a_ptr.add(i * k + kk)), b_v);
                    c1 = vfmaq_f32(c1, vdupq_n_f32(*a_ptr.add((i + 1) * k + kk)), b_v);
                    c2 = vfmaq_f32(c2, vdupq_n_f32(*a_ptr.add((i + 2) * k + kk)), b_v);
                    c3 = vfmaq_f32(c3, vdupq_n_f32(*a_ptr.add((i + 3) * k + kk)), b_v);
                }

                vst1q_f32(c_ptr.add(i * n + j), c0);
                vst1q_f32(c_ptr.add((i + 1) * n + j), c1);
                vst1q_f32(c_ptr.add((i + 2) * n + j), c2);
                vst1q_f32(c_ptr.add((i + 3) * n + j), c3);

                j += 4;
            }

            while j < n {
                for row in i..i + 4 {
                    let mut sum = 0.0f32;
                    for kk in 0..k {
                        sum += *a_ptr.add(row * k + kk) * *b_ptr.add(kk * n + j);
                    }
                    *c_ptr.add(row * n + j) = sum;
                }
                j += 1;
            }

            i += 4;
        }

        while i < m {
            let mut j = 0usize;
            while j + 4 <= n {
                let mut acc = vdupq_n_f32(0.0);
                for kk in 0..k {
                    let a_val = vdupq_n_f32(*a_ptr.add(i * k + kk));
                    let b_v = vld1q_f32(b_ptr.add(kk * n + j));
                    acc = vfmaq_f32(acc, a_val, b_v);
                }
                vst1q_f32(c_ptr.add(i * n + j), acc);
                j += 4;
            }

            while j < n {
                let mut sum = 0.0f32;
                for kk in 0..k {
                    sum += *a_ptr.add(i * k + kk) * *b_ptr.add(kk * n + j);
                }
                *c_ptr.add(i * n + j) = sum;
                j += 1;
            }

            i += 1;
        }
    }

    fn gemv_parallel(a: &[f32], x: &[f32], y: &mut [f32], m: usize, n: usize) {
        use rayon::prelude::*;

        const MIN_ROWS_PER_THREAD: usize = 32;
        const PARALLEL_THRESHOLD: usize = 256;

        if m < PARALLEL_THRESHOLD {
            return gemv_neon(a, x, y, m, n);
        }

        let num_threads = get_physical_cores();
        let chunk_size = (m / num_threads).max(MIN_ROWS_PER_THREAD);

        y.par_chunks_mut(chunk_size)
            .enumerate()
            .for_each(|(chunk_idx, y_chunk)| {
                let row_start = chunk_idx * chunk_size;
                let row_end = (row_start + y_chunk.len()).min(m);
                let chunk_rows = row_end - row_start;

                let a_start = row_start * n;
                let a_end = row_end * n;
                let a_chunk = &a[a_start..a_end];

                gemv_neon(a_chunk, x, y_chunk, chunk_rows, n);
            });
    }

    fn batched_gemm_parallel(
        a: &[f32],
        b: &[f32],
        c: &mut [f32],
        batch_size: usize,
        m: usize,
        k: usize,
        n: usize,
    ) {
        use rayon::prelude::*;

        const PARALLEL_THRESHOLD: usize = 128;

        let a_batch_stride = m * k;
        let b_batch_stride = k * n;
        let c_batch_stride = m * n;

        if batch_size <= 4 && m >= PARALLEL_THRESHOLD {
            for batch in 0..batch_size {
                let a_offset = batch * a_batch_stride;
                let b_offset = batch * b_batch_stride;
                let c_offset = batch * c_batch_stride;

                gemm_parallel(
                    &a[a_offset..a_offset + a_batch_stride],
                    &b[b_offset..b_offset + b_batch_stride],
                    &mut c[c_offset..c_offset + c_batch_stride],
                    m,
                    k,
                    n,
                );
            }
        } else {
            c.par_chunks_mut(c_batch_stride)
                .enumerate()
                .for_each(|(batch, c_batch)| {
                    let a_offset = batch * a_batch_stride;
                    let b_offset = batch * b_batch_stride;

                    gemm_neon(
                        &a[a_offset..a_offset + a_batch_stride],
                        &b[b_offset..b_offset + b_batch_stride],
                        c_batch,
                        m,
                        k,
                        n,
                    );
                });
        }
    }

    // ========================================================================
    // Benchmark functions
    // ========================================================================

    pub fn bench_gemm_parallel(c: &mut Criterion) {
        init_thread_pool();

        let mut group = c.benchmark_group("gemm_parallel");
        group.sample_size(30);

        for size in [256, 512, 1024, 2048] {
            let m = size;
            let k = size;
            let n = size;

            let mat_a = random_tensor(m * k);
            let mat_b = random_tensor(k * n);
            let mut c_out = vec![0.0; m * n];

            let flops = 2 * m * k * n;

            let id = BenchmarkId::new(format!("{}x{}x{}", m, k, n), m * k * n);

            group.throughput(Throughput::Elements(flops as u64));
            group.bench_function(id, |bencher| {
                bencher.iter(|| {
                    gemm_parallel(
                        black_box(&mat_a),
                        black_box(&mat_b),
                        black_box(&mut c_out),
                        m,
                        k,
                        n,
                    );
                })
            });
        }

        group.finish();
    }

    pub fn bench_gemv_parallel(c: &mut Criterion) {
        init_thread_pool();

        let mut group = c.benchmark_group("gemv_parallel");
        group.sample_size(50);

        for (m, n) in [(512, 512), (1024, 1024), (2048, 2048), (4096, 4096)] {
            let a = random_tensor(m * n);
            let x = random_tensor(n);
            let mut y = vec![0.0; m];

            let flops = 2 * m * n;

            let id = BenchmarkId::new(format!("{}x{}", m, n), m * n);

            group.throughput(Throughput::Elements(flops as u64));
            group.bench_function(id, |b| {
                b.iter(|| {
                    gemv_parallel(black_box(&a), black_box(&x), black_box(&mut y), m, n);
                })
            });
        }

        group.finish();
    }

    pub fn bench_batched_gemm_parallel(c: &mut Criterion) {
        init_thread_pool();

        let mut group = c.benchmark_group("batched_gemm_parallel");
        group.sample_size(30);

        for batch_size in [8, 16, 32] {
            for (m, k, n) in [(128, 128, 128), (256, 256, 256)] {
                let mat_a = random_tensor(batch_size * m * k);
                let mat_b = random_tensor(batch_size * k * n);
                let mut c_out = vec![0.0; batch_size * m * n];

                let flops = 2 * batch_size * m * k * n;

                let id = BenchmarkId::new(
                    format!("batch_{}_{}x{}x{}", batch_size, m, k, n),
                    batch_size,
                );

                group.throughput(Throughput::Elements(flops as u64));
                group.bench_function(id, |bencher| {
                    bencher.iter(|| {
                        batched_gemm_parallel(
                            black_box(&mat_a),
                            black_box(&mat_b),
                            black_box(&mut c_out),
                            batch_size,
                            m,
                            k,
                            n,
                        );
                    })
                });
            }
        }

        group.finish();
    }

    /// Compare single-threaded vs parallel for large matrices
    pub fn bench_parallel_speedup(c: &mut Criterion) {
        init_thread_pool();

        let mut group = c.benchmark_group("parallel_speedup");
        group.sample_size(20);

        let size = 512;
        let m = size;
        let k = size;
        let n = size;

        let mat_a = random_tensor(m * k);
        let mat_b = random_tensor(k * n);
        let mut c_out = vec![0.0; m * n];

        let flops = 2 * m * k * n;

        group.throughput(Throughput::Elements(flops as u64));

        group.bench_function("single_thread", |bencher| {
            bencher.iter(|| {
                gemm_neon(
                    black_box(&mat_a),
                    black_box(&mat_b),
                    black_box(&mut c_out),
                    m,
                    k,
                    n,
                );
            })
        });

        group.bench_function("parallel", |bencher| {
            bencher.iter(|| {
                gemm_parallel(
                    black_box(&mat_a),
                    black_box(&mat_b),
                    black_box(&mut c_out),
                    m,
                    k,
                    n,
                );
            })
        });

        group.finish();
    }
}

#[cfg(feature = "parallel")]
use parallel_benches::*;

#[cfg(all(target_arch = "aarch64", not(feature = "parallel")))]
criterion_group!(
    benches,
    bench_gemv,
    bench_gemm,
    bench_gemm_non_square,
    bench_batched_gemm,
    bench_gemm_nt,
    bench_dot_product,
    bench_tiling_efficiency,
    bench_memory_bandwidth,
    bench_llm_projection_sizes,
);

#[cfg(all(target_arch = "aarch64", feature = "parallel"))]
criterion_group!(
    benches,
    bench_gemv,
    bench_gemm,
    bench_gemm_non_square,
    bench_batched_gemm,
    bench_gemm_nt,
    bench_dot_product,
    bench_tiling_efficiency,
    bench_memory_bandwidth,
    bench_llm_projection_sizes,
    bench_gemm_parallel,
    bench_gemv_parallel,
    bench_batched_gemm_parallel,
    bench_parallel_speedup,
);

#[cfg(all(not(target_arch = "aarch64"), not(feature = "parallel")))]
criterion_group!(
    benches,
    bench_gemv,
    bench_gemm,
    bench_gemm_non_square,
    bench_batched_gemm,
    bench_gemm_nt,
    bench_tiling_efficiency,
    bench_memory_bandwidth,
    bench_llm_projection_sizes,
);

#[cfg(all(not(target_arch = "aarch64"), feature = "parallel"))]
criterion_group!(
    benches,
    bench_gemv,
    bench_gemm,
    bench_gemm_non_square,
    bench_batched_gemm,
    bench_gemm_nt,
    bench_tiling_efficiency,
    bench_memory_bandwidth,
    bench_llm_projection_sizes,
    bench_gemm_parallel,
    bench_gemv_parallel,
    bench_batched_gemm_parallel,
    bench_parallel_speedup,
);

criterion_main!(benches);
