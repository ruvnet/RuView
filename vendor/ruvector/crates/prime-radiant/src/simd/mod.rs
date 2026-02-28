//! # SIMD Optimizations for Prime-Radiant
//!
//! This module provides explicit SIMD (Single Instruction, Multiple Data) intrinsics
//! for high-performance coherence computation. The implementation supports multiple
//! SIMD widths with automatic runtime detection.
//!
//! ## Architecture Support
//!
//! | Architecture | SIMD Extension | Width | Features |
//! |--------------|----------------|-------|----------|
//! | x86_64 | SSE4.2 | 128-bit | Baseline vector support |
//! | x86_64 | AVX2 | 256-bit | 8x f32 parallel ops |
//! | x86_64 | AVX-512 | 512-bit | 16x f32 parallel ops |
//! | aarch64 | NEON | 128-bit | ARM vector support |
//!
//! ## Implementation Strategy
//!
//! 1. **Primary**: `std::simd` with `portable_simd` feature (nightly)
//! 2. **Fallback**: `wide` crate for stable Rust compatibility
//! 3. **Scalar**: Auto-vectorizable fallback for unsupported platforms
//!
//! ## Performance Targets
//!
//! | Operation | Scalar | SIMD (AVX2) | Speedup |
//! |-----------|--------|-------------|---------|
//! | `dot_product` (1024-dim) | 1.2us | 0.15us | ~8x |
//! | `norm_squared` (1024-dim) | 0.8us | 0.10us | ~8x |
//! | `batch_residuals` (256 edges) | 50us | 6.5us | ~7.7x |
//! | `batch_lane_assignment` (1024) | 4us | 0.5us | ~8x |
//!
//! ## Usage
//!
//! ```rust,ignore
//! use prime_radiant::simd::{SimdWidth, best_simd_width, vectors, energy};
//!
//! // Auto-detect best SIMD width at runtime
//! let width = best_simd_width();
//! println!("Using {:?}", width);
//!
//! // SIMD dot product
//! let a = [1.0f32; 256];
//! let b = [2.0f32; 256];
//! let result = vectors::dot_product_simd(&a, &b);
//! ```

pub mod energy;
pub mod matrix;
pub mod vectors;

// Re-export key types
pub use energy::{
    batch_lane_assignment_simd, batch_residual_norms_simd, batch_residuals_simd,
    weighted_energy_sum_simd,
};
pub use matrix::{matmul_simd, matvec_simd};
pub use vectors::{dot_product_simd, norm_squared_simd, scale_simd, subtract_simd};

/// Available SIMD instruction set widths.
///
/// The actual width available depends on the CPU and detected features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SimdWidth {
    /// No SIMD available, use scalar operations
    Scalar = 0,
    /// SSE4.2: 128-bit (4x f32)
    Sse42 = 1,
    /// AVX2: 256-bit (8x f32)
    Avx2 = 2,
    /// AVX-512: 512-bit (16x f32)
    Avx512 = 3,
    /// ARM NEON: 128-bit (4x f32)
    Neon = 4,
}

impl SimdWidth {
    /// Number of f32 values that can be processed in parallel.
    #[inline]
    pub const fn lanes_f32(self) -> usize {
        match self {
            SimdWidth::Scalar => 1,
            SimdWidth::Sse42 | SimdWidth::Neon => 4,
            SimdWidth::Avx2 => 8,
            SimdWidth::Avx512 => 16,
        }
    }

    /// Number of f64 values that can be processed in parallel.
    #[inline]
    pub const fn lanes_f64(self) -> usize {
        match self {
            SimdWidth::Scalar => 1,
            SimdWidth::Sse42 | SimdWidth::Neon => 2,
            SimdWidth::Avx2 => 4,
            SimdWidth::Avx512 => 8,
        }
    }

    /// Whether this SIMD width is supported on the current CPU.
    #[inline]
    pub fn is_supported(self) -> bool {
        match self {
            SimdWidth::Scalar => true,
            SimdWidth::Sse42 => cfg!(target_arch = "x86_64") && is_sse42_supported(),
            SimdWidth::Avx2 => cfg!(target_arch = "x86_64") && is_avx2_supported(),
            SimdWidth::Avx512 => cfg!(target_arch = "x86_64") && is_avx512_supported(),
            SimdWidth::Neon => cfg!(target_arch = "aarch64") && is_neon_supported(),
        }
    }

    /// Get a human-readable name for this SIMD width.
    pub const fn name(self) -> &'static str {
        match self {
            SimdWidth::Scalar => "Scalar",
            SimdWidth::Sse42 => "SSE4.2",
            SimdWidth::Avx2 => "AVX2",
            SimdWidth::Avx512 => "AVX-512",
            SimdWidth::Neon => "NEON",
        }
    }
}

impl Default for SimdWidth {
    fn default() -> Self {
        best_simd_width()
    }
}

/// Detect the best available SIMD width for the current CPU.
///
/// This function performs runtime CPU feature detection and returns
/// the highest-performance SIMD instruction set available.
///
/// # Example
///
/// ```rust,ignore
/// use prime_radiant::simd::best_simd_width;
///
/// let width = best_simd_width();
/// match width {
///     SimdWidth::Avx512 => println!("AVX-512 available!"),
///     SimdWidth::Avx2 => println!("AVX2 available"),
///     _ => println!("Using {:?}", width),
/// }
/// ```
#[inline]
pub fn best_simd_width() -> SimdWidth {
    #[cfg(target_arch = "x86_64")]
    {
        if is_avx512_supported() {
            return SimdWidth::Avx512;
        }
        if is_avx2_supported() {
            return SimdWidth::Avx2;
        }
        if is_sse42_supported() {
            return SimdWidth::Sse42;
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if is_neon_supported() {
            return SimdWidth::Neon;
        }
    }

    SimdWidth::Scalar
}

/// Check if SSE4.2 is supported (x86_64).
#[cfg(target_arch = "x86_64")]
#[inline]
fn is_sse42_supported() -> bool {
    #[cfg(target_feature = "sse4.2")]
    {
        true
    }
    #[cfg(not(target_feature = "sse4.2"))]
    {
        std::arch::is_x86_feature_detected!("sse4.2")
    }
}

#[cfg(not(target_arch = "x86_64"))]
#[inline]
fn is_sse42_supported() -> bool {
    false
}

/// Check if AVX2 is supported (x86_64).
#[cfg(target_arch = "x86_64")]
#[inline]
fn is_avx2_supported() -> bool {
    #[cfg(target_feature = "avx2")]
    {
        true
    }
    #[cfg(not(target_feature = "avx2"))]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
}

#[cfg(not(target_arch = "x86_64"))]
#[inline]
fn is_avx2_supported() -> bool {
    false
}

/// Check if AVX-512 is supported (x86_64).
#[cfg(target_arch = "x86_64")]
#[inline]
fn is_avx512_supported() -> bool {
    #[cfg(target_feature = "avx512f")]
    {
        true
    }
    #[cfg(not(target_feature = "avx512f"))]
    {
        std::arch::is_x86_feature_detected!("avx512f")
    }
}

#[cfg(not(target_arch = "x86_64"))]
#[inline]
fn is_avx512_supported() -> bool {
    false
}

/// Check if NEON is supported (aarch64).
#[cfg(target_arch = "aarch64")]
#[inline]
fn is_neon_supported() -> bool {
    // NEON is mandatory on aarch64
    true
}

#[cfg(not(target_arch = "aarch64"))]
#[inline]
fn is_neon_supported() -> bool {
    false
}

/// SIMD runtime context for operation dispatch.
///
/// Caches the detected SIMD width to avoid repeated feature detection.
#[derive(Debug, Clone)]
pub struct SimdContext {
    /// The detected SIMD width for this CPU.
    pub width: SimdWidth,
    /// Number of f32 lanes available.
    pub f32_lanes: usize,
    /// Number of f64 lanes available.
    pub f64_lanes: usize,
}

impl SimdContext {
    /// Create a new SIMD context with auto-detection.
    pub fn new() -> Self {
        let width = best_simd_width();
        Self {
            width,
            f32_lanes: width.lanes_f32(),
            f64_lanes: width.lanes_f64(),
        }
    }

    /// Create a context with a specific SIMD width (for testing).
    ///
    /// # Panics
    ///
    /// Panics if the requested width is not supported on this CPU.
    pub fn with_width(width: SimdWidth) -> Self {
        assert!(width.is_supported(), "SIMD width {:?} not supported", width);
        Self {
            width,
            f32_lanes: width.lanes_f32(),
            f64_lanes: width.lanes_f64(),
        }
    }

    /// Get a reference to the global SIMD context.
    ///
    /// This is lazily initialized on first access.
    pub fn global() -> &'static SimdContext {
        use once_cell::sync::Lazy;
        static CONTEXT: Lazy<SimdContext> = Lazy::new(SimdContext::new);
        &CONTEXT
    }
}

impl Default for SimdContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_width_detection() {
        let width = best_simd_width();
        println!("Detected SIMD width: {:?}", width);
        assert!(width.is_supported());
    }

    #[test]
    fn test_simd_lanes() {
        assert_eq!(SimdWidth::Scalar.lanes_f32(), 1);
        assert_eq!(SimdWidth::Sse42.lanes_f32(), 4);
        assert_eq!(SimdWidth::Avx2.lanes_f32(), 8);
        assert_eq!(SimdWidth::Avx512.lanes_f32(), 16);
        assert_eq!(SimdWidth::Neon.lanes_f32(), 4);
    }

    #[test]
    fn test_simd_context() {
        let ctx = SimdContext::new();
        assert!(ctx.width.is_supported());
        assert_eq!(ctx.f32_lanes, ctx.width.lanes_f32());
    }

    #[test]
    fn test_global_context() {
        let ctx1 = SimdContext::global();
        let ctx2 = SimdContext::global();
        assert_eq!(ctx1.width, ctx2.width);
    }
}
