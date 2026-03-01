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
//! Normalization Kernel Benchmarks for M4 Pro
//!
//! Benchmarks for RMSNorm and LayerNorm implementations.
//!
//! Performance targets for M4 Pro:
//! - RMSNorm (768 dim): <5us
//! - RMSNorm (2048 dim): <8us
//! - RMSNorm (4096 dim): <10us
//! - LayerNorm (4096 dim): <15us

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;

const NEON_LANE_WIDTH: usize = 4;
const UNROLL_FACTOR: usize = 4;

/// RMSNorm with NEON optimization
#[inline(always)]
fn rms_norm_neon(x: &mut [f32], weight: &[f32], eps: f32) {
    debug_assert_eq!(x.len(), weight.len());

    let len = x.len();
    if len == 0 {
        return;
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        rms_norm_neon_impl(x, weight, eps);
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        rms_norm_scalar(x, weight, eps);
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn rms_norm_neon_impl(x: &mut [f32], weight: &[f32], eps: f32) {
    use std::arch::aarch64::*;

    let len = x.len();
    let x_ptr = x.as_mut_ptr();
    let w_ptr = weight.as_ptr();

    let mut sum0 = vdupq_n_f32(0.0);
    let mut sum1 = vdupq_n_f32(0.0);
    let mut sum2 = vdupq_n_f32(0.0);
    let mut sum3 = vdupq_n_f32(0.0);

    let chunks = len / (NEON_LANE_WIDTH * UNROLL_FACTOR);
    let mut idx = 0usize;

    for _ in 0..chunks {
        let v0 = vld1q_f32(x_ptr.add(idx));
        sum0 = vfmaq_f32(sum0, v0, v0);

        let v1 = vld1q_f32(x_ptr.add(idx + 4));
        sum1 = vfmaq_f32(sum1, v1, v1);

        let v2 = vld1q_f32(x_ptr.add(idx + 8));
        sum2 = vfmaq_f32(sum2, v2, v2);

        let v3 = vld1q_f32(x_ptr.add(idx + 12));
        sum3 = vfmaq_f32(sum3, v3, v3);

        idx += 16;
    }

    let sum01 = vaddq_f32(sum0, sum1);
    let sum23 = vaddq_f32(sum2, sum3);
    let sum = vaddq_f32(sum01, sum23);

    let remaining_chunks = (len - idx) / NEON_LANE_WIDTH;
    let mut final_sum = sum;
    for _ in 0..remaining_chunks {
        let v = vld1q_f32(x_ptr.add(idx));
        final_sum = vfmaq_f32(final_sum, v, v);
        idx += 4;
    }

    let mut sum_sq = vaddvq_f32(final_sum);

    for i in idx..len {
        let v = *x_ptr.add(i);
        sum_sq += v * v;
    }

    let mean_sq = sum_sq / len as f32;
    let rms = (mean_sq + eps).sqrt();
    let inv_rms = 1.0 / rms;
    let inv_rms_vec = vdupq_n_f32(inv_rms);

    idx = 0;
    for _ in 0..chunks {
        let x0 = vld1q_f32(x_ptr.add(idx));
        let w0 = vld1q_f32(w_ptr.add(idx));
        vst1q_f32(x_ptr.add(idx), vmulq_f32(vmulq_f32(x0, inv_rms_vec), w0));

        let x1 = vld1q_f32(x_ptr.add(idx + 4));
        let w1 = vld1q_f32(w_ptr.add(idx + 4));
        vst1q_f32(
            x_ptr.add(idx + 4),
            vmulq_f32(vmulq_f32(x1, inv_rms_vec), w1),
        );

        let x2 = vld1q_f32(x_ptr.add(idx + 8));
        let w2 = vld1q_f32(w_ptr.add(idx + 8));
        vst1q_f32(
            x_ptr.add(idx + 8),
            vmulq_f32(vmulq_f32(x2, inv_rms_vec), w2),
        );

        let x3 = vld1q_f32(x_ptr.add(idx + 12));
        let w3 = vld1q_f32(w_ptr.add(idx + 12));
        vst1q_f32(
            x_ptr.add(idx + 12),
            vmulq_f32(vmulq_f32(x3, inv_rms_vec), w3),
        );

        idx += 16;
    }

    for _ in 0..remaining_chunks {
        let x_v = vld1q_f32(x_ptr.add(idx));
        let w_v = vld1q_f32(w_ptr.add(idx));
        vst1q_f32(x_ptr.add(idx), vmulq_f32(vmulq_f32(x_v, inv_rms_vec), w_v));
        idx += 4;
    }

    for i in idx..len {
        *x_ptr.add(i) = *x_ptr.add(i) * inv_rms * *w_ptr.add(i);
    }
}

#[allow(dead_code)]
fn rms_norm_scalar(x: &mut [f32], weight: &[f32], eps: f32) {
    let len = x.len();

    let sum_sq: f32 = x.iter().map(|v| v * v).sum();

    let mean_sq = sum_sq / len as f32;
    let inv_rms = 1.0 / (mean_sq + eps).sqrt();

    for (i, w) in weight.iter().enumerate() {
        x[i] = x[i] * inv_rms * w;
    }
}

/// LayerNorm with NEON optimization
#[inline(always)]
fn layer_norm_neon(x: &mut [f32], weight: &[f32], bias: &[f32], eps: f32) {
    debug_assert_eq!(x.len(), weight.len());
    debug_assert_eq!(x.len(), bias.len());

    let len = x.len();
    if len == 0 {
        return;
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        layer_norm_neon_impl(x, weight, bias, eps);
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        layer_norm_scalar(x, weight, bias, eps);
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn layer_norm_neon_impl(x: &mut [f32], weight: &[f32], bias: &[f32], eps: f32) {
    use std::arch::aarch64::*;

    let len = x.len();
    let x_ptr = x.as_mut_ptr();
    let w_ptr = weight.as_ptr();
    let b_ptr = bias.as_ptr();

    let mut sum0 = vdupq_n_f32(0.0);
    let mut sum1 = vdupq_n_f32(0.0);
    let mut sq0 = vdupq_n_f32(0.0);
    let mut sq1 = vdupq_n_f32(0.0);

    let chunks = len / (NEON_LANE_WIDTH * 2);
    let mut idx = 0usize;

    for _ in 0..chunks {
        let v0 = vld1q_f32(x_ptr.add(idx));
        sum0 = vaddq_f32(sum0, v0);
        sq0 = vfmaq_f32(sq0, v0, v0);

        let v1 = vld1q_f32(x_ptr.add(idx + 4));
        sum1 = vaddq_f32(sum1, v1);
        sq1 = vfmaq_f32(sq1, v1, v1);

        idx += 8;
    }

    let sum_vec = vaddq_f32(sum0, sum1);
    let sq_vec = vaddq_f32(sq0, sq1);

    let remaining_chunks = (len - idx) / NEON_LANE_WIDTH;
    let mut final_sum = sum_vec;
    let mut final_sq = sq_vec;
    for _ in 0..remaining_chunks {
        let v = vld1q_f32(x_ptr.add(idx));
        final_sum = vaddq_f32(final_sum, v);
        final_sq = vfmaq_f32(final_sq, v, v);
        idx += 4;
    }

    let mut sum = vaddvq_f32(final_sum);
    let mut sum_sq = vaddvq_f32(final_sq);

    for i in idx..len {
        let v = *x_ptr.add(i);
        sum += v;
        sum_sq += v * v;
    }

    let n = len as f32;
    let mean = sum / n;
    let variance = (sum_sq / n) - (mean * mean);
    let inv_std = 1.0 / (variance + eps).sqrt();

    let mean_vec = vdupq_n_f32(mean);
    let inv_std_vec = vdupq_n_f32(inv_std);

    idx = 0;
    let unroll_chunks = len / (NEON_LANE_WIDTH * UNROLL_FACTOR);
    for _ in 0..unroll_chunks {
        let x0 = vld1q_f32(x_ptr.add(idx));
        let n0 = vmulq_f32(vsubq_f32(x0, mean_vec), inv_std_vec);
        let w0 = vld1q_f32(w_ptr.add(idx));
        let b0 = vld1q_f32(b_ptr.add(idx));
        vst1q_f32(x_ptr.add(idx), vfmaq_f32(b0, n0, w0));

        let x1 = vld1q_f32(x_ptr.add(idx + 4));
        let n1 = vmulq_f32(vsubq_f32(x1, mean_vec), inv_std_vec);
        let w1 = vld1q_f32(w_ptr.add(idx + 4));
        let b1 = vld1q_f32(b_ptr.add(idx + 4));
        vst1q_f32(x_ptr.add(idx + 4), vfmaq_f32(b1, n1, w1));

        let x2 = vld1q_f32(x_ptr.add(idx + 8));
        let n2 = vmulq_f32(vsubq_f32(x2, mean_vec), inv_std_vec);
        let w2 = vld1q_f32(w_ptr.add(idx + 8));
        let b2 = vld1q_f32(b_ptr.add(idx + 8));
        vst1q_f32(x_ptr.add(idx + 8), vfmaq_f32(b2, n2, w2));

        let x3 = vld1q_f32(x_ptr.add(idx + 12));
        let n3 = vmulq_f32(vsubq_f32(x3, mean_vec), inv_std_vec);
        let w3 = vld1q_f32(w_ptr.add(idx + 12));
        let b3 = vld1q_f32(b_ptr.add(idx + 12));
        vst1q_f32(x_ptr.add(idx + 12), vfmaq_f32(b3, n3, w3));

        idx += 16;
    }

    let remaining = (len - idx) / NEON_LANE_WIDTH;
    for _ in 0..remaining {
        let x_v = vld1q_f32(x_ptr.add(idx));
        let n_v = vmulq_f32(vsubq_f32(x_v, mean_vec), inv_std_vec);
        let w_v = vld1q_f32(w_ptr.add(idx));
        let b_v = vld1q_f32(b_ptr.add(idx));
        vst1q_f32(x_ptr.add(idx), vfmaq_f32(b_v, n_v, w_v));
        idx += 4;
    }

    for i in idx..len {
        let normalized = (*x_ptr.add(i) - mean) * inv_std;
        *x_ptr.add(i) = normalized * *w_ptr.add(i) + *b_ptr.add(i);
    }
}

#[allow(dead_code)]
fn layer_norm_scalar(x: &mut [f32], weight: &[f32], bias: &[f32], eps: f32) {
    let len = x.len();
    let n = len as f32;

    let sum: f32 = x.iter().sum();
    let mean = sum / n;

    let variance: f32 = x.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
    let inv_std = 1.0 / (variance + eps).sqrt();

    for i in 0..len {
        let normalized = (x[i] - mean) * inv_std;
        x[i] = normalized * weight[i] + bias[i];
    }
}

fn batched_rms_norm_neon(x: &mut [f32], weight: &[f32], batch_size: usize, dim: usize, eps: f32) {
    debug_assert_eq!(x.len(), batch_size * dim);
    debug_assert_eq!(weight.len(), dim);

    for b in 0..batch_size {
        let offset = b * dim;
        rms_norm_neon(&mut x[offset..offset + dim], weight, eps);
    }
}

fn batched_layer_norm_neon(
    x: &mut [f32],
    weight: &[f32],
    bias: &[f32],
    batch_size: usize,
    dim: usize,
    eps: f32,
) {
    debug_assert_eq!(x.len(), batch_size * dim);
    debug_assert_eq!(weight.len(), dim);
    debug_assert_eq!(bias.len(), dim);

    for b in 0..batch_size {
        let offset = b * dim;
        layer_norm_neon(&mut x[offset..offset + dim], weight, bias, eps);
    }
}

#[inline(always)]
fn compute_rms(x: &[f32]) -> f32 {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        compute_rms_neon_impl(x)
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        compute_rms_scalar(x)
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn compute_rms_neon_impl(x: &[f32]) -> f32 {
    use std::arch::aarch64::*;

    let len = x.len();
    if len == 0 {
        return 0.0;
    }

    let x_ptr = x.as_ptr();
    let mut sum = vdupq_n_f32(0.0);

    let chunks = len / NEON_LANE_WIDTH;
    let mut idx = 0usize;

    for _ in 0..chunks {
        let v = vld1q_f32(x_ptr.add(idx));
        sum = vfmaq_f32(sum, v, v);
        idx += 4;
    }

    let mut sum_sq = vaddvq_f32(sum);

    for i in idx..len {
        let v = *x_ptr.add(i);
        sum_sq += v * v;
    }

    (sum_sq / len as f32).sqrt()
}

#[allow(dead_code)]
fn compute_rms_scalar(x: &[f32]) -> f32 {
    let sum_sq: f32 = x.iter().map(|v| v * v).sum();
    (sum_sq / x.len() as f32).sqrt()
}

// Helper function to generate random tensor data
fn random_tensor(size: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

// === Benchmark Functions ===

fn bench_rms_norm(c: &mut Criterion) {
    let mut group = c.benchmark_group("rms_norm");
    group.sample_size(100);

    // Test common hidden sizes used in LLMs
    for dim in [768, 1024, 2048, 4096, 8192] {
        let mut x = random_tensor(dim);
        let weight = random_tensor(dim);
        let eps = 1e-6;

        let id = BenchmarkId::new(format!("dim_{}", dim), dim);

        group.throughput(Throughput::Elements(dim as u64));
        group.bench_function(id, |b| {
            b.iter(|| {
                let mut x_copy = x.clone();
                rms_norm_neon(black_box(&mut x_copy), black_box(&weight), eps);
                x_copy
            })
        });
    }

    group.finish();
}

fn bench_layer_norm(c: &mut Criterion) {
    let mut group = c.benchmark_group("layer_norm");
    group.sample_size(100);

    for dim in [768, 1024, 2048, 4096, 8192] {
        let mut x = random_tensor(dim);
        let weight = random_tensor(dim);
        let bias = random_tensor(dim);
        let eps = 1e-6;

        let id = BenchmarkId::new(format!("dim_{}", dim), dim);

        group.throughput(Throughput::Elements(dim as u64));
        group.bench_function(id, |b| {
            b.iter(|| {
                let mut x_copy = x.clone();
                layer_norm_neon(
                    black_box(&mut x_copy),
                    black_box(&weight),
                    black_box(&bias),
                    eps,
                );
                x_copy
            })
        });
    }

    group.finish();
}

fn bench_batched_rms_norm(c: &mut Criterion) {
    let mut group = c.benchmark_group("batched_rms_norm");
    group.sample_size(50);

    for batch_size in [1, 8, 32, 128] {
        for dim in [768, 2048, 4096] {
            let mut x = random_tensor(batch_size * dim);
            let weight = random_tensor(dim);
            let eps = 1e-6;

            let id = BenchmarkId::new(
                format!("batch_{}_dim_{}", batch_size, dim),
                batch_size * dim,
            );

            group.throughput(Throughput::Elements((batch_size * dim) as u64));
            group.bench_function(id, |b| {
                b.iter(|| {
                    let mut x_copy = x.clone();
                    batched_rms_norm_neon(
                        black_box(&mut x_copy),
                        black_box(&weight),
                        batch_size,
                        dim,
                        eps,
                    );
                    x_copy
                })
            });
        }
    }

    group.finish();
}

fn bench_batched_layer_norm(c: &mut Criterion) {
    let mut group = c.benchmark_group("batched_layer_norm");
    group.sample_size(50);

    for batch_size in [1, 8, 32, 128] {
        for dim in [768, 2048, 4096] {
            let mut x = random_tensor(batch_size * dim);
            let weight = random_tensor(dim);
            let bias = random_tensor(dim);
            let eps = 1e-6;

            let id = BenchmarkId::new(
                format!("batch_{}_dim_{}", batch_size, dim),
                batch_size * dim,
            );

            group.throughput(Throughput::Elements((batch_size * dim) as u64));
            group.bench_function(id, |b| {
                b.iter(|| {
                    let mut x_copy = x.clone();
                    batched_layer_norm_neon(
                        black_box(&mut x_copy),
                        black_box(&weight),
                        black_box(&bias),
                        batch_size,
                        dim,
                        eps,
                    );
                    x_copy
                })
            });
        }
    }

    group.finish();
}

fn bench_rms_vs_layer_norm(c: &mut Criterion) {
    let mut group = c.benchmark_group("rms_vs_layer");
    group.sample_size(100);

    for dim in [768, 2048, 4096] {
        let x = random_tensor(dim);
        let weight = random_tensor(dim);
        let bias = random_tensor(dim);
        let eps = 1e-6;

        group.bench_function(BenchmarkId::new("rms_norm", dim), |b| {
            b.iter(|| {
                let mut x_copy = x.clone();
                rms_norm_neon(black_box(&mut x_copy), black_box(&weight), eps);
                x_copy
            })
        });

        group.bench_function(BenchmarkId::new("layer_norm", dim), |b| {
            b.iter(|| {
                let mut x_copy = x.clone();
                layer_norm_neon(
                    black_box(&mut x_copy),
                    black_box(&weight),
                    black_box(&bias),
                    eps,
                );
                x_copy
            })
        });
    }

    group.finish();
}

fn bench_compute_rms(c: &mut Criterion) {
    let mut group = c.benchmark_group("compute_rms");
    group.sample_size(100);

    for dim in [768, 2048, 4096, 8192] {
        let x = random_tensor(dim);

        let id = BenchmarkId::new(format!("dim_{}", dim), dim);

        group.throughput(Throughput::Elements(dim as u64));
        group.bench_function(id, |b| b.iter(|| compute_rms(black_box(&x))));
    }

    group.finish();
}

fn bench_norm_memory_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("norm_memory_throughput");
    group.sample_size(50);

    // Test memory bandwidth at different sizes
    for dim in [256, 512, 1024, 2048, 4096, 8192, 16384] {
        let x = random_tensor(dim);
        let weight = random_tensor(dim);
        let eps = 1e-6;

        // Memory: read x (dim * 4), read weight (dim * 4), write x (dim * 4)
        let memory_bytes = dim * 4 * 3;

        let id = BenchmarkId::new(format!("dim_{}", dim), dim);

        group.throughput(Throughput::Bytes(memory_bytes as u64));
        group.bench_function(id, |b| {
            b.iter(|| {
                let mut x_copy = x.clone();
                rms_norm_neon(black_box(&mut x_copy), black_box(&weight), eps);
                x_copy
            })
        });
    }

    group.finish();
}

fn bench_norm_llm_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("norm_llm_sizes");
    group.sample_size(50);

    // Real-world LLM hidden sizes
    let llm_configs = [
        ("llama2_7b", 4096),
        ("llama2_13b", 5120),
        ("llama2_70b", 8192),
        ("llama3_8b", 4096),
        ("mistral_7b", 4096),
        ("qwen2_7b", 3584),
    ];

    for (name, dim) in llm_configs {
        let x = random_tensor(dim);
        let weight = random_tensor(dim);
        let eps = 1e-6;

        let id = BenchmarkId::new(name, dim);

        group.throughput(Throughput::Elements(dim as u64));
        group.bench_function(id, |b| {
            b.iter(|| {
                let mut x_copy = x.clone();
                rms_norm_neon(black_box(&mut x_copy), black_box(&weight), eps);
                x_copy
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_rms_norm,
    bench_layer_norm,
    bench_batched_rms_norm,
    bench_batched_layer_norm,
    bench_rms_vs_layer_norm,
    bench_compute_rms,
    bench_norm_memory_throughput,
    bench_norm_llm_sizes,
);

criterion_main!(benches);
