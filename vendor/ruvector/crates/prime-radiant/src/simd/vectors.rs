//! # SIMD Vector Operations
//!
//! High-performance vector operations using explicit SIMD intrinsics.
//! All operations fall back to optimized scalar code when SIMD is unavailable.
//!
//! ## Supported Operations
//!
//! | Operation | Description | Complexity |
//! |-----------|-------------|------------|
//! | `dot_product_simd` | Inner product of two vectors | O(n) |
//! | `norm_squared_simd` | Squared L2 norm | O(n) |
//! | `subtract_simd` | Element-wise subtraction | O(n) |
//! | `scale_simd` | Scalar multiplication | O(n) |
//!
//! ## Performance Notes
//!
//! - Vectors should be aligned to cache line boundaries for best performance
//! - Processing 8 elements at a time with AVX2 achieves ~8x throughput
//! - Small vectors (<32 elements) may not benefit from SIMD overhead

use wide::f32x8;

/// Compute the dot product of two f32 slices using SIMD.
///
/// # Arguments
///
/// * `a` - First input vector
/// * `b` - Second input vector (must have same length as `a`)
///
/// # Returns
///
/// The dot product: sum(a[i] * b[i])
///
/// # Panics
///
/// Panics in debug mode if vectors have different lengths.
///
/// # Example
///
/// ```rust,ignore
/// use prime_radiant::simd::vectors::dot_product_simd;
///
/// let a = [1.0, 2.0, 3.0, 4.0];
/// let b = [4.0, 3.0, 2.0, 1.0];
/// let result = dot_product_simd(&a, &b);
/// assert!((result - 20.0).abs() < 1e-6);
/// ```
#[inline]
pub fn dot_product_simd(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vectors must have equal length");

    let len = a.len();

    // Fast path for small vectors - avoid SIMD overhead
    if len < 16 {
        return dot_product_scalar(a, b);
    }

    // Process 8 elements at a time with AVX2/wide
    let chunks_a = a.chunks_exact(8);
    let chunks_b = b.chunks_exact(8);
    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();

    // Use 4 accumulators for better ILP (Instruction Level Parallelism)
    let mut acc0 = f32x8::ZERO;
    let mut acc1 = f32x8::ZERO;
    let mut acc2 = f32x8::ZERO;
    let mut acc3 = f32x8::ZERO;

    let mut chunks_a_iter = chunks_a;
    let mut chunks_b_iter = chunks_b;

    // Unroll 4x for better throughput
    while let (Some(ca0), Some(cb0)) = (chunks_a_iter.next(), chunks_b_iter.next()) {
        let va0 = load_f32x8(ca0);
        let vb0 = load_f32x8(cb0);
        acc0 = va0.mul_add(vb0, acc0);

        if let (Some(ca1), Some(cb1)) = (chunks_a_iter.next(), chunks_b_iter.next()) {
            let va1 = load_f32x8(ca1);
            let vb1 = load_f32x8(cb1);
            acc1 = va1.mul_add(vb1, acc1);

            if let (Some(ca2), Some(cb2)) = (chunks_a_iter.next(), chunks_b_iter.next()) {
                let va2 = load_f32x8(ca2);
                let vb2 = load_f32x8(cb2);
                acc2 = va2.mul_add(vb2, acc2);

                if let (Some(ca3), Some(cb3)) = (chunks_a_iter.next(), chunks_b_iter.next()) {
                    let va3 = load_f32x8(ca3);
                    let vb3 = load_f32x8(cb3);
                    acc3 = va3.mul_add(vb3, acc3);
                }
            }
        }
    }

    // Combine accumulators
    let combined = acc0 + acc1 + acc2 + acc3;
    let mut sum = combined.reduce_add();

    // Handle remainder
    for (&va, &vb) in remainder_a.iter().zip(remainder_b.iter()) {
        sum += va * vb;
    }

    sum
}

/// Compute the squared L2 norm of a vector using SIMD.
///
/// # Arguments
///
/// * `v` - Input vector
///
/// # Returns
///
/// The squared norm: sum(v[i]^2)
///
/// # Example
///
/// ```rust,ignore
/// use prime_radiant::simd::vectors::norm_squared_simd;
///
/// let v = [3.0, 4.0];
/// let result = norm_squared_simd(&v);
/// assert!((result - 25.0).abs() < 1e-6);
/// ```
#[inline]
pub fn norm_squared_simd(v: &[f32]) -> f32 {
    let len = v.len();

    // Fast path for small vectors
    if len < 16 {
        return norm_squared_scalar(v);
    }

    let chunks = v.chunks_exact(8);
    let remainder = chunks.remainder();

    // Use 4 accumulators for better ILP
    let mut acc0 = f32x8::ZERO;
    let mut acc1 = f32x8::ZERO;
    let mut acc2 = f32x8::ZERO;
    let mut acc3 = f32x8::ZERO;

    let mut chunks_iter = chunks;

    // Unroll 4x
    while let Some(c0) = chunks_iter.next() {
        let v0 = load_f32x8(c0);
        acc0 = v0.mul_add(v0, acc0);

        if let Some(c1) = chunks_iter.next() {
            let v1 = load_f32x8(c1);
            acc1 = v1.mul_add(v1, acc1);

            if let Some(c2) = chunks_iter.next() {
                let v2 = load_f32x8(c2);
                acc2 = v2.mul_add(v2, acc2);

                if let Some(c3) = chunks_iter.next() {
                    let v3 = load_f32x8(c3);
                    acc3 = v3.mul_add(v3, acc3);
                }
            }
        }
    }

    // Combine accumulators
    let combined = acc0 + acc1 + acc2 + acc3;
    let mut sum = combined.reduce_add();

    // Handle remainder
    for &val in remainder {
        sum += val * val;
    }

    sum
}

/// Subtract two vectors element-wise using SIMD: out = a - b
///
/// # Arguments
///
/// * `a` - Minuend vector
/// * `b` - Subtrahend vector
/// * `out` - Output buffer (must have same length as inputs)
///
/// # Panics
///
/// Panics in debug mode if vectors have different lengths.
#[inline]
pub fn subtract_simd(a: &[f32], b: &[f32], out: &mut [f32]) {
    debug_assert_eq!(a.len(), b.len(), "Input vectors must have equal length");
    debug_assert_eq!(a.len(), out.len(), "Output must have same length as inputs");

    let len = a.len();

    // Fast path for small vectors
    if len < 16 {
        subtract_scalar(a, b, out);
        return;
    }

    let chunks_a = a.chunks_exact(8);
    let chunks_b = b.chunks_exact(8);
    let chunks_out = out.chunks_exact_mut(8);

    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();
    let offset = len - remainder_a.len();

    for ((ca, cb), cout) in chunks_a.zip(chunks_b).zip(chunks_out) {
        let va = load_f32x8(ca);
        let vb = load_f32x8(cb);
        let result = va - vb;
        store_f32x8(cout, result);
    }

    // Handle remainder
    for (i, (&va, &vb)) in remainder_a.iter().zip(remainder_b.iter()).enumerate() {
        out[offset + i] = va - vb;
    }
}

/// Scale a vector by a scalar using SIMD: out = v * scalar
///
/// # Arguments
///
/// * `v` - Input vector
/// * `scalar` - Scaling factor
/// * `out` - Output buffer (must have same length as input)
///
/// # Panics
///
/// Panics in debug mode if output has different length than input.
#[inline]
pub fn scale_simd(v: &[f32], scalar: f32, out: &mut [f32]) {
    debug_assert_eq!(v.len(), out.len(), "Output must have same length as input");

    let len = v.len();

    // Fast path for small vectors
    if len < 16 {
        scale_scalar(v, scalar, out);
        return;
    }

    let scalar_vec = f32x8::splat(scalar);

    let chunks_v = v.chunks_exact(8);
    let chunks_out = out.chunks_exact_mut(8);

    let remainder_v = chunks_v.remainder();
    let offset = len - remainder_v.len();

    for (cv, cout) in chunks_v.zip(chunks_out) {
        let vv = load_f32x8(cv);
        let result = vv * scalar_vec;
        store_f32x8(cout, result);
    }

    // Handle remainder
    for (i, &val) in remainder_v.iter().enumerate() {
        out[offset + i] = val * scalar;
    }
}

/// Compute element-wise sum of squares of differences: sum((a[i] - b[i])^2)
///
/// This is equivalent to `norm_squared_simd(subtract_simd(a, b))` but more efficient
/// as it avoids the intermediate allocation.
///
/// # Arguments
///
/// * `a` - First input vector
/// * `b` - Second input vector
///
/// # Returns
///
/// The squared distance between the vectors.
#[inline]
pub fn squared_distance_simd(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vectors must have equal length");

    let len = a.len();

    // Fast path for small vectors
    if len < 16 {
        return squared_distance_scalar(a, b);
    }

    let chunks_a = a.chunks_exact(8);
    let chunks_b = b.chunks_exact(8);
    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();

    let mut acc0 = f32x8::ZERO;
    let mut acc1 = f32x8::ZERO;
    let mut acc2 = f32x8::ZERO;
    let mut acc3 = f32x8::ZERO;

    let mut chunks_a_iter = chunks_a;
    let mut chunks_b_iter = chunks_b;

    while let (Some(ca0), Some(cb0)) = (chunks_a_iter.next(), chunks_b_iter.next()) {
        let va0 = load_f32x8(ca0);
        let vb0 = load_f32x8(cb0);
        let diff0 = va0 - vb0;
        acc0 = diff0.mul_add(diff0, acc0);

        if let (Some(ca1), Some(cb1)) = (chunks_a_iter.next(), chunks_b_iter.next()) {
            let va1 = load_f32x8(ca1);
            let vb1 = load_f32x8(cb1);
            let diff1 = va1 - vb1;
            acc1 = diff1.mul_add(diff1, acc1);

            if let (Some(ca2), Some(cb2)) = (chunks_a_iter.next(), chunks_b_iter.next()) {
                let va2 = load_f32x8(ca2);
                let vb2 = load_f32x8(cb2);
                let diff2 = va2 - vb2;
                acc2 = diff2.mul_add(diff2, acc2);

                if let (Some(ca3), Some(cb3)) = (chunks_a_iter.next(), chunks_b_iter.next()) {
                    let va3 = load_f32x8(ca3);
                    let vb3 = load_f32x8(cb3);
                    let diff3 = va3 - vb3;
                    acc3 = diff3.mul_add(diff3, acc3);
                }
            }
        }
    }

    let combined = acc0 + acc1 + acc2 + acc3;
    let mut sum = combined.reduce_add();

    // Handle remainder
    for (&va, &vb) in remainder_a.iter().zip(remainder_b.iter()) {
        let diff = va - vb;
        sum += diff * diff;
    }

    sum
}

/// Compute weighted sum: sum(a[i] * weights[i])
///
/// # Arguments
///
/// * `values` - Values to sum
/// * `weights` - Corresponding weights
///
/// # Returns
///
/// The weighted sum.
#[inline]
pub fn weighted_sum_simd(values: &[f32], weights: &[f32]) -> f32 {
    // This is just a dot product
    dot_product_simd(values, weights)
}

/// Fused multiply-add for vectors: out = a * b + c
///
/// Uses FMA instructions when available for better precision and performance.
#[inline]
pub fn fma_simd(a: &[f32], b: &[f32], c: &[f32], out: &mut [f32]) {
    debug_assert_eq!(a.len(), b.len());
    debug_assert_eq!(a.len(), c.len());
    debug_assert_eq!(a.len(), out.len());

    let len = a.len();

    if len < 16 {
        for i in 0..len {
            out[i] = a[i].mul_add(b[i], c[i]);
        }
        return;
    }

    let chunks_a = a.chunks_exact(8);
    let chunks_b = b.chunks_exact(8);
    let chunks_c = c.chunks_exact(8);
    let chunks_out = out.chunks_exact_mut(8);

    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();
    let remainder_c = chunks_c.remainder();
    let offset = len - remainder_a.len();

    for (((ca, cb), cc), cout) in chunks_a.zip(chunks_b).zip(chunks_c).zip(chunks_out) {
        let va = load_f32x8(ca);
        let vb = load_f32x8(cb);
        let vc = load_f32x8(cc);
        let result = va.mul_add(vb, vc);
        store_f32x8(cout, result);
    }

    // Handle remainder
    for (i, ((&va, &vb), &vc)) in remainder_a
        .iter()
        .zip(remainder_b.iter())
        .zip(remainder_c.iter())
        .enumerate()
    {
        out[offset + i] = va.mul_add(vb, vc);
    }
}

// ============================================================================
// Scalar Fallback Implementations
// ============================================================================

#[inline(always)]
fn dot_product_scalar(a: &[f32], b: &[f32]) -> f32 {
    // Use 4 accumulators for ILP even in scalar path
    let chunks_a = a.chunks_exact(4);
    let chunks_b = b.chunks_exact(4);
    let rem_a = chunks_a.remainder();
    let rem_b = chunks_b.remainder();

    let mut acc0 = 0.0f32;
    let mut acc1 = 0.0f32;
    let mut acc2 = 0.0f32;
    let mut acc3 = 0.0f32;

    for (ca, cb) in chunks_a.zip(chunks_b) {
        acc0 += ca[0] * cb[0];
        acc1 += ca[1] * cb[1];
        acc2 += ca[2] * cb[2];
        acc3 += ca[3] * cb[3];
    }

    let mut sum = acc0 + acc1 + acc2 + acc3;
    for (&a, &b) in rem_a.iter().zip(rem_b.iter()) {
        sum += a * b;
    }
    sum
}

#[inline(always)]
fn norm_squared_scalar(v: &[f32]) -> f32 {
    let chunks = v.chunks_exact(4);
    let remainder = chunks.remainder();

    let mut acc0 = 0.0f32;
    let mut acc1 = 0.0f32;
    let mut acc2 = 0.0f32;
    let mut acc3 = 0.0f32;

    for c in chunks {
        acc0 += c[0] * c[0];
        acc1 += c[1] * c[1];
        acc2 += c[2] * c[2];
        acc3 += c[3] * c[3];
    }

    let mut sum = acc0 + acc1 + acc2 + acc3;
    for &x in remainder {
        sum += x * x;
    }
    sum
}

#[inline(always)]
fn subtract_scalar(a: &[f32], b: &[f32], out: &mut [f32]) {
    for ((va, vb), vo) in a.iter().zip(b.iter()).zip(out.iter_mut()) {
        *vo = va - vb;
    }
}

#[inline(always)]
fn scale_scalar(v: &[f32], scalar: f32, out: &mut [f32]) {
    for (vi, vo) in v.iter().zip(out.iter_mut()) {
        *vo = vi * scalar;
    }
}

#[inline(always)]
fn squared_distance_scalar(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = 0.0f32;
    for (&va, &vb) in a.iter().zip(b.iter()) {
        let diff = va - vb;
        sum += diff * diff;
    }
    sum
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Load 8 f32 values into a SIMD register.
#[inline(always)]
fn load_f32x8(slice: &[f32]) -> f32x8 {
    debug_assert!(slice.len() >= 8);
    // Use try_into for direct memory copy instead of element-by-element
    let arr: [f32; 8] = slice[..8].try_into().unwrap();
    f32x8::from(arr)
}

/// Store 8 f32 values from a SIMD register to a slice.
#[inline(always)]
fn store_f32x8(slice: &mut [f32], v: f32x8) {
    debug_assert!(slice.len() >= 8);
    let arr: [f32; 8] = v.into();
    slice[..8].copy_from_slice(&arr);
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    fn approx_eq(a: f32, b: f32) -> bool {
        // Use relative error for larger values
        let max_abs = a.abs().max(b.abs());
        if max_abs > 1.0 {
            (a - b).abs() / max_abs < EPSILON
        } else {
            (a - b).abs() < EPSILON
        }
    }

    #[test]
    fn test_dot_product_small() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let b = [4.0, 3.0, 2.0, 1.0];
        let result = dot_product_simd(&a, &b);
        assert!(approx_eq(result, 20.0), "got {}", result);
    }

    #[test]
    fn test_dot_product_large() {
        let n = 1024;
        let a: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..n).map(|i| (n - 1 - i) as f32).collect();

        let result = dot_product_simd(&a, &b);
        let expected = dot_product_scalar(&a, &b);
        assert!(
            approx_eq(result, expected),
            "got {} expected {}",
            result,
            expected
        );
    }

    #[test]
    fn test_norm_squared_small() {
        let v = [3.0, 4.0];
        let result = norm_squared_simd(&v);
        assert!(approx_eq(result, 25.0), "got {}", result);
    }

    #[test]
    fn test_norm_squared_large() {
        let n = 1024;
        let v: Vec<f32> = (0..n).map(|i| i as f32 * 0.01).collect();

        let result = norm_squared_simd(&v);
        let expected = norm_squared_scalar(&v);
        assert!(
            approx_eq(result, expected),
            "got {} expected {}",
            result,
            expected
        );
    }

    #[test]
    fn test_subtract_small() {
        let a = [5.0, 6.0, 7.0, 8.0];
        let b = [1.0, 2.0, 3.0, 4.0];
        let mut out = [0.0f32; 4];

        subtract_simd(&a, &b, &mut out);
        assert_eq!(out, [4.0, 4.0, 4.0, 4.0]);
    }

    #[test]
    fn test_subtract_large() {
        let n = 1024;
        let a: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..n).map(|i| i as f32 * 0.5).collect();
        let mut out = vec![0.0f32; n];

        subtract_simd(&a, &b, &mut out);

        for i in 0..n {
            let expected = a[i] - b[i];
            assert!(
                approx_eq(out[i], expected),
                "at {} got {} expected {}",
                i,
                out[i],
                expected
            );
        }
    }

    #[test]
    fn test_scale_small() {
        let v = [1.0, 2.0, 3.0, 4.0];
        let mut out = [0.0f32; 4];

        scale_simd(&v, 2.0, &mut out);
        assert_eq!(out, [2.0, 4.0, 6.0, 8.0]);
    }

    #[test]
    fn test_scale_large() {
        let n = 1024;
        let v: Vec<f32> = (0..n).map(|i| i as f32).collect();
        let mut out = vec![0.0f32; n];
        let scalar = 3.5;

        scale_simd(&v, scalar, &mut out);

        for i in 0..n {
            let expected = v[i] * scalar;
            assert!(
                approx_eq(out[i], expected),
                "at {} got {} expected {}",
                i,
                out[i],
                expected
            );
        }
    }

    #[test]
    fn test_squared_distance() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        let result = squared_distance_simd(&a, &b);
        // (4-1)^2 + (5-2)^2 + (6-3)^2 = 9 + 9 + 9 = 27
        assert!(approx_eq(result, 27.0), "got {}", result);
    }

    #[test]
    fn test_squared_distance_large() {
        let n = 1024;
        let a: Vec<f32> = (0..n).map(|i| i as f32 * 0.1).collect();
        let b: Vec<f32> = (0..n).map(|i| i as f32 * 0.2).collect();

        let result = squared_distance_simd(&a, &b);
        let expected = squared_distance_scalar(&a, &b);
        assert!(
            approx_eq(result, expected),
            "got {} expected {}",
            result,
            expected
        );
    }

    #[test]
    fn test_fma() {
        let a = [1.0, 2.0, 3.0, 4.0];
        let b = [2.0, 2.0, 2.0, 2.0];
        let c = [1.0, 1.0, 1.0, 1.0];
        let mut out = [0.0f32; 4];

        fma_simd(&a, &b, &c, &mut out);
        // a * b + c = [3, 5, 7, 9]
        assert_eq!(out, [3.0, 5.0, 7.0, 9.0]);
    }

    #[test]
    fn test_edge_cases() {
        // Empty vectors
        assert!(approx_eq(dot_product_simd(&[], &[]), 0.0));
        assert!(approx_eq(norm_squared_simd(&[]), 0.0));

        // Single element
        assert!(approx_eq(dot_product_simd(&[3.0], &[4.0]), 12.0));
        assert!(approx_eq(norm_squared_simd(&[5.0]), 25.0));
    }
}
