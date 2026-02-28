//! Configuration for product manifolds

use crate::error::{MathError, Result};

/// Type of curvature for a manifold component
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurvatureType {
    /// Euclidean (flat) space, curvature = 0
    Euclidean,
    /// Hyperbolic space, curvature < 0
    Hyperbolic {
        /// Negative curvature parameter (typically -1)
        curvature: f64,
    },
    /// Spherical space, curvature > 0
    Spherical {
        /// Positive curvature parameter (typically 1)
        curvature: f64,
    },
}

impl CurvatureType {
    /// Create hyperbolic component with default curvature -1
    pub fn hyperbolic() -> Self {
        Self::Hyperbolic { curvature: -1.0 }
    }

    /// Create hyperbolic component with custom curvature
    pub fn hyperbolic_with(curvature: f64) -> Self {
        Self::Hyperbolic {
            curvature: curvature.min(-1e-6),
        }
    }

    /// Create spherical component with default curvature 1
    pub fn spherical() -> Self {
        Self::Spherical { curvature: 1.0 }
    }

    /// Create spherical component with custom curvature
    pub fn spherical_with(curvature: f64) -> Self {
        Self::Spherical {
            curvature: curvature.max(1e-6),
        }
    }

    /// Get curvature value
    pub fn curvature(&self) -> f64 {
        match self {
            Self::Euclidean => 0.0,
            Self::Hyperbolic { curvature } => *curvature,
            Self::Spherical { curvature } => *curvature,
        }
    }
}

/// Configuration for a product manifold
#[derive(Debug, Clone)]
pub struct ProductManifoldConfig {
    /// Euclidean dimension
    pub euclidean_dim: usize,
    /// Hyperbolic dimension (Poincaré ball ambient dimension)
    pub hyperbolic_dim: usize,
    /// Hyperbolic curvature (negative)
    pub hyperbolic_curvature: f64,
    /// Spherical dimension (ambient dimension)
    pub spherical_dim: usize,
    /// Spherical curvature (positive)
    pub spherical_curvature: f64,
    /// Weights for combining distances
    pub component_weights: (f64, f64, f64),
}

impl ProductManifoldConfig {
    /// Create a new product manifold configuration
    ///
    /// # Arguments
    /// * `euclidean_dim` - Dimension of Euclidean component E^e
    /// * `hyperbolic_dim` - Dimension of hyperbolic component H^h
    /// * `spherical_dim` - Dimension of spherical component S^s
    pub fn new(euclidean_dim: usize, hyperbolic_dim: usize, spherical_dim: usize) -> Self {
        Self {
            euclidean_dim,
            hyperbolic_dim,
            hyperbolic_curvature: -1.0,
            spherical_dim,
            spherical_curvature: 1.0,
            component_weights: (1.0, 1.0, 1.0),
        }
    }

    /// Create Euclidean-only configuration
    pub fn euclidean(dim: usize) -> Self {
        Self::new(dim, 0, 0)
    }

    /// Create hyperbolic-only configuration
    pub fn hyperbolic(dim: usize) -> Self {
        Self::new(0, dim, 0)
    }

    /// Create spherical-only configuration
    pub fn spherical(dim: usize) -> Self {
        Self::new(0, 0, dim)
    }

    /// Create Euclidean × Hyperbolic configuration
    pub fn euclidean_hyperbolic(euclidean_dim: usize, hyperbolic_dim: usize) -> Self {
        Self::new(euclidean_dim, hyperbolic_dim, 0)
    }

    /// Set hyperbolic curvature
    pub fn with_hyperbolic_curvature(mut self, c: f64) -> Self {
        self.hyperbolic_curvature = c.min(-1e-6);
        self
    }

    /// Set spherical curvature
    pub fn with_spherical_curvature(mut self, c: f64) -> Self {
        self.spherical_curvature = c.max(1e-6);
        self
    }

    /// Set component weights for distance computation
    pub fn with_weights(mut self, euclidean: f64, hyperbolic: f64, spherical: f64) -> Self {
        self.component_weights = (euclidean.max(0.0), hyperbolic.max(0.0), spherical.max(0.0));
        self
    }

    /// Total dimension of the product manifold
    pub fn total_dim(&self) -> usize {
        self.euclidean_dim + self.hyperbolic_dim + self.spherical_dim
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.total_dim() == 0 {
            return Err(MathError::invalid_parameter(
                "dimensions",
                "at least one component must have non-zero dimension",
            ));
        }

        if self.hyperbolic_curvature >= 0.0 {
            return Err(MathError::invalid_parameter(
                "hyperbolic_curvature",
                "must be negative",
            ));
        }

        if self.spherical_curvature <= 0.0 {
            return Err(MathError::invalid_parameter(
                "spherical_curvature",
                "must be positive",
            ));
        }

        Ok(())
    }

    /// Get slice ranges for each component
    pub fn component_ranges(
        &self,
    ) -> (
        std::ops::Range<usize>,
        std::ops::Range<usize>,
        std::ops::Range<usize>,
    ) {
        let e_end = self.euclidean_dim;
        let h_end = e_end + self.hyperbolic_dim;
        let s_end = h_end + self.spherical_dim;

        (0..e_end, e_end..h_end, h_end..s_end)
    }
}

impl Default for ProductManifoldConfig {
    fn default() -> Self {
        // Default: 64-dim Euclidean + 16-dim Hyperbolic + 8-dim Spherical
        Self::new(64, 16, 8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = ProductManifoldConfig::new(32, 16, 8);

        assert_eq!(config.euclidean_dim, 32);
        assert_eq!(config.hyperbolic_dim, 16);
        assert_eq!(config.spherical_dim, 8);
        assert_eq!(config.total_dim(), 56);
    }

    #[test]
    fn test_component_ranges() {
        let config = ProductManifoldConfig::new(10, 5, 3);
        let (e, h, s) = config.component_ranges();

        assert_eq!(e, 0..10);
        assert_eq!(h, 10..15);
        assert_eq!(s, 15..18);
    }

    #[test]
    fn test_validation() {
        let config = ProductManifoldConfig::new(0, 0, 0);
        assert!(config.validate().is_err());

        let config = ProductManifoldConfig::new(10, 5, 0);
        assert!(config.validate().is_ok());
    }
}
