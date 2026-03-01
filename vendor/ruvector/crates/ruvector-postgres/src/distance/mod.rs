//! SIMD-optimized distance functions for vector similarity search
//!
//! This module provides high-performance distance calculations with:
//! - AVX-512 support (16 floats per operation)
//! - AVX2 support (8 floats per operation)
//! - ARM NEON support (4 floats per operation)
//! - Scalar fallback for all platforms

pub mod scalar;
pub mod simd;

pub use scalar::*;
pub use simd::*;

use std::sync::OnceLock;

/// Distance metric types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceMetric {
    /// L2 (Euclidean) distance: sqrt(sum((a[i] - b[i])^2))
    Euclidean,
    /// Cosine distance: 1 - (a·b)/(‖a‖‖b‖)
    Cosine,
    /// Negative inner product: -sum(a[i] * b[i])
    InnerProduct,
    /// L1 (Manhattan) distance: sum(|a[i] - b[i]|)
    Manhattan,
    /// Hamming distance (for binary vectors)
    Hamming,
}

/// SIMD capability levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimdCapability {
    /// AVX-512 (512-bit, 16 floats)
    Avx512,
    /// AVX2 (256-bit, 8 floats)
    Avx2,
    /// ARM NEON (128-bit, 4 floats)
    Neon,
    /// Scalar fallback
    Scalar,
}

impl std::fmt::Display for SimdCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimdCapability::Avx512 => write!(f, "avx512"),
            SimdCapability::Avx2 => write!(f, "avx2"),
            SimdCapability::Neon => write!(f, "neon"),
            SimdCapability::Scalar => write!(f, "scalar"),
        }
    }
}

/// Detected SIMD capability (cached)
static SIMD_CAPABILITY: OnceLock<SimdCapability> = OnceLock::new();

/// Function pointer table for distance calculations
pub struct DistanceFunctions {
    pub euclidean: fn(&[f32], &[f32]) -> f32,
    pub cosine: fn(&[f32], &[f32]) -> f32,
    pub inner_product: fn(&[f32], &[f32]) -> f32,
    pub manhattan: fn(&[f32], &[f32]) -> f32,
}

static DISTANCE_FNS: OnceLock<DistanceFunctions> = OnceLock::new();

/// Initialize SIMD dispatch (called at extension load)
pub fn init_simd_dispatch() {
    let cap = detect_simd_capability();
    SIMD_CAPABILITY.get_or_init(|| cap);
    DISTANCE_FNS.get_or_init(|| create_distance_functions(cap));
}

/// Detect best available SIMD capability
fn detect_simd_capability() -> SimdCapability {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx512f") && is_x86_feature_detected!("avx512vl") {
            return SimdCapability::Avx512;
        }
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            return SimdCapability::Avx2;
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        // NEON is always available on aarch64
        return SimdCapability::Neon;
    }

    #[allow(unreachable_code)]
    SimdCapability::Scalar
}

/// Create distance function table for the detected capability
///
/// Uses the new optimized functions that integrate simsimd 5.9 and
/// dimension-specialized implementations for maximum performance.
fn create_distance_functions(cap: SimdCapability) -> DistanceFunctions {
    match cap {
        SimdCapability::Avx512 => DistanceFunctions {
            // Use optimized functions that auto-dispatch based on dimensions
            euclidean: simd::l2_distance_optimized,
            cosine: simd::cosine_distance_optimized,
            inner_product: simd::inner_product_distance_optimized,
            manhattan: simd::manhattan_distance_avx2_wrapper,
        },
        SimdCapability::Avx2 => DistanceFunctions {
            // Use optimized functions with simsimd + 4x unrolled AVX2
            euclidean: simd::l2_distance_optimized,
            cosine: simd::cosine_distance_optimized,
            inner_product: simd::inner_product_distance_optimized,
            manhattan: simd::manhattan_distance_avx2_wrapper,
        },
        SimdCapability::Neon => DistanceFunctions {
            // Use simsimd on NEON (auto-dispatched)
            euclidean: simd::l2_distance_simsimd,
            cosine: simd::cosine_distance_simsimd,
            inner_product: simd::inner_product_distance_simsimd,
            manhattan: scalar::manhattan_distance, // NEON manhattan not critical
        },
        SimdCapability::Scalar => DistanceFunctions {
            // Use simsimd even on scalar (it has good fallbacks)
            euclidean: simd::l2_distance_simsimd,
            cosine: simd::cosine_distance_simsimd,
            inner_product: simd::inner_product_distance_simsimd,
            manhattan: scalar::manhattan_distance,
        },
    }
}

/// Get SIMD info string
pub fn simd_info() -> &'static str {
    match SIMD_CAPABILITY.get() {
        Some(SimdCapability::Avx512) => "avx512",
        Some(SimdCapability::Avx2) => "avx2",
        Some(SimdCapability::Neon) => "neon",
        Some(SimdCapability::Scalar) => "scalar",
        None => "uninitialized",
    }
}

/// Get detailed SIMD info
pub fn simd_info_detailed() -> String {
    let cap = SIMD_CAPABILITY
        .get()
        .copied()
        .unwrap_or(SimdCapability::Scalar);

    #[cfg(target_arch = "x86_64")]
    {
        let mut features = Vec::new();
        if is_x86_feature_detected!("avx512f") {
            features.push("avx512f");
        }
        if is_x86_feature_detected!("avx512vl") {
            features.push("avx512vl");
        }
        if is_x86_feature_detected!("avx2") {
            features.push("avx2");
        }
        if is_x86_feature_detected!("fma") {
            features.push("fma");
        }
        if is_x86_feature_detected!("sse4.2") {
            features.push("sse4.2");
        }

        let floats_per_op = match cap {
            SimdCapability::Avx512 => 16,
            SimdCapability::Avx2 => 8,
            _ => 1,
        };

        return format!(
            "architecture: x86_64, active: {}, features: [{}], floats_per_op: {}",
            cap,
            features.join(", "),
            floats_per_op
        );
    }

    #[cfg(target_arch = "aarch64")]
    {
        return format!("architecture: aarch64, active: neon, floats_per_op: 4");
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        format!("architecture: unknown, active: scalar, floats_per_op: 1")
    }
}

// ============================================================================
// Public Distance Functions (dispatch to optimal implementation)
// ============================================================================

/// Calculate Euclidean (L2) distance
#[inline]
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    if let Some(fns) = DISTANCE_FNS.get() {
        (fns.euclidean)(a, b)
    } else {
        scalar::euclidean_distance(a, b)
    }
}

/// Calculate Cosine distance
#[inline]
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    if let Some(fns) = DISTANCE_FNS.get() {
        (fns.cosine)(a, b)
    } else {
        scalar::cosine_distance(a, b)
    }
}

/// Calculate negative Inner Product distance
#[inline]
pub fn inner_product_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    if let Some(fns) = DISTANCE_FNS.get() {
        (fns.inner_product)(a, b)
    } else {
        scalar::inner_product_distance(a, b)
    }
}

/// Calculate Manhattan (L1) distance
#[inline]
pub fn manhattan_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");

    if let Some(fns) = DISTANCE_FNS.get() {
        (fns.manhattan)(a, b)
    } else {
        scalar::manhattan_distance(a, b)
    }
}

/// Calculate distance using specified metric
#[inline]
pub fn distance(a: &[f32], b: &[f32], metric: DistanceMetric) -> f32 {
    match metric {
        DistanceMetric::Euclidean => euclidean_distance(a, b),
        DistanceMetric::Cosine => cosine_distance(a, b),
        DistanceMetric::InnerProduct => inner_product_distance(a, b),
        DistanceMetric::Manhattan => manhattan_distance(a, b),
        DistanceMetric::Hamming => {
            // For f32 vectors, treat as binary (sign bit)
            scalar::hamming_distance_f32(a, b)
        }
    }
}

/// Fast cosine distance for pre-normalized vectors
/// Only computes dot product (avoids norm calculation)
#[inline]
pub fn cosine_distance_normalized(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vector dimensions must match");
    simd::cosine_distance_normalized(a, b)
}

/// Batch distance calculation with parallelism
pub fn batch_distances(query: &[f32], vectors: &[&[f32]], metric: DistanceMetric) -> Vec<f32> {
    use rayon::prelude::*;

    vectors
        .par_iter()
        .map(|v| distance(query, v, metric))
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn init_for_tests() {
        let _ = SIMD_CAPABILITY.get_or_init(detect_simd_capability);
        let cap = *SIMD_CAPABILITY.get().unwrap();
        let _ = DISTANCE_FNS.get_or_init(|| create_distance_functions(cap));
    }

    #[test]
    fn test_euclidean() {
        init_for_tests();
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![3.0, 4.0, 0.0];
        let dist = euclidean_distance(&a, &b);
        assert!((dist - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine() {
        init_for_tests();
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let dist = cosine_distance(&a, &b);
        assert!(dist.abs() < 1e-5); // Same direction = 0 distance
    }

    #[test]
    fn test_inner_product() {
        init_for_tests();
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let dist = inner_product_distance(&a, &b);
        assert!((dist - (-32.0)).abs() < 1e-5); // -(1*4 + 2*5 + 3*6) = -32
    }

    #[test]
    fn test_manhattan() {
        init_for_tests();
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 6.0, 8.0];
        let dist = manhattan_distance(&a, &b);
        assert!((dist - 12.0).abs() < 1e-5); // |3| + |4| + |5| = 12
    }

    #[test]
    fn test_simd_matches_scalar() {
        init_for_tests();

        let a: Vec<f32> = (0..128).map(|i| i as f32 * 0.01).collect();
        let b: Vec<f32> = (0..128).map(|i| (128 - i) as f32 * 0.01).collect();

        let scalar_euclidean = scalar::euclidean_distance(&a, &b);
        let simd_euclidean = euclidean_distance(&a, &b);
        assert!((scalar_euclidean - simd_euclidean).abs() < 1e-4);

        let scalar_cosine = scalar::cosine_distance(&a, &b);
        let simd_cosine = cosine_distance(&a, &b);
        assert!((scalar_cosine - simd_cosine).abs() < 1e-4);
    }
}
