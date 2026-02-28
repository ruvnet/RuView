//! Restriction Maps for Sheaf Attention
//!
//! Restriction maps replace traditional learned W_q, W_k, W_v projections
//! with geometrically meaningful transformations.
//!
//! ## Mathematical Foundation
//!
//! A restriction map rho: V_U -> V_u projects from a larger stalk to a smaller one:
//!
//! ```text
//! Linear restriction: rho(x) = Ax + b
//! Residual: r = rho_i(x_i) - rho_j(x_j)
//! Energy: E = ||r||^2
//! ```
//!
//! ## Benefits
//!
//! - Geometric meaning: projects to shared semantic space
//! - Interpretable residuals: measure semantic mismatch
//! - Can be initialized from domain knowledge
//! - Residual energy provides natural attention weighting

use crate::error::{AttentionError, AttentionResult};
use serde::{Deserialize, Serialize};

/// Configuration for restriction map
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestrictionMapConfig {
    /// Input dimension (stalk dimension at source)
    pub input_dim: usize,
    /// Output dimension (stalk dimension at target)
    pub output_dim: usize,
    /// Whether to include bias term
    pub use_bias: bool,
    /// Initialization scale (Xavier scaling)
    pub init_scale: Option<f32>,
}

impl Default for RestrictionMapConfig {
    fn default() -> Self {
        Self {
            input_dim: 64,
            output_dim: 64,
            use_bias: true,
            init_scale: None,
        }
    }
}

impl RestrictionMapConfig {
    /// Create config with specified dimensions
    pub fn new(input_dim: usize, output_dim: usize) -> Self {
        Self {
            input_dim,
            output_dim,
            ..Default::default()
        }
    }

    /// Builder pattern: set input dimension
    pub fn with_input_dim(mut self, dim: usize) -> Self {
        self.input_dim = dim;
        self
    }

    /// Builder pattern: set output dimension
    pub fn with_output_dim(mut self, dim: usize) -> Self {
        self.output_dim = dim;
        self
    }

    /// Builder pattern: set bias usage
    pub fn with_bias(mut self, use_bias: bool) -> Self {
        self.use_bias = use_bias;
        self
    }

    /// Builder pattern: set initialization scale
    pub fn with_init_scale(mut self, scale: f32) -> Self {
        self.init_scale = Some(scale);
        self
    }
}

/// Linear restriction map: rho(x) = Ax + b
///
/// Projects vectors from one stalk to another, preserving geometric
/// relationships while allowing dimension changes.
#[derive(Debug, Clone)]
pub struct RestrictionMap {
    /// Weight matrix A: [output_dim x input_dim] stored row-major
    weights: Vec<f32>,
    /// Bias vector b: [output_dim]
    bias: Option<Vec<f32>>,
    /// Input dimension
    input_dim: usize,
    /// Output dimension
    output_dim: usize,
}

impl RestrictionMap {
    /// Create a new restriction map with Xavier initialization
    pub fn new(input_dim: usize, output_dim: usize) -> Self {
        Self::from_config(RestrictionMapConfig::new(input_dim, output_dim))
    }

    /// Create from configuration
    pub fn from_config(config: RestrictionMapConfig) -> Self {
        let scale = config
            .init_scale
            .unwrap_or_else(|| (2.0 / (config.input_dim + config.output_dim) as f32).sqrt());

        // Deterministic pseudo-random initialization
        let mut seed = 42u64;
        let weights: Vec<f32> = (0..config.output_dim * config.input_dim)
            .map(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let u = (seed as f32) / (u64::MAX as f32);
                (u - 0.5) * 2.0 * scale
            })
            .collect();

        let bias = if config.use_bias {
            Some(vec![0.0; config.output_dim])
        } else {
            None
        };

        Self {
            weights,
            bias,
            input_dim: config.input_dim,
            output_dim: config.output_dim,
        }
    }

    /// Create identity-like restriction map (for same dimension)
    pub fn identity(dim: usize) -> Self {
        let mut weights = vec![0.0; dim * dim];
        for i in 0..dim {
            weights[i * dim + i] = 1.0;
        }

        Self {
            weights,
            bias: None,
            input_dim: dim,
            output_dim: dim,
        }
    }

    /// Create from existing weights
    pub fn from_weights(
        weights: Vec<f32>,
        bias: Option<Vec<f32>>,
        input_dim: usize,
        output_dim: usize,
    ) -> AttentionResult<Self> {
        if weights.len() != output_dim * input_dim {
            return Err(AttentionError::DimensionMismatch {
                expected: output_dim * input_dim,
                actual: weights.len(),
            });
        }

        if let Some(ref b) = bias {
            if b.len() != output_dim {
                return Err(AttentionError::DimensionMismatch {
                    expected: output_dim,
                    actual: b.len(),
                });
            }
        }

        Ok(Self {
            weights,
            bias,
            input_dim,
            output_dim,
        })
    }

    /// Apply restriction map: rho(x) = Ax + b
    ///
    /// # Arguments
    ///
    /// * `x` - Input vector of shape [input_dim]
    ///
    /// # Returns
    ///
    /// Output vector of shape [output_dim]
    pub fn apply(&self, x: &[f32]) -> AttentionResult<Vec<f32>> {
        if x.len() != self.input_dim {
            return Err(AttentionError::DimensionMismatch {
                expected: self.input_dim,
                actual: x.len(),
            });
        }

        // Matrix-vector multiplication: y = Ax
        let mut y = vec![0.0; self.output_dim];
        for i in 0..self.output_dim {
            let row_start = i * self.input_dim;
            y[i] = x
                .iter()
                .enumerate()
                .map(|(j, &xj)| self.weights[row_start + j] * xj)
                .sum();
        }

        // Add bias: y = Ax + b
        if let Some(ref b) = self.bias {
            for (yi, bi) in y.iter_mut().zip(b.iter()) {
                *yi += bi;
            }
        }

        Ok(y)
    }

    /// Apply restriction map to batch of vectors
    ///
    /// # Arguments
    ///
    /// * `batch` - Batch of input vectors
    ///
    /// # Returns
    ///
    /// Batch of output vectors
    pub fn apply_batch(&self, batch: &[&[f32]]) -> AttentionResult<Vec<Vec<f32>>> {
        batch.iter().map(|x| self.apply(x)).collect()
    }

    /// Compute residual between two restricted vectors
    ///
    /// r_ij = rho(x_i) - rho(x_j)
    ///
    /// # Arguments
    ///
    /// * `x_i` - First input vector
    /// * `x_j` - Second input vector
    ///
    /// # Returns
    ///
    /// Residual vector
    pub fn residual(&self, x_i: &[f32], x_j: &[f32]) -> AttentionResult<Vec<f32>> {
        let rho_i = self.apply(x_i)?;
        let rho_j = self.apply(x_j)?;

        Ok(rho_i
            .iter()
            .zip(rho_j.iter())
            .map(|(&a, &b)| a - b)
            .collect())
    }

    /// Compute residual energy (squared L2 norm of residual)
    ///
    /// E_ij = ||rho(x_i) - rho(x_j)||^2
    ///
    /// # Arguments
    ///
    /// * `x_i` - First input vector
    /// * `x_j` - Second input vector
    ///
    /// # Returns
    ///
    /// Residual energy (non-negative scalar)
    pub fn energy(&self, x_i: &[f32], x_j: &[f32]) -> AttentionResult<f32> {
        let residual = self.residual(x_i, x_j)?;
        Ok(residual.iter().map(|r| r * r).sum())
    }

    /// Compute weighted residual energy
    ///
    /// E_ij = w * ||rho(x_i) - rho(x_j)||^2
    ///
    /// # Arguments
    ///
    /// * `x_i` - First input vector
    /// * `x_j` - Second input vector
    /// * `weight` - Edge weight
    ///
    /// # Returns
    ///
    /// Weighted residual energy
    pub fn weighted_energy(&self, x_i: &[f32], x_j: &[f32], weight: f32) -> AttentionResult<f32> {
        Ok(weight * self.energy(x_i, x_j)?)
    }

    /// Compute energy matrix for all pairs
    ///
    /// E[i,j] = ||rho(x_i) - rho(x_j)||^2
    ///
    /// # Arguments
    ///
    /// * `vectors` - Input vectors
    ///
    /// # Returns
    ///
    /// Energy matrix [N x N] stored row-major
    pub fn energy_matrix(&self, vectors: &[&[f32]]) -> AttentionResult<Vec<f32>> {
        let n = vectors.len();

        // First, apply restriction map to all vectors
        let restricted: Vec<Vec<f32>> = vectors
            .iter()
            .map(|v| self.apply(v))
            .collect::<AttentionResult<_>>()?;

        // Compute pairwise energies
        let mut energies = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    energies[i * n + j] = 0.0;
                } else {
                    let energy: f32 = restricted[i]
                        .iter()
                        .zip(restricted[j].iter())
                        .map(|(&a, &b)| (a - b) * (a - b))
                        .sum();
                    energies[i * n + j] = energy;
                }
            }
        }

        Ok(energies)
    }

    /// Get input dimension
    pub fn input_dim(&self) -> usize {
        self.input_dim
    }

    /// Get output dimension
    pub fn output_dim(&self) -> usize {
        self.output_dim
    }

    /// Get weight matrix (read-only)
    pub fn weights(&self) -> &[f32] {
        &self.weights
    }

    /// Get mutable weight matrix (for training)
    pub fn weights_mut(&mut self) -> &mut [f32] {
        &mut self.weights
    }

    /// Get bias vector (read-only)
    pub fn bias(&self) -> Option<&[f32]> {
        self.bias.as_deref()
    }

    /// Get mutable bias vector (for training)
    pub fn bias_mut(&mut self) -> Option<&mut [f32]> {
        self.bias.as_deref_mut()
    }

    /// Update weights with gradient
    pub fn update_weights(&mut self, gradients: &[f32], learning_rate: f32) {
        if gradients.len() == self.weights.len() {
            for (w, g) in self.weights.iter_mut().zip(gradients.iter()) {
                *w -= learning_rate * g;
            }
        }
    }

    /// Update bias with gradient
    pub fn update_bias(&mut self, gradients: &[f32], learning_rate: f32) {
        if let Some(ref mut bias) = self.bias {
            if gradients.len() == bias.len() {
                for (b, g) in bias.iter_mut().zip(gradients.iter()) {
                    *b -= learning_rate * g;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_restriction_map_creation() {
        let rmap = RestrictionMap::new(64, 32);
        assert_eq!(rmap.input_dim(), 64);
        assert_eq!(rmap.output_dim(), 32);
        assert_eq!(rmap.weights().len(), 64 * 32);
        assert!(rmap.bias().is_some());
    }

    #[test]
    fn test_identity_restriction() {
        let rmap = RestrictionMap::identity(4);
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let y = rmap.apply(&x).unwrap();

        for (xi, yi) in x.iter().zip(y.iter()) {
            assert!((xi - yi).abs() < 1e-6);
        }
    }

    #[test]
    fn test_apply() {
        let rmap = RestrictionMap::new(4, 3);
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let y = rmap.apply(&x).unwrap();

        assert_eq!(y.len(), 3);
    }

    #[test]
    fn test_apply_dimension_mismatch() {
        let rmap = RestrictionMap::new(4, 3);
        let x = vec![1.0, 2.0]; // Wrong dimension

        assert!(rmap.apply(&x).is_err());
    }

    #[test]
    fn test_residual() {
        let rmap = RestrictionMap::identity(4);
        let x_i = vec![1.0, 2.0, 3.0, 4.0];
        let x_j = vec![2.0, 3.0, 4.0, 5.0];
        let residual = rmap.residual(&x_i, &x_j).unwrap();

        // Should be x_i - x_j = [-1, -1, -1, -1]
        for r in &residual {
            assert!((*r + 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_energy() {
        let rmap = RestrictionMap::identity(4);
        let x_i = vec![1.0, 2.0, 3.0, 4.0];
        let x_j = vec![2.0, 3.0, 4.0, 5.0];
        let energy = rmap.energy(&x_i, &x_j).unwrap();

        // Residual = [-1, -1, -1, -1], energy = 4
        assert!((energy - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_energy_symmetry() {
        let rmap = RestrictionMap::new(8, 8);
        let x_i = vec![1.0; 8];
        let x_j = vec![0.5; 8];

        let e_ij = rmap.energy(&x_i, &x_j).unwrap();
        let e_ji = rmap.energy(&x_j, &x_i).unwrap();

        assert!((e_ij - e_ji).abs() < 1e-6);
    }

    #[test]
    fn test_energy_matrix() {
        let rmap = RestrictionMap::identity(4);
        let v1 = vec![1.0, 0.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0, 0.0];
        let v3 = vec![0.0, 0.0, 1.0, 0.0];
        let vectors: Vec<&[f32]> = vec![&v1, &v2, &v3];

        let energies = rmap.energy_matrix(&vectors).unwrap();

        // Diagonal should be 0
        assert!(energies[0].abs() < 1e-6); // E[0,0]
        assert!(energies[4].abs() < 1e-6); // E[1,1]
        assert!(energies[8].abs() < 1e-6); // E[2,2]

        // Off-diagonal: ||e_i - e_j||^2 = 2 for orthonormal basis
        assert!((energies[1] - 2.0).abs() < 1e-6); // E[0,1]
        assert!((energies[3] - 2.0).abs() < 1e-6); // E[1,0]
    }

    #[test]
    fn test_batch_apply() {
        let rmap = RestrictionMap::new(4, 3);
        let v1 = vec![1.0; 4];
        let v2 = vec![2.0; 4];
        let batch: Vec<&[f32]> = vec![&v1, &v2];

        let results = rmap.apply_batch(&batch).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].len(), 3);
        assert_eq!(results[1].len(), 3);
    }

    #[test]
    fn test_from_weights() {
        let weights = vec![1.0, 0.0, 0.0, 1.0]; // 2x2 identity
        let bias = Some(vec![0.5, 0.5]);

        let rmap = RestrictionMap::from_weights(weights, bias, 2, 2).unwrap();
        let x = vec![1.0, 2.0];
        let y = rmap.apply(&x).unwrap();

        assert!((y[0] - 1.5).abs() < 1e-6); // 1*1 + 0*2 + 0.5
        assert!((y[1] - 2.5).abs() < 1e-6); // 0*1 + 1*2 + 0.5
    }

    #[test]
    fn test_config_builder() {
        let config = RestrictionMapConfig::default()
            .with_input_dim(128)
            .with_output_dim(64)
            .with_bias(false)
            .with_init_scale(0.1);

        assert_eq!(config.input_dim, 128);
        assert_eq!(config.output_dim, 64);
        assert!(!config.use_bias);
        assert_eq!(config.init_scale, Some(0.1));
    }
}
