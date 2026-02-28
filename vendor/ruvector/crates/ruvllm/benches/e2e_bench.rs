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
//! End-to-End LLM Inference Benchmarks for M4 Pro
//!
//! Comprehensive benchmarks for complete inference pipeline:
//! - Time to first token (TTFT)
//! - Tokens per second (throughput)
//! - Memory usage tracking
//! - Full transformer layer forward pass
//!
//! Performance targets for M4 Pro:
//! - TTFT: <100ms for 7B model
//! - Throughput: 100+ tokens/sec for 7B model
//! - Memory: <16GB for 7B model inference

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;
use std::time::Instant;

// Simulated model configuration
#[derive(Clone, Copy)]
struct ModelConfig {
    hidden_size: usize,
    intermediate_size: usize,
    num_attention_heads: usize,
    num_kv_heads: usize,
    head_dim: usize,
    num_layers: usize,
    vocab_size: usize,
    max_seq_len: usize,
}

impl ModelConfig {
    fn llama2_7b() -> Self {
        Self {
            hidden_size: 4096,
            intermediate_size: 11008,
            num_attention_heads: 32,
            num_kv_heads: 32,
            head_dim: 128,
            num_layers: 32,
            vocab_size: 32000,
            max_seq_len: 4096,
        }
    }

    fn llama2_13b() -> Self {
        Self {
            hidden_size: 5120,
            intermediate_size: 13824,
            num_attention_heads: 40,
            num_kv_heads: 40,
            head_dim: 128,
            num_layers: 40,
            vocab_size: 32000,
            max_seq_len: 4096,
        }
    }

    fn llama3_8b() -> Self {
        Self {
            hidden_size: 4096,
            intermediate_size: 14336,
            num_attention_heads: 32,
            num_kv_heads: 8, // GQA
            head_dim: 128,
            num_layers: 32,
            vocab_size: 128256,
            max_seq_len: 8192,
        }
    }

    fn mistral_7b() -> Self {
        Self {
            hidden_size: 4096,
            intermediate_size: 14336,
            num_attention_heads: 32,
            num_kv_heads: 8, // GQA
            head_dim: 128,
            num_layers: 32,
            vocab_size: 32000,
            max_seq_len: 32768,
        }
    }

    fn params_per_layer(&self) -> usize {
        // Attention: Q, K, V, O projections
        let attn_params = self.hidden_size * self.hidden_size * 4;

        // MLP: gate, up, down projections
        let mlp_params = self.hidden_size * self.intermediate_size * 3;

        // Norms (2 per layer)
        let norm_params = self.hidden_size * 2;

        attn_params + mlp_params + norm_params
    }

    fn total_params(&self) -> usize {
        // Embedding
        let embed_params = self.vocab_size * self.hidden_size;

        // All layers
        let layer_params = self.params_per_layer() * self.num_layers;

        // Final norm + LM head
        let final_params = self.hidden_size + self.vocab_size * self.hidden_size;

        embed_params + layer_params + final_params
    }

    fn memory_bytes_fp16(&self) -> usize {
        self.total_params() * 2 // FP16
    }

    fn memory_bytes_int4(&self) -> usize {
        self.total_params() / 2 // INT4
    }
}

// Simulated transformer layer operations
struct TransformerLayer {
    // Weights (simulated)
    q_proj: Vec<f32>,
    k_proj: Vec<f32>,
    v_proj: Vec<f32>,
    o_proj: Vec<f32>,
    gate_proj: Vec<f32>,
    up_proj: Vec<f32>,
    down_proj: Vec<f32>,
    input_norm_weight: Vec<f32>,
    post_attn_norm_weight: Vec<f32>,
    config: ModelConfig,
}

impl TransformerLayer {
    fn new(config: ModelConfig) -> Self {
        let hidden = config.hidden_size;
        let intermediate = config.intermediate_size;

        Self {
            q_proj: random_tensor(hidden * hidden),
            k_proj: random_tensor(
                hidden * (hidden / config.num_attention_heads * config.num_kv_heads),
            ),
            v_proj: random_tensor(
                hidden * (hidden / config.num_attention_heads * config.num_kv_heads),
            ),
            o_proj: random_tensor(hidden * hidden),
            gate_proj: random_tensor(hidden * intermediate),
            up_proj: random_tensor(hidden * intermediate),
            down_proj: random_tensor(intermediate * hidden),
            input_norm_weight: random_tensor(hidden),
            post_attn_norm_weight: random_tensor(hidden),
            config,
        }
    }

    // Simulated forward pass for a single token
    fn forward_single_token(&self, hidden_state: &mut [f32], kv_cache_len: usize) {
        let hidden = self.config.hidden_size;

        // 1. Input LayerNorm/RMSNorm
        rms_norm_inplace(hidden_state, &self.input_norm_weight, 1e-6);

        // 2. Attention projections (Q, K, V)
        let mut q = gemv(&self.q_proj, hidden_state, hidden, hidden);
        let k = gemv(
            &self.k_proj,
            hidden_state,
            hidden,
            hidden / self.config.num_attention_heads * self.config.num_kv_heads,
        );
        let v = gemv(
            &self.v_proj,
            hidden_state,
            hidden,
            hidden / self.config.num_attention_heads * self.config.num_kv_heads,
        );

        // 3. Apply RoPE (simplified)
        apply_rope_simple(&mut q, self.config.head_dim, kv_cache_len);

        // 4. Attention (simplified - would use flash attention in practice)
        // For single token decode, this is essentially a dot product with cached KV
        let attn_output = attention_decode(
            &q,
            &k,
            &v,
            self.config.num_attention_heads,
            self.config.head_dim,
        );

        // 5. Output projection
        let attn_projected = gemv(&self.o_proj, &attn_output, hidden, hidden);

        // 6. Residual connection
        for i in 0..hidden {
            hidden_state[i] += attn_projected[i];
        }

        // 7. Post-attention LayerNorm
        rms_norm_inplace(hidden_state, &self.post_attn_norm_weight, 1e-6);

        // 8. MLP forward
        let gate_out = gemv(
            &self.gate_proj,
            hidden_state,
            hidden,
            self.config.intermediate_size,
        );
        let up_out = gemv(
            &self.up_proj,
            hidden_state,
            hidden,
            self.config.intermediate_size,
        );

        // SiLU activation and element-wise multiply
        let mut mlp_intermediate = Vec::with_capacity(self.config.intermediate_size);
        for i in 0..self.config.intermediate_size {
            let silu = gate_out[i] / (1.0 + (-gate_out[i]).exp());
            mlp_intermediate.push(silu * up_out[i]);
        }

        // Down projection
        let mlp_output = gemv(
            &self.down_proj,
            &mlp_intermediate,
            self.config.intermediate_size,
            hidden,
        );

        // 9. Residual connection
        for i in 0..hidden {
            hidden_state[i] += mlp_output[i];
        }
    }
}

// Helper functions
fn random_tensor(size: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen_range(-0.1..0.1)).collect()
}

fn rms_norm_inplace(x: &mut [f32], weight: &[f32], eps: f32) {
    let sum_sq: f32 = x.iter().map(|v| v * v).sum();
    let inv_rms = 1.0 / (sum_sq / x.len() as f32 + eps).sqrt();
    for (i, w) in weight.iter().enumerate() {
        x[i] = x[i] * inv_rms * w;
    }
}

fn gemv(matrix: &[f32], vector: &[f32], m: usize, n: usize) -> Vec<f32> {
    let mut output = vec![0.0f32; n];
    for j in 0..n {
        let mut sum = 0.0f32;
        for i in 0..m {
            sum += matrix[i * n + j] * vector[i];
        }
        output[j] = sum;
    }
    output
}

fn apply_rope_simple(x: &mut [f32], head_dim: usize, position: usize) {
    let half_dim = head_dim / 2;
    for i in 0..half_dim {
        let freq = 1.0 / 10000.0f32.powf((2 * i) as f32 / head_dim as f32);
        let theta = position as f32 * freq;
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();

        let x0 = x[i * 2];
        let x1 = x[i * 2 + 1];
        x[i * 2] = x0 * cos_theta - x1 * sin_theta;
        x[i * 2 + 1] = x1 * cos_theta + x0 * sin_theta;
    }
}

fn attention_decode(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    num_heads: usize,
    head_dim: usize,
) -> Vec<f32> {
    // Simplified single-token attention decode
    let mut output = vec![0.0f32; num_heads * head_dim];

    for h in 0..num_heads {
        let q_offset = h * head_dim;
        let q_slice = &q[q_offset..q_offset + head_dim];

        // Dot product with single K (simplified - in practice would use KV cache)
        let k_offset = (h % (k.len() / head_dim)) * head_dim;
        let k_slice = &k[k_offset..k_offset + head_dim];

        let score: f32 = q_slice.iter().zip(k_slice).map(|(q, k)| q * k).sum();
        let scale = 1.0 / (head_dim as f32).sqrt();
        let weight = (score * scale).exp(); // Simplified softmax for single token

        let v_offset = (h % (v.len() / head_dim)) * head_dim;
        let v_slice = &v[v_offset..v_offset + head_dim];

        for i in 0..head_dim {
            output[q_offset + i] = v_slice[i] * weight;
        }
    }

    output
}

// KV Cache simulation
struct KvCache {
    keys: Vec<f32>,
    values: Vec<f32>,
    num_tokens: usize,
    num_kv_heads: usize,
    head_dim: usize,
    max_seq_len: usize,
}

impl KvCache {
    fn new(config: &ModelConfig) -> Self {
        let capacity = config.max_seq_len * config.num_kv_heads * config.head_dim;
        Self {
            keys: vec![0.0; capacity],
            values: vec![0.0; capacity],
            num_tokens: 0,
            num_kv_heads: config.num_kv_heads,
            head_dim: config.head_dim,
            max_seq_len: config.max_seq_len,
        }
    }

    fn append(&mut self, k: &[f32], v: &[f32]) {
        if self.num_tokens >= self.max_seq_len {
            return;
        }

        let stride = self.num_kv_heads * self.head_dim;
        let offset = self.num_tokens * stride;

        self.keys[offset..offset + stride].copy_from_slice(&k[..stride.min(k.len())]);
        self.values[offset..offset + stride].copy_from_slice(&v[..stride.min(v.len())]);
        self.num_tokens += 1;
    }

    fn memory_bytes(&self) -> usize {
        (self.keys.len() + self.values.len()) * std::mem::size_of::<f32>()
    }
}

// === Benchmark Functions ===

fn bench_single_layer_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_layer_forward");
    group.sample_size(30);

    let configs = [
        ("llama2_7b", ModelConfig::llama2_7b()),
        ("llama3_8b", ModelConfig::llama3_8b()),
        ("mistral_7b", ModelConfig::mistral_7b()),
    ];

    for (name, config) in configs {
        let layer = TransformerLayer::new(config);
        let mut hidden_state = random_tensor(config.hidden_size);

        let id = BenchmarkId::new(name, config.hidden_size);

        group.throughput(Throughput::Elements(config.params_per_layer() as u64));
        group.bench_function(id, |b| {
            b.iter(|| {
                let mut h = hidden_state.clone();
                layer.forward_single_token(black_box(&mut h), 100);
                h
            })
        });
    }

    group.finish();
}

fn bench_multi_layer_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_layer_forward");
    group.sample_size(20);

    let config = ModelConfig::llama2_7b();

    for num_layers in [1, 4, 8, 16, 32] {
        let layers: Vec<TransformerLayer> = (0..num_layers)
            .map(|_| TransformerLayer::new(config))
            .collect();
        let mut hidden_state = random_tensor(config.hidden_size);

        let id = BenchmarkId::new(format!("{}_layers", num_layers), num_layers);

        group.throughput(Throughput::Elements(
            (config.params_per_layer() * num_layers) as u64,
        ));
        group.bench_function(id, |b| {
            b.iter(|| {
                let mut h = hidden_state.clone();
                for layer in &layers {
                    layer.forward_single_token(black_box(&mut h), 100);
                }
                h
            })
        });
    }

    group.finish();
}

fn bench_kv_cache_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("kv_cache");
    group.sample_size(50);

    let configs = [
        ("llama2_7b", ModelConfig::llama2_7b()),
        ("llama3_8b", ModelConfig::llama3_8b()),
    ];

    for (name, config) in configs {
        // Append operation
        let mut cache = KvCache::new(&config);
        let k = random_tensor(config.num_kv_heads * config.head_dim);
        let v = random_tensor(config.num_kv_heads * config.head_dim);

        group.bench_function(
            BenchmarkId::new(format!("{}_append", name), config.num_kv_heads),
            |b| {
                b.iter_batched(
                    || KvCache::new(&config),
                    |mut cache| {
                        cache.append(black_box(&k), black_box(&v));
                        cache
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );

        // Memory footprint at various sequence lengths
        for seq_len in [256, 512, 1024, 2048] {
            let mut cache = KvCache::new(&config);
            for _ in 0..seq_len {
                cache.append(&k, &v);
            }

            let memory_mb = cache.memory_bytes() / (1024 * 1024);
            let id = BenchmarkId::new(format!("{}_seq_{}_{}MB", name, seq_len, memory_mb), seq_len);

            group.throughput(Throughput::Bytes(cache.memory_bytes() as u64));
            group.bench_function(id, |b| {
                b.iter(|| {
                    let mut c = KvCache::new(&config);
                    for _ in 0..seq_len {
                        c.append(black_box(&k), black_box(&v));
                    }
                    c
                })
            });
        }
    }

    group.finish();
}

fn bench_decode_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_throughput");
    group.sample_size(20);

    // Measure tokens per second for decode phase
    let config = ModelConfig::llama2_7b();
    let layers: Vec<TransformerLayer> = (0..config.num_layers)
        .map(|_| TransformerLayer::new(config))
        .collect();

    // Simulate decoding multiple tokens
    for num_tokens in [1, 10, 50, 100] {
        let id = BenchmarkId::new(format!("{}_tokens", num_tokens), num_tokens);

        group.throughput(Throughput::Elements(num_tokens as u64));
        group.bench_function(id, |b| {
            b.iter(|| {
                let mut hidden_state = random_tensor(config.hidden_size);
                for token_idx in 0..num_tokens {
                    for layer in &layers {
                        layer.forward_single_token(black_box(&mut hidden_state), token_idx);
                    }
                }
                hidden_state
            })
        });
    }

    group.finish();
}

fn bench_prefill_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("prefill_latency");
    group.sample_size(10);

    // Simulate prefill phase (processing prompt)
    let config = ModelConfig::llama2_7b();
    let layer = TransformerLayer::new(config);

    for seq_len in [32, 64, 128, 256] {
        // Process multiple tokens (simplified - in practice would batch)
        let id = BenchmarkId::new(format!("seq_{}", seq_len), seq_len);

        group.throughput(Throughput::Elements(seq_len as u64));
        group.bench_function(id, |b| {
            b.iter(|| {
                let mut total_output = vec![0.0f32; config.hidden_size];
                for pos in 0..seq_len {
                    let mut hidden_state = random_tensor(config.hidden_size);
                    layer.forward_single_token(black_box(&mut hidden_state), pos);
                    // Accumulate (simplified)
                    for i in 0..config.hidden_size {
                        total_output[i] += hidden_state[i] / seq_len as f32;
                    }
                }
                total_output
            })
        });
    }

    group.finish();
}

fn bench_model_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("model_memory_estimate");
    group.sample_size(20);

    let configs = [
        ("llama2_7b", ModelConfig::llama2_7b()),
        ("llama2_13b", ModelConfig::llama2_13b()),
        ("llama3_8b", ModelConfig::llama3_8b()),
        ("mistral_7b", ModelConfig::mistral_7b()),
    ];

    for (name, config) in configs {
        let fp16_gb = config.memory_bytes_fp16() as f64 / (1024.0 * 1024.0 * 1024.0);
        let int4_gb = config.memory_bytes_int4() as f64 / (1024.0 * 1024.0 * 1024.0);

        println!(
            "{}: FP16={:.2}GB, INT4={:.2}GB, params={}M",
            name,
            fp16_gb,
            int4_gb,
            config.total_params() / 1_000_000
        );

        // Benchmark single layer to estimate per-layer latency
        let layer = TransformerLayer::new(config);
        let mut hidden_state = random_tensor(config.hidden_size);

        let id = BenchmarkId::new(
            format!("{}_fp16_{:.1}GB", name, fp16_gb),
            config.total_params(),
        );

        group.throughput(Throughput::Elements(config.params_per_layer() as u64));
        group.bench_function(id, |b| {
            b.iter(|| {
                let mut h = hidden_state.clone();
                layer.forward_single_token(black_box(&mut h), 100);
                h
            })
        });
    }

    group.finish();
}

fn bench_inference_components(c: &mut Criterion) {
    let mut group = c.benchmark_group("inference_components");
    group.sample_size(50);

    let config = ModelConfig::llama2_7b();
    let hidden = config.hidden_size;
    let intermediate = config.intermediate_size;

    // Individual component benchmarks
    let input = random_tensor(hidden);
    let weight = random_tensor(hidden);

    // RMSNorm
    group.bench_function("rmsnorm_4096", |b| {
        b.iter_batched(
            || input.clone(),
            |mut x| {
                rms_norm_inplace(black_box(&mut x), black_box(&weight), 1e-6);
                x
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Linear projection (hidden -> hidden)
    let proj_matrix = random_tensor(hidden * hidden);
    group.bench_function("linear_4096x4096", |b| {
        b.iter(|| gemv(black_box(&proj_matrix), black_box(&input), hidden, hidden))
    });

    // Linear projection (hidden -> intermediate)
    let mlp_up_matrix = random_tensor(hidden * intermediate);
    group.bench_function("linear_4096x11008", |b| {
        b.iter(|| {
            gemv(
                black_box(&mlp_up_matrix),
                black_box(&input),
                hidden,
                intermediate,
            )
        })
    });

    // RoPE
    let mut rope_input = random_tensor(config.num_attention_heads * config.head_dim);
    group.bench_function("rope_32heads", |b| {
        b.iter_batched(
            || rope_input.clone(),
            |mut x| {
                for h in 0..config.num_attention_heads {
                    let offset = h * config.head_dim;
                    apply_rope_simple(
                        black_box(&mut x[offset..offset + config.head_dim]),
                        config.head_dim,
                        100,
                    );
                }
                x
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_tokens_per_second_estimation(c: &mut Criterion) {
    let mut group = c.benchmark_group("tokens_per_second");
    group.sample_size(10);

    // Full model throughput estimation
    let config = ModelConfig::llama2_7b();

    // Create a simplified full model
    let layers: Vec<TransformerLayer> = (0..4) // Use 4 layers for faster benchmarking
        .map(|_| TransformerLayer::new(config))
        .collect();

    let id = BenchmarkId::new("llama2_7b_4layers", 4);

    // Time how long it takes to process tokens
    group.bench_function(id, |b| {
        b.iter_custom(|iters| {
            let mut total_time = std::time::Duration::ZERO;

            for _ in 0..iters {
                let mut hidden_state = random_tensor(config.hidden_size);
                let start = Instant::now();

                for layer in &layers {
                    layer.forward_single_token(black_box(&mut hidden_state), 100);
                }

                total_time += start.elapsed();
            }

            total_time
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_layer_forward,
    bench_multi_layer_forward,
    bench_kv_cache_operations,
    bench_decode_throughput,
    bench_prefill_latency,
    bench_model_memory,
    bench_inference_components,
    bench_tokens_per_second_estimation,
);

criterion_main!(benches);
