//! Rotary Position Embeddings (RoPE).
//!
//! Encodes positional information by rotating Q/K vectors in 2D subspaces.
//! Supports multiple scaling strategies for context extension beyond training length.
//!
//! ## Theory
//!
//! RoPE applies rotation matrices to Q/K vectors in pairs of dimensions:
//! ```text
//! [q_i]   [cos(mθ_i)  -sin(mθ_i)] [q_i]
//! [q_j] = [sin(mθ_i)   cos(mθ_i)] [q_j]
//! ```
//! where m is the position and θ_i = base^(-2i/d) is the frequency for dimension pair i.
//!
//! ## Scaling Methods
//!
//! - **None**: Standard RoPE (up to trained max_seq_len)
//! - **Linear**: Simple position interpolation (quality degrades)
//! - **NTK-Aware**: Adjusts base frequency (better quality, used in Qwen)
//! - **YaRN**: Combines NTK + attention scaling (best for extreme extension)
//!
//! ## References
//!
//! - Su et al. 2021: RoFormer: Enhanced Transformer with Rotary Position Embedding
//! - bloc97 2023: NTK-Aware Scaled RoPE
//! - Peng et al. 2023: YaRN: Efficient Context Window Extension

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Error, Result};

/// RoPE configuration
#[derive(Debug, Clone)]
pub struct RopeConfig {
    /// Dimensionality of each attention head
    pub head_dim: usize,

    /// Base frequency for position encoding (default: 10000.0)
    /// Higher base = slower frequency decay = better long-range attention
    pub base: f32,

    /// Maximum sequence length to precompute
    pub max_seq_len: usize,

    /// Scaling strategy for context extension
    pub scaling_type: RopeScaling,
}

/// Scaling strategies for extending context beyond training length
#[derive(Debug, Clone)]
pub enum RopeScaling {
    /// No scaling - standard RoPE
    None,

    /// Linear interpolation: position' = position * (trained_len / new_len)
    /// Simple but quality degrades significantly
    Linear(f32),

    /// NTK-aware scaling: adjusts base frequency instead of positions
    /// Better quality preservation, used in Qwen models
    /// alpha controls the extension factor
    NTKAware { alpha: f32 },

    /// YaRN (Yet another RoPE extensioN): combines NTK + attention scaling
    /// Best quality for extreme context extension (8k -> 128k)
    YaRN { scale: f32, original_max_len: usize },
}

/// Rotary Position Embeddings with precomputed sin/cos tables
pub struct RopeEmbedding {
    /// Cosine cache: [max_seq_len, head_dim/2]
    cos_cache: Vec<f32>,

    /// Sine cache: [max_seq_len, head_dim/2]
    sin_cache: Vec<f32>,

    /// Head dimension
    head_dim: usize,

    /// Maximum sequence length
    max_seq_len: usize,
}

impl Default for RopeConfig {
    fn default() -> Self {
        Self {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 2048,
            scaling_type: RopeScaling::None,
        }
    }
}

impl RopeEmbedding {
    /// Create new RoPE embeddings with precomputed tables
    pub fn new(config: &RopeConfig) -> Result<Self> {
        if config.head_dim % 2 != 0 {
            return Err(Error::BadConfig("head_dim must be even for RoPE"));
        }

        if config.max_seq_len == 0 {
            return Err(Error::BadConfig("max_seq_len must be > 0"));
        }

        let half_dim = config.head_dim / 2;
        let effective_base = Self::compute_effective_base(config);

        // Precompute frequencies for each dimension pair
        let freqs = Self::compute_frequencies(half_dim, effective_base, &config.scaling_type);

        // Precompute sin/cos for all positions
        let mut cos_cache = vec![0.0f32; config.max_seq_len * half_dim];
        let mut sin_cache = vec![0.0f32; config.max_seq_len * half_dim];

        for pos in 0..config.max_seq_len {
            let pos_f = pos as f32;

            // Apply linear scaling if needed
            let scaled_pos = match &config.scaling_type {
                RopeScaling::Linear(factor) => pos_f * factor,
                _ => pos_f,
            };

            for i in 0..half_dim {
                let angle = scaled_pos * freqs[i];
                let idx = pos * half_dim + i;
                cos_cache[idx] = angle.cos();
                sin_cache[idx] = angle.sin();
            }
        }

        Ok(Self {
            cos_cache,
            sin_cache,
            head_dim: config.head_dim,
            max_seq_len: config.max_seq_len,
        })
    }

    /// Compute effective base frequency based on scaling strategy
    fn compute_effective_base(config: &RopeConfig) -> f32 {
        match &config.scaling_type {
            RopeScaling::NTKAware { alpha } => {
                // NTK-aware: base' = base * alpha^(d/(d-2))
                // This adjusts the frequency decay to maintain quality
                let d = config.head_dim as f32;
                let exponent = d / (d - 2.0);
                config.base * alpha.powf(exponent)
            }
            RopeScaling::YaRN { scale, .. } => {
                // YaRN uses similar base adjustment
                let d = config.head_dim as f32;
                let exponent = d / (d - 2.0);
                config.base * scale.powf(exponent)
            }
            _ => config.base,
        }
    }

    /// Compute frequency for each dimension pair
    fn compute_frequencies(half_dim: usize, base: f32, scaling: &RopeScaling) -> Vec<f32> {
        let mut freqs = vec![0.0f32; half_dim];

        for i in 0..half_dim {
            let exponent = -2.0 * (i as f32) / (half_dim as f32 * 2.0);
            let freq = base.powf(exponent);

            // Apply YaRN frequency adjustment if needed
            freqs[i] = match scaling {
                RopeScaling::YaRN { scale, .. } => {
                    // YaRN applies different scaling to different frequency bands
                    // High frequencies get less scaling (local attention)
                    // Low frequencies get more scaling (long-range attention)
                    let normalized_freq = freq / base;
                    if normalized_freq > 1.0 / scale {
                        freq / scale
                    } else {
                        freq
                    }
                }
                _ => freq,
            };
        }

        freqs
    }

    /// Apply rotary embeddings to query and key vectors in-place
    ///
    /// # Arguments
    /// * `q` - Query vectors: [num_tokens, num_heads, head_dim]
    /// * `k` - Key vectors: [num_tokens, num_heads, head_dim]
    /// * `positions` - Position index for each token
    ///
    /// # Layout
    /// Assumes contiguous layout where each token's heads are consecutive
    pub fn apply_rotary_pos_emb(
        &self,
        q: &mut [f32],
        k: &mut [f32],
        positions: &[usize],
    ) -> Result<()> {
        let half_dim = self.head_dim / 2;
        let num_tokens = positions.len();

        if q.len() < num_tokens * self.head_dim {
            return Err(Error::BadInput("q buffer too small for RoPE"));
        }

        if k.len() < num_tokens * self.head_dim {
            return Err(Error::BadInput("k buffer too small for RoPE"));
        }

        for (token_idx, &pos) in positions.iter().enumerate() {
            if pos >= self.max_seq_len {
                return Err(Error::BadInput("position exceeds max_seq_len"));
            }

            let token_offset = token_idx * self.head_dim;

            // Rotate each dimension pair
            for i in 0..half_dim {
                let cache_idx = pos * half_dim + i;
                let cos = self.cos_cache[cache_idx];
                let sin = self.sin_cache[cache_idx];

                let i1 = token_offset + i;
                let i2 = token_offset + i + half_dim;

                // Rotate Q
                let q1 = q[i1];
                let q2 = q[i2];
                q[i1] = q1 * cos - q2 * sin;
                q[i2] = q1 * sin + q2 * cos;

                // Rotate K
                let k1 = k[i1];
                let k2 = k[i2];
                k[i1] = k1 * cos - k2 * sin;
                k[i2] = k1 * sin + k2 * cos;
            }
        }

        Ok(())
    }

    /// Apply rotary embeddings to quantized Q15 vectors
    ///
    /// For INT8/INT4 quantized models, we still use f32 RoPE then re-quantize
    pub fn apply_rotary_pos_emb_q15(
        &self,
        q: &mut [i16],
        k: &mut [i16],
        positions: &[usize],
    ) -> Result<()> {
        let half_dim = self.head_dim / 2;
        let num_tokens = positions.len();

        if q.len() < num_tokens * self.head_dim {
            return Err(Error::BadInput("q buffer too small for RoPE Q15"));
        }

        if k.len() < num_tokens * self.head_dim {
            return Err(Error::BadInput("k buffer too small for RoPE Q15"));
        }

        const Q15_SCALE: f32 = 32768.0;

        for (token_idx, &pos) in positions.iter().enumerate() {
            if pos >= self.max_seq_len {
                return Err(Error::BadInput("position exceeds max_seq_len"));
            }

            let token_offset = token_idx * self.head_dim;

            // Rotate each dimension pair
            for i in 0..half_dim {
                let cache_idx = pos * half_dim + i;
                let cos = self.cos_cache[cache_idx];
                let sin = self.sin_cache[cache_idx];

                let i1 = token_offset + i;
                let i2 = token_offset + i + half_dim;

                // Rotate Q (dequantize, rotate, requantize)
                let q1_f = q[i1] as f32 / Q15_SCALE;
                let q2_f = q[i2] as f32 / Q15_SCALE;
                let q1_rot = q1_f * cos - q2_f * sin;
                let q2_rot = q1_f * sin + q2_f * cos;
                q[i1] = (q1_rot * Q15_SCALE).round() as i16;
                q[i2] = (q2_rot * Q15_SCALE).round() as i16;

                // Rotate K
                let k1_f = k[i1] as f32 / Q15_SCALE;
                let k2_f = k[i2] as f32 / Q15_SCALE;
                let k1_rot = k1_f * cos - k2_f * sin;
                let k2_rot = k1_f * sin + k2_f * cos;
                k[i1] = (k1_rot * Q15_SCALE).round() as i16;
                k[i2] = (k2_rot * Q15_SCALE).round() as i16;
            }
        }

        Ok(())
    }

    /// Get cosine value for a specific position and dimension
    #[inline]
    pub fn get_cos(&self, pos: usize, dim: usize) -> f32 {
        let half_dim = self.head_dim / 2;
        debug_assert!(pos < self.max_seq_len);
        debug_assert!(dim < half_dim);
        self.cos_cache[pos * half_dim + dim]
    }

    /// Get sine value for a specific position and dimension
    #[inline]
    pub fn get_sin(&self, pos: usize, dim: usize) -> f32 {
        let half_dim = self.head_dim / 2;
        debug_assert!(pos < self.max_seq_len);
        debug_assert!(dim < half_dim);
        self.sin_cache[pos * half_dim + dim]
    }

    /// Get head dimension
    #[inline]
    pub fn head_dim(&self) -> usize {
        self.head_dim
    }

    /// Get maximum sequence length
    #[inline]
    pub fn max_seq_len(&self) -> usize {
        self.max_seq_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn assert_f32_near(a: f32, b: f32, msg: &str) {
        assert!((a - b).abs() < EPSILON, "{}: {} vs {}", msg, a, b);
    }

    #[test]
    fn test_rope_config_default() {
        let config = RopeConfig::default();
        assert_eq!(config.head_dim, 64);
        assert_eq!(config.base, 10000.0);
        assert_eq!(config.max_seq_len, 2048);
    }

    #[test]
    fn test_rope_initialization() {
        let config = RopeConfig {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 128,
            scaling_type: RopeScaling::None,
        };

        let rope = RopeEmbedding::new(&config).unwrap();
        assert_eq!(rope.head_dim(), 64);
        assert_eq!(rope.max_seq_len(), 128);
        assert_eq!(rope.cos_cache.len(), 128 * 32);
        assert_eq!(rope.sin_cache.len(), 128 * 32);
    }

    #[test]
    fn test_rope_invalid_config() {
        // Odd head_dim should fail
        let config = RopeConfig {
            head_dim: 63,
            base: 10000.0,
            max_seq_len: 128,
            scaling_type: RopeScaling::None,
        };
        assert!(RopeEmbedding::new(&config).is_err());

        // Zero max_seq_len should fail
        let config = RopeConfig {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 0,
            scaling_type: RopeScaling::None,
        };
        assert!(RopeEmbedding::new(&config).is_err());
    }

    #[test]
    fn test_position_zero_no_rotation() {
        // Position 0 should produce identity rotation (cos=1, sin=0)
        let config = RopeConfig {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 128,
            scaling_type: RopeScaling::None,
        };

        let rope = RopeEmbedding::new(&config).unwrap();

        // Check all dimension pairs at position 0
        for i in 0..32 {
            assert_f32_near(rope.get_cos(0, i), 1.0, "cos at pos=0 should be 1.0");
            assert_f32_near(rope.get_sin(0, i), 0.0, "sin at pos=0 should be 0.0");
        }

        // Apply to vectors - should not change them
        let mut q = vec![1.0, 2.0, 3.0, 4.0]; // Minimal 4-dim vector
        let mut k = vec![5.0, 6.0, 7.0, 8.0];
        let q_orig = q.clone();
        let k_orig = k.clone();

        let config = RopeConfig {
            head_dim: 4,
            base: 10000.0,
            max_seq_len: 128,
            scaling_type: RopeScaling::None,
        };
        let rope = RopeEmbedding::new(&config).unwrap();

        rope.apply_rotary_pos_emb(&mut q, &mut k, &[0]).unwrap();

        for i in 0..4 {
            assert_f32_near(q[i], q_orig[i], "Q should not change at pos=0");
            assert_f32_near(k[i], k_orig[i], "K should not change at pos=0");
        }
    }

    #[test]
    fn test_rotation_reversibility() {
        // Rotating by angle θ then -θ should give identity
        let config = RopeConfig {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 128,
            scaling_type: RopeScaling::None,
        };

        let rope = RopeEmbedding::new(&config).unwrap();

        // Create test vectors
        let mut q = vec![0.0f32; 64];
        let mut k = vec![0.0f32; 64];
        for i in 0..64 {
            q[i] = (i as f32) * 0.1;
            k[i] = (i as f32) * 0.2;
        }
        let q_orig = q.clone();
        let k_orig = k.clone();

        // Apply rotation at position 10
        rope.apply_rotary_pos_emb(&mut q, &mut k, &[10]).unwrap();

        // Vectors should have changed
        let mut changed = false;
        for i in 0..64 {
            if (q[i] - q_orig[i]).abs() > EPSILON {
                changed = true;
                break;
            }
        }
        assert!(changed, "Rotation should change vectors");

        // Manually reverse the rotation
        let half_dim = 32;
        for i in 0..half_dim {
            let cos = rope.get_cos(10, i);
            let sin = rope.get_sin(10, i);

            // Reverse rotation: use -sin instead of sin
            let q1 = q[i];
            let q2 = q[i + half_dim];
            q[i] = q1 * cos + q2 * sin; // Note: +sin for reverse
            q[i + half_dim] = -q1 * sin + q2 * cos;

            let k1 = k[i];
            let k2 = k[i + half_dim];
            k[i] = k1 * cos + k2 * sin;
            k[i + half_dim] = -k1 * sin + k2 * cos;
        }

        // Should recover original vectors
        for i in 0..64 {
            assert_f32_near(
                q[i],
                q_orig[i],
                "Q should be restored after reverse rotation",
            );
            assert_f32_near(
                k[i],
                k_orig[i],
                "K should be restored after reverse rotation",
            );
        }
    }

    #[test]
    fn test_ntk_aware_scaling() {
        let base_config = RopeConfig {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 2048,
            scaling_type: RopeScaling::None,
        };

        let ntk_config = RopeConfig {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 4096,
            scaling_type: RopeScaling::NTKAware { alpha: 2.0 },
        };

        let base_rope = RopeEmbedding::new(&base_config).unwrap();
        let ntk_rope = RopeEmbedding::new(&ntk_config).unwrap();

        // NTK-aware should have different (larger) effective base
        let effective_base = RopeEmbedding::compute_effective_base(&ntk_config);
        assert!(
            effective_base > base_config.base,
            "NTK should increase base frequency"
        );

        // Angles at same relative position should be similar
        // pos=1024 in base ~= pos=2048 in NTK (both are middle of context)
        let mid_base = base_config.max_seq_len / 2;
        let mid_ntk = ntk_config.max_seq_len / 2;

        // First dimension should have comparable angles
        let base_cos = base_rope.get_cos(mid_base, 0);
        let ntk_cos = ntk_rope.get_cos(mid_ntk, 0);

        // They won't be exactly equal, but should be in similar range
        assert!(
            (base_cos - ntk_cos).abs() < 0.5,
            "NTK should preserve frequency characteristics"
        );
    }

    #[test]
    fn test_linear_scaling() {
        let config = RopeConfig {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 2048,
            scaling_type: RopeScaling::Linear(0.5), // Compress positions by 2x
        };

        let rope = RopeEmbedding::new(&config).unwrap();

        // With linear scaling of 0.5, position 100 should behave like position 50
        let mut q1 = vec![0.0f32; 64];
        let mut k1 = vec![0.0f32; 64];
        for i in 0..64 {
            q1[i] = (i as f32) * 0.1;
            k1[i] = (i as f32) * 0.2;
        }
        let q1_orig = q1.clone();

        rope.apply_rotary_pos_emb(&mut q1, &mut k1, &[100]).unwrap();

        // Create unscaled rope for comparison
        let unscaled_config = RopeConfig {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 2048,
            scaling_type: RopeScaling::None,
        };
        let unscaled_rope = RopeEmbedding::new(&unscaled_config).unwrap();

        let mut q2 = q1_orig.clone();
        let mut k2 = vec![0.0f32; 64];
        for i in 0..64 {
            k2[i] = (i as f32) * 0.2;
        }

        unscaled_rope
            .apply_rotary_pos_emb(&mut q2, &mut k2, &[50])
            .unwrap();

        // Results should be very similar
        for i in 0..64 {
            assert!(
                (q1[i] - q2[i]).abs() < 0.01,
                "Linear scaling should compress positions"
            );
        }
    }

    #[test]
    fn test_yarn_scaling() {
        let config = RopeConfig {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 4096,
            scaling_type: RopeScaling::YaRN {
                scale: 2.0,
                original_max_len: 2048,
            },
        };

        let rope = RopeEmbedding::new(&config).unwrap();

        // YaRN should extend context successfully
        let mut q = vec![0.0f32; 64];
        let mut k = vec![0.0f32; 64];
        for i in 0..64 {
            q[i] = (i as f32) * 0.1;
            k[i] = (i as f32) * 0.2;
        }

        // Should handle extended positions
        rope.apply_rotary_pos_emb(&mut q, &mut k, &[3000]).unwrap();
    }

    #[test]
    fn test_q15_quantized_rope() {
        let config = RopeConfig {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 128,
            scaling_type: RopeScaling::None,
        };

        let rope = RopeEmbedding::new(&config).unwrap();

        // Create Q15 test vectors
        const Q15_SCALE: f32 = 32768.0;
        let mut q = vec![0i16; 64];
        let mut k = vec![0i16; 64];
        for i in 0..64 {
            q[i] = ((i as f32) * 0.1 * Q15_SCALE) as i16;
            k[i] = ((i as f32) * 0.2 * Q15_SCALE) as i16;
        }
        let q_orig = q.clone();

        // Apply Q15 rotation
        rope.apply_rotary_pos_emb_q15(&mut q, &mut k, &[10])
            .unwrap();

        // Vectors should have changed
        let mut changed = false;
        for i in 0..64 {
            if q[i] != q_orig[i] {
                changed = true;
                break;
            }
        }
        assert!(changed, "Q15 rotation should change vectors");

        // Position 0 should still be identity
        let mut q_zero = vec![0i16; 64];
        let mut k_zero = vec![0i16; 64];
        for i in 0..64 {
            q_zero[i] = ((i as f32) * 0.1 * Q15_SCALE) as i16;
            k_zero[i] = ((i as f32) * 0.2 * Q15_SCALE) as i16;
        }
        let q_zero_orig = q_zero.clone();
        let k_zero_orig = k_zero.clone();

        rope.apply_rotary_pos_emb_q15(&mut q_zero, &mut k_zero, &[0])
            .unwrap();

        for i in 0..64 {
            // Allow small quantization error
            assert!(
                (q_zero[i] - q_zero_orig[i]).abs() <= 1,
                "Q15 should not change at pos=0"
            );
            assert!(
                (k_zero[i] - k_zero_orig[i]).abs() <= 1,
                "Q15 should not change at pos=0"
            );
        }
    }

    #[test]
    fn test_multiple_tokens() {
        let config = RopeConfig {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 128,
            scaling_type: RopeScaling::None,
        };

        let rope = RopeEmbedding::new(&config).unwrap();

        // 4 tokens at different positions
        let positions = vec![0, 5, 10, 15];
        let num_tokens = positions.len();

        let mut q = vec![0.0f32; num_tokens * 64];
        let mut k = vec![0.0f32; num_tokens * 64];
        for i in 0..(num_tokens * 64) {
            q[i] = (i as f32) * 0.01;
            k[i] = (i as f32) * 0.02;
        }

        rope.apply_rotary_pos_emb(&mut q, &mut k, &positions)
            .unwrap();

        // First token (pos=0) should be unchanged
        for i in 0..64 {
            assert_f32_near(q[i], (i as f32) * 0.01, "First token should not rotate");
        }

        // Other tokens should have changed
        for token in 1..num_tokens {
            let mut changed = false;
            for i in 0..64 {
                let idx = token * 64 + i;
                if (q[idx] - (idx as f32) * 0.01).abs() > EPSILON {
                    changed = true;
                    break;
                }
            }
            assert!(changed, "Token {} should have rotated", token);
        }
    }

    #[test]
    fn test_position_out_of_bounds() {
        let config = RopeConfig {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 128,
            scaling_type: RopeScaling::None,
        };

        let rope = RopeEmbedding::new(&config).unwrap();

        let mut q = vec![0.0f32; 64];
        let mut k = vec![0.0f32; 64];

        // Position 200 exceeds max_seq_len=128
        let result = rope.apply_rotary_pos_emb(&mut q, &mut k, &[200]);
        assert!(result.is_err(), "Should fail for out-of-bounds position");
    }

    #[test]
    fn test_frequency_decay() {
        let config = RopeConfig {
            head_dim: 64,
            base: 10000.0,
            max_seq_len: 128,
            scaling_type: RopeScaling::None,
        };

        let rope = RopeEmbedding::new(&config).unwrap();

        // Lower dimension pairs should have higher frequencies (faster rotation)
        // Higher dimension pairs should have lower frequencies (slower rotation)

        let pos = 10;

        // Compare angles at different dimensions
        let angle_dim0 = rope.get_cos(pos, 0).acos();
        let angle_dim15 = rope.get_cos(pos, 15).acos();
        let angle_dim31 = rope.get_cos(pos, 31).acos();

        // Higher dimensions should have smaller angles (lower frequency)
        assert!(
            angle_dim0 > angle_dim15,
            "Frequency should decay with dimension"
        );
        assert!(
            angle_dim15 > angle_dim31,
            "Frequency should decay with dimension"
        );
    }
}
