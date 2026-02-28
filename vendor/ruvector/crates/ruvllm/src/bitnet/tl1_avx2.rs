//! AVX2-optimized TL1 (Ternary Level 1) GEMV kernel for BitNet b1.58.
//!
//! Computes y = W_ternary * x where W is packed 2-bit ternary weights.
//!
//! Key techniques:
//! - `_mm_shuffle_epi8` (vpshufb) as a 16-entry LUT for ternary decoding
//! - `_mm256_cvtepi8_epi16` for INT8 -> INT16 sign extension
//! - `_mm256_madd_epi16` for INT16 multiply-add producing INT32 accumulators
//! - Processes 16 ternary elements per inner iteration
//!
//! # Data Layout
//!
//! Packed ternary encoding (2-bit, LSB-first within each byte):
//! - 00 = -1, 01 = 0, 10 = +1, 11 = reserved (treated as 0)

#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

/// Ternary decode table: maps 2-bit encoding to signed value.
const DECODE: [i8; 4] = [-1, 0, 1, 0];

/// Scalar reference TL1 GEMV for validation and non-AVX2 fallback.
///
/// Computes: y[i] = sum_j(ternary[i,j] * scales[block(i,j)] * x[j])
pub fn tl1_gemv_scalar(
    packed: &[u8],
    scales: &[f32],
    x: &[f32],
    y: &mut [f32],
    m: usize,
    n: usize,
    block_size: usize,
) {
    for i in 0..m {
        let mut sum = 0.0f32;
        for j in 0..n {
            let flat = i * n + j;
            let byte_idx = flat / 4;
            let bit_off = (flat % 4) * 2;
            let code = (packed.get(byte_idx).copied().unwrap_or(0) >> bit_off) & 0x03;
            let ternary = DECODE[code as usize] as f32;
            let block_idx = flat / block_size;
            let scale = scales.get(block_idx).copied().unwrap_or(1.0);
            sum += ternary * scale * x[j];
        }
        y[i] = sum;
    }
}

/// Quantize f32 activations to INT16 for integer-domain accumulation.
///
/// Returns (quantized_values, scale) where original ~= quantized * scale.
fn quantize_activations_i16(x: &[f32]) -> (Vec<i16>, f32) {
    if x.is_empty() {
        return (vec![], 1.0);
    }
    let max_abs = x.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
    if max_abs == 0.0 {
        return (vec![0i16; x.len()], 1.0);
    }
    let scale = max_abs / 32767.0;
    let inv_scale = 1.0 / scale;
    let x_q: Vec<i16> = x
        .iter()
        .map(|&v| (v * inv_scale).round().clamp(-32767.0, 32767.0) as i16)
        .collect();
    (x_q, scale)
}

/// Horizontal sum of 8 x INT32 lanes in a __m256i register.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn hsum_epi32_avx2(v: __m256i) -> i32 {
    let hi = _mm256_extracti128_si256(v, 1);
    let lo = _mm256_castsi256_si128(v);
    let sum128 = _mm_add_epi32(lo, hi);
    let shuf1 = _mm_shuffle_epi32(sum128, 0b_01_00_11_10);
    let sum64 = _mm_add_epi32(sum128, shuf1);
    let shuf2 = _mm_shuffle_epi32(sum64, 0b_00_01_00_01);
    let sum32 = _mm_add_epi32(sum64, shuf2);
    _mm_cvtsi128_si32(sum32)
}

/// Unpack 16 consecutive ternary values starting at a flat element index
/// into a 16-byte array of 2-bit codes for vpshufb LUT lookup.
///
/// Handles arbitrary alignment (the flat index need not be a multiple of 4).
#[inline]
fn unpack_indices_16(packed: &[u8], flat_start: usize) -> [u8; 16] {
    let mut indices = [0u8; 16];
    for k in 0..16 {
        let flat = flat_start + k;
        let byte_idx = flat / 4;
        let bit_off = (flat % 4) * 2;
        let byte = packed.get(byte_idx).copied().unwrap_or(0);
        indices[k] = (byte >> bit_off) & 0x03;
    }
    indices
}

/// AVX2-accelerated TL1 GEMV.
///
/// Processes 16 ternary elements per inner iteration using:
/// 1. vpshufb LUT to decode 2-bit ternary codes to signed INT8 {-1, 0, +1}
/// 2. Sign-extension INT8 -> INT16 via `_mm256_cvtepi8_epi16`
/// 3. INT16 multiply-add to INT32 via `_mm256_madd_epi16`
/// 4. INT32 accumulation with `_mm256_add_epi32`
///
/// Activations are pre-quantized to INT16 for integer-domain computation.
///
/// # Safety
///
/// Requires AVX2 target feature. Caller must ensure slice lengths are consistent
/// with the provided m, n, and block_size dimensions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn tl1_gemv_avx2(
    packed: &[u8],
    scales: &[f32],
    x: &[f32],
    y: &mut [f32],
    m: usize,
    n: usize,
    block_size: usize,
) {
    let (x_q, x_scale) = quantize_activations_i16(x);

    // vpshufb LUT: index -> signed ternary value
    // Index 0 -> -1, 1 -> 0, 2 -> +1, 3 -> 0 (repeated 4x for 16 entries)
    // _mm_set_epi8 args are in order e15..e0 (highest index first)
    let sign_lut = _mm_set_epi8(0, 1, 0, -1, 0, 1, 0, -1, 0, 1, 0, -1, 0, 1, 0, -1);

    for row in 0..m {
        let row_flat_start = row * n;
        let mut total_sum = 0.0f32;

        let blocks_per_row = if block_size > 0 {
            (n + block_size - 1) / block_size
        } else {
            1
        };
        let effective_bs = if block_size > 0 { block_size } else { n };

        for blk in 0..blocks_per_row {
            let col_start = blk * effective_bs;
            let col_end = (col_start + effective_bs).min(n);
            let flat_block_idx = (row_flat_start + col_start) / effective_bs;
            let scale = scales.get(flat_block_idx).copied().unwrap_or(1.0);

            let mut acc = _mm256_setzero_si256();
            let chunk_count = (col_end - col_start) / 16;
            let simd_end = col_start + chunk_count * 16;

            let mut col = col_start;
            while col < simd_end {
                let flat_col = row_flat_start + col;
                let indices = unpack_indices_16(packed, flat_col);

                // LUT lookup: map 2-bit codes to signed bytes {-1, 0, +1}
                let idx_vec = _mm_loadu_si128(indices.as_ptr() as *const __m128i);
                let signs_i8 = _mm_shuffle_epi8(sign_lut, idx_vec);

                // Sign-extend 16 x INT8 -> 16 x INT16
                let ternary_i16 = _mm256_cvtepi8_epi16(signs_i8);

                // Load 16 INT16 quantized activations
                let x_ptr = x_q.as_ptr().add(col) as *const __m256i;
                let x_i16 = _mm256_loadu_si256(x_ptr);

                // Multiply adjacent INT16 pairs and sum to INT32
                let products = _mm256_madd_epi16(ternary_i16, x_i16);
                acc = _mm256_add_epi32(acc, products);

                col += 16;
            }

            let block_sum = hsum_epi32_avx2(acc);

            // Scalar remainder for columns not divisible by 16
            let mut scalar_rem = 0i32;
            for j in simd_end..col_end {
                let flat = row * n + j;
                let byte_idx = flat / 4;
                let bit_off = (flat % 4) * 2;
                let code = (packed.get(byte_idx).copied().unwrap_or(0) >> bit_off) & 0x03;
                let ternary = DECODE[code as usize] as i32;
                scalar_rem += ternary * (x_q[j] as i32);
            }

            total_sum += ((block_sum + scalar_rem) as f32) * scale;
        }

        y[row] = total_sum * x_scale;
    }
}

/// Public dispatch: uses AVX2 when available, scalar otherwise.
pub fn tl1_gemv(
    packed: &[u8],
    scales: &[f32],
    x: &[f32],
    y: &mut [f32],
    m: usize,
    n: usize,
    block_size: usize,
) {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe {
                tl1_gemv_avx2(packed, scales, x, y, m, n, block_size);
            }
            return;
        }
    }
    tl1_gemv_scalar(packed, scales, x, y, m, n, block_size);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: pack ternary values into 2-bit representation.
    /// Encoding: -1 -> 00, 0 -> 01, +1 -> 10
    fn pack_ternary_test(values: &[i8]) -> Vec<u8> {
        let num_bytes = (values.len() + 3) / 4;
        let mut packed = vec![0u8; num_bytes];
        for (i, &val) in values.iter().enumerate() {
            let byte_idx = i / 4;
            let bit_offset = (i % 4) * 2;
            let encoded: u8 = match val {
                -1 => 0b00,
                0 => 0b01,
                1 => 0b10,
                _ => panic!("Invalid ternary value: {}", val),
            };
            packed[byte_idx] |= encoded << bit_offset;
        }
        packed
    }

    /// Compute reference output using naive scalar loop.
    fn reference_gemv(
        ternary: &[i8],
        scales: &[f32],
        x: &[f32],
        m: usize,
        n: usize,
        bs: usize,
    ) -> Vec<f32> {
        let mut y = vec![0.0f32; m];
        for i in 0..m {
            for j in 0..n {
                let flat = i * n + j;
                let block_idx = flat / bs;
                let scale = scales.get(block_idx).copied().unwrap_or(1.0);
                y[i] += (ternary[flat] as f32) * scale * x[j];
            }
        }
        y
    }

    #[test]
    fn test_scalar_matches_reference() {
        let ternary = vec![1, -1, 0, 1, -1, 0, 1, -1i8];
        let packed = pack_ternary_test(&ternary);
        let scales = vec![2.0f32];
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let mut y = vec![0.0f32; 2];

        tl1_gemv_scalar(&packed, &scales, &x, &mut y, 2, 4, 256);

        let expected = reference_gemv(&ternary, &scales, &x, 2, 4, 256);
        for (a, b) in y.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-4, "scalar mismatch: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_dispatch_matches_scalar() {
        let n = 32;
        let m = 4;
        let bs = 256;

        let mut ternary = vec![0i8; m * n];
        for (i, t) in ternary.iter_mut().enumerate() {
            *t = match i % 3 {
                0 => 1,
                1 => -1,
                _ => 0,
            };
        }
        let packed = pack_ternary_test(&ternary);
        let scales = vec![1.5f32; (m * n + bs - 1) / bs];
        let x: Vec<f32> = (0..n).map(|i| (i as f32) * 0.1 - 1.0).collect();

        let mut y_scalar = vec![0.0f32; m];
        tl1_gemv_scalar(&packed, &scales, &x, &mut y_scalar, m, n, bs);

        let mut y_dispatch = vec![0.0f32; m];
        tl1_gemv(&packed, &scales, &x, &mut y_dispatch, m, n, bs);

        // AVX2 path uses INT16 quantized activations, so there is inherent
        // rounding error. Use a tolerance proportional to the activation range.
        let x_max = x.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        for (i, (a, b)) in y_dispatch.iter().zip(y_scalar.iter()).enumerate() {
            let tol = b.abs() * 0.05 + x_max * 0.01 + 1e-3;
            assert!(
                (a - b).abs() < tol,
                "row {} dispatch mismatch: {} vs {} (tol={})",
                i,
                a,
                b,
                tol,
            );
        }
    }

    #[test]
    fn test_block_aligned_size() {
        let n = 256;
        let m = 2;
        let bs = 256;

        let ternary: Vec<i8> = (0..m * n).map(|i| [1, -1, 0][i % 3]).collect();
        let packed = pack_ternary_test(&ternary);
        let scales = vec![0.5f32; (m * n) / bs];
        let x: Vec<f32> = (0..n).map(|i| ((i as f32) * 0.01).sin()).collect();

        let expected = reference_gemv(&ternary, &scales, &x, m, n, bs);

        let mut y = vec![0.0f32; m];
        tl1_gemv(&packed, &scales, &x, &mut y, m, n, bs);

        for (i, (a, b)) in y.iter().zip(expected.iter()).enumerate() {
            let tol = b.abs() * 0.02 + 1e-3;
            assert!((a - b).abs() < tol, "row {} mismatch: {} vs {}", i, a, b);
        }
    }

    #[test]
    fn test_unaligned_size() {
        let n = 19; // not divisible by 16
        let m = 3;
        let bs = 256;

        let ternary: Vec<i8> = (0..m * n).map(|i| [1, 0, -1][i % 3]).collect();
        let packed = pack_ternary_test(&ternary);
        let scales = vec![1.0f32; (m * n + bs - 1) / bs];
        let x: Vec<f32> = (0..n).map(|i| i as f32 * 0.5).collect();

        let expected = reference_gemv(&ternary, &scales, &x, m, n, bs);

        let mut y = vec![0.0f32; m];
        tl1_gemv(&packed, &scales, &x, &mut y, m, n, bs);

        for (i, (a, b)) in y.iter().zip(expected.iter()).enumerate() {
            let tol = b.abs() * 0.02 + 1e-3;
            assert!((a - b).abs() < tol, "row {} mismatch: {} vs {}", i, a, b);
        }
    }

    #[test]
    fn test_empty_input() {
        let mut y = vec![0.0f32; 0];
        tl1_gemv(&[], &[], &[], &mut y, 0, 0, 256);
        assert!(y.is_empty());
    }

    #[test]
    fn test_single_element() {
        let ternary = vec![1i8];
        let packed = pack_ternary_test(&ternary);
        let scales = vec![3.0f32];
        let x = vec![2.0f32];
        let mut y = vec![0.0f32; 1];

        tl1_gemv(&packed, &scales, &x, &mut y, 1, 1, 256);

        // Expected: 1 * 3.0 * 2.0 = 6.0
        assert!((y[0] - 6.0).abs() < 0.1, "single element: {} vs 6.0", y[0]);
    }

    #[test]
    fn test_all_zeros_ternary() {
        let n = 32;
        let m = 2;
        let ternary = vec![0i8; m * n];
        let packed = pack_ternary_test(&ternary);
        let scales = vec![1.0f32];
        let x: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let mut y = vec![0.0f32; m];

        tl1_gemv(&packed, &scales, &x, &mut y, m, n, 256);

        for &val in &y {
            assert!(
                (val).abs() < 1e-4,
                "all-zero ternary should give zero output"
            );
        }
    }

    #[test]
    fn test_maximum_accumulation() {
        // All +1 ternary, all +1.0 activations -> sum = n * scale
        let n = 256;
        let m = 1;
        let ternary = vec![1i8; n];
        let packed = pack_ternary_test(&ternary);
        let scale_val = 2.0f32;
        let scales = vec![scale_val];
        let x = vec![1.0f32; n];
        let mut y = vec![0.0f32; 1];

        tl1_gemv(&packed, &scales, &x, &mut y, m, n, 256);

        let expected = (n as f32) * scale_val;
        let tol = expected * 0.01 + 1e-2;
        assert!(
            (y[0] - expected).abs() < tol,
            "max accumulation: {} vs {}",
            y[0],
            expected
        );
    }
}
