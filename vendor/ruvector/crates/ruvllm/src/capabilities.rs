//! Ruvector Capabilities Detection
//!
//! Auto-detection and graceful fallback for Ruvector features.
//! This module provides compile-time and runtime detection of available features.
//!
//! ## Available Feature Flags
//!
//! - `HNSW_AVAILABLE`: HNSW index from ruvector-core
//! - `ATTENTION_AVAILABLE`: Flash Attention from ruvector-attention
//! - `GRAPH_AVAILABLE`: Knowledge graph from ruvector-graph
//! - `GNN_AVAILABLE`: Graph neural networks from ruvector-gnn
//! - `SONA_AVAILABLE`: SONA learning from ruvector-sona
//!
//! ## Graceful Degradation
//!
//! The integration layer will gracefully fall back to simpler implementations
//! when advanced features are unavailable:
//!
//! - Without HNSW: Uses linear search (brute force)
//! - Without Attention: Uses standard dot-product similarity
//! - Without Graph: Disables relationship learning
//! - Without GNN: Uses simpler MLP-based routing

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Compile-time feature detection for HNSW index support
pub const HNSW_AVAILABLE: bool = true; // Always available via ruvector-core

/// Compile-time feature detection for Flash Attention support
#[cfg(feature = "attention")]
pub const ATTENTION_AVAILABLE: bool = true;
#[cfg(not(feature = "attention"))]
pub const ATTENTION_AVAILABLE: bool = false;

/// Compile-time feature detection for Knowledge Graph support
#[cfg(feature = "graph")]
pub const GRAPH_AVAILABLE: bool = true;
#[cfg(not(feature = "graph"))]
pub const GRAPH_AVAILABLE: bool = false;

/// Compile-time feature detection for GNN support
#[cfg(feature = "gnn")]
pub const GNN_AVAILABLE: bool = true;
#[cfg(not(feature = "gnn"))]
pub const GNN_AVAILABLE: bool = false;

/// Compile-time feature detection for SONA learning support
pub const SONA_AVAILABLE: bool = true; // Always available via ruvector-sona

/// Compile-time feature detection for SIMD acceleration
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub const SIMD_AVAILABLE: bool = true;
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub const SIMD_AVAILABLE: bool = false;

/// Compile-time feature detection for parallel processing
#[cfg(feature = "parallel")]
pub const PARALLEL_AVAILABLE: bool = true;
#[cfg(not(feature = "parallel"))]
pub const PARALLEL_AVAILABLE: bool = false;

/// Global capabilities instance (lazily initialized)
static CAPABILITIES: OnceLock<RuvectorCapabilities> = OnceLock::new();

/// Ruvector capabilities flags
///
/// Indicates which Ruvector features are available at runtime.
/// Use `detect()` to get the current capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuvectorCapabilities {
    /// HNSW index is available for approximate nearest neighbor search
    pub hnsw: bool,
    /// Flash Attention is available for efficient inference
    pub attention: bool,
    /// Knowledge graph is available for relationship learning
    pub graph: bool,
    /// Graph neural networks are available for complex reasoning
    pub gnn: bool,
    /// SONA learning framework is available
    pub sona: bool,
    /// SIMD acceleration is available
    pub simd: bool,
    /// Parallel processing is available
    pub parallel: bool,
    /// Quantization support level (0=none, 1=scalar, 2=int4, 3=product)
    pub quantization_level: u8,
    /// Maximum embedding dimension supported efficiently
    pub max_efficient_dim: usize,
    /// Estimated ops/sec for 512-dim vectors
    pub estimated_ops_per_sec: u64,
}

impl Default for RuvectorCapabilities {
    fn default() -> Self {
        Self::detect()
    }
}

impl RuvectorCapabilities {
    /// Detect available Ruvector capabilities
    ///
    /// This function probes the system to determine which features are available.
    /// Results are cached for subsequent calls.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ruvllm::capabilities::RuvectorCapabilities;
    ///
    /// let caps = RuvectorCapabilities::detect();
    /// if caps.hnsw {
    ///     println!("HNSW indexing available");
    /// }
    /// if caps.attention {
    ///     println!("Flash Attention available");
    /// }
    /// ```
    pub fn detect() -> Self {
        *CAPABILITIES.get_or_init(|| Self::probe_capabilities())
    }

    /// Get cached capabilities (same as detect but more explicit)
    pub fn cached() -> &'static Self {
        CAPABILITIES.get_or_init(|| Self::probe_capabilities())
    }

    /// Force re-detection of capabilities
    ///
    /// Note: This is generally not needed as capabilities don't change at runtime.
    /// This function creates a new detection but cannot update the cached value.
    pub fn redetect() -> Self {
        Self::probe_capabilities()
    }

    /// Probe system for available capabilities
    fn probe_capabilities() -> Self {
        // Determine quantization level based on available features
        let quantization_level = Self::probe_quantization_level();

        // Estimate performance based on available features
        let (max_efficient_dim, estimated_ops_per_sec) = Self::probe_performance();

        Self {
            hnsw: HNSW_AVAILABLE,
            attention: ATTENTION_AVAILABLE,
            graph: GRAPH_AVAILABLE,
            gnn: GNN_AVAILABLE,
            sona: SONA_AVAILABLE,
            simd: SIMD_AVAILABLE,
            parallel: PARALLEL_AVAILABLE,
            quantization_level,
            max_efficient_dim,
            estimated_ops_per_sec,
        }
    }

    /// Probe for quantization support level
    fn probe_quantization_level() -> u8 {
        // ruvector-core always provides scalar quantization
        // Higher levels depend on additional features
        if SIMD_AVAILABLE && PARALLEL_AVAILABLE {
            3 // Full product quantization support
        } else if SIMD_AVAILABLE {
            2 // Int4 quantization support
        } else {
            1 // Scalar quantization only
        }
    }

    /// Probe performance characteristics
    fn probe_performance() -> (usize, u64) {
        // These are estimated based on benchmarks from ruvector-core
        if SIMD_AVAILABLE && PARALLEL_AVAILABLE {
            (4096, 16_000_000) // ~16M ops/sec for 512-dim
        } else if SIMD_AVAILABLE {
            (2048, 8_000_000) // ~8M ops/sec
        } else {
            (1024, 2_000_000) // ~2M ops/sec baseline
        }
    }

    /// Check if all intelligence features are available
    pub fn full_intelligence(&self) -> bool {
        self.hnsw && self.sona && self.attention
    }

    /// Check if graph reasoning is available
    pub fn graph_reasoning(&self) -> bool {
        self.graph && self.gnn
    }

    /// Get feature summary string
    pub fn summary(&self) -> String {
        let mut features = Vec::new();

        if self.hnsw {
            features.push("HNSW");
        }
        if self.attention {
            features.push("FlashAttn");
        }
        if self.graph {
            features.push("Graph");
        }
        if self.gnn {
            features.push("GNN");
        }
        if self.sona {
            features.push("SONA");
        }
        if self.simd {
            features.push("SIMD");
        }
        if self.parallel {
            features.push("Parallel");
        }

        format!(
            "Ruvector [{}] Q{} max_dim={} ~{}M ops/s",
            features.join("+"),
            self.quantization_level,
            self.max_efficient_dim,
            self.estimated_ops_per_sec / 1_000_000
        )
    }

    /// Get recommended batch size based on capabilities
    pub fn recommended_batch_size(&self) -> usize {
        if self.parallel && self.simd {
            256
        } else if self.simd {
            64
        } else {
            16
        }
    }

    /// Get recommended HNSW parameters based on capabilities
    pub fn recommended_hnsw_params(&self) -> (usize, usize, usize) {
        // Returns (m, ef_construction, ef_search)
        if self.parallel && self.simd {
            (32, 200, 100) // High performance
        } else if self.simd {
            (16, 100, 50) // Balanced
        } else {
            (8, 50, 25) // Conservative
        }
    }
}

/// Feature availability check macros for conditional compilation
#[macro_export]
macro_rules! with_hnsw {
    ($code:expr) => {
        if $crate::capabilities::HNSW_AVAILABLE {
            $code
        }
    };
    ($code:expr, $fallback:expr) => {
        if $crate::capabilities::HNSW_AVAILABLE {
            $code
        } else {
            $fallback
        }
    };
}

#[macro_export]
macro_rules! with_attention {
    ($code:expr) => {
        #[cfg(feature = "attention")]
        {
            $code
        }
    };
    ($code:expr, $fallback:expr) => {
        #[cfg(feature = "attention")]
        {
            $code
        }
        #[cfg(not(feature = "attention"))]
        {
            $fallback
        }
    };
}

#[macro_export]
macro_rules! with_graph {
    ($code:expr) => {
        #[cfg(feature = "graph")]
        {
            $code
        }
    };
    ($code:expr, $fallback:expr) => {
        #[cfg(feature = "graph")]
        {
            $code
        }
        #[cfg(not(feature = "graph"))]
        {
            $fallback
        }
    };
}

#[macro_export]
macro_rules! with_gnn {
    ($code:expr) => {
        #[cfg(feature = "gnn")]
        {
            $code
        }
    };
    ($code:expr, $fallback:expr) => {
        #[cfg(feature = "gnn")]
        {
            $code
        }
        #[cfg(not(feature = "gnn"))]
        {
            $fallback
        }
    };
}

/// Capability-based feature gate
///
/// Returns `Some(result)` if the feature is available, `None` otherwise.
pub fn gate_feature<T, F: FnOnce() -> T>(feature: bool, f: F) -> Option<T> {
    if feature {
        Some(f())
    } else {
        None
    }
}

/// Capability-based feature gate with fallback
///
/// Returns the result of `f` if the feature is available, otherwise returns `fallback`.
pub fn gate_feature_or<T, F: FnOnce() -> T>(feature: bool, f: F, fallback: T) -> T {
    if feature {
        f()
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_detection() {
        let caps = RuvectorCapabilities::detect();

        // HNSW and SONA should always be available
        assert!(caps.hnsw);
        assert!(caps.sona);

        // Quantization level should be at least 1
        assert!(caps.quantization_level >= 1);

        // Max efficient dim should be reasonable
        assert!(caps.max_efficient_dim >= 512);
    }

    #[test]
    fn test_capabilities_cached() {
        let caps1 = RuvectorCapabilities::detect();
        let caps2 = RuvectorCapabilities::cached();

        assert_eq!(caps1.hnsw, caps2.hnsw);
        assert_eq!(caps1.attention, caps2.attention);
    }

    #[test]
    fn test_capabilities_summary() {
        let caps = RuvectorCapabilities::detect();
        let summary = caps.summary();

        assert!(summary.contains("Ruvector"));
        assert!(summary.contains("HNSW"));
        assert!(summary.contains("SONA"));
    }

    #[test]
    fn test_recommended_params() {
        let caps = RuvectorCapabilities::detect();

        let batch_size = caps.recommended_batch_size();
        assert!(batch_size >= 16);

        let (m, ef_c, ef_s) = caps.recommended_hnsw_params();
        assert!(m >= 8);
        assert!(ef_c >= ef_s);
    }

    #[test]
    fn test_feature_gates() {
        let result = gate_feature(true, || 42);
        assert_eq!(result, Some(42));

        let result = gate_feature(false, || 42);
        assert_eq!(result, None);

        let result = gate_feature_or(true, || 42, 0);
        assert_eq!(result, 42);

        let result = gate_feature_or(false, || 42, 0);
        assert_eq!(result, 0);
    }
}
