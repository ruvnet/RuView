//! Adapter to ruvector-hyperbolic-hnsw
//!
//! Provides a domain-specific interface for hyperbolic coherence operations.

use super::{HyperbolicCoherenceConfig, HyperbolicCoherenceError, NodeId, Result};
use std::collections::HashMap;

/// Epsilon for numerical stability
const EPS: f32 = 1e-5;

/// Adapter wrapping ruvector-hyperbolic-hnsw functionality
///
/// This adapter provides coherence-specific operations built on top of
/// the hyperbolic HNSW index, including:
/// - Poincare ball projection
/// - Distance computation with curvature awareness
/// - Frechet mean calculation
/// - Similarity search
#[derive(Debug)]
pub struct HyperbolicAdapter {
    /// Configuration
    config: HyperbolicCoherenceConfig,
    /// Node vectors (projected to ball)
    vectors: HashMap<NodeId, Vec<f32>>,
    /// Index for similarity search (simple implementation)
    /// In production, this would use ShardedHyperbolicHnsw
    index_built: bool,
}

impl HyperbolicAdapter {
    /// Create a new adapter
    pub fn new(config: HyperbolicCoherenceConfig) -> Self {
        Self {
            config,
            vectors: HashMap::new(),
            index_built: false,
        }
    }

    /// Project a vector to the Poincare ball
    ///
    /// Ensures the vector has norm < 1 (within ball radius)
    pub fn project_to_ball(&self, vector: &[f32]) -> Result<Vec<f32>> {
        let norm_sq: f32 = vector.iter().map(|x| x * x).sum();
        let norm = norm_sq.sqrt();

        if norm < 1.0 - self.config.epsilon {
            // Already inside ball
            return Ok(vector.to_vec());
        }

        // Project to boundary with epsilon margin
        let max_norm = 1.0 - self.config.epsilon;
        let scale = max_norm / (norm + EPS);

        let projected: Vec<f32> = vector.iter().map(|x| x * scale).collect();

        Ok(projected)
    }

    /// Insert a vector (must already be projected)
    pub fn insert(&mut self, node_id: NodeId, vector: Vec<f32>) -> Result<()> {
        self.vectors.insert(node_id, vector);
        self.index_built = false; // Invalidate index
        Ok(())
    }

    /// Update a vector
    pub fn update(&mut self, node_id: NodeId, vector: Vec<f32>) -> Result<()> {
        if !self.vectors.contains_key(&node_id) {
            return Err(HyperbolicCoherenceError::NodeNotFound(node_id));
        }
        self.vectors.insert(node_id, vector);
        self.index_built = false;
        Ok(())
    }

    /// Get a vector
    pub fn get(&self, node_id: NodeId) -> Option<&Vec<f32>> {
        self.vectors.get(&node_id)
    }

    /// Compute Poincare distance between two points
    ///
    /// d(x, y) = acosh(1 + 2 * |x-y|^2 / ((1-|x|^2)(1-|y|^2))) / sqrt(-c)
    pub fn poincare_distance(&self, x: &[f32], y: &[f32]) -> f32 {
        let c = -self.config.curvature; // Make positive for computation

        let norm_x_sq: f32 = x.iter().map(|v| v * v).sum();
        let norm_y_sq: f32 = y.iter().map(|v| v * v).sum();

        let diff_sq: f32 = x.iter().zip(y.iter()).map(|(a, b)| (a - b) * (a - b)).sum();

        let denom = (1.0 - norm_x_sq).max(EPS) * (1.0 - norm_y_sq).max(EPS);
        let inner = 1.0 + 2.0 * diff_sq / denom;

        // acosh(x) = ln(x + sqrt(x^2 - 1))
        let acosh_inner = if inner >= 1.0 {
            (inner + (inner * inner - 1.0).sqrt()).ln()
        } else {
            0.0
        };

        acosh_inner / c.sqrt()
    }

    /// Compute Frechet mean of multiple points in Poincare ball
    ///
    /// Uses iterative gradient descent on the hyperbolic manifold.
    pub fn frechet_mean(&self, points: &[&Vec<f32>]) -> Result<Vec<f32>> {
        if points.is_empty() {
            return Err(HyperbolicCoherenceError::EmptyCollection);
        }

        if points.len() == 1 {
            return Ok(points[0].clone());
        }

        let dim = points[0].len();

        // Initialize with Euclidean mean projected to ball
        let mut mean: Vec<f32> = vec![0.0; dim];
        for p in points {
            for (m, &v) in mean.iter_mut().zip(p.iter()) {
                *m += v;
            }
        }
        for m in mean.iter_mut() {
            *m /= points.len() as f32;
        }
        mean = self.project_to_ball(&mean)?;

        // Iterative refinement
        for _ in 0..self.config.frechet_max_iters {
            let mut grad = vec![0.0f32; dim];
            let mut total_dist = 0.0f32;

            for &p in points {
                // Log map from mean to point
                let log = self.log_map(&mean, p);
                for (g, l) in grad.iter_mut().zip(log.iter()) {
                    *g += l;
                }
                total_dist += self.poincare_distance(&mean, p);
            }

            // Average gradient
            for g in grad.iter_mut() {
                *g /= points.len() as f32;
            }

            // Check convergence
            let grad_norm: f32 = grad.iter().map(|x| x * x).sum::<f32>().sqrt();
            if grad_norm < self.config.frechet_tolerance {
                break;
            }

            // Exponential map to move along gradient
            let step_size = 0.1f32.min(1.0 / (total_dist + 1.0));
            let step: Vec<f32> = grad.iter().map(|g| g * step_size).collect();
            mean = self.exp_map(&mean, &step)?;
            mean = self.project_to_ball(&mean)?;
        }

        Ok(mean)
    }

    /// Logarithmic map: tangent vector from base to point
    fn log_map(&self, base: &[f32], point: &[f32]) -> Vec<f32> {
        let c = -self.config.curvature;

        let diff: Vec<f32> = point.iter().zip(base.iter()).map(|(p, b)| p - b).collect();
        let diff_norm: f32 = diff.iter().map(|x| x * x).sum::<f32>().sqrt().max(EPS);

        let base_norm_sq: f32 = base.iter().map(|x| x * x).sum();
        let lambda_base = 2.0 / (1.0 - base_norm_sq).max(EPS);

        let dist = self.poincare_distance(base, point);
        let scale = dist * lambda_base.sqrt() / (c.sqrt() * diff_norm);

        diff.iter().map(|d| d * scale).collect()
    }

    /// Exponential map: move from base along tangent vector
    fn exp_map(&self, base: &[f32], tangent: &[f32]) -> Result<Vec<f32>> {
        let c = -self.config.curvature;

        let tangent_norm: f32 = tangent.iter().map(|x| x * x).sum::<f32>().sqrt();
        if tangent_norm < EPS {
            return Ok(base.to_vec());
        }

        let base_norm_sq: f32 = base.iter().map(|x| x * x).sum();
        let lambda_base = 2.0 / (1.0 - base_norm_sq).max(EPS);

        let normalized: Vec<f32> = tangent.iter().map(|t| t / tangent_norm).collect();
        let scaled_norm = tangent_norm / lambda_base.sqrt();

        // tanh(sqrt(c) * t / 2)
        let tanh_arg = c.sqrt() * scaled_norm;
        let tanh_val = tanh_arg.tanh();

        let scale = tanh_val / c.sqrt();

        let mut result: Vec<f32> = base.to_vec();
        for (r, n) in result.iter_mut().zip(normalized.iter()) {
            *r += scale * n;
        }

        self.project_to_ball(&result)
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(NodeId, f32)>> {
        if self.vectors.is_empty() {
            return Ok(vec![]);
        }

        // Simple brute-force search (in production, use HNSW)
        let mut distances: Vec<(NodeId, f32)> = self
            .vectors
            .iter()
            .map(|(&id, vec)| (id, self.poincare_distance(query, vec)))
            .collect();

        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        distances.truncate(k);

        Ok(distances)
    }

    /// Get number of vectors
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projection() {
        let config = HyperbolicCoherenceConfig {
            dimension: 4,
            curvature: -1.0,
            ..Default::default()
        };
        let adapter = HyperbolicAdapter::new(config);

        // Vector inside ball - should be unchanged
        let inside = vec![0.1, 0.1, 0.1, 0.1];
        let projected = adapter.project_to_ball(&inside).unwrap();
        assert!((projected[0] - inside[0]).abs() < 0.01);

        // Vector outside ball - should be projected
        let outside = vec![0.9, 0.9, 0.9, 0.9];
        let projected = adapter.project_to_ball(&outside).unwrap();
        let norm: f32 = projected.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(norm < 1.0);
    }

    #[test]
    fn test_poincare_distance() {
        let config = HyperbolicCoherenceConfig {
            dimension: 4,
            curvature: -1.0,
            ..Default::default()
        };
        let adapter = HyperbolicAdapter::new(config);

        let origin = vec![0.0, 0.0, 0.0, 0.0];
        let point = vec![0.5, 0.0, 0.0, 0.0];

        let dist = adapter.poincare_distance(&origin, &point);
        assert!(dist > 0.0);

        // Distance from point to itself should be 0
        let self_dist = adapter.poincare_distance(&point, &point);
        assert!(self_dist < 0.01);
    }

    #[test]
    fn test_frechet_mean() {
        let config = HyperbolicCoherenceConfig {
            dimension: 4,
            curvature: -1.0,
            ..Default::default()
        };
        let adapter = HyperbolicAdapter::new(config);

        let points = vec![
            vec![0.1, 0.0, 0.0, 0.0],
            vec![-0.1, 0.0, 0.0, 0.0],
            vec![0.0, 0.1, 0.0, 0.0],
            vec![0.0, -0.1, 0.0, 0.0],
        ];

        let refs: Vec<&Vec<f32>> = points.iter().collect();
        let mean = adapter.frechet_mean(&refs).unwrap();

        // Mean should be near origin
        let mean_norm: f32 = mean.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(mean_norm < 0.1);
    }

    #[test]
    fn test_search() {
        let config = HyperbolicCoherenceConfig {
            dimension: 4,
            curvature: -1.0,
            ..Default::default()
        };
        let mut adapter = HyperbolicAdapter::new(config);

        adapter.insert(1, vec![0.1, 0.0, 0.0, 0.0]).unwrap();
        adapter.insert(2, vec![0.2, 0.0, 0.0, 0.0]).unwrap();
        adapter.insert(3, vec![0.5, 0.0, 0.0, 0.0]).unwrap();

        let query = vec![0.15, 0.0, 0.0, 0.0];
        let results = adapter.search(&query, 2).unwrap();

        assert_eq!(results.len(), 2);
        // Closest should be node 1 or 2
        assert!(results[0].0 == 1 || results[0].0 == 2);
    }
}
