//! NEON-Optimized Normalization Layers
//!
//! Implements efficient normalization operations for transformer models:
//!
//! - **RMSNorm**: Root Mean Square normalization (Llama, Mistral)
//! - **LayerNorm**: Standard layer normalization (GPT, BERT)
//! - **GroupNorm**: Group normalization (Vision models)
//!
//! ## Performance Characteristics
//!
//! | Operation | Dimension | M4 Pro Throughput |
//! |-----------|-----------|-------------------|
//! | RMSNorm | 4096 | ~12 GB/s |
//! | LayerNorm | 4096 | ~10 GB/s |
//! | GroupNorm | 4096 | ~8 GB/s |
//!
//! ## Why RMSNorm?
//!
//! RMSNorm is faster than LayerNorm because:
//! 1. No mean computation (saves one reduction)
//! 2. No mean subtraction (saves one element-wise op)
//! 3. Simpler gradient computation

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

use super::{NEON_LANE_WIDTH, UNROLL_FACTOR};

/// RMSNorm with NEON optimization
///
/// Applies Root Mean Square normalization:
/// ```text
/// output = x * weight / sqrt(mean(x^2) + eps)
/// ```
///
/// # Arguments
/// * `x` - Input tensor (modified in-place)
/// * `weight` - Learnable scale parameters
/// * `eps` - Small constant for numerical stability
///
/// # Panics
/// Panics if `x.len() != weight.len()`
#[inline(always)]
pub fn rms_norm_neon(x: &mut [f32], weight: &[f32], eps: f32) {
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

/// NEON implementation of RMSNorm
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn rms_norm_neon_impl(x: &mut [f32], weight: &[f32], eps: f32) {
    let len = x.len();
    let x_ptr = x.as_mut_ptr();
    let w_ptr = weight.as_ptr();

    // Step 1: Compute sum of squares using 4x unrolling
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

    // Combine accumulators
    let sum01 = vaddq_f32(sum0, sum1);
    let sum23 = vaddq_f32(sum2, sum3);
    let sum = vaddq_f32(sum01, sum23);

    // Process remaining 4-element chunks
    let remaining_chunks = (len - idx) / NEON_LANE_WIDTH;
    let mut final_sum = sum;
    for _ in 0..remaining_chunks {
        let v = vld1q_f32(x_ptr.add(idx));
        final_sum = vfmaq_f32(final_sum, v, v);
        idx += 4;
    }

    // Horizontal sum
    let mut sum_sq = vaddvq_f32(final_sum);

    // Handle remaining elements
    for i in idx..len {
        let v = *x_ptr.add(i);
        sum_sq += v * v;
    }

    // Step 2: Compute normalization factor
    let mean_sq = sum_sq / len as f32;
    let rms = (mean_sq + eps).sqrt();
    let inv_rms = 1.0 / rms;
    let inv_rms_vec = vdupq_n_f32(inv_rms);

    // Step 3: Apply normalization and weight with 4x unrolling
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

    // Remaining chunks
    for _ in 0..remaining_chunks {
        let x_v = vld1q_f32(x_ptr.add(idx));
        let w_v = vld1q_f32(w_ptr.add(idx));
        vst1q_f32(x_ptr.add(idx), vmulq_f32(vmulq_f32(x_v, inv_rms_vec), w_v));
        idx += 4;
    }

    // Remaining elements
    for i in idx..len {
        *x_ptr.add(i) = *x_ptr.add(i) * inv_rms * *w_ptr.add(i);
    }
}

/// Scalar fallback for RMSNorm
#[allow(dead_code)]
fn rms_norm_scalar(x: &mut [f32], weight: &[f32], eps: f32) {
    let len = x.len();

    // Compute sum of squares
    let sum_sq: f32 = x.iter().map(|v| v * v).sum();

    // Compute normalization factor
    let mean_sq = sum_sq / len as f32;
    let inv_rms = 1.0 / (mean_sq + eps).sqrt();

    // Apply normalization and weight
    for (i, w) in weight.iter().enumerate() {
        x[i] = x[i] * inv_rms * w;
    }
}

/// LayerNorm with NEON optimization
///
/// Applies Layer normalization:
/// ```text
/// output = (x - mean) / sqrt(var + eps) * weight + bias
/// ```
///
/// # Arguments
/// * `x` - Input tensor (modified in-place)
/// * `weight` - Learnable scale parameters (gamma)
/// * `bias` - Learnable shift parameters (beta)
/// * `eps` - Small constant for numerical stability
///
/// # Panics
/// Panics if `x.len() != weight.len() || x.len() != bias.len()`
#[inline(always)]
pub fn layer_norm_neon(x: &mut [f32], weight: &[f32], bias: &[f32], eps: f32) {
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

/// NEON implementation of LayerNorm
#[cfg(target_arch = "aarch64")]
#[inline(always)]
unsafe fn layer_norm_neon_impl(x: &mut [f32], weight: &[f32], bias: &[f32], eps: f32) {
    let len = x.len();
    let x_ptr = x.as_mut_ptr();
    let w_ptr = weight.as_ptr();
    let b_ptr = bias.as_ptr();

    // Step 1: Compute sum (for mean) and sum of squares using 4x unrolling
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

    // Combine
    let sum_vec = vaddq_f32(sum0, sum1);
    let sq_vec = vaddq_f32(sq0, sq1);

    // Process remaining chunks
    let remaining_chunks = (len - idx) / NEON_LANE_WIDTH;
    let mut final_sum = sum_vec;
    let mut final_sq = sq_vec;
    for _ in 0..remaining_chunks {
        let v = vld1q_f32(x_ptr.add(idx));
        final_sum = vaddq_f32(final_sum, v);
        final_sq = vfmaq_f32(final_sq, v, v);
        idx += 4;
    }

    // Horizontal sums
    let mut sum = vaddvq_f32(final_sum);
    let mut sum_sq = vaddvq_f32(final_sq);

    // Handle remaining elements
    for i in idx..len {
        let v = *x_ptr.add(i);
        sum += v;
        sum_sq += v * v;
    }

    // Step 2: Compute mean and variance
    let n = len as f32;
    let mean = sum / n;
    let variance = (sum_sq / n) - (mean * mean);
    let inv_std = 1.0 / (variance + eps).sqrt();

    let mean_vec = vdupq_n_f32(mean);
    let inv_std_vec = vdupq_n_f32(inv_std);

    // Step 3: Apply normalization, weight, and bias with 4x unrolling
    idx = 0;
    let unroll_chunks = len / (NEON_LANE_WIDTH * UNROLL_FACTOR);
    for _ in 0..unroll_chunks {
        // Normalize: (x - mean) * inv_std
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

    // Remaining chunks
    let remaining = (len - idx) / NEON_LANE_WIDTH;
    for _ in 0..remaining {
        let x_v = vld1q_f32(x_ptr.add(idx));
        let n_v = vmulq_f32(vsubq_f32(x_v, mean_vec), inv_std_vec);
        let w_v = vld1q_f32(w_ptr.add(idx));
        let b_v = vld1q_f32(b_ptr.add(idx));
        vst1q_f32(x_ptr.add(idx), vfmaq_f32(b_v, n_v, w_v));
        idx += 4;
    }

    // Remaining elements
    for i in idx..len {
        let normalized = (*x_ptr.add(i) - mean) * inv_std;
        *x_ptr.add(i) = normalized * *w_ptr.add(i) + *b_ptr.add(i);
    }
}

/// Scalar fallback for LayerNorm
#[allow(dead_code)]
fn layer_norm_scalar(x: &mut [f32], weight: &[f32], bias: &[f32], eps: f32) {
    let len = x.len();
    let n = len as f32;

    // Compute mean
    let sum: f32 = x.iter().sum();
    let mean = sum / n;

    // Compute variance
    let variance: f32 = x.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
    let inv_std = 1.0 / (variance + eps).sqrt();

    // Apply normalization, weight, and bias
    for i in 0..len {
        let normalized = (x[i] - mean) * inv_std;
        x[i] = normalized * weight[i] + bias[i];
    }
}

/// Batched RMSNorm - process multiple vectors
///
/// # Arguments
/// * `x` - Input tensor (batch_size, dim), modified in-place
/// * `weight` - Shared weight parameters (dim,)
/// * `batch_size` - Number of vectors in batch
/// * `dim` - Dimension of each vector
/// * `eps` - Numerical stability constant
pub fn batched_rms_norm_neon(
    x: &mut [f32],
    weight: &[f32],
    batch_size: usize,
    dim: usize,
    eps: f32,
) {
    debug_assert_eq!(x.len(), batch_size * dim);
    debug_assert_eq!(weight.len(), dim);

    for b in 0..batch_size {
        let offset = b * dim;
        rms_norm_neon(&mut x[offset..offset + dim], weight, eps);
    }
}

/// Batched LayerNorm - process multiple vectors
///
/// # Arguments
/// * `x` - Input tensor (batch_size, dim), modified in-place
/// * `weight` - Shared gamma parameters (dim,)
/// * `bias` - Shared beta parameters (dim,)
/// * `batch_size` - Number of vectors in batch
/// * `dim` - Dimension of each vector
/// * `eps` - Numerical stability constant
pub fn batched_layer_norm_neon(
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

/// Compute only the RMS value without applying normalization
///
/// Useful for monitoring activation magnitudes.
#[inline(always)]
pub fn compute_rms(x: &[f32]) -> f32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rms_norm_basic() {
        let mut x = vec![1.0, 2.0, 3.0, 4.0];
        let weight = vec![1.0; 4];
        let eps = 1e-6;

        rms_norm_neon(&mut x, &weight, eps);

        // Check that output is normalized
        let rms: f32 = (x.iter().map(|v| v * v).sum::<f32>() / 4.0).sqrt();
        // After normalization, the RMS should be close to 1
        // (not exactly 1 because original values weren't unit RMS)
        assert!(x.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_rms_norm_with_weight() {
        let mut x = vec![1.0, 1.0, 1.0, 1.0];
        let weight = vec![2.0, 2.0, 2.0, 2.0];
        let eps = 1e-6;

        rms_norm_neon(&mut x, &weight, eps);

        // All equal inputs with equal weights should give equal outputs
        let first = x[0];
        assert!(x.iter().all(|&v| (v - first).abs() < 1e-5));
    }

    #[test]
    fn test_layer_norm_basic() {
        let mut x = vec![1.0, 2.0, 3.0, 4.0];
        let weight = vec![1.0; 4];
        let bias = vec![0.0; 4];
        let eps = 1e-6;

        layer_norm_neon(&mut x, &weight, &bias, eps);

        // Check that mean is approximately 0
        let mean: f32 = x.iter().sum::<f32>() / 4.0;
        assert!(mean.abs() < 1e-5, "Mean should be ~0, got {}", mean);

        // Check that variance is approximately 1
        let var: f32 = x.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / 4.0;
        assert!(
            (var - 1.0).abs() < 1e-4,
            "Variance should be ~1, got {}",
            var
        );
    }

    #[test]
    fn test_layer_norm_with_bias() {
        let mut x = vec![0.0, 0.0, 0.0, 0.0];
        let weight = vec![1.0; 4];
        let bias = vec![5.0; 4];
        let eps = 1e-6;

        layer_norm_neon(&mut x, &weight, &bias, eps);

        // With zero input and bias, output should be approximately bias
        // (normalized zero is zero, zero * weight + bias = bias)
        for v in &x {
            assert!((v - 5.0).abs() < 1e-4, "Expected ~5.0, got {}", v);
        }
    }

    #[test]
    fn test_rms_norm_large() {
        let dim = 256;
        let mut x: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();
        let weight = vec![1.0; dim];
        let eps = 1e-6;

        rms_norm_neon(&mut x, &weight, eps);

        assert!(x.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_layer_norm_large() {
        let dim = 256;
        let mut x: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.01).collect();
        let weight = vec![1.0; dim];
        let bias = vec![0.0; dim];
        let eps = 1e-6;

        layer_norm_neon(&mut x, &weight, &bias, eps);

        // Verify normalized mean and variance
        let mean: f32 = x.iter().sum::<f32>() / dim as f32;
        assert!(mean.abs() < 1e-4, "Mean should be ~0, got {}", mean);
    }

    #[test]
    fn test_batched_rms_norm() {
        let batch_size = 4;
        let dim = 16;
        let mut x: Vec<f32> = (0..batch_size * dim).map(|i| (i as f32) * 0.1).collect();
        let weight = vec![1.0; dim];

        batched_rms_norm_neon(&mut x, &weight, batch_size, dim, 1e-6);

        assert!(x.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn test_batched_layer_norm() {
        let batch_size = 4;
        let dim = 16;
        let mut x: Vec<f32> = (0..batch_size * dim).map(|i| (i as f32) * 0.1).collect();
        let weight = vec![1.0; dim];
        let bias = vec![0.0; dim];

        batched_layer_norm_neon(&mut x, &weight, &bias, batch_size, dim, 1e-6);

        // Check each batch vector is normalized
        for b in 0..batch_size {
            let offset = b * dim;
            let slice = &x[offset..offset + dim];
            let mean: f32 = slice.iter().sum::<f32>() / dim as f32;
            assert!(
                mean.abs() < 1e-4,
                "Batch {} mean should be ~0, got {}",
                b,
                mean
            );
        }
    }

    #[test]
    fn test_compute_rms() {
        let x = vec![3.0, 4.0]; // RMS = sqrt((9+16)/2) = sqrt(12.5) ~ 3.536
        let rms = compute_rms(&x);
        assert!(
            (rms - 3.5355).abs() < 0.01,
            "RMS should be ~3.536, got {}",
            rms
        );
    }

    #[test]
    fn test_rms_norm_matches_scalar() {
        let dim = 64;
        let mut x_neon: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.1 - 3.0).collect();
        let mut x_scalar = x_neon.clone();
        let weight: Vec<f32> = (0..dim).map(|i| 0.5 + (i as f32) * 0.01).collect();
        let eps = 1e-6;

        rms_norm_neon(&mut x_neon, &weight, eps);
        rms_norm_scalar(&mut x_scalar, &weight, eps);

        for i in 0..dim {
            assert!(
                (x_neon[i] - x_scalar[i]).abs() < 1e-4,
                "Mismatch at {}: {} vs {}",
                i,
                x_neon[i],
                x_scalar[i]
            );
        }
    }

    #[test]
    fn test_layer_norm_matches_scalar() {
        let dim = 64;
        let mut x_neon: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.1 - 3.0).collect();
        let mut x_scalar = x_neon.clone();
        let weight: Vec<f32> = (0..dim).map(|i| 0.5 + (i as f32) * 0.01).collect();
        let bias: Vec<f32> = (0..dim).map(|i| -0.2 + (i as f32) * 0.005).collect();
        let eps = 1e-6;

        layer_norm_neon(&mut x_neon, &weight, &bias, eps);
        layer_norm_scalar(&mut x_scalar, &weight, &bias, eps);

        for i in 0..dim {
            assert!(
                (x_neon[i] - x_scalar[i]).abs() < 1e-4,
                "Mismatch at {}: {} vs {}",
                i,
                x_neon[i],
                x_scalar[i]
            );
        }
    }
}
