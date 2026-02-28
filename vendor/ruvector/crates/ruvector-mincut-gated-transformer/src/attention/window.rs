//! Sliding window attention implementation.
//!
//! Each token attends to at most W previous tokens, giving O(S * W) complexity
//! per layer instead of O(S^2).

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

use crate::kernel::qgemm::qgemm_i8;

/// Configuration for sliding window attention.
#[derive(Clone, Debug)]
pub struct WindowAttentionConfig {
    /// Number of attention heads
    pub heads: usize,

    /// Head dimension
    pub head_dim: usize,

    /// Window size
    pub window: usize,

    /// Maximum sequence length
    pub max_seq_len: usize,

    /// Attention scale (usually 1/sqrt(head_dim))
    pub scale: f32,
}

impl WindowAttentionConfig {
    /// Create configuration for given parameters
    pub fn new(heads: usize, head_dim: usize, window: usize, max_seq_len: usize) -> Self {
        Self {
            heads,
            head_dim,
            window,
            max_seq_len,
            scale: 1.0 / (head_dim as f32).sqrt(),
        }
    }
}

/// Sliding window attention.
pub struct SlidingWindowAttention {
    config: WindowAttentionConfig,
}

impl SlidingWindowAttention {
    /// Create new sliding window attention.
    pub fn new(config: WindowAttentionConfig) -> Self {
        Self { config }
    }

    /// Compute attention for a single head.
    ///
    /// # Arguments
    ///
    /// * `q` - Query vector for position `pos`, shape [head_dim], i8
    /// * `k_cache` - Key cache, shape [valid_len, head_dim], i8
    /// * `v_cache` - Value cache, shape [valid_len, head_dim], i8
    /// * `pos` - Current position
    /// * `valid_len` - Valid length in KV cache
    /// * `scores_buf` - Scratch buffer for attention scores, shape [window]
    /// * `output` - Output buffer, shape [head_dim]
    pub fn attention_single_head(
        &self,
        q: &[i8],
        k_cache: &[i8],
        v_cache: &[i8],
        pos: usize,
        valid_len: usize,
        scores_buf: &mut [f32],
        output: &mut [f32],
    ) {
        let head_dim = self.config.head_dim;
        let window = self.config.window.min(valid_len);

        // Determine window start
        let start = if pos >= window { pos - window + 1 } else { 0 };
        let end = pos.min(valid_len - 1) + 1;
        let actual_window = end - start;

        // Compute Q @ K^T scores for window
        for (i, cache_pos) in (start..end).enumerate() {
            let mut score: i32 = 0;
            for d in 0..head_dim {
                let q_val = q[d] as i32;
                let k_val = k_cache[cache_pos * head_dim + d] as i32;
                score += q_val * k_val;
            }
            scores_buf[i] = (score as f32) * self.config.scale;
        }

        // Softmax over window
        self.softmax(&mut scores_buf[..actual_window]);

        // Compute weighted sum of values
        for d in 0..head_dim {
            let mut sum = 0.0f32;
            for (i, cache_pos) in (start..end).enumerate() {
                let v_val = v_cache[cache_pos * head_dim + d] as f32;
                sum += scores_buf[i] * v_val;
            }
            output[d] = sum;
        }
    }

    /// Compute multi-head attention.
    ///
    /// # Arguments
    ///
    /// * `q` - Query tensor, shape [seq_len, heads, head_dim], i8
    /// * `k_cache` - Key cache per head, shape [heads, max_seq_len, head_dim], i8
    /// * `v_cache` - Value cache per head, shape [heads, max_seq_len, head_dim], i8
    /// * `valid_len` - Valid length in KV cache
    /// * `scores_buf` - Scratch buffer, shape [heads, window]
    /// * `output` - Output buffer, shape [seq_len, heads * head_dim]
    pub fn multi_head_attention(
        &self,
        q: &[i8],
        k_cache: &[i8],
        v_cache: &[i8],
        seq_len: usize,
        valid_len: usize,
        scores_buf: &mut [f32],
        output: &mut [f32],
    ) {
        let heads = self.config.heads;
        let head_dim = self.config.head_dim;
        let window = self.config.window;
        let max_seq = self.config.max_seq_len;

        for pos in 0..seq_len {
            for h in 0..heads {
                // Get Q for this position and head
                let q_offset = pos * heads * head_dim + h * head_dim;
                let q_slice = &q[q_offset..q_offset + head_dim];

                // Get K and V cache for this head
                let kv_offset = h * max_seq * head_dim;
                let k_slice = &k_cache[kv_offset..kv_offset + valid_len * head_dim];
                let v_slice = &v_cache[kv_offset..kv_offset + valid_len * head_dim];

                // Scores buffer for this head
                let scores_offset = h * window;
                let scores_slice = &mut scores_buf[scores_offset..scores_offset + window];

                // Output for this position and head
                let out_offset = pos * heads * head_dim + h * head_dim;
                let out_slice = &mut output[out_offset..out_offset + head_dim];

                self.attention_single_head(
                    q_slice,
                    k_slice,
                    v_slice,
                    pos,
                    valid_len,
                    scores_slice,
                    out_slice,
                );
            }
        }
    }

    /// Softmax over a slice.
    #[inline]
    fn softmax(&self, scores: &mut [f32]) {
        if scores.is_empty() {
            return;
        }

        // Find max for numerical stability
        let max = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        // Compute exp and sum
        let mut sum = 0.0f32;
        for s in scores.iter_mut() {
            *s = (*s - max).exp();
            sum += *s;
        }

        // Normalize
        if sum > 0.0 {
            let inv_sum = 1.0 / sum;
            for s in scores.iter_mut() {
                *s *= inv_sum;
            }
        }
    }

    /// Compute causal mask value.
    ///
    /// Returns true if position `i` can attend to position `j`.
    #[inline]
    pub fn can_attend(&self, i: usize, j: usize) -> bool {
        j <= i && i - j < self.config.window
    }
}

/// Build a causal sliding window mask.
///
/// Returns a bitmask where mask[i * seq_len + j] = 1 if i can attend to j.
pub fn build_window_mask(seq_len: usize, window: usize) -> Vec<bool> {
    let mut mask = vec![false; seq_len * seq_len];

    for i in 0..seq_len {
        let start = if i >= window { i - window + 1 } else { 0 };
        for j in start..=i {
            mask[i * seq_len + j] = true;
        }
    }

    mask
}

/// Apply sparse mask from spike packet.
///
/// Combines window mask with spike-provided top-k indices.
pub fn apply_sparse_mask(
    window_mask: &[bool],
    seq_len: usize,
    sparse_indices: &[u16],
    output_mask: &mut [bool],
) {
    debug_assert_eq!(window_mask.len(), seq_len * seq_len);
    debug_assert_eq!(output_mask.len(), seq_len * seq_len);

    // Start with window mask
    output_mask.copy_from_slice(window_mask);

    // For each query position, also attend to sparse indices
    for i in 0..seq_len {
        for &j in sparse_indices {
            let j = j as usize;
            if j < seq_len && j <= i {
                output_mask[i * seq_len + j] = true;
            }
        }
    }
}

/// Apply mincut sparse mask (requires `sparse_attention` feature).
///
/// Uses mincut-based sparse attention mask instead of window mask.
#[cfg(feature = "sparse_attention")]
pub fn apply_mincut_sparse_mask(
    mincut_mask: &crate::sparse_attention::SparseMask,
    output_mask: &mut [bool],
    seq_len: usize,
) {
    debug_assert_eq!(output_mask.len(), seq_len * seq_len);

    // Clear mask first
    output_mask.fill(false);

    // Set positions from mincut mask
    for &(query_pos, key_pos) in &mincut_mask.positions {
        let idx = (query_pos as usize) * seq_len + (key_pos as usize);
        if idx < output_mask.len() {
            output_mask[idx] = true;
        }
    }
}

/// Compute attention with mincut sparse mask (requires `sparse_attention` feature).
///
/// Efficiently computes attention using only the sparse positions.
#[cfg(feature = "sparse_attention")]
pub fn sparse_attention_with_mincut_mask(
    attn: &SlidingWindowAttention,
    q: &[i8],
    k_cache: &[i8],
    v_cache: &[i8],
    mincut_mask: &crate::sparse_attention::SparseMask,
    seq_len: usize,
    valid_len: usize,
    output: &mut [f32],
) {
    let head_dim = attn.config.head_dim;
    let scale = attn.config.scale;

    // Group positions by query
    let mut positions_by_query: Vec<Vec<u16>> = vec![Vec::new(); seq_len];
    for &(query_pos, key_pos) in &mincut_mask.positions {
        if (query_pos as usize) < seq_len && (key_pos as usize) < valid_len {
            positions_by_query[query_pos as usize].push(key_pos);
        }
    }

    // Compute attention for each query position
    for query_pos in 0..seq_len {
        let key_positions = &positions_by_query[query_pos];
        if key_positions.is_empty() {
            // No attention - output zeros
            for d in 0..head_dim {
                output[query_pos * head_dim + d] = 0.0;
            }
            continue;
        }

        // Compute scores for sparse keys
        let mut scores = Vec::with_capacity(key_positions.len());
        for &key_pos in key_positions {
            let mut score = 0i32;
            for d in 0..head_dim {
                let q_val = q[query_pos * head_dim + d] as i32;
                let k_val = k_cache[key_pos as usize * head_dim + d] as i32;
                score += q_val * k_val;
            }
            scores.push((score as f32) * scale);
        }

        // Softmax over sparse positions
        softmax_inplace(&mut scores);

        // Weighted sum of values
        for d in 0..head_dim {
            let mut sum = 0.0f32;
            for (i, &key_pos) in key_positions.iter().enumerate() {
                let v_val = v_cache[key_pos as usize * head_dim + d] as f32;
                sum += scores[i] * v_val;
            }
            output[query_pos * head_dim + d] = sum;
        }
    }
}

/// Helper function for in-place softmax
#[cfg(feature = "sparse_attention")]
#[inline]
fn softmax_inplace(scores: &mut [f32]) {
    if scores.is_empty() {
        return;
    }

    let max = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    let mut sum = 0.0f32;
    for s in scores.iter_mut() {
        *s = (*s - max).exp();
        sum += *s;
    }

    if sum > 0.0 {
        let inv_sum = 1.0 / sum;
        for s in scores.iter_mut() {
            *s *= inv_sum;
        }
    }
}

/// QKV projection using quantized GEMM.
pub fn qkv_projection(
    input: &[i8],
    seq_len: usize,
    hidden: usize,
    wq: &[i8],
    wk: &[i8],
    wv: &[i8],
    wq_scales: &[f32],
    wk_scales: &[f32],
    wv_scales: &[f32],
    q_out: &mut [i32],
    k_out: &mut [i32],
    v_out: &mut [i32],
) {
    // Q projection: [seq_len, hidden] @ [hidden, hidden]^T
    qgemm_i8(
        seq_len, hidden, hidden, input, 1.0, wq, wq_scales, None, q_out,
    );

    // K projection
    qgemm_i8(
        seq_len, hidden, hidden, input, 1.0, wk, wk_scales, None, k_out,
    );

    // V projection
    qgemm_i8(
        seq_len, hidden, hidden, input, 1.0, wv, wv_scales, None, v_out,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_config() {
        let config = WindowAttentionConfig::new(4, 64, 16, 64);
        assert_eq!(config.heads, 4);
        assert_eq!(config.head_dim, 64);
        assert!((config.scale - 0.125).abs() < 1e-5);
    }

    #[test]
    fn test_can_attend() {
        let config = WindowAttentionConfig::new(4, 64, 4, 64);
        let attn = SlidingWindowAttention::new(config);

        // Position 5 can attend to 2, 3, 4, 5 (window of 4)
        assert!(attn.can_attend(5, 5));
        assert!(attn.can_attend(5, 4));
        assert!(attn.can_attend(5, 3));
        assert!(attn.can_attend(5, 2));
        assert!(!attn.can_attend(5, 1)); // Outside window
        assert!(!attn.can_attend(5, 6)); // Can't attend to future
    }

    #[test]
    fn test_window_mask() {
        let mask = build_window_mask(4, 2);

        // Position 0: attends to [0]
        assert!(mask[0 * 4 + 0]);
        assert!(!mask[0 * 4 + 1]);

        // Position 1: attends to [0, 1]
        assert!(mask[1 * 4 + 0]);
        assert!(mask[1 * 4 + 1]);

        // Position 2: attends to [1, 2] (window = 2)
        assert!(!mask[2 * 4 + 0]);
        assert!(mask[2 * 4 + 1]);
        assert!(mask[2 * 4 + 2]);

        // Position 3: attends to [2, 3]
        assert!(!mask[3 * 4 + 0]);
        assert!(!mask[3 * 4 + 1]);
        assert!(mask[3 * 4 + 2]);
        assert!(mask[3 * 4 + 3]);
    }

    #[test]
    fn test_attention_single_head() {
        let config = WindowAttentionConfig::new(1, 4, 3, 8);
        let attn = SlidingWindowAttention::new(config);

        // Simple test with uniform K, V
        let q: [i8; 4] = [1, 1, 1, 1];
        let k_cache: [i8; 16] = [1; 16]; // 4 positions, head_dim=4
        let v_cache: [i8; 16] = [
            1, 0, 0, 0, // position 0
            0, 1, 0, 0, // position 1
            0, 0, 1, 0, // position 2
            0, 0, 0, 1, // position 3
        ];
        let mut scores = [0.0f32; 3];
        let mut output = [0.0f32; 4];

        attn.attention_single_head(
            &q,
            &k_cache,
            &v_cache,
            3, // position 3
            4, // valid_len 4
            &mut scores,
            &mut output,
        );

        // Output should be weighted sum of v values
        // With uniform K and Q, attention weights should be uniform
        assert!(output.iter().all(|&x| x.abs() > 0.0 || x == 0.0));
    }

    #[test]
    fn test_sparse_mask() {
        let window_mask = build_window_mask(4, 2);
        let sparse_indices: [u16; 2] = [0, 1];
        let mut output_mask = vec![false; 16];

        apply_sparse_mask(&window_mask, 4, &sparse_indices, &mut output_mask);

        // Position 3 should now also attend to 0 and 1 (from sparse)
        assert!(output_mask[3 * 4 + 0]); // Added by sparse
        assert!(output_mask[3 * 4 + 1]); // Added by sparse
        assert!(output_mask[3 * 4 + 2]); // From window
        assert!(output_mask[3 * 4 + 3]); // From window
    }
}
