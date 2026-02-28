//! Quantized Feed-Forward Network (FFN) layer.
//!
//! Implements the FFN sublayer of the transformer with INT8 quantization:
//! FFN(x) = activation(x @ W1) @ W2
//!
//! Uses GELU activation as standard in transformer architectures (Vaswani et al., 2017).
//! Quantization reduces memory bandwidth and enables SIMD acceleration.
//!
//! ## SIMD Optimization
//!
//! When the `simd` feature is enabled, uses vectorized GELU and quantization:
//! - x86_64: AVX2 for 8 f32 ops/cycle (6-8× speedup)
//! - aarch64: NEON for 4 f32 ops/cycle (4× speedup)
//!
//! ## References
//!
//! - Vaswani, A., et al. (2017). Attention is all you need. NeurIPS 2017.

extern crate alloc;

#[cfg(all(feature = "simd", target_arch = "x86_64"))]
use core::arch::x86_64::*;

use crate::kernel::qgemm::qgemm_i8;

/// GELU approximation.
///
/// Uses the fast approximation: 0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
#[inline]
pub fn gelu_approx(x: f32) -> f32 {
    const SQRT_2_OVER_PI: f32 = 0.7978845608;
    const COEFF: f32 = 0.044715;

    let x3 = x * x * x;
    let inner = SQRT_2_OVER_PI * (x + COEFF * x3);
    0.5 * x * (1.0 + fast_tanh(inner))
}

/// Fast tanh approximation.
#[inline]
fn fast_tanh(x: f32) -> f32 {
    // Pade approximation
    let x2 = x * x;
    let num = x * (27.0 + x2);
    let den = 27.0 + 9.0 * x2;
    num / den
}

/// SIMD GELU for 8 f32 values using AVX2.
///
/// Expected speedup: 6-8× over scalar.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn gelu_approx_avx2(x: __m256) -> __m256 {
    // Constants
    let sqrt_2_over_pi = _mm256_set1_ps(0.7978845608);
    let coeff = _mm256_set1_ps(0.044715);
    let half = _mm256_set1_ps(0.5);
    let one = _mm256_set1_ps(1.0);
    let c27 = _mm256_set1_ps(27.0);
    let c9 = _mm256_set1_ps(9.0);

    // x^3
    let x2 = _mm256_mul_ps(x, x);
    let x3 = _mm256_mul_ps(x2, x);

    // inner = sqrt(2/pi) * (x + 0.044715 * x^3)
    let inner = _mm256_mul_ps(sqrt_2_over_pi, _mm256_add_ps(x, _mm256_mul_ps(coeff, x3)));

    // fast_tanh: (x * (27 + x^2)) / (27 + 9*x^2)
    let inner2 = _mm256_mul_ps(inner, inner);
    let num = _mm256_mul_ps(inner, _mm256_add_ps(c27, inner2));
    let den = _mm256_add_ps(c27, _mm256_mul_ps(c9, inner2));
    let tanh_val = _mm256_div_ps(num, den);

    // 0.5 * x * (1 + tanh)
    _mm256_mul_ps(half, _mm256_mul_ps(x, _mm256_add_ps(one, tanh_val)))
}

/// Apply GELU activation using SIMD when available.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn apply_gelu_simd(input: &[i32], scale: f32, output: &mut [f32]) {
    let scale_vec = _mm256_set1_ps(scale);
    let chunks = input.len() / 8;

    // Process 8 elements at a time
    for i in 0..chunks {
        let offset = i * 8;

        // Load 8 i32 values
        let i32_vec = _mm256_loadu_si256(input[offset..].as_ptr() as *const __m256i);

        // Convert to f32
        let f32_vec = _mm256_cvtepi32_ps(i32_vec);

        // Scale
        let scaled = _mm256_mul_ps(f32_vec, scale_vec);

        // Apply GELU
        let result = gelu_approx_avx2(scaled);

        // Store
        _mm256_storeu_ps(output[offset..].as_mut_ptr(), result);
    }

    // Handle remainder
    for i in (chunks * 8)..input.len() {
        let x_f32 = (input[i] as f32) * scale;
        output[i] = gelu_approx(x_f32);
    }
}

/// SIMD quantize f32 to i8 using AVX2.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn quantize_f32_to_i8_simd(input: &[f32], inv_scale: f32, output: &mut [i8]) {
    let inv_scale_vec = _mm256_set1_ps(inv_scale);
    let min_val = _mm256_set1_ps(-128.0);
    let max_val = _mm256_set1_ps(127.0);
    let chunks = input.len() / 8;

    for i in 0..chunks {
        let offset = i * 8;

        // Load 8 f32 values
        let f32_vec = _mm256_loadu_ps(input[offset..].as_ptr());

        // Scale and round
        let scaled = _mm256_mul_ps(f32_vec, inv_scale_vec);
        let rounded = _mm256_round_ps(scaled, _MM_FROUND_TO_NEAREST_INT | _MM_FROUND_NO_EXC);

        // Clamp to [-128, 127]
        let clamped = _mm256_min_ps(_mm256_max_ps(rounded, min_val), max_val);

        // Convert to i32
        let i32_vec = _mm256_cvtps_epi32(clamped);

        // Pack i32 -> i16 -> i8 (we need to extract and pack manually)
        // Extract to scalar and pack
        let mut temp = [0i32; 8];
        _mm256_storeu_si256(temp.as_mut_ptr() as *mut __m256i, i32_vec);

        for j in 0..8 {
            output[offset + j] = temp[j] as i8;
        }
    }

    // Handle remainder
    for i in (chunks * 8)..input.len() {
        let q = (input[i] * inv_scale).round();
        output[i] = q.clamp(-128.0, 127.0) as i8;
    }
}

// =============================================================================
// NEON SIMD implementations for aarch64
// =============================================================================

/// SIMD GELU for 4 f32 values using NEON.
///
/// Expected speedup: 4× over scalar.
#[cfg(all(feature = "simd", target_arch = "aarch64"))]
#[inline]
unsafe fn gelu_approx_neon(
    x: core::arch::aarch64::float32x4_t,
) -> core::arch::aarch64::float32x4_t {
    use core::arch::aarch64::*;

    // Constants
    let sqrt_2_over_pi = vdupq_n_f32(0.7978845608);
    let coeff = vdupq_n_f32(0.044715);
    let half = vdupq_n_f32(0.5);
    let one = vdupq_n_f32(1.0);
    let c27 = vdupq_n_f32(27.0);
    let c9 = vdupq_n_f32(9.0);

    // x^3
    let x2 = vmulq_f32(x, x);
    let x3 = vmulq_f32(x2, x);

    // inner = sqrt(2/pi) * (x + 0.044715 * x^3)
    let inner = vmulq_f32(sqrt_2_over_pi, vaddq_f32(x, vmulq_f32(coeff, x3)));

    // fast_tanh: (x * (27 + x^2)) / (27 + 9*x^2)
    let inner2 = vmulq_f32(inner, inner);
    let num = vmulq_f32(inner, vaddq_f32(c27, inner2));
    let den = vaddq_f32(c27, vmulq_f32(c9, inner2));

    // Division using reciprocal estimate + Newton-Raphson
    let den_recip = vrecpeq_f32(den);
    let den_recip = vmulq_f32(vrecpsq_f32(den, den_recip), den_recip);
    let tanh_val = vmulq_f32(num, den_recip);

    // 0.5 * x * (1 + tanh)
    vmulq_f32(half, vmulq_f32(x, vaddq_f32(one, tanh_val)))
}

/// Apply GELU activation using NEON SIMD.
#[cfg(all(feature = "simd", target_arch = "aarch64"))]
unsafe fn apply_gelu_neon(input: &[i32], scale: f32, output: &mut [f32]) {
    use core::arch::aarch64::*;

    let scale_vec = vdupq_n_f32(scale);
    let chunks = input.len() / 4;

    // Process 4 elements at a time
    for i in 0..chunks {
        let offset = i * 4;

        // Load 4 i32 values
        let i32_vec = vld1q_s32(input[offset..].as_ptr());

        // Convert to f32
        let f32_vec = vcvtq_f32_s32(i32_vec);

        // Scale
        let scaled = vmulq_f32(f32_vec, scale_vec);

        // Apply GELU
        let result = gelu_approx_neon(scaled);

        // Store
        vst1q_f32(output[offset..].as_mut_ptr(), result);
    }

    // Handle remainder
    for i in (chunks * 4)..input.len() {
        let x_f32 = (input[i] as f32) * scale;
        output[i] = gelu_approx(x_f32);
    }
}

/// SIMD quantize f32 to i8 using NEON.
#[cfg(all(feature = "simd", target_arch = "aarch64"))]
unsafe fn quantize_f32_to_i8_neon(input: &[f32], inv_scale: f32, output: &mut [i8]) {
    use core::arch::aarch64::*;

    let inv_scale_vec = vdupq_n_f32(inv_scale);
    let min_val = vdupq_n_f32(-128.0);
    let max_val = vdupq_n_f32(127.0);
    let chunks = input.len() / 4;

    for i in 0..chunks {
        let offset = i * 4;

        // Load 4 f32 values
        let f32_vec = vld1q_f32(input[offset..].as_ptr());

        // Scale
        let scaled = vmulq_f32(f32_vec, inv_scale_vec);

        // Round to nearest
        let rounded = vrndnq_f32(scaled);

        // Clamp to [-128, 127]
        let clamped = vminq_f32(vmaxq_f32(rounded, min_val), max_val);

        // Convert to i32
        let i32_vec = vcvtq_s32_f32(clamped);

        // Narrow to i16 then i8
        let i16_vec = vmovn_s32(i32_vec);
        let i16_vec_q = vcombine_s16(i16_vec, i16_vec);
        let i8_vec = vmovn_s16(i16_vec_q);

        // Store only 4 bytes
        for j in 0..4 {
            output[offset + j] = vget_lane_s8(i8_vec, j as i32) as i8;
        }
    }

    // Handle remainder
    for i in (chunks * 4)..input.len() {
        let q = (input[i] * inv_scale).round();
        output[i] = q.clamp(-128.0, 127.0) as i8;
    }
}

/// ReLU activation.
#[inline]
pub fn relu(x: f32) -> f32 {
    x.max(0.0)
}

/// Apply activation function to i32 buffer, producing f32.
///
/// This handles the dequantization and activation in one pass.
/// Uses SIMD when available for 6-8× speedup on GELU.
#[inline]
pub fn apply_activation_i32_to_f32(
    input: &[i32],
    scale: f32,
    activation: ActivationType,
    output: &mut [f32],
) {
    debug_assert_eq!(input.len(), output.len());

    match activation {
        ActivationType::Gelu => {
            // Use SIMD path when available
            #[cfg(all(feature = "simd", target_arch = "x86_64", target_feature = "avx2"))]
            {
                // SAFETY: target_feature check ensures AVX2 is available
                unsafe {
                    apply_gelu_simd(input, scale, output);
                }
                return;
            }

            // NEON path for aarch64
            #[cfg(all(feature = "simd", target_arch = "aarch64"))]
            {
                // SAFETY: NEON is always available on aarch64
                unsafe {
                    apply_gelu_neon(input, scale, output);
                }
                return;
            }

            // Scalar fallback
            #[allow(unreachable_code)]
            for (i, &x) in input.iter().enumerate() {
                let x_f32 = (x as f32) * scale;
                output[i] = gelu_approx(x_f32);
            }
        }
        ActivationType::Relu => {
            for (i, &x) in input.iter().enumerate() {
                let x_f32 = (x as f32) * scale;
                output[i] = relu(x_f32);
            }
        }
        ActivationType::None => {
            for (i, &x) in input.iter().enumerate() {
                output[i] = (x as f32) * scale;
            }
        }
    }
}

/// Activation function type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivationType {
    /// GELU activation (default for transformers)
    Gelu,
    /// ReLU activation
    Relu,
    /// No activation (linear)
    None,
}

impl Default for ActivationType {
    fn default() -> Self {
        ActivationType::Gelu
    }
}

/// FFN layer configuration.
#[derive(Clone, Debug)]
pub struct FfnConfig {
    /// Hidden dimension
    pub hidden: usize,

    /// Intermediate dimension (usually 4 * hidden)
    pub intermediate: usize,

    /// Activation type
    pub activation: ActivationType,
}

impl FfnConfig {
    /// Create FFN config with default GELU activation.
    pub fn new(hidden: usize, intermediate: usize) -> Self {
        Self {
            hidden,
            intermediate,
            activation: ActivationType::Gelu,
        }
    }

    /// Create FFN config with specified activation.
    pub fn with_activation(hidden: usize, intermediate: usize, activation: ActivationType) -> Self {
        Self {
            hidden,
            intermediate,
            activation,
        }
    }
}

/// Quantized FFN layer.
///
/// Computes: output = activation(input @ W1 + b1) @ W2 + b2
pub struct QuantizedFfn {
    config: FfnConfig,
}

impl QuantizedFfn {
    /// Create new FFN layer.
    pub fn new(config: FfnConfig) -> Self {
        Self { config }
    }

    /// Forward pass for FFN.
    ///
    /// # Arguments
    ///
    /// * `input` - Input tensor, shape [seq_len, hidden], i8
    /// * `input_scale` - Scale for input
    /// * `w1` - First layer weights, shape [intermediate, hidden], i8
    /// * `w1_scales` - Per-row scales for W1
    /// * `b1` - First layer bias, shape [intermediate], i32
    /// * `w2` - Second layer weights, shape [hidden, intermediate], i8
    /// * `w2_scales` - Per-row scales for W2
    /// * `b2` - Second layer bias, shape [hidden], i32
    /// * `intermediate_buf` - Scratch buffer, shape [seq_len, intermediate], i32
    /// * `activation_buf` - Scratch buffer, shape [seq_len, intermediate], f32
    /// * `activation_i8_buf` - Scratch buffer for quantized activations, shape [seq_len, intermediate], i8
    /// * `output` - Output buffer, shape [seq_len, hidden], i32
    ///
    /// # Allocation-Free Guarantee
    ///
    /// This function performs no heap allocations. All buffers must be pre-allocated.
    #[allow(clippy::too_many_arguments)]
    pub fn forward(
        &self,
        input: &[i8],
        input_scale: f32,
        w1: &[i8],
        w1_scales: &[f32],
        b1: Option<&[i32]>,
        w2: &[i8],
        w2_scales: &[f32],
        b2: Option<&[i32]>,
        seq_len: usize,
        intermediate_buf: &mut [i32],
        activation_buf: &mut [f32],
        activation_i8_buf: &mut [i8],
        output: &mut [i32],
    ) {
        let hidden = self.config.hidden;
        let intermediate = self.config.intermediate;

        // First linear: [seq_len, hidden] @ [intermediate, hidden]^T -> [seq_len, intermediate]
        qgemm_i8(
            seq_len,
            intermediate,
            hidden,
            input,
            input_scale,
            w1,
            w1_scales,
            b1,
            intermediate_buf,
        );

        // Apply activation
        let scale = w1_scales.get(0).copied().unwrap_or(1.0) * input_scale;
        apply_activation_i32_to_f32(
            intermediate_buf,
            scale,
            self.config.activation,
            activation_buf,
        );

        // Quantize back to i8 for second matmul (allocation-free)
        let activation_scale = compute_activation_scale(activation_buf);
        let buf_len = activation_i8_buf
            .len()
            .min(seq_len.saturating_mul(intermediate));
        quantize_f32_to_i8(
            &activation_buf[..buf_len],
            activation_scale,
            &mut activation_i8_buf[..buf_len],
        );

        // Second linear: [seq_len, intermediate] @ [hidden, intermediate]^T -> [seq_len, hidden]
        qgemm_i8(
            seq_len,
            hidden,
            intermediate,
            activation_i8_buf,
            activation_scale,
            w2,
            w2_scales,
            b2,
            output,
        );
    }

    /// Get configuration
    pub fn config(&self) -> &FfnConfig {
        &self.config
    }
}

/// Compute appropriate scale for activation values.
#[inline]
fn compute_activation_scale(values: &[f32]) -> f32 {
    let max_abs = values.iter().map(|&v| v.abs()).fold(0.0f32, f32::max);
    if max_abs == 0.0 {
        1.0
    } else {
        max_abs / 127.0
    }
}

/// Quantize f32 to i8.
///
/// Uses SIMD when available for 4-8× speedup.
#[inline]
fn quantize_f32_to_i8(input: &[f32], scale: f32, output: &mut [i8]) {
    let inv_scale = 1.0 / scale;

    // Use SIMD path when available
    #[cfg(all(feature = "simd", target_arch = "x86_64", target_feature = "avx2"))]
    {
        // SAFETY: target_feature check ensures AVX2 is available
        unsafe {
            quantize_f32_to_i8_simd(input, inv_scale, output);
        }
        return;
    }

    // NEON path for aarch64
    #[cfg(all(feature = "simd", target_arch = "aarch64"))]
    {
        // SAFETY: NEON is always available on aarch64
        unsafe {
            quantize_f32_to_i8_neon(input, inv_scale, output);
        }
        return;
    }

    // Scalar fallback
    #[allow(unreachable_code)]
    for (i, &v) in input.iter().enumerate() {
        let q = (v * inv_scale).round();
        output[i] = q.clamp(-128.0, 127.0) as i8;
    }
}

/// Fused residual + FFN operation.
///
/// Computes: output = residual + FFN(input)
pub fn residual_ffn(
    residual: &[i8],
    ffn_output: &[i32],
    ffn_scale: f32,
    output: &mut [i8],
    output_scale: f32,
) {
    debug_assert_eq!(residual.len(), ffn_output.len());
    debug_assert_eq!(residual.len(), output.len());

    let inv_out_scale = 1.0 / output_scale;

    for i in 0..residual.len() {
        let res = residual[i] as f32 * output_scale;
        let ffn = ffn_output[i] as f32 * ffn_scale;
        let sum = res + ffn;
        let q = (sum * inv_out_scale).round();
        output[i] = q.clamp(-128.0, 127.0) as i8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gelu_approx() {
        // GELU(0) = 0
        assert!(gelu_approx(0.0).abs() < 1e-5);

        // GELU is monotonic for positive values
        assert!(gelu_approx(1.0) > gelu_approx(0.5));
        assert!(gelu_approx(2.0) > gelu_approx(1.0));

        // GELU is approximately x for large positive x
        let large = gelu_approx(3.0);
        assert!((large - 3.0).abs() < 0.1);
    }

    #[test]
    fn test_relu() {
        assert_eq!(relu(1.0), 1.0);
        assert_eq!(relu(-1.0), 0.0);
        assert_eq!(relu(0.0), 0.0);
    }

    #[test]
    fn test_apply_activation() {
        let input: [i32; 4] = [100, -100, 0, 50];
        let mut output = [0.0f32; 4];

        apply_activation_i32_to_f32(&input, 0.01, ActivationType::Relu, &mut output);

        assert!((output[0] - 1.0).abs() < 1e-5);
        assert!((output[1] - 0.0).abs() < 1e-5); // ReLU clips negative
        assert!((output[2] - 0.0).abs() < 1e-5);
        assert!((output[3] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_ffn_config() {
        let config = FfnConfig::new(256, 1024);
        assert_eq!(config.hidden, 256);
        assert_eq!(config.intermediate, 1024);
        assert_eq!(config.activation, ActivationType::Gelu);
    }

    #[test]
    fn test_quantize_f32_i8() {
        let input: [f32; 4] = [1.0, -1.0, 0.5, -0.5];
        let mut output = [0i8; 4];

        quantize_f32_to_i8(&input, 1.0 / 127.0, &mut output);

        assert_eq!(output[0], 127);
        assert_eq!(output[1], -127);
        assert!(output[2] > 0);
        assert!(output[3] < 0);
    }
}
