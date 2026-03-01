//! Attention mechanisms for ESP32
//!
//! Implements simplified attention patterns optimized for microcontrollers.

// Quantized operations for attention

/// Simplified single-head attention for ESP32
///
/// This is a memory-efficient attention that processes one head at a time
/// to minimize activation memory.
pub struct MicroAttention {
    /// Head dimension
    head_dim: usize,
    /// Number of heads
    num_heads: usize,
    /// Cached attention scaling factor (1/sqrt(head_dim) as fixed-point)
    scale_shift: u8,
}

impl MicroAttention {
    /// Create new attention module
    pub fn new(embed_dim: usize, num_heads: usize) -> Self {
        let head_dim = embed_dim / num_heads;

        // Approximate 1/sqrt(head_dim) as right shift
        // sqrt(64) = 8, so shift by 3
        // sqrt(32) ≈ 5.66, so shift by 2-3
        let scale_shift = match head_dim {
            d if d >= 64 => 3,
            d if d >= 32 => 3,
            d if d >= 16 => 2,
            _ => 1,
        };

        Self {
            head_dim,
            num_heads,
            scale_shift,
        }
    }

    /// Compute attention scores between query and keys
    ///
    /// Returns scores in i32 format (scaled by 256)
    #[inline]
    pub fn compute_scores(
        &self,
        query: &[i8],      // [head_dim]
        keys: &[&[i8]],    // [seq_len, head_dim]
        scores: &mut [i32], // [seq_len]
    ) {
        for (i, key) in keys.iter().enumerate() {
            let mut dot: i32 = 0;
            for j in 0..self.head_dim {
                dot += query[j] as i32 * key[j] as i32;
            }
            // Scale by 1/sqrt(d_k)
            scores[i] = dot >> self.scale_shift;
        }
    }

    /// Apply causal mask (set future positions to minimum)
    #[inline]
    pub fn apply_causal_mask(&self, scores: &mut [i32], current_pos: usize) {
        for i in (current_pos + 1)..scores.len() {
            scores[i] = i32::MIN / 2; // Avoid overflow in softmax
        }
    }

    /// Fixed-point softmax optimized for ESP32
    ///
    /// Uses integer arithmetic only, suitable for chips without FPU.
    /// Output is scaled by 256 (i.e., 256 = 1.0)
    #[inline]
    pub fn softmax_fixed(&self, scores: &mut [i32]) {
        if scores.is_empty() {
            return;
        }

        // Find maximum for numerical stability
        let max_score = scores.iter().cloned().max().unwrap_or(0);

        // Compute exp approximation and sum
        // exp(x) ≈ 1 + x + x²/2 for small x
        // We use simpler linear: exp(x) ≈ 256 + x for x in [-256, 0]
        let mut sum: i64 = 0;
        for score in scores.iter_mut() {
            let x = *score - max_score;
            // Clamp to prevent overflow
            let x_clamped = x.max(-512).min(0);
            // Linear approximation of exp, result in range [0, 256]
            *score = (256 + x_clamped / 2).max(1) as i32;
            sum += *score as i64;
        }

        // Normalize: output[i] = score[i] * 256 / sum
        if sum > 0 {
            for score in scores.iter_mut() {
                *score = ((*score as i64 * 256) / sum) as i32;
            }
        }
    }

    /// Compute weighted sum of values
    ///
    /// output = sum(attention_weights[i] * values[i])
    #[inline]
    pub fn weighted_sum(
        &self,
        weights: &[i32],    // [seq_len], scaled by 256
        values: &[&[i8]],   // [seq_len, head_dim]
        output: &mut [i32], // [head_dim]
    ) {
        // Clear output
        for o in output.iter_mut() {
            *o = 0;
        }

        // Accumulate weighted values
        for (&weight, value) in weights.iter().zip(values.iter()) {
            for j in 0..self.head_dim {
                output[j] += weight * value[j] as i32;
            }
        }

        // Descale (weights were scaled by 256)
        for o in output.iter_mut() {
            *o >>= 8;
        }
    }
}

/// Linear attention approximation for very long sequences
///
/// Uses kernel feature maps to achieve O(n) complexity instead of O(n²)
pub struct LinearAttention {
    /// Feature dimension for kernel
    feature_dim: usize,
}

impl LinearAttention {
    pub fn new(feature_dim: usize) -> Self {
        Self { feature_dim }
    }

    /// ELU-based feature map: φ(x) = elu(x) + 1
    /// For INT8: approximate as max(x, 0) + 1
    #[inline]
    pub fn feature_map(&self, x: i8) -> i16 {
        (x.max(0) as i16) + 1
    }

    /// Compute linear attention
    /// Instead of softmax(QK^T)V, computes φ(Q)(φ(K)^T V)
    pub fn forward(
        &self,
        query: &[i8],      // [dim]
        keys: &[&[i8]],    // [seq_len, dim]
        values: &[&[i8]],  // [seq_len, dim]
        output: &mut [i32], // [dim]
    ) {
        let dim = query.len();

        // Compute φ(K)^T V: [dim, dim] accumulated over sequence
        // This is O(n * dim²) but can be incrementally updated
        let mut kv_cache = [[0i32; 64]; 64]; // Fixed size for embedded

        for (key, value) in keys.iter().zip(values.iter()) {
            for i in 0..dim.min(64) {
                let phi_k = self.feature_map(key[i]);
                for j in 0..dim.min(64) {
                    kv_cache[i][j] += phi_k as i32 * value[j] as i32;
                }
            }
        }

        // Compute φ(Q) @ (φ(K)^T V)
        for i in 0..dim.min(64) {
            let phi_q = self.feature_map(query[i]);
            let mut sum: i32 = 0;
            for j in 0..dim.min(64) {
                sum += phi_q as i32 * kv_cache[j][i];
            }
            output[i] = sum >> 8;
        }

        // Compute denominator: φ(Q) @ sum(φ(K))
        let mut k_sum = [0i32; 64];
        for key in keys.iter() {
            for i in 0..dim.min(64) {
                k_sum[i] += self.feature_map(key[i]) as i32;
            }
        }

        let mut denom: i32 = 0;
        for i in 0..dim.min(64) {
            denom += self.feature_map(query[i]) as i32 * k_sum[i];
        }

        // Normalize
        if denom > 0 {
            for o in output.iter_mut() {
                *o = (*o << 8) / denom;
            }
        }
    }
}

/// Sliding window attention for memory efficiency
///
/// Only attends to the last N tokens, reducing memory from O(n²) to O(n*window)
pub struct SlidingWindowAttention {
    window_size: usize,
    head_dim: usize,
}

impl SlidingWindowAttention {
    pub fn new(window_size: usize, head_dim: usize) -> Self {
        Self { window_size, head_dim }
    }

    /// Compute attention with sliding window
    pub fn forward(
        &self,
        query: &[i8],
        keys: &[[i8; 64]],    // Ring buffer of keys
        values: &[[i8; 64]],  // Ring buffer of values
        cache_len: usize,
        output: &mut [i32],
    ) {
        let window_start = cache_len.saturating_sub(self.window_size);
        let mut scores = [0i32; 32]; // Max window size

        // Compute attention scores for window
        for i in window_start..cache_len {
            let mut dot: i32 = 0;
            for j in 0..self.head_dim {
                dot += query[j] as i32 * keys[i % self.window_size][j] as i32;
            }
            scores[i - window_start] = dot >> 3;
        }

        // Softmax over window
        let window_len = cache_len - window_start;
        let scores_slice = &mut scores[..window_len];

        // Find max
        let max = scores_slice.iter().cloned().max().unwrap_or(0);
        let mut sum: i32 = 0;
        for s in scores_slice.iter_mut() {
            *s = (256 + (*s - max) / 2).max(1);
            sum += *s;
        }

        // Normalize and compute output
        for o in output[..self.head_dim].iter_mut() {
            *o = 0;
        }

        for i in 0..window_len {
            let weight = (scores[i] * 256) / sum.max(1);
            let value = &values[(window_start + i) % self.window_size];
            for j in 0..self.head_dim {
                output[j] += weight * value[j] as i32;
            }
        }

        for o in output[..self.head_dim].iter_mut() {
            *o >>= 8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_micro_attention() {
        let attn = MicroAttention::new(64, 4);

        let query = [10i8; 16];
        let key1 = [10i8; 16];
        let key2 = [5i8; 16];
        let keys: [&[i8]; 2] = [&key1, &key2];

        let mut scores = [0i32; 2];
        attn.compute_scores(&query, &keys, &mut scores);

        // First key should have higher score (same as query)
        assert!(scores[0] > scores[1]);
    }

    #[test]
    fn test_softmax_fixed() {
        let attn = MicroAttention::new(64, 4);

        let mut scores = [100i32, 50, 0, -50];
        attn.softmax_fixed(&mut scores);

        // Check that scores sum to ~256
        let sum: i32 = scores.iter().sum();
        assert!((sum - 256).abs() < 10);

        // Check ordering preserved
        assert!(scores[0] > scores[1]);
        assert!(scores[1] > scores[2]);
        assert!(scores[2] > scores[3]);
    }

    #[test]
    fn test_linear_attention() {
        let attn = LinearAttention::new(16);

        let query = [10i8; 16];
        let key = [10i8; 16];
        let value = [5i8; 16];
        let keys: [&[i8]; 1] = [&key];
        let values: [&[i8]; 1] = [&value];

        let mut output = [0i32; 16];
        attn.forward(&query, &keys, &values, &mut output);

        // Output should be non-zero
        assert!(output.iter().any(|&x| x != 0));
    }
}
