//! Dynamic Hierarchical j-Tree Decomposition for Approximate Cut Structure
//!
//! This module implements the j-tree decomposition architecture from ADR-002,
//! integrating with BMSSP WASM for accelerated shortest-path/cut-duality queries.
//!
//! # Architecture Overview
//!
//! The j-tree hierarchy provides a two-tier dynamic cut architecture:
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────────────┐
//! │                    TWO-TIER DYNAMIC CUT ARCHITECTURE                   │
//! ├────────────────────────────────────────────────────────────────────────┤
//! │  TIER 1: J-Tree Hierarchy (Fast Approximate)                          │
//! │  ├── Level L: O(1) vertices (root)                                    │
//! │  ├── Level L-1: O(α) vertices                                         │
//! │  └── Level 0: n vertices (original graph)                             │
//! │                                                                        │
//! │  TIER 2: Exact Min-Cut (SubpolynomialMinCut)                          │
//! │  └── Triggered when approximate cut < threshold                       │
//! └────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # BMSSP Integration (Path-Cut Duality)
//!
//! The module leverages BMSSP WASM for O(m·log^(2/3) n) complexity:
//!
//! - **Point-to-point cut**: Computed via shortest path in dual graph
//! - **Multi-terminal cut**: BMSSP multi-source queries
//! - **Neural sparsification**: WasmNeuralBMSSP for learned edge selection
//!
//! # Features
//!
//! - **O(n^ε) Updates**: Amortized for any ε > 0
//! - **Poly-log Approximation**: Sufficient for structure detection
//! - **Low Recourse**: Vertex-split-tolerant sparsifier with O(log² n / ε²) recourse
//! - **WASM Acceleration**: 10-15x speedup over pure Rust for path queries
//!
//! # Example
//!
//! ```rust,no_run
//! use ruvector_mincut::jtree::{JTreeHierarchy, JTreeConfig};
//! use ruvector_mincut::graph::DynamicGraph;
//! use std::sync::Arc;
//!
//! // Create a graph
//! let graph = Arc::new(DynamicGraph::new());
//! graph.insert_edge(1, 2, 1.0).unwrap();
//! graph.insert_edge(2, 3, 1.0).unwrap();
//! graph.insert_edge(3, 1, 1.0).unwrap();
//!
//! // Build j-tree hierarchy
//! let config = JTreeConfig::default();
//! let mut jtree = JTreeHierarchy::build(graph, config).unwrap();
//!
//! // Query approximate min-cut (Tier 1)
//! let approx = jtree.approximate_min_cut().unwrap();
//! println!("Approximate min-cut: {} (factor: {})", approx.value, approx.approximation_factor);
//!
//! // Handle dynamic updates
//! jtree.insert_edge(3, 4, 2.0).unwrap();
//! ```
//!
//! # References
//!
//! - ADR-002: Dynamic Hierarchical j-Tree Decomposition
//! - arXiv:2601.09139 (Goranci/Henzinger/Kiss/Momeni/Zöcklein, SODA 2026)
//! - arXiv:2501.00660 (BMSSP: Breaking the Sorting Barrier)

pub mod coordinator;
pub mod hierarchy;
pub mod level;
pub mod sparsifier;

// Re-exports for convenient access
pub use coordinator::{
    EscalationPolicy, EscalationTrigger, QueryResult, TierMetrics, TwoTierCoordinator,
};
pub use hierarchy::{
    ApproximateCut, CutResult, JTreeConfig, JTreeHierarchy, JTreeStatistics, Tier,
};
pub use level::{
    BmsspJTreeLevel, ContractedGraph, JTreeLevel, LevelConfig, LevelStatistics, PathCutResult,
};
pub use sparsifier::{
    DynamicCutSparsifier, ForestPacking, RecourseTracker, SparsifierConfig, SparsifierStatistics,
    VertexSplitResult,
};

use crate::error::{MinCutError, Result};

/// J-tree specific error types
#[derive(Debug, Clone)]
pub enum JTreeError {
    /// Invalid configuration parameter
    InvalidConfig(String),
    /// Level index out of bounds
    LevelOutOfBounds {
        /// The requested level
        level: usize,
        /// The maximum valid level
        max_level: usize,
    },
    /// WASM module initialization failed
    WasmInitError(String),
    /// Vertex not found in hierarchy
    VertexNotFound(u64),
    /// FFI boundary error
    FfiBoundaryError(String),
    /// Sparsifier recourse exceeded
    RecourseExceeded {
        /// The actual recourse observed
        actual: usize,
        /// The configured limit
        limit: usize,
    },
    /// Cut computation failed
    CutComputationFailed(String),
}

impl std::fmt::Display for JTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidConfig(msg) => write!(f, "Invalid j-tree configuration: {msg}"),
            Self::LevelOutOfBounds { level, max_level } => {
                write!(f, "Level {level} out of bounds (max: {max_level})")
            }
            Self::WasmInitError(msg) => write!(f, "WASM initialization failed: {msg}"),
            Self::VertexNotFound(v) => write!(f, "Vertex {v} not found in j-tree hierarchy"),
            Self::FfiBoundaryError(msg) => write!(f, "FFI boundary error: {msg}"),
            Self::RecourseExceeded { actual, limit } => {
                write!(f, "Sparsifier recourse {actual} exceeded limit {limit}")
            }
            Self::CutComputationFailed(msg) => write!(f, "Cut computation failed: {msg}"),
        }
    }
}

impl std::error::Error for JTreeError {}

impl From<JTreeError> for MinCutError {
    fn from(err: JTreeError) -> Self {
        MinCutError::InternalError(err.to_string())
    }
}

/// Convert epsilon to alpha (approximation quality per level)
///
/// The j-tree hierarchy uses α^ℓ approximation at level ℓ, where:
/// - α = 2^(1/ε) for user-provided ε
/// - L = O(log n / log α) levels total
///
/// Smaller ε → larger α → fewer levels → worse approximation but faster updates
/// Larger ε → smaller α → more levels → better approximation but slower updates
#[inline]
pub fn compute_alpha(epsilon: f64) -> f64 {
    debug_assert!(epsilon > 0.0 && epsilon <= 1.0, "epsilon must be in (0, 1]");
    2.0_f64.powf(1.0 / epsilon)
}

/// Compute the number of levels for a given vertex count and alpha
///
/// L = ceil(log_α(n)) = ceil(log n / log α)
#[inline]
pub fn compute_num_levels(vertex_count: usize, alpha: f64) -> usize {
    if vertex_count <= 1 {
        return 1;
    }
    let n = vertex_count as f64;
    (n.ln() / alpha.ln()).ceil() as usize
}

/// Validate j-tree configuration parameters
pub fn validate_config(config: &JTreeConfig) -> Result<()> {
    if config.epsilon <= 0.0 || config.epsilon > 1.0 {
        return Err(JTreeError::InvalidConfig(format!(
            "epsilon must be in (0, 1], got {}",
            config.epsilon
        ))
        .into());
    }

    if config.critical_threshold < 0.0 {
        return Err(JTreeError::InvalidConfig(format!(
            "critical_threshold must be non-negative, got {}",
            config.critical_threshold
        ))
        .into());
    }

    if config.max_approximation_factor < 1.0 {
        return Err(JTreeError::InvalidConfig(format!(
            "max_approximation_factor must be >= 1.0, got {}",
            config.max_approximation_factor
        ))
        .into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_alpha() {
        // ε = 1.0 → α = 2.0
        let alpha = compute_alpha(1.0);
        assert!((alpha - 2.0).abs() < 1e-10);

        // ε = 0.5 → α = 4.0
        let alpha = compute_alpha(0.5);
        assert!((alpha - 4.0).abs() < 1e-10);

        // ε = 0.1 → α = 2^10 = 1024
        let alpha = compute_alpha(0.1);
        assert!((alpha - 1024.0).abs() < 1e-6);
    }

    #[test]
    fn test_compute_num_levels() {
        // Single vertex → 1 level
        assert_eq!(compute_num_levels(1, 2.0), 1);

        // 16 vertices, α = 2 → log₂(16) = 4 levels
        assert_eq!(compute_num_levels(16, 2.0), 4);

        // 1000 vertices, α = 2 → ceil(log₂(1000)) ≈ 10 levels
        assert_eq!(compute_num_levels(1000, 2.0), 10);

        // 1000 vertices, α = 10 → ceil(log₁₀(1000)) = 3 levels
        assert_eq!(compute_num_levels(1000, 10.0), 3);
    }

    #[test]
    fn test_validate_config_valid() {
        let config = JTreeConfig {
            epsilon: 0.5,
            critical_threshold: 10.0,
            max_approximation_factor: 2.0,
            ..Default::default()
        };
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_config_invalid_epsilon() {
        let config = JTreeConfig {
            epsilon: 0.0,
            ..Default::default()
        };
        assert!(validate_config(&config).is_err());

        let config = JTreeConfig {
            epsilon: 1.5,
            ..Default::default()
        };
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_jtree_error_display() {
        let err = JTreeError::LevelOutOfBounds {
            level: 5,
            max_level: 3,
        };
        assert_eq!(err.to_string(), "Level 5 out of bounds (max: 3)");

        let err = JTreeError::VertexNotFound(42);
        assert_eq!(err.to_string(), "Vertex 42 not found in j-tree hierarchy");
    }

    #[test]
    fn test_jtree_error_to_mincut_error() {
        let jtree_err = JTreeError::WasmInitError("test error".to_string());
        let mincut_err: MinCutError = jtree_err.into();
        assert!(matches!(mincut_err, MinCutError::InternalError(_)));
    }
}
