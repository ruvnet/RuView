//! Hyperbolic Coherence Configuration
//!
//! Configuration for hyperbolic coherence computation.

use serde::{Deserialize, Serialize};

/// Configuration for hyperbolic coherence computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperbolicCoherenceConfig {
    /// State vector dimension
    pub dimension: usize,

    /// Curvature of the hyperbolic space (must be negative)
    /// Typical values: -1.0 (unit curvature), -0.5 (flatter), -2.0 (more curved)
    pub curvature: f32,

    /// Epsilon for numerical stability (projection boundary)
    pub epsilon: f32,

    /// Maximum number of iterations for Frechet mean computation
    pub frechet_max_iters: usize,

    /// Convergence threshold for Frechet mean
    pub frechet_tolerance: f32,

    /// Depth weight function type
    pub depth_weight_type: DepthWeightType,

    /// HNSW M parameter (max connections per node)
    pub hnsw_m: usize,

    /// HNSW ef_construction parameter
    pub hnsw_ef_construction: usize,

    /// Enable sharding for large collections
    pub enable_sharding: bool,

    /// Default shard curvature
    pub default_shard_curvature: f32,
}

impl Default for HyperbolicCoherenceConfig {
    fn default() -> Self {
        Self {
            dimension: 64,
            curvature: -1.0,
            epsilon: 1e-5,
            frechet_max_iters: 100,
            frechet_tolerance: 1e-6,
            depth_weight_type: DepthWeightType::Logarithmic,
            hnsw_m: 16,
            hnsw_ef_construction: 200,
            enable_sharding: false,
            default_shard_curvature: -1.0,
        }
    }
}

impl HyperbolicCoherenceConfig {
    /// Create a configuration for small collections (< 10K nodes)
    pub fn small() -> Self {
        Self {
            dimension: 64,
            curvature: -1.0,
            hnsw_m: 8,
            hnsw_ef_construction: 100,
            enable_sharding: false,
            ..Default::default()
        }
    }

    /// Create a configuration for large collections (> 100K nodes)
    pub fn large() -> Self {
        Self {
            dimension: 64,
            curvature: -1.0,
            hnsw_m: 32,
            hnsw_ef_construction: 400,
            enable_sharding: true,
            ..Default::default()
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.curvature >= 0.0 {
            return Err(format!(
                "Curvature must be negative, got {}",
                self.curvature
            ));
        }
        if self.dimension == 0 {
            return Err("Dimension must be positive".to_string());
        }
        if self.epsilon <= 0.0 {
            return Err("Epsilon must be positive".to_string());
        }
        Ok(())
    }

    /// Compute depth weight using configured function type
    pub fn depth_weight_fn(&self, depth: f32) -> f32 {
        self.depth_weight_type.compute(depth)
    }
}

/// Type of depth weighting function
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DepthWeightType {
    /// Constant weight (no depth scaling)
    Constant,
    /// Linear: 1 + depth
    Linear,
    /// Logarithmic: 1 + ln(max(depth, 1))
    Logarithmic,
    /// Quadratic: 1 + depth^2
    Quadratic,
    /// Exponential: e^(depth * scale)
    Exponential,
}

impl Default for DepthWeightType {
    fn default() -> Self {
        Self::Logarithmic
    }
}

impl DepthWeightType {
    /// Compute depth weight
    pub fn compute(&self, depth: f32) -> f32 {
        match self {
            Self::Constant => 1.0,
            Self::Linear => 1.0 + depth,
            Self::Logarithmic => 1.0 + depth.max(1.0).ln(),
            Self::Quadratic => 1.0 + depth * depth,
            Self::Exponential => (depth * 0.5).exp().min(10.0), // Capped at 10x
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HyperbolicCoherenceConfig::default();
        assert_eq!(config.curvature, -1.0);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_curvature() {
        let config = HyperbolicCoherenceConfig {
            curvature: 1.0, // Invalid - must be negative
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_depth_weights() {
        assert_eq!(DepthWeightType::Constant.compute(5.0), 1.0);
        assert_eq!(DepthWeightType::Linear.compute(5.0), 6.0);

        let log_weight = DepthWeightType::Logarithmic.compute(2.718281828);
        assert!((log_weight - 2.0).abs() < 0.01);
    }
}
