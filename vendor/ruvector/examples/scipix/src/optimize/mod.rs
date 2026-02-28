//! Performance optimization utilities for scipix OCR
//!
//! This module provides runtime feature detection and optimized code paths
//! for different CPU architectures and capabilities.

pub mod batch;
pub mod memory;
pub mod parallel;
pub mod quantize;
pub mod simd;

use std::sync::OnceLock;

/// CPU features detected at runtime
#[derive(Debug, Clone, Copy)]
pub struct CpuFeatures {
    pub avx2: bool,
    pub avx512f: bool,
    pub neon: bool,
    pub sse4_2: bool,
}

static CPU_FEATURES: OnceLock<CpuFeatures> = OnceLock::new();

/// Detect CPU features at runtime
pub fn detect_features() -> CpuFeatures {
    *CPU_FEATURES.get_or_init(|| {
        #[cfg(target_arch = "x86_64")]
        {
            CpuFeatures {
                avx2: is_x86_feature_detected!("avx2"),
                avx512f: is_x86_feature_detected!("avx512f"),
                neon: false,
                sse4_2: is_x86_feature_detected!("sse4.2"),
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
            CpuFeatures {
                avx2: false,
                avx512f: false,
                neon: std::arch::is_aarch64_feature_detected!("neon"),
                sse4_2: false,
            }
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            CpuFeatures {
                avx2: false,
                avx512f: false,
                neon: false,
                sse4_2: false,
            }
        }
    })
}

/// Get the detected CPU features
pub fn get_features() -> CpuFeatures {
    detect_features()
}

/// Runtime dispatch to optimized implementation
pub trait OptimizedOp<T> {
    /// Execute the operation with the best available implementation
    fn execute(&self, input: T) -> T;

    /// Execute with SIMD if available, fallback to scalar
    fn execute_auto(&self, input: T) -> T {
        let features = get_features();
        if features.avx2 || features.avx512f || features.neon {
            self.execute_simd(input)
        } else {
            self.execute_scalar(input)
        }
    }

    /// SIMD implementation
    fn execute_simd(&self, input: T) -> T;

    /// Scalar fallback implementation
    fn execute_scalar(&self, input: T) -> T;
}

/// Optimization level configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    /// No optimizations, scalar code only
    None,
    /// Use SIMD when available
    Simd,
    /// Use SIMD + parallel processing
    Parallel,
    /// All optimizations including memory optimizations
    Full,
}

impl Default for OptLevel {
    fn default() -> Self {
        OptLevel::Full
    }
}

/// Global optimization configuration
static OPT_LEVEL: OnceLock<OptLevel> = OnceLock::new();

/// Set the optimization level
pub fn set_opt_level(level: OptLevel) {
    OPT_LEVEL.set(level).ok();
}

/// Get the current optimization level
pub fn get_opt_level() -> OptLevel {
    *OPT_LEVEL.get_or_init(OptLevel::default)
}

/// Check if SIMD optimizations are enabled
pub fn simd_enabled() -> bool {
    matches!(
        get_opt_level(),
        OptLevel::Simd | OptLevel::Parallel | OptLevel::Full
    )
}

/// Check if parallel optimizations are enabled
pub fn parallel_enabled() -> bool {
    matches!(get_opt_level(), OptLevel::Parallel | OptLevel::Full)
}

/// Check if memory optimizations are enabled
pub fn memory_opt_enabled() -> bool {
    matches!(get_opt_level(), OptLevel::Full)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_detection() {
        let features = detect_features();
        println!("Detected features: {:?}", features);

        // Should always succeed on any platform
        assert!(
            features.avx2
                || features.avx512f
                || features.neon
                || features.sse4_2
                || (!features.avx2 && !features.avx512f && !features.neon && !features.sse4_2)
        );
    }

    #[test]
    fn test_opt_level() {
        assert_eq!(get_opt_level(), OptLevel::Full);

        set_opt_level(OptLevel::Simd);
        // Can't change after first init, should still be Full
        assert_eq!(get_opt_level(), OptLevel::Full);
    }

    #[test]
    fn test_optimization_checks() {
        assert!(simd_enabled());
        assert!(parallel_enabled());
        assert!(memory_opt_enabled());
    }
}
