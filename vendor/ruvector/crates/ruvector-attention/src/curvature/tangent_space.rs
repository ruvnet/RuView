//! Tangent Space Mapping for Fast Hyperbolic Operations
//!
//! Instead of computing full geodesic distances in hyperbolic space,
//! we map points to the tangent space at a learned origin and use
//! dot products. This is 10-100x faster while preserving hierarchy.

use serde::{Deserialize, Serialize};

/// Configuration for tangent space mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TangentSpaceConfig {
    /// Dimension of hyperbolic component
    pub hyperbolic_dim: usize,
    /// Curvature (negative, e.g., -1.0)
    pub curvature: f32,
    /// Whether to learn the origin
    pub learnable_origin: bool,
}

impl Default for TangentSpaceConfig {
    fn default() -> Self {
        Self {
            hyperbolic_dim: 32,
            curvature: -1.0,
            learnable_origin: true,
        }
    }
}

/// Tangent space mapper for hyperbolic geometry
///
/// Maps points from Poincaré ball to tangent space at origin,
/// enabling fast dot-product similarity instead of geodesic distance.
#[derive(Debug, Clone)]
pub struct TangentSpaceMapper {
    config: TangentSpaceConfig,
    /// Origin point in Poincaré ball
    origin: Vec<f32>,
    /// Conformal factor at origin
    lambda_origin: f32,
}

impl TangentSpaceMapper {
    /// Create new mapper with config
    pub fn new(config: TangentSpaceConfig) -> Self {
        let origin = vec![0.0f32; config.hyperbolic_dim];
        let c = -config.curvature;
        let origin_norm_sq: f32 = origin.iter().map(|x| x * x).sum();
        let lambda_origin = 2.0 / (1.0 - c * origin_norm_sq).max(1e-8);

        Self {
            config,
            origin,
            lambda_origin,
        }
    }

    /// Set custom origin (for learned origins)
    pub fn set_origin(&mut self, origin: Vec<f32>) {
        let c = -self.config.curvature;
        let origin_norm_sq: f32 = origin.iter().map(|x| x * x).sum();
        self.lambda_origin = 2.0 / (1.0 - c * origin_norm_sq).max(1e-8);
        self.origin = origin;
    }

    /// Map point from Poincaré ball to tangent space at origin
    ///
    /// log_o(x) = (2 / λ_o) * arctanh(√c ||−o ⊕ x||) * (−o ⊕ x) / ||−o ⊕ x||
    ///
    /// For origin at 0, this simplifies to:
    /// log_0(x) = 2 * arctanh(√c ||x||) * x / (√c ||x||)
    #[inline]
    pub fn log_map(&self, point: &[f32]) -> Vec<f32> {
        let c = -self.config.curvature;
        let sqrt_c = c.sqrt();

        // For origin at 0, Möbius addition −o ⊕ x = x
        if self.origin.iter().all(|&x| x.abs() < 1e-8) {
            return self.log_map_at_origin(point, sqrt_c);
        }

        // General case: compute -origin ⊕ point
        let neg_origin: Vec<f32> = self.origin.iter().map(|x| -x).collect();
        let diff = self.mobius_add(&neg_origin, point, c);

        let diff_norm: f32 = diff.iter().map(|x| x * x).sum::<f32>().sqrt();

        if diff_norm < 1e-8 {
            return vec![0.0f32; point.len()];
        }

        let scale =
            (2.0 / self.lambda_origin) * (sqrt_c * diff_norm).atanh() / (sqrt_c * diff_norm);

        diff.iter().map(|&d| scale * d).collect()
    }

    /// Fast log map at origin (most common case)
    #[inline]
    fn log_map_at_origin(&self, point: &[f32], sqrt_c: f32) -> Vec<f32> {
        let norm: f32 = point.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm < 1e-8 {
            return vec![0.0f32; point.len()];
        }

        // Clamp to avoid infinity
        let arg = (sqrt_c * norm).min(0.99);
        let scale = 2.0 * arg.atanh() / (sqrt_c * norm);

        point.iter().map(|&p| scale * p).collect()
    }

    /// Möbius addition in Poincaré ball
    fn mobius_add(&self, x: &[f32], y: &[f32], c: f32) -> Vec<f32> {
        let x_norm_sq: f32 = x.iter().map(|xi| xi * xi).sum();
        let y_norm_sq: f32 = y.iter().map(|yi| yi * yi).sum();
        let xy_dot: f32 = x.iter().zip(y.iter()).map(|(&xi, &yi)| xi * yi).sum();

        let num_coef = 1.0 + 2.0 * c * xy_dot + c * y_norm_sq;
        let denom = 1.0 + 2.0 * c * xy_dot + c * c * x_norm_sq * y_norm_sq;

        if denom.abs() < 1e-8 {
            return x.to_vec();
        }

        let y_coef = 1.0 - c * x_norm_sq;

        x.iter()
            .zip(y.iter())
            .map(|(&xi, &yi)| (num_coef * xi + y_coef * yi) / denom)
            .collect()
    }

    /// Compute tangent space similarity (dot product in tangent space)
    ///
    /// This approximates hyperbolic distance but is much faster.
    #[inline]
    pub fn tangent_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        // Map both to tangent space
        let ta = self.log_map(a);
        let tb = self.log_map(b);

        // Dot product
        ta.iter().zip(tb.iter()).map(|(&ai, &bi)| ai * bi).sum()
    }

    /// Batch map points to tangent space (cache for window)
    pub fn batch_log_map(&self, points: &[&[f32]]) -> Vec<Vec<f32>> {
        points.iter().map(|p| self.log_map(p)).collect()
    }

    /// Compute similarities in tangent space (all pairwise with query)
    pub fn batch_tangent_similarity(
        &self,
        query_tangent: &[f32],
        keys_tangent: &[&[f32]],
    ) -> Vec<f32> {
        keys_tangent
            .iter()
            .map(|k| Self::dot_product_simd(query_tangent, k))
            .collect()
    }

    /// SIMD-friendly dot product
    #[inline(always)]
    fn dot_product_simd(a: &[f32], b: &[f32]) -> f32 {
        let len = a.len().min(b.len());
        let chunks = len / 4;
        let remainder = len % 4;

        let mut sum0 = 0.0f32;
        let mut sum1 = 0.0f32;
        let mut sum2 = 0.0f32;
        let mut sum3 = 0.0f32;

        for i in 0..chunks {
            let base = i * 4;
            sum0 += a[base] * b[base];
            sum1 += a[base + 1] * b[base + 1];
            sum2 += a[base + 2] * b[base + 2];
            sum3 += a[base + 3] * b[base + 3];
        }

        let base = chunks * 4;
        for i in 0..remainder {
            sum0 += a[base + i] * b[base + i];
        }

        sum0 + sum1 + sum2 + sum3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_map_at_origin() {
        let config = TangentSpaceConfig {
            hyperbolic_dim: 4,
            curvature: -1.0,
            learnable_origin: false,
        };
        let mapper = TangentSpaceMapper::new(config);

        // Point at origin maps to zero
        let origin = vec![0.0f32; 4];
        let result = mapper.log_map(&origin);
        assert!(result.iter().all(|&x| x.abs() < 1e-6));

        // Non-zero point
        let point = vec![0.1, 0.2, 0.0, 0.0];
        let tangent = mapper.log_map(&point);
        assert_eq!(tangent.len(), 4);
    }

    #[test]
    fn test_tangent_similarity() {
        let config = TangentSpaceConfig {
            hyperbolic_dim: 4,
            curvature: -1.0,
            learnable_origin: false,
        };
        let mapper = TangentSpaceMapper::new(config);

        let a = vec![0.1, 0.1, 0.0, 0.0];
        let b = vec![0.1, 0.1, 0.0, 0.0];

        // Same points should have high similarity
        let sim = mapper.tangent_similarity(&a, &b);
        assert!(sim > 0.0);
    }

    #[test]
    fn test_batch_operations() {
        let config = TangentSpaceConfig::default();
        let mapper = TangentSpaceMapper::new(config);

        let points: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32 * 0.05; 32]).collect();
        let points_refs: Vec<&[f32]> = points.iter().map(|p| p.as_slice()).collect();

        let tangents = mapper.batch_log_map(&points_refs);
        assert_eq!(tangents.len(), 10);
    }
}
