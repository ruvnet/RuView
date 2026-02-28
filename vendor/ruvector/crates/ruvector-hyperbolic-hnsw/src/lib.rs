//! Hyperbolic Embeddings with HNSW Integration for RuVector
//!
//! This crate provides hyperbolic (Poincaré ball) embeddings integrated with
//! HNSW (Hierarchical Navigable Small World) graphs for hierarchy-aware
//! vector search.
//!
//! # Overview
//!
//! Hierarchies compress naturally in hyperbolic space. Taxonomies, catalogs,
//! ICD trees, product facets, org charts, and long-tail tags all fit better
//! than in Euclidean space, which means higher recall on deep leaves without
//! blowing up memory or latency.
//!
//! # Key Features
//!
//! - **Poincaré Ball Model**: Store vectors in the Poincaré ball with proper
//!   geometric operations (Möbius addition, exp/log maps)
//! - **Tangent Space Pruning**: Prune HNSW candidates with cheap Euclidean
//!   distance in tangent space before exact hyperbolic ranking
//! - **Per-Shard Curvature**: Different parts of the hierarchy can have
//!   different optimal curvatures
//! - **Dual-Space Index**: Keep a synchronized Euclidean index for fallback
//!   and mutual ranking fusion
//!
//! # Quick Start
//!
//! ```rust
//! use ruvector_hyperbolic_hnsw::{HyperbolicHnsw, HyperbolicHnswConfig};
//!
//! // Create index with default settings
//! let mut index = HyperbolicHnsw::default_config();
//!
//! // Insert vectors (automatically projected to Poincaré ball)
//! index.insert(vec![0.1, 0.2, 0.3]).unwrap();
//! index.insert(vec![-0.1, 0.15, 0.25]).unwrap();
//! index.insert(vec![0.2, -0.1, 0.1]).unwrap();
//!
//! // Search for nearest neighbors
//! let results = index.search(&[0.15, 0.1, 0.2], 2).unwrap();
//! for r in results {
//!     println!("ID: {}, Distance: {:.4}", r.id, r.distance);
//! }
//! ```
//!
//! # HNSW Speed Trick
//!
//! The core optimization is:
//! 1. Precompute `u = log_c(x)` at a shard centroid `c`
//! 2. During neighbor selection, use Euclidean `||u_q - u_p||` to prune
//! 3. Run exact Poincaré distance only on top N candidates before final ranking
//!
//! ```rust
//! use ruvector_hyperbolic_hnsw::{HyperbolicHnsw, HyperbolicHnswConfig};
//!
//! let mut config = HyperbolicHnswConfig::default();
//! config.use_tangent_pruning = true;
//! config.prune_factor = 10; // Consider 10x candidates in tangent space
//!
//! let mut index = HyperbolicHnsw::new(config);
//! // ... insert vectors ...
//!
//! // Build tangent cache for pruning optimization
//! # index.insert(vec![0.1, 0.2]).unwrap();
//! index.build_tangent_cache().unwrap();
//!
//! // Search with pruning
//! let results = index.search_with_pruning(&[0.1, 0.15], 5).unwrap();
//! ```
//!
//! # Sharded Index with Per-Shard Curvature
//!
//! ```rust
//! use ruvector_hyperbolic_hnsw::{ShardedHyperbolicHnsw, ShardStrategy};
//!
//! let mut manager = ShardedHyperbolicHnsw::new(1.0);
//!
//! // Insert with hierarchy depth information
//! manager.insert(vec![0.1, 0.2], Some(0)).unwrap(); // Root level
//! manager.insert(vec![0.3, 0.1], Some(3)).unwrap(); // Deeper level
//!
//! // Update curvature for specific shard
//! manager.update_curvature("radius_1", 0.5).unwrap();
//!
//! // Search across all shards
//! let results = manager.search(&[0.2, 0.15], 5).unwrap();
//! ```
//!
//! # Mathematical Operations
//!
//! The `poincare` module provides low-level hyperbolic geometry operations:
//!
//! ```rust
//! use ruvector_hyperbolic_hnsw::poincare::{
//!     mobius_add, exp_map, log_map, poincare_distance, project_to_ball
//! };
//!
//! let x = vec![0.3, 0.2];
//! let y = vec![-0.1, 0.4];
//! let c = 1.0; // Curvature
//!
//! // Möbius addition (hyperbolic vector addition)
//! let z = mobius_add(&x, &y, c);
//!
//! // Geodesic distance in hyperbolic space
//! let d = poincare_distance(&x, &y, c);
//!
//! // Map to tangent space at x
//! let v = log_map(&y, &x, c);
//!
//! // Map back to manifold
//! let y_recovered = exp_map(&v, &x, c);
//! ```
//!
//! # Numerical Stability
//!
//! All operations include numerical safeguards:
//! - Norm clamping with `eps = 1e-5`
//! - Projection after every update
//! - Stable `acosh` and `log1p` implementations
//!
//! # Feature Flags
//!
//! - `simd`: Enable SIMD acceleration (default)
//! - `parallel`: Enable parallel processing with rayon (default)
//! - `wasm`: Enable WebAssembly compatibility

pub mod error;
pub mod hnsw;
pub mod poincare;
pub mod shard;
pub mod tangent;

// Re-exports
pub use error::{HyperbolicError, HyperbolicResult};
pub use hnsw::{
    DistanceMetric, DualSpaceIndex, HnswNode, HyperbolicHnsw, HyperbolicHnswConfig, SearchResult,
};
pub use poincare::{
    conformal_factor, conformal_factor_from_norm_sq, dot, exp_map, frechet_mean, fused_norms,
    hyperbolic_midpoint, log_map, log_map_at_centroid, mobius_add, mobius_add_inplace,
    mobius_scalar_mult, norm, norm_squared, parallel_transport, poincare_distance,
    poincare_distance_batch, poincare_distance_from_norms, poincare_distance_squared,
    project_to_ball, project_to_ball_inplace, PoincareConfig, DEFAULT_CURVATURE, EPS,
};
pub use shard::{
    CurvatureRegistry, HierarchyMetrics, HyperbolicShard, ShardCurvature, ShardStrategy,
    ShardedHyperbolicHnsw,
};
pub use tangent::{tangent_micro_update, PrunedCandidate, TangentCache, TangentPruner};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Prelude for common imports
pub mod prelude {
    pub use crate::error::{HyperbolicError, HyperbolicResult};
    pub use crate::hnsw::{HyperbolicHnsw, HyperbolicHnswConfig, SearchResult};
    pub use crate::poincare::{exp_map, log_map, mobius_add, poincare_distance, project_to_ball};
    pub use crate::shard::{ShardedHyperbolicHnsw, ShardStrategy};
    pub use crate::tangent::{TangentCache, TangentPruner};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_workflow() {
        // Create index
        let mut index = HyperbolicHnsw::default_config();

        // Insert vectors
        for i in 0..10 {
            let v = vec![0.1 * i as f32, 0.05 * i as f32, 0.02 * i as f32];
            index.insert(v).unwrap();
        }

        // Search
        let query = vec![0.35, 0.175, 0.07];
        let results = index.search(&query, 3).unwrap();

        assert_eq!(results.len(), 3);
        // Results should be sorted by distance
        for i in 1..results.len() {
            assert!(results[i - 1].distance <= results[i].distance);
        }
    }

    #[test]
    fn test_hierarchy_preservation() {
        // Create points at different "depths"
        let points: Vec<Vec<f32>> = (0..20)
            .map(|i| {
                // Points further from origin represent deeper hierarchy
                let depth = i / 4;
                let radius = 0.1 + 0.15 * depth as f32;
                let angle = (i % 4) as f32 * std::f32::consts::PI / 2.0;
                vec![radius * angle.cos(), radius * angle.sin()]
            })
            .collect();

        let depths: Vec<usize> = (0..20).map(|i| i / 4).collect();

        // Compute metrics
        let metrics = HierarchyMetrics::compute(&points, &depths, 1.0).unwrap();

        // Radius should correlate positively with depth
        assert!(metrics.radius_depth_correlation > 0.5);
    }
}
