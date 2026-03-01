//! Distance functions for vector similarity search.
//!
//! Provides L2 (Euclidean), cosine, and inner product distance metrics.
//! Includes platform-specific SIMD implementations (AVX2+FMA on x86_64,
//! NEON on aarch64) with automatic runtime dispatch.

// ── Scalar implementations ─────────────────────────────────────────

/// Scalar squared L2 (Euclidean) distance between two vectors.
///
/// Returns the sum of squared differences. Does NOT take the square root
/// because the ordering is preserved and sqrt is monotonic.
#[inline]
fn l2_distance_scalar(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let d = x - y;
            d * d
        })
        .sum()
}

/// Scalar cosine distance: `1 - cosine_similarity`.
///
/// Returns a value in `[0, 2]` where 0 means identical direction.
/// If either vector has zero norm, returns `1.0`.
#[inline]
fn cosine_distance_scalar(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = (norm_a * norm_b).sqrt();
    if denom < f32::EPSILON {
        return 1.0;
    }
    1.0 - dot / denom
}

/// Scalar inner (dot) product distance: `-dot(a, b)`.
///
/// Negated so that higher similarity yields a lower distance value,
/// which is consistent with the min-heap search ordering.
#[inline]
fn dot_product_scalar(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    -dot
}

// ── x86_64 AVX2+FMA implementations ────────────────────────────────

#[cfg(target_arch = "x86_64")]
mod avx2 {
    #[target_feature(enable = "avx2", enable = "fma")]
    pub(super) unsafe fn l2_distance_avx2(a: &[f32], b: &[f32]) -> f32 {
        use core::arch::x86_64::*;
        debug_assert_eq!(a.len(), b.len());
        let n = a.len();
        let chunks = n / 8;
        let remainder = n % 8;

        let mut sum = _mm256_setzero_ps();
        let a_ptr = a.as_ptr();
        let b_ptr = b.as_ptr();

        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_ps(a_ptr.add(offset));
            let vb = _mm256_loadu_ps(b_ptr.add(offset));
            let diff = _mm256_sub_ps(va, vb);
            sum = _mm256_fmadd_ps(diff, diff, sum);
        }

        // Horizontal sum of the 8 lanes.
        // sum = [s0, s1, s2, s3, s4, s5, s6, s7]
        let hi128 = _mm256_extractf128_ps(sum, 1);
        let lo128 = _mm256_castps256_ps128(sum);
        let sum128 = _mm_add_ps(lo128, hi128);
        let shuf = _mm_movehdup_ps(sum128);
        let sums = _mm_add_ps(sum128, shuf);
        let shuf2 = _mm_movehl_ps(sums, sums);
        let result = _mm_add_ss(sums, shuf2);
        let mut total = _mm_cvtss_f32(result);

        // Handle remainder with scalar.
        let base = chunks * 8;
        for i in 0..remainder {
            let d = a[base + i] - b[base + i];
            total += d * d;
        }

        total
    }

    #[target_feature(enable = "avx2", enable = "fma")]
    pub(super) unsafe fn cosine_distance_avx2(a: &[f32], b: &[f32]) -> f32 {
        use core::arch::x86_64::*;
        debug_assert_eq!(a.len(), b.len());
        let n = a.len();
        let chunks = n / 8;
        let remainder = n % 8;

        let mut dot_acc = _mm256_setzero_ps();
        let mut norm_a_acc = _mm256_setzero_ps();
        let mut norm_b_acc = _mm256_setzero_ps();
        let a_ptr = a.as_ptr();
        let b_ptr = b.as_ptr();

        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_ps(a_ptr.add(offset));
            let vb = _mm256_loadu_ps(b_ptr.add(offset));
            dot_acc = _mm256_fmadd_ps(va, vb, dot_acc);
            norm_a_acc = _mm256_fmadd_ps(va, va, norm_a_acc);
            norm_b_acc = _mm256_fmadd_ps(vb, vb, norm_b_acc);
        }

        // Horizontal sums.
        let hsum = |v: __m256| -> f32 {
            let hi128 = _mm256_extractf128_ps(v, 1);
            let lo128 = _mm256_castps256_ps128(v);
            let sum128 = _mm_add_ps(lo128, hi128);
            let shuf = _mm_movehdup_ps(sum128);
            let sums = _mm_add_ps(sum128, shuf);
            let shuf2 = _mm_movehl_ps(sums, sums);
            let result = _mm_add_ss(sums, shuf2);
            _mm_cvtss_f32(result)
        };

        let mut dot = hsum(dot_acc);
        let mut norm_a = hsum(norm_a_acc);
        let mut norm_b = hsum(norm_b_acc);

        // Remainder.
        let base = chunks * 8;
        for i in 0..remainder {
            let x = a[base + i];
            let y = b[base + i];
            dot += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }

        let denom = (norm_a * norm_b).sqrt();
        if denom < f32::EPSILON {
            return 1.0;
        }
        1.0 - dot / denom
    }

    #[target_feature(enable = "avx2", enable = "fma")]
    pub(super) unsafe fn dot_product_avx2(a: &[f32], b: &[f32]) -> f32 {
        use core::arch::x86_64::*;
        debug_assert_eq!(a.len(), b.len());
        let n = a.len();
        let chunks = n / 8;
        let remainder = n % 8;

        let mut dot_acc = _mm256_setzero_ps();
        let a_ptr = a.as_ptr();
        let b_ptr = b.as_ptr();

        for i in 0..chunks {
            let offset = i * 8;
            let va = _mm256_loadu_ps(a_ptr.add(offset));
            let vb = _mm256_loadu_ps(b_ptr.add(offset));
            dot_acc = _mm256_fmadd_ps(va, vb, dot_acc);
        }

        let hi128 = _mm256_extractf128_ps(dot_acc, 1);
        let lo128 = _mm256_castps256_ps128(dot_acc);
        let sum128 = _mm_add_ps(lo128, hi128);
        let shuf = _mm_movehdup_ps(sum128);
        let sums = _mm_add_ps(sum128, shuf);
        let shuf2 = _mm_movehl_ps(sums, sums);
        let result = _mm_add_ss(sums, shuf2);
        let mut dot = _mm_cvtss_f32(result);

        let base = chunks * 8;
        for i in 0..remainder {
            dot += a[base + i] * b[base + i];
        }

        -dot
    }
}

// ── aarch64 NEON implementations ────────────────────────────────────

#[cfg(target_arch = "aarch64")]
mod neon {
    #[target_feature(enable = "neon")]
    pub(super) unsafe fn l2_distance_neon(a: &[f32], b: &[f32]) -> f32 {
        use core::arch::aarch64::*;
        debug_assert_eq!(a.len(), b.len());
        let n = a.len();
        let chunks = n / 4;
        let remainder = n % 4;

        let mut sum = vdupq_n_f32(0.0);
        let a_ptr = a.as_ptr();
        let b_ptr = b.as_ptr();

        for i in 0..chunks {
            let offset = i * 4;
            let va = vld1q_f32(a_ptr.add(offset));
            let vb = vld1q_f32(b_ptr.add(offset));
            let diff = vsubq_f32(va, vb);
            sum = vfmaq_f32(sum, diff, diff);
        }

        let mut total = vaddvq_f32(sum);

        let base = chunks * 4;
        for i in 0..remainder {
            let d = a[base + i] - b[base + i];
            total += d * d;
        }

        total
    }

    #[target_feature(enable = "neon")]
    pub(super) unsafe fn cosine_distance_neon(a: &[f32], b: &[f32]) -> f32 {
        use core::arch::aarch64::*;
        debug_assert_eq!(a.len(), b.len());
        let n = a.len();
        let chunks = n / 4;
        let remainder = n % 4;

        let mut dot_acc = vdupq_n_f32(0.0);
        let mut norm_a_acc = vdupq_n_f32(0.0);
        let mut norm_b_acc = vdupq_n_f32(0.0);
        let a_ptr = a.as_ptr();
        let b_ptr = b.as_ptr();

        for i in 0..chunks {
            let offset = i * 4;
            let va = vld1q_f32(a_ptr.add(offset));
            let vb = vld1q_f32(b_ptr.add(offset));
            dot_acc = vfmaq_f32(dot_acc, va, vb);
            norm_a_acc = vfmaq_f32(norm_a_acc, va, va);
            norm_b_acc = vfmaq_f32(norm_b_acc, vb, vb);
        }

        let mut dot = vaddvq_f32(dot_acc);
        let mut norm_a = vaddvq_f32(norm_a_acc);
        let mut norm_b = vaddvq_f32(norm_b_acc);

        let base = chunks * 4;
        for i in 0..remainder {
            let x = a[base + i];
            let y = b[base + i];
            dot += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }

        let denom = (norm_a * norm_b).sqrt();
        if denom < f32::EPSILON {
            return 1.0;
        }
        1.0 - dot / denom
    }

    #[target_feature(enable = "neon")]
    pub(super) unsafe fn dot_product_neon(a: &[f32], b: &[f32]) -> f32 {
        use core::arch::aarch64::*;
        debug_assert_eq!(a.len(), b.len());
        let n = a.len();
        let chunks = n / 4;
        let remainder = n % 4;

        let mut dot_acc = vdupq_n_f32(0.0);
        let a_ptr = a.as_ptr();
        let b_ptr = b.as_ptr();

        for i in 0..chunks {
            let offset = i * 4;
            let va = vld1q_f32(a_ptr.add(offset));
            let vb = vld1q_f32(b_ptr.add(offset));
            dot_acc = vfmaq_f32(dot_acc, va, vb);
        }

        let mut dot = vaddvq_f32(dot_acc);

        let base = chunks * 4;
        for i in 0..remainder {
            dot += a[base + i] * b[base + i];
        }

        -dot
    }
}

// ── Runtime dispatch ────────────────────────────────────────────────

/// Squared L2 (Euclidean) distance between two vectors.
///
/// Returns the sum of squared differences. Does NOT take the square root
/// because the ordering is preserved and sqrt is monotonic.
///
/// Automatically selects the best SIMD implementation at runtime:
/// - x86_64: AVX2+FMA (processes 8 floats per cycle)
/// - aarch64: NEON (processes 4 floats per cycle)
/// - Fallback: scalar loop
#[inline]
pub fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            return unsafe { avx2::l2_distance_avx2(a, b) };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { neon::l2_distance_neon(a, b) };
        }
    }
    l2_distance_scalar(a, b)
}

/// Cosine distance: `1 - cosine_similarity`.
///
/// Returns a value in `[0, 2]` where 0 means identical direction.
/// If either vector has zero norm, returns `1.0`.
///
/// Automatically selects the best SIMD implementation at runtime.
#[inline]
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            return unsafe { avx2::cosine_distance_avx2(a, b) };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { neon::cosine_distance_neon(a, b) };
        }
    }
    cosine_distance_scalar(a, b)
}

/// Inner (dot) product distance: `-dot(a, b)`.
///
/// Negated so that higher similarity yields a lower distance value,
/// which is consistent with the min-heap search ordering.
///
/// Automatically selects the best SIMD implementation at runtime.
#[inline]
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            return unsafe { avx2::dot_product_avx2(a, b) };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { neon::dot_product_neon(a, b) };
        }
    }
    dot_product_scalar(a, b)
}

// ── SIMD feature-gated wrappers (backward compatibility) ────────────

/// SIMD-accelerated squared L2 distance (same as `l2_distance` with runtime dispatch).
#[cfg(feature = "simd")]
#[inline]
pub fn l2_distance_simd(a: &[f32], b: &[f32]) -> f32 {
    l2_distance(a, b)
}

/// SIMD-accelerated cosine distance (same as `cosine_distance` with runtime dispatch).
#[cfg(feature = "simd")]
#[inline]
pub fn cosine_distance_simd(a: &[f32], b: &[f32]) -> f32 {
    cosine_distance(a, b)
}

/// SIMD-accelerated negative dot product distance (same as `dot_product` with runtime dispatch).
#[cfg(feature = "simd")]
#[inline]
pub fn dot_product_simd(a: &[f32], b: &[f32]) -> f32 {
    dot_product(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l2_identical_is_zero() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((l2_distance(&v, &v) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn l2_known_value() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        assert!((l2_distance(&a, &b) - 25.0).abs() < f32::EPSILON);
    }

    #[test]
    fn l2_large_vector() {
        // Test with a vector large enough to exercise SIMD paths (>8 elements).
        let n = 256;
        let a: Vec<f32> = (0..n).map(|i| i as f32 * 0.1).collect();
        let b: Vec<f32> = (0..n).map(|i| i as f32 * 0.1 + 0.5).collect();
        let dist = l2_distance(&a, &b);
        let expected = l2_distance_scalar(&a, &b);
        assert!(
            (dist - expected).abs() < 1e-3,
            "SIMD L2 mismatch: got {dist}, expected {expected}"
        );
    }

    #[test]
    fn l2_odd_length() {
        // Non-multiple-of-8 length to test remainder handling.
        let a: Vec<f32> = (0..13).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..13).map(|i| (i as f32) + 1.0).collect();
        let dist = l2_distance(&a, &b);
        // Each diff is 1.0, so sum = 13.0.
        assert!((dist - 13.0).abs() < 1e-4);
    }

    #[test]
    fn cosine_identical_is_zero() {
        let v = vec![1.0, 2.0, 3.0];
        assert!(cosine_distance(&v, &v) < 1e-6);
    }

    #[test]
    fn cosine_orthogonal_is_one() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!((cosine_distance(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_zero_vector() {
        let a = vec![0.0, 0.0];
        let b = vec![1.0, 2.0];
        assert!((cosine_distance(&a, &b) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn cosine_large_vector() {
        let n = 256;
        let a: Vec<f32> = (0..n).map(|i| (i as f32 + 1.0).sin()).collect();
        let b: Vec<f32> = (0..n).map(|i| (i as f32 + 2.0).cos()).collect();
        let dist = cosine_distance(&a, &b);
        let expected = cosine_distance_scalar(&a, &b);
        assert!(
            (dist - expected).abs() < 1e-4,
            "SIMD cosine mismatch: got {dist}, expected {expected}"
        );
    }

    #[test]
    fn dot_product_known_value() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        // dot = 4 + 10 + 18 = 32, negated = -32
        assert!((dot_product(&a, &b) - (-32.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn dot_product_large_vector() {
        let n = 256;
        let a: Vec<f32> = (0..n).map(|i| i as f32 * 0.01).collect();
        let b: Vec<f32> = (0..n).map(|i| (n - i) as f32 * 0.01).collect();
        let dist = dot_product(&a, &b);
        let expected = dot_product_scalar(&a, &b);
        assert!(
            (dist - expected).abs() < 1e-2,
            "SIMD dot mismatch: got {dist}, expected {expected}"
        );
    }

    #[test]
    fn scalar_matches_dispatch() {
        // Ensure the dispatched version matches scalar on various sizes.
        for n in [1, 2, 3, 7, 8, 9, 15, 16, 17, 31, 32, 100] {
            let a: Vec<f32> = (0..n).map(|i| (i as f32 * 1.7).sin()).collect();
            let b: Vec<f32> = (0..n).map(|i| (i as f32 * 2.3).cos()).collect();

            let l2 = l2_distance(&a, &b);
            let l2s = l2_distance_scalar(&a, &b);
            assert!((l2 - l2s).abs() < 1e-3, "L2 mismatch for n={n}");

            let cos = cosine_distance(&a, &b);
            let coss = cosine_distance_scalar(&a, &b);
            assert!((cos - coss).abs() < 1e-4, "Cosine mismatch for n={n}");

            let dp = dot_product(&a, &b);
            let dps = dot_product_scalar(&a, &b);
            assert!((dp - dps).abs() < 1e-3, "Dot mismatch for n={n}");
        }
    }
}
