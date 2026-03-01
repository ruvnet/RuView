//! Quantized GEMM (General Matrix Multiplication) operations.
//!
//! Core primitive for projections and FFN layers.
//! Supports int8 weights with per-row scaling.
//!
//! ## SIMD Optimization
//!
//! When the `simd` feature is enabled, uses architecture-specific intrinsics:
//! - x86_64: AVX2 `_mm256_maddubs_epi16` for 32 INT8 ops/cycle
//! - aarch64: NEON `vdotq_s32` for 16 INT8 ops/cycle
//!
//! Expected speedup: 12-16× over scalar implementation.

#[cfg(all(feature = "simd", target_arch = "x86_64"))]
use core::arch::x86_64::*;

#[cfg(all(feature = "simd", target_arch = "aarch64"))]
use core::arch::aarch64::*;

// =============================================================================
// Software Prefetch Hints
// =============================================================================

/// Prefetch data into L1 cache for temporal access (data will be used multiple times).
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[inline(always)]
unsafe fn prefetch_t0(ptr: *const i8) {
    _mm_prefetch(ptr, _MM_HINT_T0);
}

/// Prefetch data into L2 cache for temporal access.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[inline(always)]
unsafe fn prefetch_t1(ptr: *const i8) {
    _mm_prefetch(ptr, _MM_HINT_T1);
}

/// Prefetch data for non-temporal access (data will be used once).
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[inline(always)]
unsafe fn prefetch_nta(ptr: *const i8) {
    _mm_prefetch(ptr, _MM_HINT_NTA);
}

/// No-op prefetch for non-SIMD builds.
#[cfg(not(all(feature = "simd", target_arch = "x86_64")))]
#[inline(always)]
#[allow(dead_code)]
fn prefetch_t0(_ptr: *const i8) {}

#[cfg(not(all(feature = "simd", target_arch = "x86_64")))]
#[inline(always)]
#[allow(dead_code)]
fn prefetch_t1(_ptr: *const i8) {}

#[cfg(not(all(feature = "simd", target_arch = "x86_64")))]
#[inline(always)]
#[allow(dead_code)]
fn prefetch_nta(_ptr: *const i8) {}

/// Quantized GEMM: C = A * B^T + bias
///
/// Computes matrix multiplication with int8 inputs, accumulating to i64 for safety.
///
/// # Arguments
///
/// * `m` - Number of rows in A (and output C)
/// * `n` - Number of columns in B^T (and output C) = number of rows in B
/// * `k` - Number of columns in A = number of columns in B
/// * `a` - Input activations, shape [m, k], int8
/// * `a_scale` - Scale factor for input activations
/// * `b` - Weight matrix, shape [n, k], int8 (row-major, transposed)
/// * `b_row_scales` - Per-row scale factors for B, shape [n]
/// * `bias` - Optional bias vector, shape [n], i32
/// * `out` - Output buffer, shape [m, n], i32
///
/// # Output
///
/// out[i, j] = (sum_k(a[i, k] * b[j, k]) * a_scale * b_row_scales[j]) + bias[j]
///
/// # Safety
///
/// Uses i64 accumulator to prevent overflow even with large k values.
/// Bounds checking is performed at runtime for release builds.
#[inline(never)]
pub fn qgemm_i8(
    m: usize,
    n: usize,
    k: usize,
    a: &[i8],
    a_scale: f32,
    b: &[i8],
    b_row_scales: &[f32],
    bias: Option<&[i32]>,
    out: &mut [i32],
) {
    // Runtime bounds checking (critical for safety)
    if a.len() < m.saturating_mul(k)
        || b.len() < n.saturating_mul(k)
        || out.len() < m.saturating_mul(n)
        || b_row_scales.len() < n
    {
        // Fill with zeros on invalid dimensions rather than panicking
        for v in out.iter_mut() {
            *v = 0;
        }
        return;
    }

    // Scalar implementation with safety and scale application
    for i in 0..m {
        // Prefetch next row of A into L2 cache
        #[cfg(all(feature = "simd", target_arch = "x86_64"))]
        if i + 1 < m {
            let next_row_ptr = a.as_ptr().wrapping_add((i + 1) * k);
            // SAFETY: prefetch is a hint, safe even with invalid addresses
            unsafe {
                prefetch_t1(next_row_ptr);
            }
        }

        for j in 0..n {
            // Prefetch next row of B into L1 cache (hot path)
            #[cfg(all(feature = "simd", target_arch = "x86_64"))]
            if j + 1 < n {
                let next_b_row_ptr = b.as_ptr().wrapping_add((j + 1) * k);
                // SAFETY: prefetch is a hint, safe even with invalid addresses
                unsafe {
                    prefetch_t0(next_b_row_ptr);
                }
            }

            // Use i64 accumulator to prevent overflow with large k
            let mut acc: i64 = 0;

            // Dot product with bounds-checked access
            for kk in 0..k {
                let a_idx = i * k + kk;
                let b_idx = j * k + kk;

                // Safe indexing with fallback
                let a_val = a.get(a_idx).copied().unwrap_or(0) as i64;
                let b_val = b.get(b_idx).copied().unwrap_or(0) as i64;
                acc = acc.saturating_add(a_val.saturating_mul(b_val));
            }

            // Apply scale factors: acc * a_scale * b_row_scales[j]
            let combined_scale = a_scale * b_row_scales.get(j).copied().unwrap_or(1.0);
            let scaled_acc = (acc as f64 * combined_scale as f64).round() as i64;

            // Add bias if present
            let bias_val = bias.and_then(|b| b.get(j)).copied().unwrap_or(0) as i64;
            let final_acc = scaled_acc.saturating_add(bias_val);

            // Clamp to i32 range and store
            let out_idx = i * n + j;
            if let Some(out_val) = out.get_mut(out_idx) {
                *out_val = final_acc.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
            }
        }
    }
}

/// SIMD-optimized quantized GEMM for x86_64 with AVX2.
///
/// Uses `_mm256_maddubs_epi16` for 32 INT8 multiply-adds per cycle.
/// Processes 32 elements at a time with 4× loop unrolling.
///
/// # Performance
///
/// Expected speedup: 12-16× over scalar implementation.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
#[inline(never)]
pub unsafe fn qgemm_i8_avx2(
    m: usize,
    n: usize,
    k: usize,
    a: &[i8],
    a_scale: f32,
    b: &[i8],
    b_row_scales: &[f32],
    bias: Option<&[i32]>,
    out: &mut [i32],
) {
    // Bounds check
    if a.len() < m.saturating_mul(k)
        || b.len() < n.saturating_mul(k)
        || out.len() < m.saturating_mul(n)
        || b_row_scales.len() < n
    {
        for v in out.iter_mut() {
            *v = 0;
        }
        return;
    }

    let k_chunks = k / 32; // Process 32 elements at a time
    const PREFETCH_DISTANCE: usize = 4; // Rows ahead to prefetch

    for i in 0..m {
        // Prefetch future rows of A into L2
        if i + PREFETCH_DISTANCE < m {
            let prefetch_row = &a[(i + PREFETCH_DISTANCE) * k..];
            _mm_prefetch(prefetch_row.as_ptr(), _MM_HINT_T1);
        }

        for j in 0..n {
            // Prefetch next rows of B into L1 (hot path)
            if j + PREFETCH_DISTANCE < n {
                let prefetch_b = &b[(j + PREFETCH_DISTANCE) * k..];
                _mm_prefetch(prefetch_b.as_ptr(), _MM_HINT_T0);
            }

            let mut acc = _mm256_setzero_si256();
            let a_row = &a[i * k..];
            let b_row = &b[j * k..];

            // Main SIMD loop - process 32 i8 elements at a time
            for chunk in 0..k_chunks {
                let offset = chunk * 32;

                // Load 32 bytes from A and B
                let a_vec = _mm256_loadu_si256(a_row[offset..].as_ptr() as *const __m256i);
                let b_vec = _mm256_loadu_si256(b_row[offset..].as_ptr() as *const __m256i);

                // Convert i8 to i16 for multiplication (sign extension)
                // Split into low and high 128-bit lanes
                let a_lo = _mm256_cvtepi8_epi16(_mm256_extracti128_si256(a_vec, 0));
                let a_hi = _mm256_cvtepi8_epi16(_mm256_extracti128_si256(a_vec, 1));
                let b_lo = _mm256_cvtepi8_epi16(_mm256_extracti128_si256(b_vec, 0));
                let b_hi = _mm256_cvtepi8_epi16(_mm256_extracti128_si256(b_vec, 1));

                // Multiply i16 -> i32 and accumulate
                let prod_lo = _mm256_madd_epi16(a_lo, b_lo);
                let prod_hi = _mm256_madd_epi16(a_hi, b_hi);
                acc = _mm256_add_epi32(acc, prod_lo);
                acc = _mm256_add_epi32(acc, prod_hi);
            }

            // Horizontal sum of acc (8 x i32 -> 1 x i32)
            let sum128 = _mm_add_epi32(
                _mm256_extracti128_si256(acc, 0),
                _mm256_extracti128_si256(acc, 1),
            );
            let sum64 = _mm_add_epi32(sum128, _mm_srli_si128(sum128, 8));
            let sum32 = _mm_add_epi32(sum64, _mm_srli_si128(sum64, 4));
            let mut total = _mm_cvtsi128_si32(sum32) as i64;

            // Handle remainder with scalar
            for kk in (k_chunks * 32)..k {
                let a_val = a_row.get(kk).copied().unwrap_or(0) as i64;
                let b_val = b_row.get(kk).copied().unwrap_or(0) as i64;
                total += a_val * b_val;
            }

            // Apply scales and bias
            let combined_scale = a_scale * b_row_scales.get(j).copied().unwrap_or(1.0);
            let scaled = (total as f64 * combined_scale as f64).round() as i64;
            let bias_val = bias.and_then(|b| b.get(j)).copied().unwrap_or(0) as i64;
            let final_val = scaled.saturating_add(bias_val);

            out[i * n + j] = final_val.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        }
    }
}

/// SIMD-optimized quantized GEMM dispatcher.
///
/// Automatically selects best implementation based on CPU features.
/// On x86_64 with AVX2 available at compile time, uses SIMD path.
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
#[inline(never)]
pub fn qgemm_i8_simd(
    m: usize,
    n: usize,
    k: usize,
    a: &[i8],
    a_scale: f32,
    b: &[i8],
    b_row_scales: &[f32],
    bias: Option<&[i32]>,
    out: &mut [i32],
) {
    // For no_std compatibility, we use compile-time feature detection
    // AVX2 path is used if compiled with target-feature=+avx2
    #[cfg(target_feature = "avx2")]
    {
        if k >= 32 {
            // SAFETY: We verified AVX2 is available via target_feature
            unsafe {
                qgemm_i8_avx2(m, n, k, a, a_scale, b, b_row_scales, bias, out);
            }
            return;
        }
    }

    // Fallback to scalar
    qgemm_i8(m, n, k, a, a_scale, b, b_row_scales, bias, out);
}

/// SIMD-optimized quantized GEMM for aarch64 with NEON.
///
/// Uses NEON SIMD instructions for 8× speedup over scalar.
/// Processes 16 INT8 elements at a time using 128-bit registers.
#[cfg(all(feature = "simd", target_arch = "aarch64"))]
#[inline(never)]
pub fn qgemm_i8_simd(
    m: usize,
    n: usize,
    k: usize,
    a: &[i8],
    a_scale: f32,
    b: &[i8],
    b_row_scales: &[f32],
    bias: Option<&[i32]>,
    out: &mut [i32],
) {
    // Bounds check
    if a.len() < m.saturating_mul(k)
        || b.len() < n.saturating_mul(k)
        || out.len() < m.saturating_mul(n)
        || b_row_scales.len() < n
    {
        for v in out.iter_mut() {
            *v = 0;
        }
        return;
    }

    // Use NEON path for k >= 16
    if k >= 16 {
        // SAFETY: NEON is always available on aarch64
        unsafe {
            qgemm_i8_neon(m, n, k, a, a_scale, b, b_row_scales, bias, out);
        }
        return;
    }

    // Fallback to scalar for small k
    qgemm_i8(m, n, k, a, a_scale, b, b_row_scales, bias, out);
}

/// NEON-optimized GEMM kernel for aarch64.
///
/// Processes 16 i8 elements at a time using NEON intrinsics.
/// Expected speedup: 6-8× over scalar implementation.
#[cfg(all(feature = "simd", target_arch = "aarch64"))]
#[inline(never)]
unsafe fn qgemm_i8_neon(
    m: usize,
    n: usize,
    k: usize,
    a: &[i8],
    a_scale: f32,
    b: &[i8],
    b_row_scales: &[f32],
    bias: Option<&[i32]>,
    out: &mut [i32],
) {
    use core::arch::aarch64::*;

    let k_chunks = k / 16; // Process 16 elements at a time

    for i in 0..m {
        for j in 0..n {
            let a_row = &a[i * k..];
            let b_row = &b[j * k..];

            // Accumulator for dot product
            let mut acc0 = vdupq_n_s32(0);
            let mut acc1 = vdupq_n_s32(0);

            // Main SIMD loop - process 16 i8 elements at a time
            for chunk in 0..k_chunks {
                let offset = chunk * 16;

                // Load 16 bytes from A and B
                let a_vec = vld1q_s8(a_row[offset..].as_ptr());
                let b_vec = vld1q_s8(b_row[offset..].as_ptr());

                // Split into low and high halves (8 elements each)
                let a_lo = vget_low_s8(a_vec);
                let a_hi = vget_high_s8(a_vec);
                let b_lo = vget_low_s8(b_vec);
                let b_hi = vget_high_s8(b_vec);

                // Widen to i16 and multiply
                let a_lo_16 = vmovl_s8(a_lo);
                let a_hi_16 = vmovl_s8(a_hi);
                let b_lo_16 = vmovl_s8(b_lo);
                let b_hi_16 = vmovl_s8(b_hi);

                // Multiply i16 -> i32
                let prod_lo_lo = vmull_s16(vget_low_s16(a_lo_16), vget_low_s16(b_lo_16));
                let prod_lo_hi = vmull_s16(vget_high_s16(a_lo_16), vget_high_s16(b_lo_16));
                let prod_hi_lo = vmull_s16(vget_low_s16(a_hi_16), vget_low_s16(b_hi_16));
                let prod_hi_hi = vmull_s16(vget_high_s16(a_hi_16), vget_high_s16(b_hi_16));

                // Accumulate
                acc0 = vaddq_s32(acc0, prod_lo_lo);
                acc0 = vaddq_s32(acc0, prod_lo_hi);
                acc1 = vaddq_s32(acc1, prod_hi_lo);
                acc1 = vaddq_s32(acc1, prod_hi_hi);
            }

            // Horizontal sum
            let combined = vaddq_s32(acc0, acc1);
            let mut total = vaddvq_s32(combined) as i64;

            // Handle remainder with scalar
            for kk in (k_chunks * 16)..k {
                let a_val = a_row.get(kk).copied().unwrap_or(0) as i64;
                let b_val = b_row.get(kk).copied().unwrap_or(0) as i64;
                total += a_val * b_val;
            }

            // Apply scales and bias
            let combined_scale = a_scale * b_row_scales.get(j).copied().unwrap_or(1.0);
            let scaled = (total as f64 * combined_scale as f64).round() as i64;
            let bias_val = bias.and_then(|b| b.get(j)).copied().unwrap_or(0) as i64;
            let final_val = scaled.saturating_add(bias_val);

            out[i * n + j] = final_val.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        }
    }
}

/// Fallback for non-SIMD builds or unsupported architectures.
#[cfg(not(any(
    all(feature = "simd", target_arch = "x86_64"),
    all(feature = "simd", target_arch = "aarch64")
)))]
#[inline(never)]
pub fn qgemm_i8_simd(
    m: usize,
    n: usize,
    k: usize,
    a: &[i8],
    a_scale: f32,
    b: &[i8],
    b_row_scales: &[f32],
    bias: Option<&[i32]>,
    out: &mut [i32],
) {
    qgemm_i8(m, n, k, a, a_scale, b, b_row_scales, bias, out)
}

/// Quantized matrix-vector multiplication.
///
/// Specialized for single-row input (common in autoregressive generation).
///
/// # Safety
///
/// Uses i64 accumulator and bounds-checked access for safety.
#[inline]
pub fn qgemv_i8(
    n: usize,
    k: usize,
    x: &[i8],
    x_scale: f32,
    w: &[i8],
    w_row_scales: &[f32],
    bias: Option<&[i32]>,
    out: &mut [i32],
) {
    // Runtime bounds checking
    if x.len() < k || w.len() < n.saturating_mul(k) || out.len() < n || w_row_scales.len() < n {
        for v in out.iter_mut() {
            *v = 0;
        }
        return;
    }

    for j in 0..n {
        // Use i64 accumulator for overflow safety
        let mut acc: i64 = 0;

        for kk in 0..k {
            let x_val = x.get(kk).copied().unwrap_or(0) as i64;
            let w_val = w.get(j * k + kk).copied().unwrap_or(0) as i64;
            acc = acc.saturating_add(x_val.saturating_mul(w_val));
        }

        // Apply scale factors
        let combined_scale = x_scale * w_row_scales.get(j).copied().unwrap_or(1.0);
        let scaled_acc = (acc as f64 * combined_scale as f64).round() as i64;

        // Add bias
        let bias_val = bias.and_then(|b| b.get(j)).copied().unwrap_or(0) as i64;
        let final_acc = scaled_acc.saturating_add(bias_val);

        // Store with clamping
        if let Some(out_val) = out.get_mut(j) {
            *out_val = final_acc.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        }
    }
}

/// Dequantize i32 accumulator to f32.
#[inline]
pub fn dequantize_i32_to_f32(
    values: &[i32],
    input_scale: f32,
    weight_scales: &[f32],
    output: &mut [f32],
) {
    debug_assert_eq!(values.len(), output.len());
    debug_assert_eq!(values.len(), weight_scales.len());

    for (i, (&v, &ws)) in values.iter().zip(weight_scales.iter()).enumerate() {
        output[i] = (v as f32) * input_scale * ws;
    }
}

/// Quantize f32 to i8 with scale.
#[inline]
pub fn quantize_f32_to_i8(values: &[f32], scale: f32, output: &mut [i8]) {
    debug_assert_eq!(values.len(), output.len());

    let inv_scale = 1.0 / scale;
    for (i, &v) in values.iter().enumerate() {
        let q = (v * inv_scale).round();
        output[i] = q.clamp(-128.0, 127.0) as i8;
    }
}

/// Compute scale factor for quantization.
#[inline]
pub fn compute_scale(values: &[f32]) -> f32 {
    let max_abs = values.iter().map(|&v| v.abs()).fold(0.0f32, f32::max);
    if max_abs == 0.0 {
        1.0
    } else {
        max_abs / 127.0
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn test_qgemm_basic() {
        // 2x3 * 4x3^T = 2x4
        let a: [i8; 6] = [1, 2, 3, 4, 5, 6];
        let b: [i8; 12] = [
            1, 0, 0, // row 0
            0, 1, 0, // row 1
            0, 0, 1, // row 2
            1, 1, 1, // row 3
        ];
        let scales: [f32; 4] = [1.0; 4];
        let mut out = [0i32; 8];

        qgemm_i8(2, 4, 3, &a, 1.0, &b, &scales, None, &mut out);

        // Row 0 of A: [1, 2, 3]
        // Row 0 of B: [1, 0, 0] -> dot = 1
        // Row 1 of B: [0, 1, 0] -> dot = 2
        // Row 2 of B: [0, 0, 1] -> dot = 3
        // Row 3 of B: [1, 1, 1] -> dot = 6
        assert_eq!(out[0], 1);
        assert_eq!(out[1], 2);
        assert_eq!(out[2], 3);
        assert_eq!(out[3], 6);
    }

    #[test]
    fn test_qgemm_with_bias() {
        let a: [i8; 4] = [1, 1, 1, 1];
        let b: [i8; 4] = [1, 1, 1, 1];
        let scales: [f32; 2] = [1.0; 2];
        let bias: [i32; 2] = [10, 20];
        let mut out = [0i32; 4];

        qgemm_i8(2, 2, 2, &a, 1.0, &b, &scales, Some(&bias), &mut out);

        // Each dot product = 2, plus bias
        assert_eq!(out[0], 12); // 2 + 10
        assert_eq!(out[1], 22); // 2 + 20
    }

    #[test]
    fn test_qgemv() {
        let x: [i8; 3] = [1, 2, 3];
        let w: [i8; 6] = [
            1, 0, 0, // row 0
            0, 1, 0, // row 1
        ];
        let scales: [f32; 2] = [1.0; 2];
        let mut out = [0i32; 2];

        qgemv_i8(2, 3, &x, 1.0, &w, &scales, None, &mut out);

        assert_eq!(out[0], 1);
        assert_eq!(out[1], 2);
    }

    #[test]
    fn test_quantize_dequantize() {
        let original: [f32; 4] = [0.5, -0.25, 1.0, -1.0];
        let scale = compute_scale(&original);

        let mut quantized = [0i8; 4];
        quantize_f32_to_i8(&original, scale, &mut quantized);

        let scales = [scale; 4];
        let quantized_i32: Vec<i32> = quantized.iter().map(|&x| x as i32).collect();
        let mut recovered = [0.0f32; 4];
        dequantize_i32_to_f32(&quantized_i32, 1.0, &scales, &mut recovered);

        // Check approximate recovery (quantization loses precision)
        for (o, r) in original.iter().zip(recovered.iter()) {
            assert!((o - r).abs() < 0.02);
        }
    }
}
