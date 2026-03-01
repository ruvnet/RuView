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
//! Attention Kernel Benchmarks for M4 Pro
//!
//! Benchmarks for Flash Attention 2, Paged Attention, MQA, and GQA implementations.
//!
//! Performance targets for M4 Pro:
//! - Flash attention (256 seq): <2ms
//! - Flash attention (512 seq): <5ms
//! - Flash attention (1024 seq): <15ms
//! - Paged attention: Similar to flash attention + 10% overhead

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;

// Re-create the kernel functions inline since we can't import from the crate easily in benches
// In production, these would be imported from ruvllm::kernels

/// SIMD lane width for NEON (128-bit = 4 floats)
const NEON_LANE_WIDTH: usize = 4;
const UNROLL_FACTOR: usize = 4;

/// Paged KV cache for efficient memory management
#[derive(Clone)]
struct PagedKvCache {
    key_blocks: Vec<Vec<f32>>,
    value_blocks: Vec<Vec<f32>>,
    block_size: usize,
    num_kv_heads: usize,
    head_dim: usize,
    num_tokens: usize,
}

impl PagedKvCache {
    fn new(block_size: usize, num_kv_heads: usize, head_dim: usize) -> Self {
        Self {
            key_blocks: Vec::new(),
            value_blocks: Vec::new(),
            block_size,
            num_kv_heads,
            head_dim,
            num_tokens: 0,
        }
    }

    fn append(&mut self, keys: &[f32], values: &[f32]) {
        let stride = self.num_kv_heads * self.head_dim;
        let num_tokens = keys.len() / stride;

        for i in 0..num_tokens {
            let offset = i * stride;

            if self.num_tokens % self.block_size == 0 {
                let block_capacity = self.block_size * stride;
                self.key_blocks.push(vec![0.0; block_capacity]);
                self.value_blocks.push(vec![0.0; block_capacity]);
            }

            let block_idx = self.num_tokens / self.block_size;
            let pos_in_block = (self.num_tokens % self.block_size) * stride;

            self.key_blocks[block_idx][pos_in_block..pos_in_block + stride]
                .copy_from_slice(&keys[offset..offset + stride]);
            self.value_blocks[block_idx][pos_in_block..pos_in_block + stride]
                .copy_from_slice(&values[offset..offset + stride]);

            self.num_tokens += 1;
        }
    }

    fn get_keys(&self) -> Vec<f32> {
        let stride = self.num_kv_heads * self.head_dim;
        let mut result = Vec::with_capacity(self.num_tokens * stride);
        for (block_idx, block) in self.key_blocks.iter().enumerate() {
            let tokens_in_block = if block_idx == self.key_blocks.len() - 1 {
                let rem = self.num_tokens % self.block_size;
                if rem == 0 {
                    self.block_size
                } else {
                    rem
                }
            } else {
                self.block_size
            };
            result.extend_from_slice(&block[..tokens_in_block * stride]);
        }
        result
    }

    fn get_values(&self) -> Vec<f32> {
        let stride = self.num_kv_heads * self.head_dim;
        let mut result = Vec::with_capacity(self.num_tokens * stride);
        for (block_idx, block) in self.value_blocks.iter().enumerate() {
            let tokens_in_block = if block_idx == self.value_blocks.len() - 1 {
                let rem = self.num_tokens % self.block_size;
                if rem == 0 {
                    self.block_size
                } else {
                    rem
                }
            } else {
                self.block_size
            };
            result.extend_from_slice(&block[..tokens_in_block * stride]);
        }
        result
    }
}

/// Attention configuration
#[derive(Clone, Copy)]
struct AttentionConfig {
    num_heads: usize,
    num_kv_heads: usize,
    head_dim: usize,
    max_seq_len: usize,
    causal: bool,
    scale: f32,
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self {
            num_heads: 32,
            num_kv_heads: 8,
            head_dim: 128,
            max_seq_len: 4096,
            causal: true,
            scale: 0.0,
        }
    }
}

impl AttentionConfig {
    fn effective_scale(&self) -> f32 {
        if self.scale == 0.0 {
            1.0 / (self.head_dim as f32).sqrt()
        } else {
            self.scale
        }
    }

    fn gqa_ratio(&self) -> usize {
        self.num_heads / self.num_kv_heads
    }
}

/// Flash Attention 2 with NEON SIMD optimization
#[inline(always)]
fn flash_attention_neon(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    scale: f32,
    causal: bool,
) -> Vec<f32> {
    let head_dim = if !query.is_empty() && !key.is_empty() {
        query.len()
    } else {
        return vec![];
    };

    let kv_len = key.len() / head_dim;
    if kv_len == 0 {
        return vec![0.0; head_dim];
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        flash_attention_neon_impl(query, key, value, head_dim, kv_len, scale, causal)
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        flash_attention_scalar(query, key, value, head_dim, kv_len, scale, causal)
    }
}

#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn flash_attention_neon_impl(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    head_dim: usize,
    kv_len: usize,
    scale: f32,
    _causal: bool,
) -> Vec<f32> {
    use std::arch::aarch64::*;

    let q_ptr = query.as_ptr();
    let k_ptr = key.as_ptr();
    let v_ptr = value.as_ptr();

    let mut max_score = f32::NEG_INFINITY;
    let mut sum_exp = 0.0f32;
    let mut output = vec![0.0f32; head_dim];
    let out_ptr = output.as_mut_ptr();

    let scale_vec = vdupq_n_f32(scale);

    for t in 0..kv_len {
        let k_offset = t * head_dim;

        let mut dot = vdupq_n_f32(0.0);
        let chunks = head_dim / (NEON_LANE_WIDTH * UNROLL_FACTOR);

        let mut idx = 0usize;
        for _ in 0..chunks {
            let q0 = vld1q_f32(q_ptr.add(idx));
            let k0 = vld1q_f32(k_ptr.add(k_offset + idx));
            dot = vfmaq_f32(dot, q0, k0);

            let q1 = vld1q_f32(q_ptr.add(idx + 4));
            let k1 = vld1q_f32(k_ptr.add(k_offset + idx + 4));
            dot = vfmaq_f32(dot, q1, k1);

            let q2 = vld1q_f32(q_ptr.add(idx + 8));
            let k2 = vld1q_f32(k_ptr.add(k_offset + idx + 8));
            dot = vfmaq_f32(dot, q2, k2);

            let q3 = vld1q_f32(q_ptr.add(idx + 12));
            let k3 = vld1q_f32(k_ptr.add(k_offset + idx + 12));
            dot = vfmaq_f32(dot, q3, k3);

            idx += 16;
        }

        let remaining_chunks = (head_dim - idx) / NEON_LANE_WIDTH;
        for _ in 0..remaining_chunks {
            let q_v = vld1q_f32(q_ptr.add(idx));
            let k_v = vld1q_f32(k_ptr.add(k_offset + idx));
            dot = vfmaq_f32(dot, q_v, k_v);
            idx += 4;
        }

        let mut score = vaddvq_f32(vmulq_f32(dot, scale_vec));

        for i in idx..head_dim {
            score += *q_ptr.add(i) * *k_ptr.add(k_offset + i) * scale;
        }

        if score > max_score {
            let exp_diff = (max_score - score).exp();
            sum_exp = sum_exp * exp_diff + 1.0;
            max_score = score;

            let rescale = vdupq_n_f32(exp_diff);
            let mut out_idx = 0usize;
            let out_chunks = head_dim / NEON_LANE_WIDTH;
            for _ in 0..out_chunks {
                let out_v = vld1q_f32(out_ptr.add(out_idx));
                vst1q_f32(out_ptr.add(out_idx), vmulq_f32(out_v, rescale));
                out_idx += 4;
            }
            for i in out_idx..head_dim {
                *out_ptr.add(i) *= exp_diff;
            }
        } else {
            sum_exp += (score - max_score).exp();
        }

        let weight = (score - max_score).exp();
        let weight_vec = vdupq_n_f32(weight);

        let mut out_idx = 0usize;
        let out_chunks = head_dim / (NEON_LANE_WIDTH * UNROLL_FACTOR);
        for _ in 0..out_chunks {
            let v0 = vld1q_f32(v_ptr.add(t * head_dim + out_idx));
            let o0 = vld1q_f32(out_ptr.add(out_idx));
            vst1q_f32(out_ptr.add(out_idx), vfmaq_f32(o0, v0, weight_vec));

            let v1 = vld1q_f32(v_ptr.add(t * head_dim + out_idx + 4));
            let o1 = vld1q_f32(out_ptr.add(out_idx + 4));
            vst1q_f32(out_ptr.add(out_idx + 4), vfmaq_f32(o1, v1, weight_vec));

            let v2 = vld1q_f32(v_ptr.add(t * head_dim + out_idx + 8));
            let o2 = vld1q_f32(out_ptr.add(out_idx + 8));
            vst1q_f32(out_ptr.add(out_idx + 8), vfmaq_f32(o2, v2, weight_vec));

            let v3 = vld1q_f32(v_ptr.add(t * head_dim + out_idx + 12));
            let o3 = vld1q_f32(out_ptr.add(out_idx + 12));
            vst1q_f32(out_ptr.add(out_idx + 12), vfmaq_f32(o3, v3, weight_vec));

            out_idx += 16;
        }

        let remaining_out = (head_dim - out_idx) / NEON_LANE_WIDTH;
        for _ in 0..remaining_out {
            let v_v = vld1q_f32(v_ptr.add(t * head_dim + out_idx));
            let o_v = vld1q_f32(out_ptr.add(out_idx));
            vst1q_f32(out_ptr.add(out_idx), vfmaq_f32(o_v, v_v, weight_vec));
            out_idx += 4;
        }

        for i in out_idx..head_dim {
            *out_ptr.add(i) += weight * *v_ptr.add(t * head_dim + i);
        }
    }

    if sum_exp > 0.0 {
        let inv_sum = 1.0 / sum_exp;
        let inv_sum_vec = vdupq_n_f32(inv_sum);

        let mut idx = 0usize;
        let chunks = head_dim / NEON_LANE_WIDTH;
        for _ in 0..chunks {
            let o = vld1q_f32(out_ptr.add(idx));
            vst1q_f32(out_ptr.add(idx), vmulq_f32(o, inv_sum_vec));
            idx += 4;
        }
        for i in idx..head_dim {
            *out_ptr.add(i) *= inv_sum;
        }
    }

    output
}

#[allow(dead_code)]
fn flash_attention_scalar(
    query: &[f32],
    key: &[f32],
    value: &[f32],
    head_dim: usize,
    kv_len: usize,
    scale: f32,
    _causal: bool,
) -> Vec<f32> {
    let mut scores = Vec::with_capacity(kv_len);

    for t in 0..kv_len {
        let k_offset = t * head_dim;
        let score: f32 = query
            .iter()
            .zip(&key[k_offset..k_offset + head_dim])
            .map(|(q, k)| q * k * scale)
            .sum();
        scores.push(score);
    }

    let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exp_scores: Vec<f32> = scores.iter().map(|s| (s - max_score).exp()).collect();
    let sum_exp: f32 = exp_scores.iter().sum();
    let attn_weights: Vec<f32> = exp_scores.iter().map(|e| e / sum_exp).collect();

    let mut output = vec![0.0; head_dim];
    for (t, weight) in attn_weights.iter().enumerate() {
        let v_offset = t * head_dim;
        for (i, v) in value[v_offset..v_offset + head_dim].iter().enumerate() {
            output[i] += weight * v;
        }
    }

    output
}

fn paged_attention_neon(
    query: &[f32],
    kv_cache: &PagedKvCache,
    _block_tables: &[usize],
    scale: f32,
) -> Vec<f32> {
    if kv_cache.num_tokens == 0 {
        return vec![0.0; query.len()];
    }

    let keys = kv_cache.get_keys();
    let values = kv_cache.get_values();

    flash_attention_neon(query, &keys, &values, scale, false)
}

fn multi_query_attention_neon(
    queries: &[f32],
    key: &[f32],
    value: &[f32],
    config: &AttentionConfig,
) -> Vec<f32> {
    let head_dim = config.head_dim;
    let num_heads = config.num_heads;
    let scale = config.effective_scale();

    let mut output = vec![0.0; num_heads * head_dim];

    for h in 0..num_heads {
        let q_offset = h * head_dim;
        let q_slice = &queries[q_offset..q_offset + head_dim];

        let head_output = flash_attention_neon(q_slice, key, value, scale, config.causal);

        output[q_offset..q_offset + head_dim].copy_from_slice(&head_output);
    }

    output
}

fn grouped_query_attention_neon(
    queries: &[f32],
    keys: &[f32],
    values: &[f32],
    config: &AttentionConfig,
) -> Vec<f32> {
    let head_dim = config.head_dim;
    let num_heads = config.num_heads;
    let num_kv_heads = config.num_kv_heads;
    let gqa_ratio = config.gqa_ratio();
    let scale = config.effective_scale();

    let kv_len = keys.len() / (num_kv_heads * head_dim);
    let mut output = vec![0.0; num_heads * head_dim];

    for h in 0..num_heads {
        let kv_head = h / gqa_ratio;
        let q_offset = h * head_dim;
        let q_slice = &queries[q_offset..q_offset + head_dim];

        let mut kv_keys = Vec::with_capacity(kv_len * head_dim);
        let mut kv_values = Vec::with_capacity(kv_len * head_dim);

        for t in 0..kv_len {
            let kv_offset = (t * num_kv_heads + kv_head) * head_dim;
            kv_keys.extend_from_slice(&keys[kv_offset..kv_offset + head_dim]);
            kv_values.extend_from_slice(&values[kv_offset..kv_offset + head_dim]);
        }

        let head_output = flash_attention_neon(q_slice, &kv_keys, &kv_values, scale, config.causal);

        output[q_offset..q_offset + head_dim].copy_from_slice(&head_output);
    }

    output
}

// Helper function to generate random tensor data
fn random_tensor(size: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

// === Benchmark Functions ===

fn bench_flash_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("flash_attention");
    group.sample_size(50);

    // Test various sequence lengths and head dimensions
    for seq_len in [128, 256, 512, 1024, 2048] {
        for head_dim in [64, 128] {
            let query = random_tensor(head_dim);
            let key = random_tensor(seq_len * head_dim);
            let value = random_tensor(seq_len * head_dim);
            let scale = 1.0 / (head_dim as f32).sqrt();

            let id = BenchmarkId::new(
                format!("seq_{}_head_{}", seq_len, head_dim),
                seq_len * head_dim,
            );

            group.throughput(Throughput::Elements((seq_len * head_dim) as u64));
            group.bench_with_input(
                id,
                &(query.clone(), key.clone(), value.clone()),
                |b, (q, k, v)| {
                    b.iter(|| {
                        flash_attention_neon(black_box(q), black_box(k), black_box(v), scale, true)
                    })
                },
            );
        }
    }

    group.finish();
}

fn bench_flash_attention_batched(c: &mut Criterion) {
    let mut group = c.benchmark_group("flash_attention_batched");
    group.sample_size(30);

    // Test batch processing for multi-head attention
    let head_dim = 128;
    let num_heads = 32;

    for seq_len in [128, 256, 512] {
        let queries = random_tensor(num_heads * head_dim);
        let key = random_tensor(seq_len * head_dim);
        let value = random_tensor(seq_len * head_dim);
        let scale = 1.0 / (head_dim as f32).sqrt();

        let id = BenchmarkId::new(format!("heads_{}_seq_{}", num_heads, seq_len), seq_len);

        group.throughput(Throughput::Elements(
            (num_heads * seq_len * head_dim) as u64,
        ));
        group.bench_with_input(
            id,
            &(queries.clone(), key.clone(), value.clone()),
            |b, (q, k, v)| {
                b.iter(|| {
                    // Process all heads
                    let mut outputs = Vec::with_capacity(num_heads * head_dim);
                    for h in 0..num_heads {
                        let q_offset = h * head_dim;
                        let q_slice = &q[q_offset..q_offset + head_dim];
                        let out = flash_attention_neon(
                            black_box(q_slice),
                            black_box(k),
                            black_box(v),
                            scale,
                            true,
                        );
                        outputs.extend(out);
                    }
                    outputs
                })
            },
        );
    }

    group.finish();
}

fn bench_paged_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("paged_attention");
    group.sample_size(50);

    // Test various block sizes and sequence lengths
    for block_size in [16, 32, 64] {
        for num_tokens in [64, 128, 256, 512] {
            let head_dim = 128;
            let num_kv_heads = 8;

            // Create and populate KV cache
            let mut kv_cache = PagedKvCache::new(block_size, num_kv_heads, head_dim);
            let stride = num_kv_heads * head_dim;

            for _ in 0..num_tokens {
                let keys = random_tensor(stride);
                let values = random_tensor(stride);
                kv_cache.append(&keys, &values);
            }

            let query = random_tensor(head_dim);
            let scale = 1.0 / (head_dim as f32).sqrt();

            let id = BenchmarkId::new(
                format!("block_{}_tokens_{}", block_size, num_tokens),
                num_tokens,
            );

            group.throughput(Throughput::Elements((num_tokens * head_dim) as u64));
            group.bench_with_input(id, &(query.clone(), kv_cache.clone()), |b, (q, cache)| {
                b.iter(|| paged_attention_neon(black_box(q), black_box(cache), &[], scale))
            });
        }
    }

    group.finish();
}

fn bench_mqa(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_query_attention");
    group.sample_size(30);

    for num_heads in [8, 16, 32] {
        for seq_len in [128, 256, 512] {
            let head_dim = 128;

            let config = AttentionConfig {
                num_heads,
                num_kv_heads: 1, // MQA: single KV head
                head_dim,
                causal: true,
                ..Default::default()
            };

            let queries = random_tensor(num_heads * head_dim);
            let key = random_tensor(seq_len * head_dim);
            let value = random_tensor(seq_len * head_dim);

            let id = BenchmarkId::new(format!("heads_{}_seq_{}", num_heads, seq_len), seq_len);

            group.throughput(Throughput::Elements(
                (num_heads * seq_len * head_dim) as u64,
            ));
            group.bench_with_input(
                id,
                &(queries.clone(), key.clone(), value.clone(), config),
                |b, (q, k, v, cfg)| {
                    b.iter(|| {
                        multi_query_attention_neon(black_box(q), black_box(k), black_box(v), cfg)
                    })
                },
            );
        }
    }

    group.finish();
}

fn bench_gqa(c: &mut Criterion) {
    let mut group = c.benchmark_group("grouped_query_attention");
    group.sample_size(30);

    // Test various GQA ratios (num_heads / num_kv_heads)
    for (num_heads, num_kv_heads) in [(32, 8), (32, 4), (16, 4), (16, 2)] {
        for seq_len in [128, 256, 512] {
            let head_dim = 128;

            let config = AttentionConfig {
                num_heads,
                num_kv_heads,
                head_dim,
                causal: true,
                ..Default::default()
            };

            let queries = random_tensor(num_heads * head_dim);
            let keys = random_tensor(seq_len * num_kv_heads * head_dim);
            let values = random_tensor(seq_len * num_kv_heads * head_dim);

            let ratio = num_heads / num_kv_heads;
            let id = BenchmarkId::new(format!("ratio_{}_seq_{}", ratio, seq_len), seq_len);

            group.throughput(Throughput::Elements(
                (num_heads * seq_len * head_dim) as u64,
            ));
            group.bench_with_input(
                id,
                &(queries.clone(), keys.clone(), values.clone(), config),
                |b, (q, k, v, cfg)| {
                    b.iter(|| {
                        grouped_query_attention_neon(black_box(q), black_box(k), black_box(v), cfg)
                    })
                },
            );
        }
    }

    group.finish();
}

fn bench_attention_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("attention_memory");
    group.sample_size(20);

    // Compare memory usage at different sequence lengths
    for seq_len in [256, 512, 1024, 2048, 4096] {
        let head_dim = 128;

        let query = random_tensor(head_dim);
        let key = random_tensor(seq_len * head_dim);
        let value = random_tensor(seq_len * head_dim);
        let scale = 1.0 / (head_dim as f32).sqrt();

        // Memory for Q, K, V in bytes
        let memory_bytes = (1 + seq_len * 2) * head_dim * 4; // f32 = 4 bytes

        let id = BenchmarkId::new(
            format!("seq_{}_mem_{}KB", seq_len, memory_bytes / 1024),
            seq_len,
        );

        group.throughput(Throughput::Bytes(memory_bytes as u64));
        group.bench_with_input(
            id,
            &(query.clone(), key.clone(), value.clone()),
            |b, (q, k, v)| {
                b.iter(|| {
                    flash_attention_neon(black_box(q), black_box(k), black_box(v), scale, true)
                })
            },
        );
    }

    group.finish();
}

fn bench_attention_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("attention_scaling");
    group.sample_size(20);

    // Test scaling behavior with increasing sequence length
    let head_dim = 128;
    let scale = 1.0 / (head_dim as f32).sqrt();

    for power in 7..=12 {
        // 128 to 4096
        let seq_len = 1 << power;

        let query = random_tensor(head_dim);
        let key = random_tensor(seq_len * head_dim);
        let value = random_tensor(seq_len * head_dim);

        let id = BenchmarkId::new(format!("seq_{}", seq_len), seq_len);

        // Measure FLOPs: 2*seq_len*head_dim for QK^T + 2*seq_len*head_dim for AV
        let flops = 4 * seq_len * head_dim;
        group.throughput(Throughput::Elements(flops as u64));

        group.bench_with_input(
            id,
            &(query.clone(), key.clone(), value.clone()),
            |b, (q, k, v)| {
                b.iter(|| {
                    flash_attention_neon(black_box(q), black_box(k), black_box(v), scale, true)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_flash_attention,
    bench_flash_attention_batched,
    bench_paged_attention,
    bench_mqa,
    bench_gqa,
    bench_attention_memory_efficiency,
    bench_attention_scaling,
);

criterion_main!(benches);
