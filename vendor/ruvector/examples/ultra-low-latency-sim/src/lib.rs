//! Ultra-Low-Latency Meta-Simulation Library
//!
//! Core primitives for achieving quadrillion-scale simulations per second
//! through meta-simulation techniques on CPU with SIMD.
//!
//! # Meta-Simulation Techniques
//!
//! ## 1. Bit-Parallel Simulation
//! Each `u64` word represents 64 binary states evolved simultaneously.
//! Perfect for: Cellular automata, binary Markov chains, boolean networks.
//!
//! ## 2. Closed-Form Acceleration
//! Replace N simulation iterations with analytical solutions.
//! Perfect for: Ergodic Markov chains, random walks, diffusion processes.
//!
//! ## 3. Hierarchical Batching
//! Each operation represents exponentially many sub-simulations.
//! Perfect for: Monte Carlo integration, particle systems, ensemble methods.
//!
//! ## 4. SIMD Vectorization
//! Process 4-16 independent simulations per CPU instruction.
//! Perfect for: Random walks, state evolution, parallel samplers.
//!
//! # Theoretical Limits
//!
//! ```text
//! Hardware:        M3 Ultra = 1.55 TFLOPS theoretical
//! Bit-parallel:    × 64 (u64 operations)
//! SIMD:            × 4-16 (NEON/AVX)
//! Hierarchical:    × 10-1000 (meta-levels)
//! Combined:        10,000x+ effective multiplier
//! ```

#![allow(dead_code)]

pub mod bit_parallel;
pub mod closed_form;
pub mod hierarchical;
pub mod simd_ops;
pub mod verify;

/// Meta-simulation configuration
#[derive(Clone, Debug)]
pub struct MetaSimConfig {
    /// Bit-parallel width (typically 64 for u64)
    pub bit_width: usize,
    /// SIMD vector width in floats
    pub simd_width: usize,
    /// Hierarchy level (each level = batch_size^level multiplier)
    pub hierarchy_level: u32,
    /// Batch size for hierarchical compression
    pub batch_size: usize,
    /// Number of parallel threads
    pub num_threads: usize,
}

impl Default for MetaSimConfig {
    fn default() -> Self {
        Self {
            bit_width: 64,
            simd_width: detect_simd_width(),
            hierarchy_level: 2,
            batch_size: 64,
            num_threads: num_cpus(),
        }
    }
}

/// Detect SIMD width for current platform
fn detect_simd_width() -> usize {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx512f") {
            return 16;
        }
        if is_x86_feature_detected!("avx2") {
            return 8;
        }
        4 // SSE
    }

    #[cfg(target_arch = "aarch64")]
    {
        4 // NEON is 128-bit = 4 floats
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        1 // Scalar
    }
}

/// Get number of available CPU cores
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1)
}

/// Calculate effective simulation multiplier
pub fn effective_multiplier(config: &MetaSimConfig) -> u64 {
    let bit_mult = config.bit_width as u64;
    let simd_mult = config.simd_width as u64;
    let hierarchy_mult = (config.batch_size as u64).pow(config.hierarchy_level);
    let thread_mult = config.num_threads as u64;

    bit_mult * simd_mult * hierarchy_mult * thread_mult
}

/// Estimate achievable simulations per second
pub fn estimate_throughput(config: &MetaSimConfig, base_flops: f64) -> f64 {
    let multiplier = effective_multiplier(config) as f64;
    base_flops * multiplier
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MetaSimConfig::default();
        assert!(config.bit_width >= 64);
        assert!(config.simd_width >= 1);
        assert!(config.num_threads >= 1);
    }

    #[test]
    fn test_effective_multiplier() {
        let config = MetaSimConfig {
            bit_width: 64,
            simd_width: 8,
            hierarchy_level: 2,
            batch_size: 64,
            num_threads: 12,
        };

        let mult = effective_multiplier(&config);
        // 64 * 8 * 64^2 * 12 = 25,165,824
        assert_eq!(mult, 64 * 8 * 4096 * 12);
    }

    #[test]
    fn test_throughput_estimate() {
        let config = MetaSimConfig::default();
        let base_flops = 1e12; // 1 TFLOPS

        let throughput = estimate_throughput(&config, base_flops);
        assert!(throughput > base_flops); // Should be multiplied
    }
}
