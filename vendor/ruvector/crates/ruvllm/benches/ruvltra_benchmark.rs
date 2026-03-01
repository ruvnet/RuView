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
//! RuvLTRA-Small Model Benchmark Suite
//!
//! Comprehensive benchmarks for the RuvLTRA-Small (0.5B parameter) model
//! optimized for Apple Silicon M4 Pro.
//!
//! ## Performance Targets (M4 Pro)
//!
//! | Metric | Target | Notes |
//! |--------|--------|-------|
//! | Decode throughput (Q4) | 80+ tok/s | Single stream |
//! | First token latency | <50ms | Cold start |
//! | Memory usage (Q4) | <500MB | Model + KV cache |
//! | Prefill throughput | 2000+ tok/s | Batch=1 |
//!
//! ## Benchmark Scenarios
//!
//! 1. **Short prompt (32 tokens) -> 128 token output**
//!    - Prefill latency, decode throughput, E2E latency
//!
//! 2. **Medium prompt (256 tokens) -> 256 token output**
//!    - Sustained throughput, memory pressure
//!
//! 3. **Long prompt (1024 tokens) -> 512 token output**
//!    - KV cache scaling, attention efficiency
//!
//! ## Backend Comparison
//!
//! - Pure NEON (CPU SIMD baseline)
//! - Pure ANE (Apple Neural Engine via CoreML)
//! - Hybrid (ANE matmul + NEON activations)
//! - Metal GPU
//!
//! ## Quantization Comparison
//!
//! - Q4_K_M: 4-bit quantization, medium quality
//! - Q5_K_M: 5-bit quantization, high quality
//! - Q8_0: 8-bit quantization, highest quality
//!
//! ## Running Benchmarks
//!
//! ```bash
//! # Full benchmark suite
//! cargo bench -p ruvllm --bench ruvltra_benchmark
//!
//! # Specific scenario
//! cargo bench -p ruvllm --bench ruvltra_benchmark -- short_prompt
//!
//! # With Metal GPU
//! cargo bench -p ruvllm --features metal-compute --bench ruvltra_benchmark
//!
//! # With ANE
//! cargo bench -p ruvllm --features coreml --bench ruvltra_benchmark
//!
//! # With parallel execution
//! cargo bench -p ruvllm --features parallel --bench ruvltra_benchmark
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;
use std::alloc::{alloc, dealloc, Layout};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// ============================================================================
// RuvLTRA-Small Model Configuration
// ============================================================================

/// RuvLTRA-Small model configuration (0.5B parameters)
///
/// Architecture: LLaMA-style with optimizations for edge deployment
/// - 24 layers (reduced from 32 for 7B)
/// - 2048 hidden dimension
/// - 5632 intermediate dimension (2.75x hidden)
/// - 16 attention heads
/// - 4 KV heads (GQA 4:1)
/// - 128 head dimension
/// - 32000 vocab size
/// - 4096 max context
#[derive(Debug, Clone, Copy)]
pub struct RuvLtraSmallConfig {
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub num_attention_heads: usize,
    pub num_kv_heads: usize,
    pub head_dim: usize,
    pub num_layers: usize,
    pub vocab_size: usize,
    pub max_seq_len: usize,
    pub rope_theta: f32,
}

impl Default for RuvLtraSmallConfig {
    fn default() -> Self {
        Self {
            hidden_size: 2048,
            intermediate_size: 5632,
            num_attention_heads: 16,
            num_kv_heads: 4, // GQA 4:1
            head_dim: 128,
            num_layers: 24,
            vocab_size: 32000,
            max_seq_len: 4096,
            rope_theta: 10000.0,
        }
    }
}

impl RuvLtraSmallConfig {
    /// Total parameters (approximate)
    pub fn total_params(&self) -> usize {
        // Embedding: vocab * hidden
        let embed_params = self.vocab_size * self.hidden_size;

        // Per layer:
        // - QKV projection: hidden * (hidden + 2 * kv_hidden)
        // - O projection: hidden * hidden
        // - MLP: hidden * intermediate * 3
        // - Norms: hidden * 2
        let kv_hidden = self.num_kv_heads * self.head_dim;
        let attn_params = self.hidden_size * self.hidden_size  // Q
            + self.hidden_size * kv_hidden * 2                  // K, V
            + self.hidden_size * self.hidden_size; // O
        let mlp_params = self.hidden_size * self.intermediate_size * 3;
        let norm_params = self.hidden_size * 2;
        let layer_params = attn_params + mlp_params + norm_params;

        // Final: LM head + norm
        let final_params = self.vocab_size * self.hidden_size + self.hidden_size;

        embed_params + layer_params * self.num_layers + final_params
    }

    /// Memory in bytes for different quantization levels
    pub fn memory_bytes(&self, quant: QuantFormat) -> usize {
        let params = self.total_params();
        match quant {
            QuantFormat::F16 => params * 2,
            QuantFormat::Q8_0 => params,
            QuantFormat::Q5_K_M => (params * 5 + 7) / 8 + params / 32 * 2, // 5 bits + scales
            QuantFormat::Q4_K_M => params / 2 + params / 32 * 2,           // 4 bits + scales
        }
    }

    /// KV cache memory for given sequence length
    pub fn kv_cache_bytes(&self, seq_len: usize, quant: QuantFormat) -> usize {
        let kv_elements = seq_len * self.num_kv_heads * self.head_dim * 2 * self.num_layers;
        match quant {
            QuantFormat::F16 => kv_elements * 2,
            QuantFormat::Q8_0 => kv_elements,
            QuantFormat::Q5_K_M => (kv_elements * 5 + 7) / 8,
            QuantFormat::Q4_K_M => kv_elements / 2,
        }
    }
}

/// Quantization format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantFormat {
    F16,
    Q8_0,
    Q5_K_M,
    Q4_K_M,
}

impl QuantFormat {
    pub fn name(&self) -> &'static str {
        match self {
            QuantFormat::F16 => "F16",
            QuantFormat::Q8_0 => "Q8_0",
            QuantFormat::Q5_K_M => "Q5_K_M",
            QuantFormat::Q4_K_M => "Q4_K_M",
        }
    }

    /// Bits per weight
    pub fn bits(&self) -> f32 {
        match self {
            QuantFormat::F16 => 16.0,
            QuantFormat::Q8_0 => 8.0,
            QuantFormat::Q5_K_M => 5.5, // includes scales overhead
            QuantFormat::Q4_K_M => 4.5,
        }
    }
}

/// Compute backend
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    PureNeon,
    PureAne,
    Hybrid, // ANE for matmul, NEON for activations
    MetalGpu,
}

impl Backend {
    pub fn name(&self) -> &'static str {
        match self {
            Backend::PureNeon => "NEON",
            Backend::PureAne => "ANE",
            Backend::Hybrid => "Hybrid",
            Backend::MetalGpu => "Metal",
        }
    }
}

// ============================================================================
// Memory Tracking
// ============================================================================

/// Thread-safe memory tracker
static PEAK_MEMORY: AtomicU64 = AtomicU64::new(0);
static CURRENT_MEMORY: AtomicU64 = AtomicU64::new(0);

fn track_alloc(bytes: usize) {
    let prev = CURRENT_MEMORY.fetch_add(bytes as u64, Ordering::SeqCst);
    let current = prev + bytes as u64;
    PEAK_MEMORY.fetch_max(current, Ordering::SeqCst);
}

fn track_dealloc(bytes: usize) {
    CURRENT_MEMORY.fetch_sub(bytes as u64, Ordering::SeqCst);
}

fn reset_memory_tracking() {
    PEAK_MEMORY.store(0, Ordering::SeqCst);
    CURRENT_MEMORY.store(0, Ordering::SeqCst);
}

fn get_peak_memory() -> u64 {
    PEAK_MEMORY.load(Ordering::SeqCst)
}

/// Tracked allocation for memory benchmarking
pub struct TrackedBuffer {
    ptr: *mut u8,
    layout: Layout,
}

impl TrackedBuffer {
    pub fn new(size: usize) -> Self {
        let layout = Layout::from_size_align(size, 64).unwrap();
        let ptr = unsafe { alloc(layout) };
        track_alloc(size);
        Self { ptr, layout }
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.layout.size()) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.layout.size()) }
    }
}

impl Drop for TrackedBuffer {
    fn drop(&mut self) {
        track_dealloc(self.layout.size());
        unsafe { dealloc(self.ptr, self.layout) }
    }
}

// ============================================================================
// Simulated Transformer Operations
// ============================================================================

/// Simulated transformer layer for RuvLTRA-Small
struct RuvLtraLayer {
    config: RuvLtraSmallConfig,
    // Weights (simulated as random data)
    q_proj: Vec<f32>,
    k_proj: Vec<f32>,
    v_proj: Vec<f32>,
    o_proj: Vec<f32>,
    gate_proj: Vec<f32>,
    up_proj: Vec<f32>,
    down_proj: Vec<f32>,
    input_norm: Vec<f32>,
    post_attn_norm: Vec<f32>,
}

impl RuvLtraLayer {
    fn new(config: RuvLtraSmallConfig) -> Self {
        let hidden = config.hidden_size;
        let kv_hidden = config.num_kv_heads * config.head_dim;
        let intermediate = config.intermediate_size;

        Self {
            config,
            q_proj: random_tensor(hidden * hidden),
            k_proj: random_tensor(hidden * kv_hidden),
            v_proj: random_tensor(hidden * kv_hidden),
            o_proj: random_tensor(hidden * hidden),
            gate_proj: random_tensor(hidden * intermediate),
            up_proj: random_tensor(hidden * intermediate),
            down_proj: random_tensor(intermediate * hidden),
            input_norm: random_tensor(hidden),
            post_attn_norm: random_tensor(hidden),
        }
    }

    /// Prefill forward pass (batch of tokens)
    fn prefill(&self, hidden_states: &mut [f32], seq_len: usize, _kv_cache: &mut KvCache) {
        let hidden = self.config.hidden_size;

        for pos in 0..seq_len {
            let offset = pos * hidden;
            let state = &mut hidden_states[offset..offset + hidden];

            // RMSNorm
            rms_norm_inplace(state, &self.input_norm, 1e-6);

            // QKV projection (simplified)
            let q = gemv(&self.q_proj, state, hidden, hidden);
            let _k = gemv(
                &self.k_proj,
                state,
                hidden,
                self.config.num_kv_heads * self.config.head_dim,
            );
            let _v = gemv(
                &self.v_proj,
                state,
                hidden,
                self.config.num_kv_heads * self.config.head_dim,
            );

            // Attention output projection
            let attn_out = gemv(&self.o_proj, &q, hidden, hidden);

            // Residual
            for i in 0..hidden {
                state[i] += attn_out[i];
            }

            // Post-attention norm
            rms_norm_inplace(state, &self.post_attn_norm, 1e-6);

            // MLP
            let gate = gemv(
                &self.gate_proj,
                state,
                hidden,
                self.config.intermediate_size,
            );
            let up = gemv(&self.up_proj, state, hidden, self.config.intermediate_size);

            // SiLU * up
            let mut mlp_out = Vec::with_capacity(self.config.intermediate_size);
            for i in 0..self.config.intermediate_size {
                let silu = gate[i] / (1.0 + (-gate[i]).exp());
                mlp_out.push(silu * up[i]);
            }

            // Down projection
            let down = gemv(
                &self.down_proj,
                &mlp_out,
                self.config.intermediate_size,
                hidden,
            );

            // Residual
            for i in 0..hidden {
                state[i] += down[i];
            }
        }
    }

    /// Decode forward pass (single token)
    fn decode(&self, hidden_state: &mut [f32], kv_cache_len: usize) {
        let hidden = self.config.hidden_size;

        // RMSNorm
        rms_norm_inplace(hidden_state, &self.input_norm, 1e-6);

        // QKV projection
        let mut q = gemv(&self.q_proj, hidden_state, hidden, hidden);
        let _k = gemv(
            &self.k_proj,
            hidden_state,
            hidden,
            self.config.num_kv_heads * self.config.head_dim,
        );
        let _v = gemv(
            &self.v_proj,
            hidden_state,
            hidden,
            self.config.num_kv_heads * self.config.head_dim,
        );

        // RoPE
        apply_rope(
            &mut q,
            self.config.head_dim,
            kv_cache_len,
            self.config.rope_theta,
        );

        // Simplified attention output
        let attn_out = gemv(&self.o_proj, &q, hidden, hidden);

        // Residual
        for i in 0..hidden {
            hidden_state[i] += attn_out[i];
        }

        // Post-attention norm
        rms_norm_inplace(hidden_state, &self.post_attn_norm, 1e-6);

        // MLP
        let gate = gemv(
            &self.gate_proj,
            hidden_state,
            hidden,
            self.config.intermediate_size,
        );
        let up = gemv(
            &self.up_proj,
            hidden_state,
            hidden,
            self.config.intermediate_size,
        );

        let mut mlp_out = Vec::with_capacity(self.config.intermediate_size);
        for i in 0..self.config.intermediate_size {
            let silu = gate[i] / (1.0 + (-gate[i]).exp());
            mlp_out.push(silu * up[i]);
        }

        let down = gemv(
            &self.down_proj,
            &mlp_out,
            self.config.intermediate_size,
            hidden,
        );

        for i in 0..hidden {
            hidden_state[i] += down[i];
        }
    }
}

/// Simple KV cache for benchmarking
struct KvCache {
    keys: Vec<f32>,
    values: Vec<f32>,
    num_tokens: usize,
    config: RuvLtraSmallConfig,
}

impl KvCache {
    fn new(config: RuvLtraSmallConfig, max_seq_len: usize) -> Self {
        let capacity = max_seq_len * config.num_kv_heads * config.head_dim * config.num_layers;
        Self {
            keys: vec![0.0; capacity],
            values: vec![0.0; capacity],
            num_tokens: 0,
            config,
        }
    }

    fn append(&mut self, _k: &[f32], _v: &[f32], _layer: usize) {
        self.num_tokens += 1;
    }

    fn len(&self) -> usize {
        self.num_tokens
    }

    fn memory_bytes(&self) -> usize {
        (self.keys.len() + self.values.len()) * std::mem::size_of::<f32>()
    }
}

/// Full model for benchmarking
struct RuvLtraModel {
    config: RuvLtraSmallConfig,
    layers: Vec<RuvLtraLayer>,
    embed_weights: Vec<f32>,
    lm_head: Vec<f32>,
    final_norm: Vec<f32>,
}

impl RuvLtraModel {
    fn new(config: RuvLtraSmallConfig) -> Self {
        let layers: Vec<_> = (0..config.num_layers)
            .map(|_| RuvLtraLayer::new(config))
            .collect();

        Self {
            config,
            layers,
            embed_weights: random_tensor(config.vocab_size * config.hidden_size),
            lm_head: random_tensor(config.hidden_size * config.vocab_size),
            final_norm: random_tensor(config.hidden_size),
        }
    }

    /// Prefill phase: process prompt
    fn prefill(&self, tokens: &[u32], kv_cache: &mut KvCache) -> Vec<f32> {
        let seq_len = tokens.len();
        let hidden = self.config.hidden_size;

        // Embed tokens
        let mut hidden_states = vec![0.0f32; seq_len * hidden];
        for (i, &token) in tokens.iter().enumerate() {
            let offset = (token as usize % self.config.vocab_size) * hidden;
            hidden_states[i * hidden..(i + 1) * hidden]
                .copy_from_slice(&self.embed_weights[offset..offset + hidden]);
        }

        // Forward through layers
        for layer in &self.layers {
            layer.prefill(&mut hidden_states, seq_len, kv_cache);
        }

        // Return last position's hidden state
        hidden_states[(seq_len - 1) * hidden..].to_vec()
    }

    /// Decode phase: generate single token
    fn decode(&self, prev_token: u32, kv_cache: &mut KvCache) -> u32 {
        let hidden = self.config.hidden_size;

        // Embed token
        let offset = (prev_token as usize % self.config.vocab_size) * hidden;
        let mut hidden_state = self.embed_weights[offset..offset + hidden].to_vec();

        // Forward through layers
        let kv_len = kv_cache.len();
        for layer in &self.layers {
            layer.decode(&mut hidden_state, kv_len);
        }

        // Final norm
        rms_norm_inplace(&mut hidden_state, &self.final_norm, 1e-6);

        // LM head (simplified - just pick argmax of first 100 logits)
        let logits = gemv(&self.lm_head[..hidden * 100], &hidden_state, hidden, 100);
        logits
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i as u32)
            .unwrap_or(0)
    }

    /// E2E inference: prefill + decode
    fn generate(
        &self,
        prompt_tokens: &[u32],
        max_new_tokens: usize,
    ) -> (Duration, Duration, Vec<u32>) {
        let mut kv_cache = KvCache::new(self.config, self.config.max_seq_len);

        // Prefill
        let prefill_start = Instant::now();
        let _last_hidden = self.prefill(prompt_tokens, &mut kv_cache);
        let prefill_time = prefill_start.elapsed();

        // Decode
        let mut output_tokens = Vec::with_capacity(max_new_tokens);
        let mut prev_token = prompt_tokens.last().copied().unwrap_or(0);

        let decode_start = Instant::now();
        for _ in 0..max_new_tokens {
            let next_token = self.decode(prev_token, &mut kv_cache);
            output_tokens.push(next_token);
            prev_token = next_token;
            kv_cache.num_tokens += 1;
        }
        let decode_time = decode_start.elapsed();

        (prefill_time, decode_time, output_tokens)
    }
}

// ============================================================================
// SONA Integration Benchmarks
// ============================================================================

/// Simulated SONA instant loop overhead measurement
struct SonaOverhead {
    trajectory_buffer: Vec<f32>,
    pattern_cache: Vec<f32>,
    ewc_fisher: Vec<f32>,
}

impl SonaOverhead {
    fn new(hidden_dim: usize) -> Self {
        Self {
            trajectory_buffer: Vec::with_capacity(1024 * hidden_dim),
            pattern_cache: random_tensor(100 * hidden_dim),
            ewc_fisher: random_tensor(hidden_dim),
        }
    }

    /// Measure instant loop overhead (<1ms target)
    fn instant_loop(&mut self, query_embedding: &[f32], quality_score: f32) -> Duration {
        let start = Instant::now();

        // 1. Store trajectory (ring buffer append)
        self.trajectory_buffer.extend_from_slice(query_embedding);
        if self.trajectory_buffer.len() > 1024 * query_embedding.len() {
            self.trajectory_buffer.drain(0..query_embedding.len());
        }

        // 2. Update micro-LoRA (simplified gradient step)
        let lr = 0.01 * quality_score;
        for (i, x) in query_embedding.iter().enumerate() {
            if i < self.ewc_fisher.len() {
                self.ewc_fisher[i] += lr * x * x;
            }
        }

        // 3. Pattern similarity search (simplified)
        let _similarity: f32 = self
            .pattern_cache
            .chunks(query_embedding.len())
            .take(10)
            .map(|p| {
                p.iter()
                    .zip(query_embedding)
                    .map(|(a, b)| a * b)
                    .sum::<f32>()
            })
            .sum();

        start.elapsed()
    }

    /// Measure pattern retrieval latency
    fn pattern_search(&self, query: &[f32], k: usize) -> Duration {
        let start = Instant::now();

        let mut scores: Vec<(usize, f32)> = self
            .pattern_cache
            .chunks(query.len())
            .enumerate()
            .map(|(i, p)| {
                let sim: f32 = p.iter().zip(query).map(|(a, b)| a * b).sum();
                (i, sim)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        black_box(&scores[..k.min(scores.len())]);

        start.elapsed()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn random_tensor(size: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..size).map(|_| rng.gen_range(-0.1..0.1)).collect()
}

fn rms_norm_inplace(x: &mut [f32], weight: &[f32], eps: f32) {
    let sum_sq: f32 = x.iter().map(|v| v * v).sum();
    let inv_rms = 1.0 / (sum_sq / x.len() as f32 + eps).sqrt();
    for (i, w) in weight.iter().enumerate().take(x.len()) {
        x[i] = x[i] * inv_rms * w;
    }
}

fn gemv(matrix: &[f32], vector: &[f32], m: usize, n: usize) -> Vec<f32> {
    let mut output = vec![0.0f32; n];

    #[cfg(target_arch = "aarch64")]
    unsafe {
        gemv_neon_impl(matrix, vector, &mut output, m, n);
    }

    #[cfg(not(target_arch = "aarch64"))]
    {
        for j in 0..n {
            let mut sum = 0.0f32;
            for i in 0..m {
                sum += matrix[i * n + j] * vector[i];
            }
            output[j] = sum;
        }
    }

    output
}

#[cfg(target_arch = "aarch64")]
unsafe fn gemv_neon_impl(matrix: &[f32], vector: &[f32], output: &mut [f32], m: usize, n: usize) {
    use std::arch::aarch64::*;

    let m_ptr = matrix.as_ptr();
    let v_ptr = vector.as_ptr();
    let o_ptr = output.as_mut_ptr();

    let mut j = 0usize;
    while j + 4 <= n {
        let mut acc = vdupq_n_f32(0.0);

        for i in 0..m {
            let v_val = vdupq_n_f32(*v_ptr.add(i));
            let m_v = vld1q_f32(m_ptr.add(i * n + j));
            acc = vfmaq_f32(acc, v_val, m_v);
        }

        vst1q_f32(o_ptr.add(j), acc);
        j += 4;
    }

    while j < n {
        let mut sum = 0.0f32;
        for i in 0..m {
            sum += *m_ptr.add(i * n + j) * *v_ptr.add(i);
        }
        *o_ptr.add(j) = sum;
        j += 1;
    }
}

fn apply_rope(x: &mut [f32], head_dim: usize, position: usize, theta: f32) {
    let half_dim = head_dim / 2;
    for i in 0..half_dim {
        let freq = 1.0 / theta.powf((2 * i) as f32 / head_dim as f32);
        let angle = position as f32 * freq;
        let cos_theta = angle.cos();
        let sin_theta = angle.sin();

        if i * 2 + 1 < x.len() {
            let x0 = x[i * 2];
            let x1 = x[i * 2 + 1];
            x[i * 2] = x0 * cos_theta - x1 * sin_theta;
            x[i * 2 + 1] = x1 * cos_theta + x0 * sin_theta;
        }
    }
}

// ============================================================================
// Benchmark Functions
// ============================================================================

/// Benchmark prefill phase (prompt processing)
fn bench_prefill(c: &mut Criterion) {
    let mut group = c.benchmark_group("ruvltra_prefill");
    group.sample_size(20);

    let config = RuvLtraSmallConfig::default();
    let model = RuvLtraModel::new(config);

    // Test different prompt lengths
    let prompt_lengths = [32, 256, 1024];

    for &prompt_len in &prompt_lengths {
        let prompt_tokens: Vec<u32> = (0..prompt_len).map(|i| i as u32 % 32000).collect();
        let mut kv_cache = KvCache::new(config, config.max_seq_len);

        let throughput = prompt_len as u64;
        let id = BenchmarkId::new(format!("seq_{}", prompt_len), prompt_len);

        group.throughput(Throughput::Elements(throughput));
        group.bench_function(id, |b| {
            b.iter(|| {
                kv_cache.num_tokens = 0;
                model.prefill(black_box(&prompt_tokens), black_box(&mut kv_cache))
            })
        });
    }

    group.finish();
}

/// Benchmark decode phase (token generation)
fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("ruvltra_decode");
    group.sample_size(50);

    let config = RuvLtraSmallConfig::default();
    let model = RuvLtraModel::new(config);

    // Test with different KV cache lengths
    let kv_lengths = [32, 256, 1024];

    for &kv_len in &kv_lengths {
        let mut kv_cache = KvCache::new(config, config.max_seq_len);
        kv_cache.num_tokens = kv_len;

        let id = BenchmarkId::new(format!("kv_len_{}", kv_len), kv_len);

        group.throughput(Throughput::Elements(1)); // 1 token per iteration
        group.bench_function(id, |b| {
            b.iter(|| model.decode(black_box(42), black_box(&mut kv_cache)))
        });
    }

    group.finish();
}

/// Benchmark E2E latency (first token + total time)
fn bench_e2e_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("ruvltra_e2e_latency");
    group.sample_size(10);

    let config = RuvLtraSmallConfig::default();
    let model = RuvLtraModel::new(config);

    // Benchmark scenarios
    let scenarios = [
        ("short", 32, 128),   // Short prompt -> 128 tokens
        ("medium", 256, 256), // Medium prompt -> 256 tokens
        ("long", 1024, 512),  // Long prompt -> 512 tokens
    ];

    for (name, prompt_len, output_len) in scenarios {
        let prompt_tokens: Vec<u32> = (0..prompt_len).map(|i| i as u32 % 32000).collect();

        let id = BenchmarkId::new(
            format!("{}_p{}_o{}", name, prompt_len, output_len),
            prompt_len,
        );

        group.throughput(Throughput::Elements((prompt_len + output_len) as u64));
        group.bench_function(id, |b| {
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    let (prefill, decode, _) =
                        model.generate(black_box(&prompt_tokens), output_len);
                    total += prefill + decode;
                }
                total
            })
        });
    }

    group.finish();
}

/// Benchmark throughput (tokens/sec)
fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("ruvltra_throughput");
    group.sample_size(10);

    let config = RuvLtraSmallConfig::default();
    let model = RuvLtraModel::new(config);

    // Measure decode throughput at different batch points
    let decode_batches = [10, 50, 100];

    for &num_tokens in &decode_batches {
        let mut kv_cache = KvCache::new(config, config.max_seq_len);
        kv_cache.num_tokens = 256; // Assume 256 context

        let id = BenchmarkId::new(format!("decode_{}_tokens", num_tokens), num_tokens);

        group.throughput(Throughput::Elements(num_tokens as u64));
        group.bench_function(id, |b| {
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    let start = Instant::now();
                    let mut prev_token = 42u32;
                    for _ in 0..num_tokens {
                        prev_token = model.decode(black_box(prev_token), black_box(&mut kv_cache));
                    }
                    total += start.elapsed();
                    kv_cache.num_tokens = 256; // Reset
                }
                total
            })
        });
    }

    group.finish();
}

/// Benchmark memory usage
fn bench_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("ruvltra_memory");
    group.sample_size(20);

    let config = RuvLtraSmallConfig::default();

    // Print memory estimates
    println!("\n=== RuvLTRA-Small Memory Estimates ===");
    println!("Total parameters: {}M", config.total_params() / 1_000_000);

    for quant in [
        QuantFormat::F16,
        QuantFormat::Q8_0,
        QuantFormat::Q5_K_M,
        QuantFormat::Q4_K_M,
    ] {
        let model_mb = config.memory_bytes(quant) / (1024 * 1024);
        let kv_1k_mb = config.kv_cache_bytes(1024, quant) / (1024 * 1024);
        let kv_4k_mb = config.kv_cache_bytes(4096, quant) / (1024 * 1024);

        println!(
            "{}: Model={}MB, KV@1K={}MB, KV@4K={}MB, Total@1K={}MB",
            quant.name(),
            model_mb,
            kv_1k_mb,
            kv_4k_mb,
            model_mb + kv_1k_mb
        );
    }
    println!();

    // Benchmark actual allocation patterns
    let seq_lengths = [256, 512, 1024, 2048];

    for &seq_len in &seq_lengths {
        let id = BenchmarkId::new(format!("kv_cache_seq_{}", seq_len), seq_len);

        reset_memory_tracking();

        group.bench_function(id, |b| {
            b.iter(|| {
                let kv_cache = KvCache::new(config, seq_len);
                black_box(kv_cache.memory_bytes())
            })
        });
    }

    group.finish();
}

/// Benchmark quantization comparison
fn bench_quantization(c: &mut Criterion) {
    let mut group = c.benchmark_group("ruvltra_quantization");
    group.sample_size(30);

    let config = RuvLtraSmallConfig::default();

    // Simulate quantized weight loading and dequant
    let hidden = config.hidden_size;
    let weights_f32 = random_tensor(hidden * hidden);

    // Q8_0 simulation
    let weights_q8: Vec<i8> = weights_f32
        .iter()
        .map(|&x| (x * 127.0).clamp(-127.0, 127.0) as i8)
        .collect();

    // Q4 simulation (packed)
    let weights_q4: Vec<u8> = weights_f32
        .chunks(2)
        .map(|chunk| {
            let q0 = ((chunk[0] + 1.0) * 7.5).clamp(0.0, 15.0) as u8;
            let q1 = ((chunk.get(1).copied().unwrap_or(0.0) + 1.0) * 7.5).clamp(0.0, 15.0) as u8;
            (q1 << 4) | q0
        })
        .collect();

    // Benchmark dequantization overhead
    group.bench_function("dequant_q8_0", |b| {
        let scale = 1.0f32 / 127.0;
        b.iter(|| {
            let dequant: Vec<f32> = weights_q8
                .iter()
                .map(|&q| black_box(q as f32 * scale))
                .collect();
            black_box(dequant)
        })
    });

    group.bench_function("dequant_q4_k_m", |b| {
        let scale = 1.0f32 / 7.5;
        b.iter(|| {
            let dequant: Vec<f32> = weights_q4
                .iter()
                .flat_map(|&packed| {
                    let q0 = (packed & 0x0F) as f32 * scale - 1.0;
                    let q1 = ((packed >> 4) & 0x0F) as f32 * scale - 1.0;
                    [q0, q1]
                })
                .collect();
            black_box(dequant)
        })
    });

    group.finish();
}

/// Benchmark SONA overhead
fn bench_sona_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("ruvltra_sona_overhead");
    group.sample_size(100);

    let config = RuvLtraSmallConfig::default();
    let mut sona = SonaOverhead::new(config.hidden_size);

    let query_embedding = random_tensor(config.hidden_size);

    // Instant loop overhead (target: <1ms)
    group.bench_function("instant_loop", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                total += sona.instant_loop(black_box(&query_embedding), 0.8);
            }
            total
        })
    });

    // Pattern retrieval latency
    for k in [5, 10, 20] {
        let id = BenchmarkId::new(format!("pattern_search_top{}", k), k);
        group.bench_function(id, |b| {
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    total += sona.pattern_search(black_box(&query_embedding), k);
                }
                total
            })
        });
    }

    // Combined: with vs without SONA
    let model = RuvLtraModel::new(config);
    let mut kv_cache = KvCache::new(config, config.max_seq_len);
    kv_cache.num_tokens = 256;

    group.bench_function("decode_without_sona", |b| {
        b.iter(|| model.decode(black_box(42), black_box(&mut kv_cache)))
    });

    group.bench_function("decode_with_sona_instant", |b| {
        b.iter(|| {
            let token = model.decode(black_box(42), black_box(&mut kv_cache));
            sona.instant_loop(&query_embedding, 0.8);
            token
        })
    });

    group.finish();
}

/// Benchmark backend comparison (simulated)
fn bench_backend_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("ruvltra_backend_comparison");
    group.sample_size(30);

    let config = RuvLtraSmallConfig::default();
    let hidden = config.hidden_size;

    // Simulate different backend speeds with scaling factors
    // These represent relative performance characteristics
    let matrix_a = random_tensor(hidden * hidden);
    let vector_x = random_tensor(hidden);
    let mut output = vec![0.0f32; hidden];

    // Pure NEON baseline
    group.bench_function("neon_gemv", |b| {
        b.iter(|| {
            gemv(black_box(&matrix_a), black_box(&vector_x), hidden, hidden);
        })
    });

    // Simulated ANE (typically 1.3-1.5x faster for supported ops)
    #[cfg(all(target_os = "macos", feature = "coreml"))]
    {
        group.bench_function("ane_gemv_simulated", |b| {
            b.iter(|| {
                // In practice, this would use ruvllm::kernels::ane_ops
                let result = gemv(black_box(&matrix_a), black_box(&vector_x), hidden, hidden);
                // ANE would have ~30% less overhead in practice
                black_box(result)
            })
        });
    }

    // Simulated hybrid (ANE matmul + NEON activations)
    #[cfg(all(target_os = "macos", feature = "coreml"))]
    {
        group.bench_function("hybrid_layer_simulated", |b| {
            let gate_proj = random_tensor(hidden * config.intermediate_size);
            let up_proj = random_tensor(hidden * config.intermediate_size);
            let down_proj = random_tensor(config.intermediate_size * hidden);

            b.iter(|| {
                // ANE: matmul
                let gate = gemv(&gate_proj, &vector_x, hidden, config.intermediate_size);
                let up = gemv(&up_proj, &vector_x, hidden, config.intermediate_size);

                // NEON: SiLU activation
                let mut intermediate = Vec::with_capacity(config.intermediate_size);
                for i in 0..config.intermediate_size {
                    let silu = gate[i] / (1.0 + (-gate[i]).exp());
                    intermediate.push(silu * up[i]);
                }

                // ANE: matmul
                let output = gemv(&down_proj, &intermediate, config.intermediate_size, hidden);
                black_box(output)
            })
        });
    }

    // Metal GPU comparison placeholder
    #[cfg(all(target_os = "macos", feature = "metal-compute"))]
    {
        group.bench_function("metal_gemv_simulated", |b| {
            // In practice, this would use Metal compute shaders
            b.iter(|| gemv(black_box(&matrix_a), black_box(&vector_x), hidden, hidden))
        });
    }

    group.finish();
}

/// Summary benchmark with target metrics
fn bench_targets_summary(c: &mut Criterion) {
    let mut group = c.benchmark_group("ruvltra_targets");
    group.sample_size(10);

    let config = RuvLtraSmallConfig::default();
    let model = RuvLtraModel::new(config);

    // Target: 80+ tok/s decode (Q4)
    // Measure actual throughput
    {
        let mut kv_cache = KvCache::new(config, config.max_seq_len);
        kv_cache.num_tokens = 256;

        group.bench_function("target_decode_80_toks", |b| {
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    let start = Instant::now();
                    for _ in 0..80 {
                        black_box(model.decode(42, &mut kv_cache));
                    }
                    total += start.elapsed();
                    kv_cache.num_tokens = 256;
                }
                total
            })
        });
    }

    // Target: <50ms first token
    {
        let prompt_tokens: Vec<u32> = (0..256).map(|i| i as u32 % 32000).collect();

        group.bench_function("target_first_token_50ms", |b| {
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    let mut kv_cache = KvCache::new(config, config.max_seq_len);
                    let start = Instant::now();
                    black_box(model.prefill(&prompt_tokens, &mut kv_cache));
                    black_box(
                        model.decode(prompt_tokens.last().copied().unwrap_or(0), &mut kv_cache),
                    );
                    total += start.elapsed();
                }
                total
            })
        });
    }

    // Memory target: <500MB for Q4
    {
        let model_mem = config.memory_bytes(QuantFormat::Q4_K_M);
        let kv_mem = config.kv_cache_bytes(1024, QuantFormat::Q4_K_M);
        let total_mb = (model_mem + kv_mem) / (1024 * 1024);

        println!("\n=== Memory Target Check ===");
        println!("Q4_K_M model: {} MB", model_mem / (1024 * 1024));
        println!("KV cache @1K: {} MB", kv_mem / (1024 * 1024));
        println!("Total: {} MB (target: <500MB)", total_mb);
        println!("Status: {}", if total_mb < 500 { "PASS" } else { "FAIL" });
        println!();
    }

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    name = prefill_benches;
    config = Criterion::default()
        .significance_level(0.05)
        .noise_threshold(0.02);
    targets = bench_prefill
);

criterion_group!(
    name = decode_benches;
    config = Criterion::default()
        .significance_level(0.05)
        .noise_threshold(0.02);
    targets = bench_decode
);

criterion_group!(
    name = e2e_benches;
    config = Criterion::default()
        .significance_level(0.05)
        .noise_threshold(0.05);
    targets = bench_e2e_latency, bench_throughput
);

criterion_group!(
    name = memory_benches;
    config = Criterion::default()
        .significance_level(0.05);
    targets = bench_memory, bench_quantization
);

criterion_group!(
    name = sona_benches;
    config = Criterion::default()
        .significance_level(0.05)
        .noise_threshold(0.02);
    targets = bench_sona_overhead
);

criterion_group!(
    name = backend_benches;
    config = Criterion::default()
        .significance_level(0.05);
    targets = bench_backend_comparison
);

criterion_group!(
    name = target_benches;
    config = Criterion::default()
        .significance_level(0.05)
        .sample_size(10);
    targets = bench_targets_summary
);

criterion_main!(
    prefill_benches,
    decode_benches,
    e2e_benches,
    memory_benches,
    sona_benches,
    backend_benches,
    target_benches
);
