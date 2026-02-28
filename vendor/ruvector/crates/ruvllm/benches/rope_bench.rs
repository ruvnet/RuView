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
//! RoPE (Rotary Position Embedding) Benchmarks for M4 Pro
//!
//! Benchmarks for RoPE operations including:
//! - Standard RoPE application
//! - Table precomputation
//! - Scaled RoPE variants (NTK, YaRN)
//!
//! Performance targets for M4 Pro:
//! - RoPE apply (128 head_dim, 1 token): <5us
//! - RoPE apply (128 head_dim, 32 tokens): <50us
//! - Table precomputation (4096 seq): <1ms

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;

const NEON_LANE_WIDTH: usize = 4;
const UNROLL_FACTOR: usize = 4;

/// RoPE configuration
#[derive(Clone, Copy)]
struct RopeConfig {
    base: f32,
    head_dim: usize,
    max_seq_len: usize,
    scaling_factor: f32,
    ntk_aware: bool,
    original_max_len: usize,
}

impl Default for RopeConfig {
    fn default() -> Self {
        Self {
            base: 10000.0,
            head_dim: 128,
            max_seq_len: 4096,
            scaling_factor: 1.0,
            ntk_aware: false,
            original_max_len: 4096,
        }
    }
}

impl RopeConfig {
    fn llama2(head_dim: usize, max_seq_len: usize) -> Self {
        Self {
            base: 10000.0,
            head_dim,
            max_seq_len,
            ..Default::default()
        }
    }

    fn llama3(head_dim: usize, max_seq_len: usize) -> Self {
        Self {
            base: 500000.0,
            head_dim,
            max_seq_len,
            ..Default::default()
        }
    }

    fn with_ntk(mut self, original_max_len: usize) -> Self {
        self.ntk_aware = true;
        self.original_max_len = original_max_len;
        self
    }

    fn with_scaling(mut self, scaling_factor: f32) -> Self {
        self.scaling_factor = scaling_factor;
        self
    }

    fn effective_base(&self) -> f32 {
        if self.ntk_aware && self.max_seq_len > self.original_max_len {
            let scale = self.max_seq_len as f32 / self.original_max_len as f32;
            self.base * scale.powf((self.head_dim as f32) / (self.head_dim as f32 - 2.0))
        } else {
            self.base
        }
    }
}

#[derive(Clone)]
struct RopeTables {
    cos: Vec<f32>,
    sin: Vec<f32>,
    half_dim: usize,
    max_seq_len: usize,
}

impl RopeTables {
    fn get(&self, position: usize) -> (&[f32], &[f32]) {
        let offset = position * self.half_dim;
        (
            &self.cos[offset..offset + self.half_dim],
            &self.sin[offset..offset + self.half_dim],
        )
    }
}

fn precompute_rope_tables(max_seq_len: usize, head_dim: usize, base: f32) -> (Vec<f32>, Vec<f32>) {
    let half_dim = head_dim / 2;
    let mut cos_table = vec![0.0; max_seq_len * half_dim];
    let mut sin_table = vec![0.0; max_seq_len * half_dim];

    let inv_freq: Vec<f32> = (0..half_dim)
        .map(|i| 1.0 / base.powf((2 * i) as f32 / head_dim as f32))
        .collect();

    for pos in 0..max_seq_len {
        let offset = pos * half_dim;
        for (i, &freq) in inv_freq.iter().enumerate() {
            let theta = pos as f32 * freq;
            cos_table[offset + i] = theta.cos();
            sin_table[offset + i] = theta.sin();
        }
    }

    (cos_table, sin_table)
}

fn precompute_rope_tables_with_config(config: &RopeConfig) -> RopeTables {
    let base = config.effective_base();
    let (cos, sin) = precompute_rope_tables(config.max_seq_len, config.head_dim, base);

    let (cos, sin) = if config.scaling_factor != 1.0 {
        let half_dim = config.head_dim / 2;
        let mut scaled_cos = vec![0.0; config.max_seq_len * half_dim];
        let mut scaled_sin = vec![0.0; config.max_seq_len * half_dim];

        for pos in 0..config.max_seq_len {
            let scaled_pos = pos as f32 / config.scaling_factor;
            let lower_pos = scaled_pos.floor() as usize;
            let upper_pos = (lower_pos + 1).min(config.max_seq_len - 1);
            let frac = scaled_pos - lower_pos as f32;

            let offset = pos * half_dim;
            let lower_offset = lower_pos * half_dim;
            let upper_offset = upper_pos * half_dim;

            for i in 0..half_dim {
                scaled_cos[offset + i] =
                    cos[lower_offset + i] * (1.0 - frac) + cos[upper_offset + i] * frac;
                scaled_sin[offset + i] =
                    sin[lower_offset + i] * (1.0 - frac) + sin[upper_offset + i] * frac;
            }
        }

        (scaled_cos, scaled_sin)
    } else {
        (cos, sin)
    };

    RopeTables {
        cos,
        sin,
        half_dim: config.head_dim / 2,
        max_seq_len: config.max_seq_len,
    }
}

#[inline(always)]
fn apply_rope_neon(x: &mut [f32], positions: &[usize], head_dim: usize, base: f32) {
    let half_dim = head_dim / 2;
    let num_tokens = positions.len();
    let stride = head_dim;

    debug_assert_eq!(x.len(), num_tokens * head_dim);

    let inv_freq: Vec<f32> = (0..half_dim)
        .map(|i| 1.0 / base.powf((2 * i) as f32 / head_dim as f32))
        .collect();

    #[cfg(target_arch = "aarch64")]
    unsafe {
        apply_rope_neon_impl(x, positions, &inv_freq, half_dim, stride);
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        apply_rope_scalar(x, positions, &inv_freq, half_dim, stride);
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn apply_rope_neon_impl(
    x: &mut [f32],
    positions: &[usize],
    inv_freq: &[f32],
    half_dim: usize,
    stride: usize,
) {
    let x_ptr = x.as_mut_ptr();
    let inv_freq_ptr = inv_freq.as_ptr();

    for (tok_idx, &pos) in positions.iter().enumerate() {
        let tok_offset = tok_idx * stride;

        let chunks = half_dim / (NEON_LANE_WIDTH / 2);

        let mut freq_idx = 0usize;
        for _ in 0..chunks {
            let freq0 = *inv_freq_ptr.add(freq_idx);
            let freq1 = *inv_freq_ptr.add(freq_idx + 1);

            let theta0 = pos as f32 * freq0;
            let theta1 = pos as f32 * freq1;

            let cos0 = theta0.cos();
            let sin0 = theta0.sin();
            let cos1 = theta1.cos();
            let sin1 = theta1.sin();

            let x_offset = tok_offset + freq_idx * 2;
            let x0 = *x_ptr.add(x_offset);
            let x1 = *x_ptr.add(x_offset + 1);
            let x2 = *x_ptr.add(x_offset + 2);
            let x3 = *x_ptr.add(x_offset + 3);

            *x_ptr.add(x_offset) = x0 * cos0 - x1 * sin0;
            *x_ptr.add(x_offset + 1) = x1 * cos0 + x0 * sin0;
            *x_ptr.add(x_offset + 2) = x2 * cos1 - x3 * sin1;
            *x_ptr.add(x_offset + 3) = x3 * cos1 + x2 * sin1;

            freq_idx += 2;
        }

        while freq_idx < half_dim {
            let freq = *inv_freq_ptr.add(freq_idx);
            let theta = pos as f32 * freq;
            let cos_val = theta.cos();
            let sin_val = theta.sin();

            let x_offset = tok_offset + freq_idx * 2;
            let x0 = *x_ptr.add(x_offset);
            let x1 = *x_ptr.add(x_offset + 1);

            *x_ptr.add(x_offset) = x0 * cos_val - x1 * sin_val;
            *x_ptr.add(x_offset + 1) = x1 * cos_val + x0 * sin_val;

            freq_idx += 1;
        }
    }
}

#[allow(dead_code)]
fn apply_rope_scalar(
    x: &mut [f32],
    positions: &[usize],
    inv_freq: &[f32],
    half_dim: usize,
    stride: usize,
) {
    for (tok_idx, &pos) in positions.iter().enumerate() {
        let tok_offset = tok_idx * stride;

        for (i, &freq) in inv_freq.iter().enumerate() {
            let theta = pos as f32 * freq;
            let cos_val = theta.cos();
            let sin_val = theta.sin();

            let x_offset = tok_offset + i * 2;
            let x0 = x[x_offset];
            let x1 = x[x_offset + 1];

            x[x_offset] = x0 * cos_val - x1 * sin_val;
            x[x_offset + 1] = x1 * cos_val + x0 * sin_val;
        }
    }
}

#[inline(always)]
fn apply_rope_with_tables(x: &mut [f32], positions: &[usize], tables: &RopeTables) {
    let half_dim = tables.half_dim;
    let num_tokens = positions.len();
    let head_dim = half_dim * 2;

    debug_assert_eq!(x.len(), num_tokens * head_dim);

    #[cfg(target_arch = "aarch64")]
    unsafe {
        apply_rope_tables_neon_impl(x, positions, tables, half_dim);
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        apply_rope_tables_scalar(x, positions, tables, half_dim);
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn apply_rope_tables_neon_impl(
    x: &mut [f32],
    positions: &[usize],
    tables: &RopeTables,
    half_dim: usize,
) {
    use std::arch::aarch64::*;

    let x_ptr = x.as_mut_ptr();
    let head_dim = half_dim * 2;

    for (tok_idx, &pos) in positions.iter().enumerate() {
        debug_assert!(pos < tables.max_seq_len);

        let tok_offset = tok_idx * head_dim;
        let table_offset = pos * half_dim;

        let cos_ptr = tables.cos.as_ptr().add(table_offset);
        let sin_ptr = tables.sin.as_ptr().add(table_offset);

        let chunks = half_dim / UNROLL_FACTOR;

        let mut freq_idx = 0usize;
        for _ in 0..chunks {
            let cos_vec = vld1q_f32(cos_ptr.add(freq_idx));
            let sin_vec = vld1q_f32(sin_ptr.add(freq_idx));

            let x_offset = tok_offset + freq_idx * 2;

            let x_01 = vld1q_f32(x_ptr.add(x_offset));
            let x_23 = vld1q_f32(x_ptr.add(x_offset + 4));

            let x_even = vuzp1q_f32(x_01, x_23);
            let x_odd = vuzp2q_f32(x_01, x_23);

            let x_new_even = vfmsq_f32(vmulq_f32(x_even, cos_vec), x_odd, sin_vec);
            let x_new_odd = vfmaq_f32(vmulq_f32(x_odd, cos_vec), x_even, sin_vec);

            let out_01 = vzip1q_f32(x_new_even, x_new_odd);
            let out_23 = vzip2q_f32(x_new_even, x_new_odd);

            vst1q_f32(x_ptr.add(x_offset), out_01);
            vst1q_f32(x_ptr.add(x_offset + 4), out_23);

            freq_idx += 4;
        }

        while freq_idx < half_dim {
            let cos_val = *cos_ptr.add(freq_idx);
            let sin_val = *sin_ptr.add(freq_idx);

            let x_offset = tok_offset + freq_idx * 2;
            let x0 = *x_ptr.add(x_offset);
            let x1 = *x_ptr.add(x_offset + 1);

            *x_ptr.add(x_offset) = x0 * cos_val - x1 * sin_val;
            *x_ptr.add(x_offset + 1) = x1 * cos_val + x0 * sin_val;

            freq_idx += 1;
        }
    }
}

#[allow(dead_code)]
fn apply_rope_tables_scalar(
    x: &mut [f32],
    positions: &[usize],
    tables: &RopeTables,
    half_dim: usize,
) {
    let head_dim = half_dim * 2;

    for (tok_idx, &pos) in positions.iter().enumerate() {
        let tok_offset = tok_idx * head_dim;
        let (cos_slice, sin_slice) = tables.get(pos);

        for i in 0..half_dim {
            let cos_val = cos_slice[i];
            let sin_val = sin_slice[i];

            let x_offset = tok_offset + i * 2;
            let x0 = x[x_offset];
            let x1 = x[x_offset + 1];

            x[x_offset] = x0 * cos_val - x1 * sin_val;
            x[x_offset + 1] = x1 * cos_val + x0 * sin_val;
        }
    }
}

fn apply_inverse_rope_neon(x: &mut [f32], positions: &[usize], head_dim: usize, base: f32) {
    let half_dim = head_dim / 2;
    let stride = head_dim;

    let inv_freq: Vec<f32> = (0..half_dim)
        .map(|i| -1.0 / base.powf((2 * i) as f32 / head_dim as f32))
        .collect();

    #[cfg(target_arch = "aarch64")]
    unsafe {
        apply_rope_neon_impl(x, positions, &inv_freq, half_dim, stride);
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        apply_rope_scalar(x, positions, &inv_freq, half_dim, stride);
    }
}

// Helper function to generate random tensor data
fn random_tensor(size: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

// === Benchmark Functions ===

fn bench_apply_rope(c: &mut Criterion) {
    let mut group = c.benchmark_group("rope_apply");
    group.sample_size(100);

    for head_dim in [64, 128] {
        for num_tokens in [1, 8, 32, 128] {
            let mut x = random_tensor(num_tokens * head_dim);
            let positions: Vec<usize> = (0..num_tokens).collect();
            let base = 10000.0;

            let id = BenchmarkId::new(
                format!("dim_{}_tokens_{}", head_dim, num_tokens),
                num_tokens,
            );

            group.throughput(Throughput::Elements((num_tokens * head_dim) as u64));
            group.bench_function(id, |b| {
                b.iter(|| {
                    let mut x_copy = x.clone();
                    apply_rope_neon(
                        black_box(&mut x_copy),
                        black_box(&positions),
                        head_dim,
                        base,
                    );
                    x_copy
                })
            });
        }
    }

    group.finish();
}

fn bench_apply_rope_with_tables(c: &mut Criterion) {
    let mut group = c.benchmark_group("rope_apply_tables");
    group.sample_size(100);

    for head_dim in [64, 128] {
        let config = RopeConfig {
            head_dim,
            max_seq_len: 4096,
            base: 10000.0,
            ..Default::default()
        };
        let tables = precompute_rope_tables_with_config(&config);

        for num_tokens in [1, 8, 32, 128] {
            let x = random_tensor(num_tokens * head_dim);
            let positions: Vec<usize> = (0..num_tokens).collect();

            let id = BenchmarkId::new(
                format!("dim_{}_tokens_{}", head_dim, num_tokens),
                num_tokens,
            );

            group.throughput(Throughput::Elements((num_tokens * head_dim) as u64));
            group.bench_with_input(id, &(x.clone(), tables.clone()), |b, (x, tables)| {
                b.iter(|| {
                    let mut x_copy = x.clone();
                    apply_rope_with_tables(black_box(&mut x_copy), black_box(&positions), tables);
                    x_copy
                })
            });
        }
    }

    group.finish();
}

fn bench_precompute_tables(c: &mut Criterion) {
    let mut group = c.benchmark_group("rope_precompute");
    group.sample_size(50);

    for max_seq_len in [512, 1024, 2048, 4096, 8192] {
        for head_dim in [64, 128] {
            let id = BenchmarkId::new(format!("seq_{}_dim_{}", max_seq_len, head_dim), max_seq_len);

            group.throughput(Throughput::Elements((max_seq_len * head_dim) as u64));
            group.bench_function(id, |b| {
                b.iter(|| {
                    precompute_rope_tables(black_box(max_seq_len), black_box(head_dim), 10000.0)
                })
            });
        }
    }

    group.finish();
}

fn bench_precompute_with_config(c: &mut Criterion) {
    let mut group = c.benchmark_group("rope_precompute_config");
    group.sample_size(50);

    // Test different model configurations
    let configs = [
        ("llama2_4k", RopeConfig::llama2(128, 4096)),
        ("llama3_4k", RopeConfig::llama3(128, 4096)),
        (
            "llama2_8k_ntk",
            RopeConfig::llama2(128, 8192).with_ntk(4096),
        ),
        (
            "llama2_8k_scaled",
            RopeConfig::llama2(128, 8192).with_scaling(2.0),
        ),
    ];

    for (name, config) in configs {
        let id = BenchmarkId::new(name, config.max_seq_len);

        group.throughput(Throughput::Elements(
            (config.max_seq_len * config.head_dim) as u64,
        ));
        group.bench_with_input(id, &config, |b, cfg| {
            b.iter(|| precompute_rope_tables_with_config(black_box(cfg)))
        });
    }

    group.finish();
}

fn bench_rope_vs_tables(c: &mut Criterion) {
    let mut group = c.benchmark_group("rope_comparison");
    group.sample_size(100);

    let head_dim = 128;
    let max_seq_len = 4096;
    let num_tokens = 32;
    let base = 10000.0;

    let config = RopeConfig {
        head_dim,
        max_seq_len,
        base,
        ..Default::default()
    };
    let tables = precompute_rope_tables_with_config(&config);

    let x = random_tensor(num_tokens * head_dim);
    let positions: Vec<usize> = (0..num_tokens).collect();

    // Benchmark without tables
    group.bench_function("without_tables", |b| {
        b.iter(|| {
            let mut x_copy = x.clone();
            apply_rope_neon(
                black_box(&mut x_copy),
                black_box(&positions),
                head_dim,
                base,
            );
            x_copy
        })
    });

    // Benchmark with tables
    group.bench_with_input("with_tables", &tables, |b, tables| {
        b.iter(|| {
            let mut x_copy = x.clone();
            apply_rope_with_tables(black_box(&mut x_copy), black_box(&positions), tables);
            x_copy
        })
    });

    group.finish();
}

fn bench_inverse_rope(c: &mut Criterion) {
    let mut group = c.benchmark_group("rope_inverse");
    group.sample_size(100);

    for head_dim in [64, 128] {
        for num_tokens in [1, 8, 32] {
            let mut x = random_tensor(num_tokens * head_dim);
            let positions: Vec<usize> = (0..num_tokens).collect();
            let base = 10000.0;

            let id = BenchmarkId::new(
                format!("dim_{}_tokens_{}", head_dim, num_tokens),
                num_tokens,
            );

            group.throughput(Throughput::Elements((num_tokens * head_dim) as u64));
            group.bench_function(id, |b| {
                b.iter(|| {
                    let mut x_copy = x.clone();
                    apply_inverse_rope_neon(
                        black_box(&mut x_copy),
                        black_box(&positions),
                        head_dim,
                        base,
                    );
                    x_copy
                })
            });
        }
    }

    group.finish();
}

fn bench_rope_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("rope_roundtrip");
    group.sample_size(50);

    let head_dim = 128;
    let base = 10000.0;

    for num_tokens in [1, 8, 32] {
        let x = random_tensor(num_tokens * head_dim);
        let positions: Vec<usize> = (0..num_tokens).collect();

        let id = BenchmarkId::new(format!("tokens_{}", num_tokens), num_tokens);

        group.throughput(Throughput::Elements((num_tokens * head_dim * 2) as u64));
        group.bench_function(id, |b| {
            b.iter(|| {
                let mut x_copy = x.clone();
                apply_rope_neon(
                    black_box(&mut x_copy),
                    black_box(&positions),
                    head_dim,
                    base,
                );
                apply_inverse_rope_neon(
                    black_box(&mut x_copy),
                    black_box(&positions),
                    head_dim,
                    base,
                );
                x_copy
            })
        });
    }

    group.finish();
}

fn bench_rope_scaling_variants(c: &mut Criterion) {
    let mut group = c.benchmark_group("rope_scaling");
    group.sample_size(50);

    let head_dim = 128;
    let num_tokens = 32;
    let x = random_tensor(num_tokens * head_dim);
    let positions: Vec<usize> = (0..num_tokens).collect();

    // Different scaling configurations
    let configs = [
        ("standard", RopeConfig::llama2(head_dim, 4096)),
        ("ntk_2x", RopeConfig::llama2(head_dim, 8192).with_ntk(4096)),
        ("ntk_4x", RopeConfig::llama2(head_dim, 16384).with_ntk(4096)),
        (
            "linear_2x",
            RopeConfig::llama2(head_dim, 8192).with_scaling(2.0),
        ),
        (
            "linear_4x",
            RopeConfig::llama2(head_dim, 16384).with_scaling(4.0),
        ),
    ];

    for (name, config) in configs {
        let tables = precompute_rope_tables_with_config(&config);

        let id = BenchmarkId::new(name, config.max_seq_len);

        group.bench_with_input(id, &tables, |b, tables| {
            b.iter(|| {
                let mut x_copy = x.clone();
                apply_rope_with_tables(black_box(&mut x_copy), black_box(&positions), tables);
                x_copy
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_apply_rope,
    bench_apply_rope_with_tables,
    bench_precompute_tables,
    bench_precompute_with_config,
    bench_rope_vs_tables,
    bench_inverse_rope,
    bench_rope_roundtrip,
    bench_rope_scaling_variants,
);

criterion_main!(benches);
