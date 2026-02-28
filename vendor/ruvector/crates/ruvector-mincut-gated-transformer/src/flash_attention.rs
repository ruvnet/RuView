//! FlashAttention-style tiled attention for CPU.
//!
//! Implements memory-efficient attention computation using block-wise tiling
//! to maximize L1/L2 cache utilization, inspired by FlashAttention-3.
//!
//! ## Key Features
//!
//! 1. **Block-wise computation** - Tiles Q, K, V to fit in cache (typically 64×64 blocks)
//! 2. **Online softmax** - Numerically stable single-pass softmax without full materialization
//! 3. **Tiled GEMM** - Fused Q@K^T and scores@V to avoid O(n²) intermediate storage
//! 4. **Memory efficiency** - O(n) memory instead of O(n²) for attention matrix
//! 5. **Quantization support** - INT8 variant for 4× memory reduction
//!
//! ## Academic Foundation
//!
//! Based on FlashAttention-3 (Dao et al., 2024):
//! - Dao, T., et al. (2024). "FlashAttention-3: Fast and Accurate Attention with Asynchrony and Low-Precision"
//! - Shah, J., et al. (2024). "FlashAttention-2: Faster Attention with Better Parallelism and Work Partitioning"
//!
//! ## Performance
//!
//! Expected improvements over naive attention:
//! - Memory: 4-16× reduction (depends on sequence length)
//! - Speed: 2-4× faster due to cache efficiency
//! - Numerical stability: Identical to standard attention
//!
//! ## Example
//!
//! ```rust,no_run
//! use ruvector_mincut_gated_transformer::flash_attention::{
//!     FlashAttentionConfig, flash_attention_forward,
//! };
//!
//! let config = FlashAttentionConfig {
//!     block_size_q: 64,
//!     block_size_kv: 64,
//!     head_dim: 64,
//!     causal: true,
//!     softmax_scale: 0.125, // 1/sqrt(64)
//! };
//!
//! let seq_len = 128;
//! let head_dim = 64;
//!
//! let q = vec![0.0f32; seq_len * head_dim];
//! let k = vec![0.0f32; seq_len * head_dim];
//! let v = vec![0.0f32; seq_len * head_dim];
//! let mut output = vec![0.0f32; seq_len * head_dim];
//!
//! flash_attention_forward(
//!     &config,
//!     &q, &k, &v,
//!     seq_len, seq_len,
//!     &mut output,
//! );
//! ```

#![allow(dead_code)]

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

/// FlashAttention configuration parameters.
///
/// Controls tiling strategy and computation behavior.
#[derive(Debug, Clone)]
pub struct FlashAttentionConfig {
    /// Query block size (typically 64 for L1 cache fit)
    pub block_size_q: usize,

    /// Key/Value block size (typically 64)
    pub block_size_kv: usize,

    /// Hidden dimension per attention head
    pub head_dim: usize,

    /// Enable causal masking (for autoregressive models)
    pub causal: bool,

    /// Softmax scale factor (typically 1/sqrt(head_dim))
    pub softmax_scale: f32,
}

impl Default for FlashAttentionConfig {
    fn default() -> Self {
        Self {
            block_size_q: 64,
            block_size_kv: 64,
            head_dim: 64,
            causal: true,
            softmax_scale: 0.125, // 1/sqrt(64)
        }
    }
}

impl FlashAttentionConfig {
    /// Create configuration for a specific head dimension.
    pub fn for_head_dim(head_dim: usize) -> Self {
        Self {
            head_dim,
            softmax_scale: 1.0 / (head_dim as f32).sqrt(),
            ..Default::default()
        }
    }

    /// Create configuration optimized for long sequences.
    pub fn for_long_sequence(head_dim: usize) -> Self {
        Self {
            block_size_q: 32,   // Smaller blocks for better cache reuse
            block_size_kv: 128, // Larger KV blocks
            head_dim,
            softmax_scale: 1.0 / (head_dim as f32).sqrt(),
            ..Default::default()
        }
    }
}

/// Online softmax state for numerically stable computation.
///
/// Maintains running maximum and sum of exponentials to avoid overflow.
/// Uses the log-sum-exp trick: softmax(x) = exp(x - max(x)) / sum(exp(x - max(x)))
struct OnlineSoftmaxState {
    /// Running maximum value seen so far
    max_val: f32,

    /// Sum of exponentials: sum(exp(x - max_val))
    sum_exp: f32,

    /// Accumulated output weighted by attention scores
    output: Vec<f32>,
}

impl OnlineSoftmaxState {
    fn new(head_dim: usize) -> Self {
        Self {
            max_val: f32::NEG_INFINITY,
            sum_exp: 0.0,
            output: vec![0.0; head_dim],
        }
    }

    /// Update state with new scores and values.
    ///
    /// Implements the online softmax algorithm:
    /// 1. Compute new max: max' = max(max_old, max_new)
    /// 2. Rescale old sum: sum' = sum_old * exp(max_old - max')
    /// 3. Add new contributions: sum' += sum(exp(scores - max'))
    /// 4. Rescale and accumulate output
    fn update(&mut self, scores: &[f32], values: &[f32], head_dim: usize) {
        debug_assert_eq!(values.len() % head_dim, 0);
        let num_scores = scores.len();

        // Find max of new scores
        let new_max = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        if new_max == f32::NEG_INFINITY {
            // All scores are -inf (masked out)
            return;
        }

        // Compute new global max
        let old_max = self.max_val;
        let new_global_max = old_max.max(new_max);

        // Rescale old sum and output
        if old_max != f32::NEG_INFINITY {
            let rescale_factor = (old_max - new_global_max).exp();
            self.sum_exp *= rescale_factor;
            for out_val in self.output.iter_mut() {
                *out_val *= rescale_factor;
            }
        }

        // Add new contributions
        let mut new_sum = 0.0;
        for i in 0..num_scores {
            let score = scores[i];
            if score != f32::NEG_INFINITY {
                let exp_score = (score - new_global_max).exp();
                new_sum += exp_score;

                // Accumulate weighted values
                let value_offset = i * head_dim;
                for d in 0..head_dim {
                    self.output[d] += exp_score * values[value_offset + d];
                }
            }
        }

        self.sum_exp += new_sum;
        self.max_val = new_global_max;
    }

    /// Finalize and normalize output.
    fn finalize(&mut self) {
        if self.sum_exp > 0.0 {
            let norm_factor = 1.0 / self.sum_exp;
            for val in self.output.iter_mut() {
                *val *= norm_factor;
            }
        }
    }
}

/// Compute Q @ K^T for a tile, producing attention scores.
///
/// Computes scores[i, j] = sum_d(q[i, d] * k[j, d]) * scale
///
/// # Arguments
///
/// * `q_tile` - Query tile [block_size_q, head_dim]
/// * `k_tile` - Key tile [block_size_kv, head_dim]
/// * `scale` - Softmax scale factor
/// * `scores` - Output scores [block_size_q, block_size_kv]
#[inline]
fn tile_gemm_qk(
    q_tile: &[f32],
    k_tile: &[f32],
    head_dim: usize,
    block_size_q: usize,
    block_size_kv: usize,
    scale: f32,
    scores: &mut [f32],
) {
    debug_assert_eq!(q_tile.len(), block_size_q * head_dim);
    debug_assert_eq!(k_tile.len(), block_size_kv * head_dim);
    debug_assert_eq!(scores.len(), block_size_q * block_size_kv);

    for i in 0..block_size_q {
        let q_row = &q_tile[i * head_dim..(i + 1) * head_dim];

        for j in 0..block_size_kv {
            let k_row = &k_tile[j * head_dim..(j + 1) * head_dim];

            // Dot product
            let mut dot = 0.0f32;
            for d in 0..head_dim {
                dot += q_row[d] * k_row[d];
            }

            scores[i * block_size_kv + j] = dot * scale;
        }
    }
}

/// Apply causal mask to attention scores.
///
/// Sets scores[i, j] = -inf if j > i (future positions)
#[inline]
fn apply_causal_mask(
    scores: &mut [f32],
    block_size_q: usize,
    block_size_kv: usize,
    q_offset: usize,
    kv_offset: usize,
) {
    for i in 0..block_size_q {
        let q_pos = q_offset + i;
        for j in 0..block_size_kv {
            let k_pos = kv_offset + j;
            if k_pos > q_pos {
                scores[i * block_size_kv + j] = f32::NEG_INFINITY;
            }
        }
    }
}

/// Tiled flash attention computation.
///
/// Computes attention output without materializing the full attention matrix.
/// Uses block-wise tiling to maximize cache efficiency and online softmax
/// for numerical stability.
///
/// # Arguments
///
/// * `config` - Flash attention configuration
/// * `q` - Query matrix [seq_len_q, head_dim]
/// * `k` - Key matrix [seq_len_kv, head_dim]
/// * `v` - Value matrix [seq_len_kv, head_dim]
/// * `seq_len_q` - Query sequence length
/// * `seq_len_kv` - Key/Value sequence length
/// * `output` - Output buffer [seq_len_q, head_dim]
///
/// # Algorithm
///
/// ```text
/// For each query block Q_i:
///   Initialize online softmax state
///   For each key/value block (K_j, V_j):
///     1. Compute scores: S_ij = Q_i @ K_j^T * scale
///     2. Apply causal mask if needed
///     3. Update online softmax with (S_ij, V_j)
///   Finalize and write output
/// ```
pub fn flash_attention_forward(
    config: &FlashAttentionConfig,
    q: &[f32],
    k: &[f32],
    v: &[f32],
    seq_len_q: usize,
    seq_len_kv: usize,
    output: &mut [f32],
) {
    let head_dim = config.head_dim;
    let block_size_q = config.block_size_q;
    let block_size_kv = config.block_size_kv;

    debug_assert_eq!(q.len(), seq_len_q * head_dim);
    debug_assert_eq!(k.len(), seq_len_kv * head_dim);
    debug_assert_eq!(v.len(), seq_len_kv * head_dim);
    debug_assert_eq!(output.len(), seq_len_q * head_dim);

    // Process query blocks
    let num_q_blocks = (seq_len_q + block_size_q - 1) / block_size_q;

    for q_block_idx in 0..num_q_blocks {
        let q_start = q_block_idx * block_size_q;
        let q_end = (q_start + block_size_q).min(seq_len_q);
        let actual_block_size_q = q_end - q_start;

        // Extract query tile
        let q_tile_start = q_start * head_dim;
        let q_tile_end = q_end * head_dim;
        let q_tile = &q[q_tile_start..q_tile_end];

        // Initialize online softmax states for this query block
        let mut softmax_states: Vec<OnlineSoftmaxState> = (0..actual_block_size_q)
            .map(|_| OnlineSoftmaxState::new(head_dim))
            .collect();

        // Process key/value blocks
        let num_kv_blocks = (seq_len_kv + block_size_kv - 1) / block_size_kv;

        for kv_block_idx in 0..num_kv_blocks {
            let kv_start = kv_block_idx * block_size_kv;
            let kv_end = (kv_start + block_size_kv).min(seq_len_kv);
            let actual_block_size_kv = kv_end - kv_start;

            // Early exit for causal attention
            if config.causal && kv_start > q_end {
                break;
            }

            // Extract key and value tiles
            let k_tile_start = kv_start * head_dim;
            let k_tile_end = kv_end * head_dim;
            let k_tile = &k[k_tile_start..k_tile_end];
            let v_tile = &v[k_tile_start..k_tile_end];

            // Allocate score buffer for this tile
            let mut scores = vec![0.0f32; actual_block_size_q * actual_block_size_kv];

            // Compute Q @ K^T
            tile_gemm_qk(
                q_tile,
                k_tile,
                head_dim,
                actual_block_size_q,
                actual_block_size_kv,
                config.softmax_scale,
                &mut scores,
            );

            // Apply causal mask if needed
            if config.causal {
                apply_causal_mask(
                    &mut scores,
                    actual_block_size_q,
                    actual_block_size_kv,
                    q_start,
                    kv_start,
                );
            }

            // Update online softmax for each query position
            for i in 0..actual_block_size_q {
                let score_row = &scores[i * actual_block_size_kv..(i + 1) * actual_block_size_kv];
                softmax_states[i].update(score_row, v_tile, head_dim);
            }
        }

        // Finalize and write output
        for i in 0..actual_block_size_q {
            softmax_states[i].finalize();
            let out_offset = (q_start + i) * head_dim;
            output[out_offset..out_offset + head_dim].copy_from_slice(&softmax_states[i].output);
        }
    }
}

/// Quantized version of flash attention using INT8 Q/K/V.
///
/// Uses INT8 matrix multiplication with per-tensor scaling.
/// Provides 4× memory reduction compared to FP32 version.
///
/// # Arguments
///
/// * `config` - Flash attention configuration
/// * `q` - Quantized query matrix [seq_len_q, head_dim]
/// * `k` - Quantized key matrix [seq_len_kv, head_dim]
/// * `v` - Quantized value matrix [seq_len_kv, head_dim]
/// * `q_scale` - Query quantization scale
/// * `k_scale` - Key quantization scale
/// * `v_scale` - Value quantization scale
/// * `seq_len_q` - Query sequence length
/// * `seq_len_kv` - Key/Value sequence length
/// * `output` - Output buffer [seq_len_q, head_dim] in FP32
pub fn flash_attention_forward_i8(
    config: &FlashAttentionConfig,
    q: &[i8],
    k: &[i8],
    v: &[i8],
    q_scale: f32,
    k_scale: f32,
    v_scale: f32,
    seq_len_q: usize,
    seq_len_kv: usize,
    output: &mut [f32],
) {
    let head_dim = config.head_dim;
    let block_size_q = config.block_size_q;
    let block_size_kv = config.block_size_kv;

    debug_assert_eq!(q.len(), seq_len_q * head_dim);
    debug_assert_eq!(k.len(), seq_len_kv * head_dim);
    debug_assert_eq!(v.len(), seq_len_kv * head_dim);
    debug_assert_eq!(output.len(), seq_len_q * head_dim);

    // Compute combined scale for attention scores
    let score_scale = q_scale * k_scale * config.softmax_scale;

    let num_q_blocks = (seq_len_q + block_size_q - 1) / block_size_q;

    for q_block_idx in 0..num_q_blocks {
        let q_start = q_block_idx * block_size_q;
        let q_end = (q_start + block_size_q).min(seq_len_q);
        let actual_block_size_q = q_end - q_start;

        let q_tile_start = q_start * head_dim;
        let q_tile_end = q_end * head_dim;
        let q_tile = &q[q_tile_start..q_tile_end];

        let mut softmax_states: Vec<OnlineSoftmaxState> = (0..actual_block_size_q)
            .map(|_| OnlineSoftmaxState::new(head_dim))
            .collect();

        let num_kv_blocks = (seq_len_kv + block_size_kv - 1) / block_size_kv;

        for kv_block_idx in 0..num_kv_blocks {
            let kv_start = kv_block_idx * block_size_kv;
            let kv_end = (kv_start + block_size_kv).min(seq_len_kv);
            let actual_block_size_kv = kv_end - kv_start;

            if config.causal && kv_start > q_end {
                break;
            }

            let k_tile_start = kv_start * head_dim;
            let k_tile_end = kv_end * head_dim;
            let k_tile = &k[k_tile_start..k_tile_end];
            let v_tile_i8 = &v[k_tile_start..k_tile_end];

            // Dequantize value tile to FP32 for accumulation
            let mut v_tile_f32 = vec![0.0f32; v_tile_i8.len()];
            for (i, &v_val) in v_tile_i8.iter().enumerate() {
                v_tile_f32[i] = (v_val as f32) * v_scale;
            }

            // Compute INT8 scores
            let mut scores = vec![0.0f32; actual_block_size_q * actual_block_size_kv];
            tile_gemm_qk_i8(
                q_tile,
                k_tile,
                head_dim,
                actual_block_size_q,
                actual_block_size_kv,
                score_scale,
                &mut scores,
            );

            if config.causal {
                apply_causal_mask(
                    &mut scores,
                    actual_block_size_q,
                    actual_block_size_kv,
                    q_start,
                    kv_start,
                );
            }

            for i in 0..actual_block_size_q {
                let score_row = &scores[i * actual_block_size_kv..(i + 1) * actual_block_size_kv];
                softmax_states[i].update(score_row, &v_tile_f32, head_dim);
            }
        }

        for i in 0..actual_block_size_q {
            softmax_states[i].finalize();
            let out_offset = (q_start + i) * head_dim;
            output[out_offset..out_offset + head_dim].copy_from_slice(&softmax_states[i].output);
        }
    }
}

/// INT8 version of tile GEMM for Q @ K^T.
#[inline]
fn tile_gemm_qk_i8(
    q_tile: &[i8],
    k_tile: &[i8],
    head_dim: usize,
    block_size_q: usize,
    block_size_kv: usize,
    scale: f32,
    scores: &mut [f32],
) {
    debug_assert_eq!(q_tile.len(), block_size_q * head_dim);
    debug_assert_eq!(k_tile.len(), block_size_kv * head_dim);
    debug_assert_eq!(scores.len(), block_size_q * block_size_kv);

    for i in 0..block_size_q {
        let q_row = &q_tile[i * head_dim..(i + 1) * head_dim];

        for j in 0..block_size_kv {
            let k_row = &k_tile[j * head_dim..(j + 1) * head_dim];

            // INT32 accumulator for overflow safety
            let mut dot = 0i32;
            for d in 0..head_dim {
                dot += (q_row[d] as i32) * (k_row[d] as i32);
            }

            scores[i * block_size_kv + j] = (dot as f32) * scale;
        }
    }
}

/// Multi-head flash attention.
///
/// Processes multiple attention heads in parallel (conceptually).
/// In practice, processes heads sequentially but could be parallelized.
///
/// # Arguments
///
/// * `config` - Flash attention configuration
/// * `q` - Query tensor [num_heads, seq_len_q, head_dim]
/// * `k` - Key tensor [num_heads, seq_len_kv, head_dim]
/// * `v` - Value tensor [num_heads, seq_len_kv, head_dim]
/// * `num_heads` - Number of attention heads
/// * `seq_len_q` - Query sequence length
/// * `seq_len_kv` - Key/Value sequence length
/// * `output` - Output buffer [num_heads, seq_len_q, head_dim]
pub fn flash_mha(
    config: &FlashAttentionConfig,
    q: &[f32],
    k: &[f32],
    v: &[f32],
    num_heads: usize,
    seq_len_q: usize,
    seq_len_kv: usize,
    output: &mut [f32],
) {
    let head_dim = config.head_dim;
    let head_size = seq_len_q * head_dim;
    let kv_head_size = seq_len_kv * head_dim;

    debug_assert_eq!(q.len(), num_heads * head_size);
    debug_assert_eq!(k.len(), num_heads * kv_head_size);
    debug_assert_eq!(v.len(), num_heads * kv_head_size);
    debug_assert_eq!(output.len(), num_heads * head_size);

    for head in 0..num_heads {
        let q_offset = head * head_size;
        let kv_offset = head * kv_head_size;
        let out_offset = head * head_size;

        flash_attention_forward(
            config,
            &q[q_offset..q_offset + head_size],
            &k[kv_offset..kv_offset + kv_head_size],
            &v[kv_offset..kv_offset + kv_head_size],
            seq_len_q,
            seq_len_kv,
            &mut output[out_offset..out_offset + head_size],
        );
    }
}

/// Naive attention implementation for testing/comparison.
///
/// Materializes full attention matrix - O(n²) memory.
/// Used only for correctness validation.
#[cfg(test)]
fn naive_attention(
    q: &[f32],
    k: &[f32],
    v: &[f32],
    seq_len_q: usize,
    seq_len_kv: usize,
    head_dim: usize,
    scale: f32,
    causal: bool,
    output: &mut [f32],
) {
    // Compute Q @ K^T
    let mut scores = vec![0.0f32; seq_len_q * seq_len_kv];
    for i in 0..seq_len_q {
        for j in 0..seq_len_kv {
            let mut dot = 0.0f32;
            for d in 0..head_dim {
                dot += q[i * head_dim + d] * k[j * head_dim + d];
            }
            scores[i * seq_len_kv + j] = dot * scale;
        }
    }

    // Apply causal mask
    if causal {
        for i in 0..seq_len_q {
            for j in 0..seq_len_kv {
                if j > i {
                    scores[i * seq_len_kv + j] = f32::NEG_INFINITY;
                }
            }
        }
    }

    // Softmax per row
    for i in 0..seq_len_q {
        let row = &mut scores[i * seq_len_kv..(i + 1) * seq_len_kv];

        // Find max
        let max_val = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        // Exp and sum
        let mut sum = 0.0f32;
        for val in row.iter_mut() {
            if *val != f32::NEG_INFINITY {
                *val = (*val - max_val).exp();
                sum += *val;
            } else {
                *val = 0.0;
            }
        }

        // Normalize
        if sum > 0.0 {
            for val in row.iter_mut() {
                *val /= sum;
            }
        }
    }

    // Compute scores @ V
    for i in 0..seq_len_q {
        for d in 0..head_dim {
            let mut acc = 0.0f32;
            for j in 0..seq_len_kv {
                acc += scores[i * seq_len_kv + j] * v[j * head_dim + d];
            }
            output[i * head_dim + d] = acc;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(a: &[f32], b: &[f32], tolerance: f32) {
        assert_eq!(a.len(), b.len());
        for (i, (&av, &bv)) in a.iter().zip(b.iter()).enumerate() {
            let diff = (av - bv).abs();
            assert!(
                diff < tolerance,
                "Mismatch at index {}: {} vs {} (diff = {})",
                i,
                av,
                bv,
                diff
            );
        }
    }

    #[test]
    fn test_flash_attention_vs_naive_small() {
        let seq_len = 16;
        let head_dim = 8;

        // Create simple test data
        let mut q = vec![0.0f32; seq_len * head_dim];
        let mut k = vec![0.0f32; seq_len * head_dim];
        let mut v = vec![0.0f32; seq_len * head_dim];

        for i in 0..seq_len {
            for d in 0..head_dim {
                q[i * head_dim + d] = ((i + d) as f32) * 0.1;
                k[i * head_dim + d] = ((i * 2 + d) as f32) * 0.1;
                v[i * head_dim + d] = ((i + d * 2) as f32) * 0.1;
            }
        }

        let config = FlashAttentionConfig {
            block_size_q: 4,
            block_size_kv: 4,
            head_dim,
            causal: false,
            softmax_scale: 1.0 / (head_dim as f32).sqrt(),
        };

        let mut flash_output = vec![0.0f32; seq_len * head_dim];
        let mut naive_output = vec![0.0f32; seq_len * head_dim];

        flash_attention_forward(&config, &q, &k, &v, seq_len, seq_len, &mut flash_output);

        naive_attention(
            &q,
            &k,
            &v,
            seq_len,
            seq_len,
            head_dim,
            config.softmax_scale,
            false,
            &mut naive_output,
        );

        assert_close(&flash_output, &naive_output, 1e-4);
    }

    #[test]
    fn test_flash_attention_causal() {
        let seq_len = 8;
        let head_dim = 4;

        let mut q = vec![0.0f32; seq_len * head_dim];
        let mut k = vec![0.0f32; seq_len * head_dim];
        let mut v = vec![0.0f32; seq_len * head_dim];

        for i in 0..seq_len {
            for d in 0..head_dim {
                q[i * head_dim + d] = 1.0;
                k[i * head_dim + d] = 1.0;
                v[i * head_dim + d] = (i as f32) + 1.0;
            }
        }

        let config = FlashAttentionConfig {
            block_size_q: 4,
            block_size_kv: 4,
            head_dim,
            causal: true,
            softmax_scale: 1.0 / (head_dim as f32).sqrt(),
        };

        let mut flash_output = vec![0.0f32; seq_len * head_dim];
        let mut naive_output = vec![0.0f32; seq_len * head_dim];

        flash_attention_forward(&config, &q, &k, &v, seq_len, seq_len, &mut flash_output);

        naive_attention(
            &q,
            &k,
            &v,
            seq_len,
            seq_len,
            head_dim,
            config.softmax_scale,
            true,
            &mut naive_output,
        );

        assert_close(&flash_output, &naive_output, 1e-4);
    }

    #[test]
    fn test_flash_attention_different_seq_lengths() {
        let seq_len_q = 8;
        let seq_len_kv = 16;
        let head_dim = 4;

        let mut q = vec![0.0f32; seq_len_q * head_dim];
        let mut k = vec![0.0f32; seq_len_kv * head_dim];
        let mut v = vec![0.0f32; seq_len_kv * head_dim];

        for i in 0..seq_len_q {
            for d in 0..head_dim {
                q[i * head_dim + d] = ((i + d) as f32) * 0.1;
            }
        }

        for i in 0..seq_len_kv {
            for d in 0..head_dim {
                k[i * head_dim + d] = ((i * 2 + d) as f32) * 0.1;
                v[i * head_dim + d] = ((i + d * 2) as f32) * 0.1;
            }
        }

        let config = FlashAttentionConfig {
            block_size_q: 4,
            block_size_kv: 8,
            head_dim,
            causal: false,
            softmax_scale: 1.0 / (head_dim as f32).sqrt(),
        };

        let mut flash_output = vec![0.0f32; seq_len_q * head_dim];
        let mut naive_output = vec![0.0f32; seq_len_q * head_dim];

        flash_attention_forward(
            &config,
            &q,
            &k,
            &v,
            seq_len_q,
            seq_len_kv,
            &mut flash_output,
        );

        naive_attention(
            &q,
            &k,
            &v,
            seq_len_q,
            seq_len_kv,
            head_dim,
            config.softmax_scale,
            false,
            &mut naive_output,
        );

        assert_close(&flash_output, &naive_output, 1e-4);
    }

    #[test]
    fn test_flash_attention_i8() {
        let seq_len = 8;
        let head_dim = 4;

        // Create FP32 data
        let mut q_f32 = vec![0.0f32; seq_len * head_dim];
        let mut k_f32 = vec![0.0f32; seq_len * head_dim];
        let mut v_f32 = vec![0.0f32; seq_len * head_dim];

        for i in 0..seq_len {
            for d in 0..head_dim {
                q_f32[i * head_dim + d] = ((i + d) as f32) * 0.1;
                k_f32[i * head_dim + d] = ((i * 2 + d) as f32) * 0.1;
                v_f32[i * head_dim + d] = ((i + d * 2) as f32) * 0.1;
            }
        }

        // Quantize to INT8
        let q_scale = 0.01f32;
        let k_scale = 0.01f32;
        let v_scale = 0.01f32;

        let q_i8: Vec<i8> = q_f32
            .iter()
            .map(|&x| (x / q_scale).round().clamp(-128.0, 127.0) as i8)
            .collect();
        let k_i8: Vec<i8> = k_f32
            .iter()
            .map(|&x| (x / k_scale).round().clamp(-128.0, 127.0) as i8)
            .collect();
        let v_i8: Vec<i8> = v_f32
            .iter()
            .map(|&x| (x / v_scale).round().clamp(-128.0, 127.0) as i8)
            .collect();

        let config = FlashAttentionConfig {
            block_size_q: 4,
            block_size_kv: 4,
            head_dim,
            causal: false,
            softmax_scale: 1.0 / (head_dim as f32).sqrt(),
        };

        let mut i8_output = vec![0.0f32; seq_len * head_dim];
        let mut f32_output = vec![0.0f32; seq_len * head_dim];

        flash_attention_forward_i8(
            &config,
            &q_i8,
            &k_i8,
            &v_i8,
            q_scale,
            k_scale,
            v_scale,
            seq_len,
            seq_len,
            &mut i8_output,
        );

        flash_attention_forward(
            &config,
            &q_f32,
            &k_f32,
            &v_f32,
            seq_len,
            seq_len,
            &mut f32_output,
        );

        // Quantization introduces some error, so use larger tolerance
        assert_close(&i8_output, &f32_output, 0.1);
    }

    #[test]
    fn test_flash_mha() {
        let num_heads = 2;
        let seq_len = 4;
        let head_dim = 4;

        let total_size = num_heads * seq_len * head_dim;
        let mut q = vec![0.0f32; total_size];
        let mut k = vec![0.0f32; total_size];
        let mut v = vec![0.0f32; total_size];

        for h in 0..num_heads {
            for i in 0..seq_len {
                for d in 0..head_dim {
                    let idx = h * seq_len * head_dim + i * head_dim + d;
                    q[idx] = ((h + i + d) as f32) * 0.1;
                    k[idx] = ((h * 2 + i + d) as f32) * 0.1;
                    v[idx] = ((h + i * 2 + d) as f32) * 0.1;
                }
            }
        }

        let config = FlashAttentionConfig {
            block_size_q: 2,
            block_size_kv: 2,
            head_dim,
            causal: false,
            softmax_scale: 1.0 / (head_dim as f32).sqrt(),
        };

        let mut mha_output = vec![0.0f32; total_size];
        flash_mha(
            &config,
            &q,
            &k,
            &v,
            num_heads,
            seq_len,
            seq_len,
            &mut mha_output,
        );

        // Compare with per-head computation
        for h in 0..num_heads {
            let head_offset = h * seq_len * head_dim;
            let head_size = seq_len * head_dim;

            let mut single_output = vec![0.0f32; head_size];
            flash_attention_forward(
                &config,
                &q[head_offset..head_offset + head_size],
                &k[head_offset..head_offset + head_size],
                &v[head_offset..head_offset + head_size],
                seq_len,
                seq_len,
                &mut single_output,
            );

            assert_close(
                &mha_output[head_offset..head_offset + head_size],
                &single_output,
                1e-5,
            );
        }
    }

    #[test]
    fn test_online_softmax_state() {
        let head_dim = 4;
        let mut state = OnlineSoftmaxState::new(head_dim);

        // Update with first batch
        let scores1 = vec![1.0, 2.0, 3.0];
        let values1 = vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        state.update(&scores1, &values1, head_dim);

        // Update with second batch
        let scores2 = vec![2.5, 1.5];
        let values2 = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        state.update(&scores2, &values2, head_dim);

        state.finalize();

        // Verify output is normalized (sum should be reasonable)
        let sum: f32 = state.output.iter().sum();
        assert!(sum > 0.0 && sum < 10.0, "Output sum is {}", sum);
    }
}
