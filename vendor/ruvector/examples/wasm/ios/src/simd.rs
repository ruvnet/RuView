//! SIMD-Optimized Vector Operations for iOS WASM
//!
//! Provides 4-8x speedup on iOS devices with Safari 16.4+ (iOS 16.4+)
//! Uses WebAssembly SIMD128 instructions for vectorized math.
//!
//! ## Supported Operations
//! - Dot product (cosine similarity numerator)
//! - L2 distance (Euclidean)
//! - Vector normalization
//! - Batch similarity computation
//!
//! ## Requirements
//! - Build with: `RUSTFLAGS="-C target-feature=+simd128"`
//! - Runtime: Safari 16.4+ / iOS 16.4+ / WasmKit with SIMD

#[cfg(target_feature = "simd128")]
use core::arch::wasm32::*;

/// Check if SIMD is available at compile time
#[inline]
pub const fn simd_available() -> bool {
    cfg!(target_feature = "simd128")
}

// ============================================
// SIMD-Optimized Operations
// ============================================

#[cfg(target_feature = "simd128")]
mod simd_impl {
    use super::*;

    /// SIMD dot product - processes 4 floats per instruction
    ///
    /// Performance: ~4x faster than scalar for vectors >= 16 elements
    #[inline]
    pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len());

        let len = a.len();
        let simd_len = len - (len % 4);

        let mut sum = f32x4_splat(0.0);

        // Process 4 elements at a time
        let mut i = 0;
        while i < simd_len {
            unsafe {
                let va = v128_load(a.as_ptr().add(i) as *const v128);
                let vb = v128_load(b.as_ptr().add(i) as *const v128);
                sum = f32x4_add(sum, f32x4_mul(va, vb));
            }
            i += 4;
        }

        // Horizontal sum of SIMD lanes
        let mut result = f32x4_extract_lane::<0>(sum)
            + f32x4_extract_lane::<1>(sum)
            + f32x4_extract_lane::<2>(sum)
            + f32x4_extract_lane::<3>(sum);

        // Handle remainder
        for j in simd_len..len {
            result += a[j] * b[j];
        }

        result
    }

    /// SIMD L2 norm (vector magnitude)
    #[inline]
    pub fn l2_norm(v: &[f32]) -> f32 {
        dot_product(v, v).sqrt()
    }

    /// SIMD L2 distance between two vectors
    #[inline]
    pub fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
        assert_eq!(a.len(), b.len());

        let len = a.len();
        let simd_len = len - (len % 4);

        let mut sum = f32x4_splat(0.0);

        let mut i = 0;
        while i < simd_len {
            unsafe {
                let va = v128_load(a.as_ptr().add(i) as *const v128);
                let vb = v128_load(b.as_ptr().add(i) as *const v128);
                let diff = f32x4_sub(va, vb);
                sum = f32x4_add(sum, f32x4_mul(diff, diff));
            }
            i += 4;
        }

        let mut result = f32x4_extract_lane::<0>(sum)
            + f32x4_extract_lane::<1>(sum)
            + f32x4_extract_lane::<2>(sum)
            + f32x4_extract_lane::<3>(sum);

        for j in simd_len..len {
            let diff = a[j] - b[j];
            result += diff * diff;
        }

        result.sqrt()
    }

    /// SIMD vector normalization (in-place)
    #[inline]
    pub fn normalize(v: &mut [f32]) {
        let norm = l2_norm(v);
        if norm < 1e-8 {
            return;
        }

        let len = v.len();
        let simd_len = len - (len % 4);
        let inv_norm = f32x4_splat(1.0 / norm);

        let mut i = 0;
        while i < simd_len {
            unsafe {
                let ptr = v.as_mut_ptr().add(i) as *mut v128;
                let val = v128_load(ptr as *const v128);
                let normalized = f32x4_mul(val, inv_norm);
                v128_store(ptr, normalized);
            }
            i += 4;
        }

        let scalar_inv = 1.0 / norm;
        for j in simd_len..len {
            v[j] *= scalar_inv;
        }
    }

    /// SIMD cosine similarity
    #[inline]
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot = dot_product(a, b);
        let norm_a = l2_norm(a);
        let norm_b = l2_norm(b);

        if norm_a < 1e-8 || norm_b < 1e-8 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// Batch dot products - compute similarity of query against multiple vectors
    /// Returns scores in the output slice
    #[inline]
    pub fn batch_dot_products(query: &[f32], vectors: &[&[f32]], out: &mut [f32]) {
        for (i, vec) in vectors.iter().enumerate() {
            if i < out.len() {
                out[i] = dot_product(query, vec);
            }
        }
    }

    /// SIMD vector addition (out = a + b)
    #[inline]
    pub fn add(a: &[f32], b: &[f32], out: &mut [f32]) {
        assert_eq!(a.len(), b.len());
        assert_eq!(a.len(), out.len());

        let len = a.len();
        let simd_len = len - (len % 4);

        let mut i = 0;
        while i < simd_len {
            unsafe {
                let va = v128_load(a.as_ptr().add(i) as *const v128);
                let vb = v128_load(b.as_ptr().add(i) as *const v128);
                let sum = f32x4_add(va, vb);
                v128_store(out.as_mut_ptr().add(i) as *mut v128, sum);
            }
            i += 4;
        }

        for j in simd_len..len {
            out[j] = a[j] + b[j];
        }
    }

    /// SIMD scalar multiply (out = a * scalar)
    #[inline]
    pub fn scale(a: &[f32], scalar: f32, out: &mut [f32]) {
        assert_eq!(a.len(), out.len());

        let len = a.len();
        let simd_len = len - (len % 4);
        let vscalar = f32x4_splat(scalar);

        let mut i = 0;
        while i < simd_len {
            unsafe {
                let va = v128_load(a.as_ptr().add(i) as *const v128);
                let scaled = f32x4_mul(va, vscalar);
                v128_store(out.as_mut_ptr().add(i) as *mut v128, scaled);
            }
            i += 4;
        }

        for j in simd_len..len {
            out[j] = a[j] * scalar;
        }
    }

    /// SIMD max element
    #[inline]
    pub fn max(v: &[f32]) -> f32 {
        if v.is_empty() {
            return f32::NEG_INFINITY;
        }

        let len = v.len();
        let simd_len = len - (len % 4);

        let mut max_vec = f32x4_splat(f32::NEG_INFINITY);

        let mut i = 0;
        while i < simd_len {
            unsafe {
                let val = v128_load(v.as_ptr().add(i) as *const v128);
                max_vec = f32x4_pmax(max_vec, val);
            }
            i += 4;
        }

        let mut result = f32x4_extract_lane::<0>(max_vec)
            .max(f32x4_extract_lane::<1>(max_vec))
            .max(f32x4_extract_lane::<2>(max_vec))
            .max(f32x4_extract_lane::<3>(max_vec));

        for j in simd_len..len {
            result = result.max(v[j]);
        }

        result
    }

    /// SIMD softmax (in-place, numerically stable)
    pub fn softmax(v: &mut [f32]) {
        if v.is_empty() {
            return;
        }

        // Find max for numerical stability
        let max_val = max(v);

        // Subtract max and exp
        let len = v.len();
        let mut sum = 0.0f32;

        for x in v.iter_mut() {
            *x = (*x - max_val).exp();
            sum += *x;
        }

        // Normalize
        if sum > 1e-8 {
            let inv_sum = 1.0 / sum;
            let simd_len = len - (len % 4);
            let vinv = f32x4_splat(inv_sum);

            let mut i = 0;
            while i < simd_len {
                unsafe {
                    let ptr = v.as_mut_ptr().add(i) as *mut v128;
                    let val = v128_load(ptr as *const v128);
                    v128_store(ptr, f32x4_mul(val, vinv));
                }
                i += 4;
            }

            for j in simd_len..len {
                v[j] *= inv_sum;
            }
        }
    }
}

// ============================================
// Scalar Fallback (when SIMD not available)
// ============================================

#[cfg(not(target_feature = "simd128"))]
mod scalar_impl {
    /// Scalar dot product fallback
    #[inline]
    pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    /// Scalar L2 norm fallback
    #[inline]
    pub fn l2_norm(v: &[f32]) -> f32 {
        v.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    /// Scalar L2 distance fallback
    #[inline]
    pub fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| {
                let d = x - y;
                d * d
            })
            .sum::<f32>()
            .sqrt()
    }

    /// Scalar normalize fallback
    #[inline]
    pub fn normalize(v: &mut [f32]) {
        let norm = l2_norm(v);
        if norm > 1e-8 {
            for x in v.iter_mut() {
                *x /= norm;
            }
        }
    }

    /// Scalar cosine similarity fallback
    #[inline]
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot = dot_product(a, b);
        let norm_a = l2_norm(a);
        let norm_b = l2_norm(b);
        if norm_a < 1e-8 || norm_b < 1e-8 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }

    /// Scalar batch dot products fallback
    #[inline]
    pub fn batch_dot_products(query: &[f32], vectors: &[&[f32]], out: &mut [f32]) {
        for (i, vec) in vectors.iter().enumerate() {
            if i < out.len() {
                out[i] = dot_product(query, vec);
            }
        }
    }

    /// Scalar add fallback
    #[inline]
    pub fn add(a: &[f32], b: &[f32], out: &mut [f32]) {
        for i in 0..a.len().min(b.len()).min(out.len()) {
            out[i] = a[i] + b[i];
        }
    }

    /// Scalar scale fallback
    #[inline]
    pub fn scale(a: &[f32], scalar: f32, out: &mut [f32]) {
        for i in 0..a.len().min(out.len()) {
            out[i] = a[i] * scalar;
        }
    }

    /// Scalar max fallback
    #[inline]
    pub fn max(v: &[f32]) -> f32 {
        v.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
    }

    /// Scalar softmax fallback
    pub fn softmax(v: &mut [f32]) {
        let max_val = max(v);
        let mut sum = 0.0f32;
        for x in v.iter_mut() {
            *x = (*x - max_val).exp();
            sum += *x;
        }
        if sum > 1e-8 {
            for x in v.iter_mut() {
                *x /= sum;
            }
        }
    }
}

// ============================================
// Public API (auto-selects SIMD or scalar)
// ============================================

#[cfg(target_feature = "simd128")]
pub use simd_impl::*;

#[cfg(not(target_feature = "simd128"))]
pub use scalar_impl::*;

// ============================================
// iOS-Specific Optimizations
// ============================================

/// Prefetch hint for upcoming memory access (no-op in WASM, hint for future)
#[inline]
pub fn prefetch(_ptr: *const f32) {
    // WASM doesn't have prefetch, but this is a placeholder for future
    // When WebAssembly gains prefetch hints, we can enable this
}

/// Aligned allocation hint for SIMD (16-byte alignment for v128)
#[inline]
pub const fn simd_alignment() -> usize {
    16 // 128-bit SIMD requires 16-byte alignment
}

/// Check if a slice is properly aligned for SIMD
#[inline]
pub fn is_simd_aligned(ptr: *const f32) -> bool {
    (ptr as usize) % simd_alignment() == 0
}

// ============================================
// Benchmarking Utilities
// ============================================

/// Benchmark a single dot product operation
#[no_mangle]
pub extern "C" fn bench_dot_product(a_ptr: *const f32, b_ptr: *const f32, len: u32) -> f32 {
    unsafe {
        let a = core::slice::from_raw_parts(a_ptr, len as usize);
        let b = core::slice::from_raw_parts(b_ptr, len as usize);
        dot_product(a, b)
    }
}

/// Benchmark L2 distance
#[no_mangle]
pub extern "C" fn bench_l2_distance(a_ptr: *const f32, b_ptr: *const f32, len: u32) -> f32 {
    unsafe {
        let a = core::slice::from_raw_parts(a_ptr, len as usize);
        let b = core::slice::from_raw_parts(b_ptr, len as usize);
        l2_distance(a, b)
    }
}

/// Get SIMD capability flag for runtime detection
#[no_mangle]
pub extern "C" fn has_simd() -> i32 {
    if simd_available() { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let b = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let result = dot_product(&a, &b);
        assert!((result - 36.0).abs() < 0.001);
    }

    #[test]
    fn test_l2_norm() {
        let v = vec![3.0, 4.0];
        let result = l2_norm(&v);
        assert!((result - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_normalize() {
        let mut v = vec![3.0, 4.0, 0.0, 0.0];
        normalize(&mut v);
        assert!((v[0] - 0.6).abs() < 0.001);
        assert!((v[1] - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0, 0.0];
        let result = cosine_similarity(&a, &b);
        assert!((result - 1.0).abs() < 0.001);
    }
}
