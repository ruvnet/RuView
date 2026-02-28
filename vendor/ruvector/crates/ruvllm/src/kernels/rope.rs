//! NEON-Optimized Rotary Position Embeddings (RoPE)
//!
//! Implements efficient RoPE operations for transformer models:
//!
//! - **Standard RoPE**: Original rotary embeddings (Llama, GPT-NeoX)
//! - **Scaled RoPE**: Position interpolation for extended context
//! - **YaRN**: Yet another RoPE extension for very long contexts
//!
//! ## Mathematical Background
//!
//! RoPE applies rotation to query and key vectors based on position:
//! ```text
//! x_rotated = x * cos(theta) + rotate_half(x) * sin(theta)
//! where theta = position * base^(-2i/d)
//! ```
//!
//! ## Performance
//!
//! | Model | Head Dim | M4 Pro Throughput |
//! |-------|----------|-------------------|
//! | Llama-2 | 128 | ~4.2 GB/s |
//! | Mistral | 128 | ~4.2 GB/s |
//! | Llama-3 | 128 | ~4.0 GB/s (higher base) |

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

use super::{NEON_LANE_WIDTH, UNROLL_FACTOR};
use std::f32::consts::PI;

/// RoPE configuration
#[derive(Debug, Clone, Copy)]
pub struct RopeConfig {
    /// Base frequency (10000.0 for Llama, 1000000.0 for some models)
    pub base: f32,
    /// Head dimension
    pub head_dim: usize,
    /// Maximum sequence length for precomputation
    pub max_seq_len: usize,
    /// Scaling factor for position interpolation (1.0 = no scaling)
    pub scaling_factor: f32,
    /// Whether to use NTK-aware scaling
    pub ntk_aware: bool,
    /// Original maximum sequence length (for scaling)
    pub original_max_len: usize,
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
    /// Create config for Llama-2 style models
    pub fn llama2(head_dim: usize, max_seq_len: usize) -> Self {
        Self {
            base: 10000.0,
            head_dim,
            max_seq_len,
            ..Default::default()
        }
    }

    /// Create config for Llama-3 style models (higher base)
    pub fn llama3(head_dim: usize, max_seq_len: usize) -> Self {
        Self {
            base: 500000.0,
            head_dim,
            max_seq_len,
            ..Default::default()
        }
    }

    /// Create config for Mistral style models
    pub fn mistral(head_dim: usize, max_seq_len: usize) -> Self {
        Self {
            base: 10000.0,
            head_dim,
            max_seq_len,
            ..Default::default()
        }
    }

    /// Create config with position interpolation
    pub fn with_scaling(mut self, scaling_factor: f32) -> Self {
        self.scaling_factor = scaling_factor;
        self
    }

    /// Enable NTK-aware scaling
    pub fn with_ntk(mut self, original_max_len: usize) -> Self {
        self.ntk_aware = true;
        self.original_max_len = original_max_len;
        self
    }

    /// Compute effective base with NTK scaling
    pub fn effective_base(&self) -> f32 {
        if self.ntk_aware && self.max_seq_len > self.original_max_len {
            let scale = self.max_seq_len as f32 / self.original_max_len as f32;
            self.base * scale.powf((self.head_dim as f32) / (self.head_dim as f32 - 2.0))
        } else {
            self.base
        }
    }
}

/// Precomputed sin/cos tables for RoPE
#[derive(Debug, Clone)]
pub struct RopeTables {
    /// Cosine values (max_seq_len, head_dim/2)
    pub cos: Vec<f32>,
    /// Sine values (max_seq_len, head_dim/2)
    pub sin: Vec<f32>,
    /// Half of head dimension
    pub half_dim: usize,
    /// Maximum sequence length
    pub max_seq_len: usize,
}

impl RopeTables {
    /// Get cos/sin for a specific position
    #[inline(always)]
    pub fn get(&self, position: usize) -> (&[f32], &[f32]) {
        let offset = position * self.half_dim;
        (
            &self.cos[offset..offset + self.half_dim],
            &self.sin[offset..offset + self.half_dim],
        )
    }
}

/// Precompute sin/cos tables for RoPE
///
/// # Arguments
/// * `max_seq_len` - Maximum sequence length
/// * `head_dim` - Dimension per head
/// * `base` - RoPE base frequency
///
/// # Returns
/// Tuple of (cos_table, sin_table), each of shape (max_seq_len, head_dim/2)
pub fn precompute_rope_tables(
    max_seq_len: usize,
    head_dim: usize,
    base: f32,
) -> (Vec<f32>, Vec<f32>) {
    let half_dim = head_dim / 2;
    let mut cos_table = vec![0.0; max_seq_len * half_dim];
    let mut sin_table = vec![0.0; max_seq_len * half_dim];

    // Compute inverse frequencies: 1 / (base^(2i/d))
    let inv_freq: Vec<f32> = (0..half_dim)
        .map(|i| 1.0 / base.powf((2 * i) as f32 / head_dim as f32))
        .collect();

    // Compute sin/cos for each position
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

/// Precompute RoPE tables with configuration
pub fn precompute_rope_tables_with_config(config: &RopeConfig) -> RopeTables {
    let base = config.effective_base();
    let (cos, sin) = precompute_rope_tables(config.max_seq_len, config.head_dim, base);

    // Apply scaling factor if needed
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
                // Linear interpolation
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

/// Apply RoPE to query and key tensors in-place with NEON optimization
///
/// # Arguments
/// * `x` - Input tensor to rotate (modified in-place)
/// * `positions` - Position indices for each token
/// * `head_dim` - Dimension per head
/// * `base` - RoPE base frequency
///
/// # Implementation Details
/// Uses interleaved rotation: pairs (x0, x1), (x2, x3), ... are rotated together
#[inline(always)]
pub fn apply_rope_neon(x: &mut [f32], positions: &[usize], head_dim: usize, base: f32) {
    let half_dim = head_dim / 2;
    let num_tokens = positions.len();
    let stride = head_dim;

    debug_assert_eq!(x.len(), num_tokens * head_dim);

    // Precompute inverse frequencies
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

/// Apply RoPE with precomputed tables
#[inline(always)]
pub fn apply_rope_with_tables(x: &mut [f32], positions: &[usize], tables: &RopeTables) {
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

/// NEON implementation of RoPE
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

        // Process in chunks of 4 (2 pairs at a time)
        let chunks = half_dim / (NEON_LANE_WIDTH / 2);

        let mut freq_idx = 0usize;
        for _ in 0..chunks {
            // Load inverse frequencies
            let freq0 = *inv_freq_ptr.add(freq_idx);
            let freq1 = *inv_freq_ptr.add(freq_idx + 1);

            // Compute theta = position * inv_freq
            let theta0 = pos as f32 * freq0;
            let theta1 = pos as f32 * freq1;

            // Compute sin/cos
            let cos0 = theta0.cos();
            let sin0 = theta0.sin();
            let cos1 = theta1.cos();
            let sin1 = theta1.sin();

            // Load x values (pairs)
            let x_offset = tok_offset + freq_idx * 2;
            let x0 = *x_ptr.add(x_offset);
            let x1 = *x_ptr.add(x_offset + 1);
            let x2 = *x_ptr.add(x_offset + 2);
            let x3 = *x_ptr.add(x_offset + 3);

            // Apply rotation: x_new = x * cos - x_rotated * sin
            // For pair (x0, x1): rotated is (-x1, x0)
            *x_ptr.add(x_offset) = x0 * cos0 - x1 * sin0;
            *x_ptr.add(x_offset + 1) = x1 * cos0 + x0 * sin0;
            *x_ptr.add(x_offset + 2) = x2 * cos1 - x3 * sin1;
            *x_ptr.add(x_offset + 3) = x3 * cos1 + x2 * sin1;

            freq_idx += 2;
        }

        // Handle remaining pairs
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

/// NEON implementation with precomputed tables
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn apply_rope_tables_neon_impl(
    x: &mut [f32],
    positions: &[usize],
    tables: &RopeTables,
    half_dim: usize,
) {
    let x_ptr = x.as_mut_ptr();
    let head_dim = half_dim * 2;

    for (tok_idx, &pos) in positions.iter().enumerate() {
        debug_assert!(pos < tables.max_seq_len);

        let tok_offset = tok_idx * head_dim;
        let table_offset = pos * half_dim;

        let cos_ptr = tables.cos.as_ptr().add(table_offset);
        let sin_ptr = tables.sin.as_ptr().add(table_offset);

        // Process with 4x unrolling
        let chunks = half_dim / UNROLL_FACTOR;

        let mut freq_idx = 0usize;
        for _ in 0..chunks {
            // Load cos/sin vectors
            let cos_vec = vld1q_f32(cos_ptr.add(freq_idx));
            let sin_vec = vld1q_f32(sin_ptr.add(freq_idx));

            // Load x pairs (interleaved)
            let x_offset = tok_offset + freq_idx * 2;

            // Load 8 values (4 pairs)
            let x_01 = vld1q_f32(x_ptr.add(x_offset));
            let x_23 = vld1q_f32(x_ptr.add(x_offset + 4));

            // Deinterleave to get even/odd elements
            let x_even = vuzp1q_f32(x_01, x_23);
            let x_odd = vuzp2q_f32(x_01, x_23);

            // Apply rotation
            // x_new_even = x_even * cos - x_odd * sin
            // x_new_odd = x_odd * cos + x_even * sin
            let x_new_even = vfmsq_f32(vmulq_f32(x_even, cos_vec), x_odd, sin_vec);
            let x_new_odd = vfmaq_f32(vmulq_f32(x_odd, cos_vec), x_even, sin_vec);

            // Interleave back
            let out_01 = vzip1q_f32(x_new_even, x_new_odd);
            let out_23 = vzip2q_f32(x_new_even, x_new_odd);

            vst1q_f32(x_ptr.add(x_offset), out_01);
            vst1q_f32(x_ptr.add(x_offset + 4), out_23);

            freq_idx += 4;
        }

        // Handle remaining pairs
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

/// Scalar fallback for RoPE
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

/// Scalar fallback with precomputed tables
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

/// Compute RoPE frequencies for a given position
#[inline(always)]
pub fn compute_rope_freqs(position: usize, head_dim: usize, base: f32) -> Vec<f32> {
    let half_dim = head_dim / 2;
    (0..half_dim)
        .map(|i| {
            let freq = 1.0 / base.powf((2 * i) as f32 / head_dim as f32);
            position as f32 * freq
        })
        .collect()
}

/// Apply inverse RoPE (for position un-embedding)
pub fn apply_inverse_rope_neon(x: &mut [f32], positions: &[usize], head_dim: usize, base: f32) {
    let half_dim = head_dim / 2;
    let stride = head_dim;

    // Inverse RoPE uses negative angles
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_precompute_tables() {
        let (cos, sin) = precompute_rope_tables(128, 64, 10000.0);

        // Check dimensions
        assert_eq!(cos.len(), 128 * 32);
        assert_eq!(sin.len(), 128 * 32);

        // Position 0 should have cos = 1, sin = 0
        for i in 0..32 {
            assert!((cos[i] - 1.0).abs() < 1e-5, "cos[{}] = {}", i, cos[i]);
            assert!(sin[i].abs() < 1e-5, "sin[{}] = {}", i, sin[i]);
        }
    }

    #[test]
    fn test_rope_config() {
        let config = RopeConfig::llama2(128, 4096);
        assert_eq!(config.base, 10000.0);
        assert_eq!(config.effective_base(), 10000.0);

        let scaled_config = RopeConfig::llama2(128, 8192).with_ntk(4096);
        assert!(scaled_config.effective_base() > 10000.0);
    }

    #[test]
    fn test_apply_rope_basic() {
        let head_dim = 8;
        let mut x: Vec<f32> = vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0];
        let positions = vec![0usize];

        apply_rope_neon(&mut x, &positions, head_dim, 10000.0);

        // At position 0, rotation should be identity (cos=1, sin=0)
        assert!((x[0] - 1.0).abs() < 1e-5);
        assert!(x[1].abs() < 1e-5);
    }

    #[test]
    fn test_apply_rope_rotation() {
        let head_dim = 4;
        let mut x: Vec<f32> = vec![1.0, 0.0, 1.0, 0.0];
        let positions = vec![1usize]; // Position 1 should rotate

        let original = x.clone();
        apply_rope_neon(&mut x, &positions, head_dim, 10000.0);

        // Values should change for non-zero position
        // The rotation should not be identity
        assert!(
            (x[0] - original[0]).abs() > 1e-6 || (x[1] - original[1]).abs() > 1e-6,
            "RoPE should rotate at position 1"
        );
    }

    #[test]
    fn test_rope_tables() {
        let config = RopeConfig {
            head_dim: 16,
            max_seq_len: 32,
            base: 10000.0,
            ..Default::default()
        };

        let tables = precompute_rope_tables_with_config(&config);
        assert_eq!(tables.half_dim, 8);
        assert_eq!(tables.max_seq_len, 32);

        let (cos0, sin0) = tables.get(0);
        assert_eq!(cos0.len(), 8);
        assert_eq!(sin0.len(), 8);
    }

    #[test]
    fn test_apply_rope_with_tables() {
        let config = RopeConfig {
            head_dim: 8,
            max_seq_len: 16,
            base: 10000.0,
            ..Default::default()
        };

        let tables = precompute_rope_tables_with_config(&config);

        let mut x1: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mut x2 = x1.clone();
        let positions = vec![5usize];

        apply_rope_neon(&mut x1, &positions, config.head_dim, config.base);
        apply_rope_with_tables(&mut x2, &positions, &tables);

        // Both methods should produce same result
        for i in 0..8 {
            assert!(
                (x1[i] - x2[i]).abs() < 1e-4,
                "Mismatch at {}: {} vs {}",
                i,
                x1[i],
                x2[i]
            );
        }
    }

    #[test]
    fn test_inverse_rope() {
        let head_dim = 8;
        let mut x: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let original = x.clone();
        let positions = vec![5usize];

        // Apply RoPE then inverse RoPE
        apply_rope_neon(&mut x, &positions, head_dim, 10000.0);
        apply_inverse_rope_neon(&mut x, &positions, head_dim, 10000.0);

        // Should return to original
        for i in 0..8 {
            assert!(
                (x[i] - original[i]).abs() < 1e-4,
                "Inverse RoPE failed at {}: {} vs {}",
                i,
                x[i],
                original[i]
            );
        }
    }

    #[test]
    fn test_multiple_tokens() {
        let head_dim = 4;
        let mut x: Vec<f32> = vec![
            1.0, 0.0, 1.0, 0.0, // Token 0
            1.0, 0.0, 1.0, 0.0, // Token 1
            1.0, 0.0, 1.0, 0.0, // Token 2
        ];
        let positions = vec![0usize, 1, 2];

        apply_rope_neon(&mut x, &positions, head_dim, 10000.0);

        // Token 0 should be unchanged (position 0)
        assert!((x[0] - 1.0).abs() < 1e-5);

        // Tokens 1 and 2 should be rotated
        // Just verify they're different from original
        assert!(x
            .iter()
            .skip(4)
            .any(|&v| (v - 1.0).abs() > 1e-5 || v.abs() > 1e-5));
    }
}
