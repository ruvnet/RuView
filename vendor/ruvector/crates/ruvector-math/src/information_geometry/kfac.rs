//! K-FAC: Kronecker-Factored Approximate Curvature
//!
//! K-FAC approximates the Fisher Information Matrix for neural networks using
//! Kronecker products, reducing storage from O(n²) to O(n) and inversion from
//! O(n³) to O(n^{3/2}).
//!
//! ## Theory
//!
//! For a layer with weights W ∈ R^{m×n}:
//! - Gradient: ∇W = g ⊗ a (outer product of pre/post activations)
//! - FIM block: F_W ≈ E[gg^T] ⊗ E[aa^T] = G ⊗ A (Kronecker factorization)
//!
//! ## Benefits
//!
//! - **Memory efficient**: Store two small matrices instead of one huge one
//! - **Fast inversion**: (G ⊗ A)⁻¹ = G⁻¹ ⊗ A⁻¹
//! - **Practical natural gradient**: Scales to large networks
//!
//! ## References
//!
//! - Martens & Grosse (2015): "Optimizing Neural Networks with Kronecker-factored
//!   Approximate Curvature"

use crate::error::{MathError, Result};
use crate::utils::EPS;

/// K-FAC approximation for a single layer
#[derive(Debug, Clone)]
pub struct KFACLayer {
    /// Input-side factor A = E[aa^T]
    pub a_factor: Vec<Vec<f64>>,
    /// Output-side factor G = E[gg^T]
    pub g_factor: Vec<Vec<f64>>,
    /// Damping factor
    damping: f64,
    /// EMA factor for running estimates
    ema_factor: f64,
    /// Number of updates
    num_updates: usize,
}

impl KFACLayer {
    /// Create a new K-FAC layer approximation
    ///
    /// # Arguments
    /// * `input_dim` - Size of input activations
    /// * `output_dim` - Size of output gradients
    pub fn new(input_dim: usize, output_dim: usize) -> Self {
        Self {
            a_factor: vec![vec![0.0; input_dim]; input_dim],
            g_factor: vec![vec![0.0; output_dim]; output_dim],
            damping: 1e-3,
            ema_factor: 0.95,
            num_updates: 0,
        }
    }

    /// Set damping factor
    pub fn with_damping(mut self, damping: f64) -> Self {
        self.damping = damping.max(EPS);
        self
    }

    /// Set EMA factor
    pub fn with_ema(mut self, ema: f64) -> Self {
        self.ema_factor = ema.clamp(0.0, 1.0);
        self
    }

    /// Update factors with new activations and gradients
    ///
    /// # Arguments
    /// * `activations` - Pre-activation inputs, shape [batch, input_dim]
    /// * `gradients` - Post-activation gradients, shape [batch, output_dim]
    pub fn update(&mut self, activations: &[Vec<f64>], gradients: &[Vec<f64>]) -> Result<()> {
        if activations.is_empty() || gradients.is_empty() {
            return Err(MathError::empty_input("batch"));
        }

        let batch_size = activations.len();
        if gradients.len() != batch_size {
            return Err(MathError::dimension_mismatch(batch_size, gradients.len()));
        }

        let input_dim = self.a_factor.len();
        let output_dim = self.g_factor.len();

        // Compute A = E[aa^T]
        let mut new_a = vec![vec![0.0; input_dim]; input_dim];
        for act in activations {
            if act.len() != input_dim {
                return Err(MathError::dimension_mismatch(input_dim, act.len()));
            }
            for i in 0..input_dim {
                for j in 0..input_dim {
                    new_a[i][j] += act[i] * act[j] / batch_size as f64;
                }
            }
        }

        // Compute G = E[gg^T]
        let mut new_g = vec![vec![0.0; output_dim]; output_dim];
        for grad in gradients {
            if grad.len() != output_dim {
                return Err(MathError::dimension_mismatch(output_dim, grad.len()));
            }
            for i in 0..output_dim {
                for j in 0..output_dim {
                    new_g[i][j] += grad[i] * grad[j] / batch_size as f64;
                }
            }
        }

        // EMA update
        if self.num_updates == 0 {
            self.a_factor = new_a;
            self.g_factor = new_g;
        } else {
            for i in 0..input_dim {
                for j in 0..input_dim {
                    self.a_factor[i][j] = self.ema_factor * self.a_factor[i][j]
                        + (1.0 - self.ema_factor) * new_a[i][j];
                }
            }
            for i in 0..output_dim {
                for j in 0..output_dim {
                    self.g_factor[i][j] = self.ema_factor * self.g_factor[i][j]
                        + (1.0 - self.ema_factor) * new_g[i][j];
                }
            }
        }

        self.num_updates += 1;
        Ok(())
    }

    /// Compute natural gradient for weight matrix
    ///
    /// nat_grad = G⁻¹ ∇W A⁻¹
    ///
    /// # Arguments
    /// * `weight_grad` - Gradient w.r.t. weights, shape [output_dim, input_dim]
    pub fn natural_gradient(&self, weight_grad: &[Vec<f64>]) -> Result<Vec<Vec<f64>>> {
        let output_dim = self.g_factor.len();
        let input_dim = self.a_factor.len();

        if weight_grad.len() != output_dim {
            return Err(MathError::dimension_mismatch(output_dim, weight_grad.len()));
        }

        // Add damping to factors
        let a_damped = self.add_damping(&self.a_factor);
        let g_damped = self.add_damping(&self.g_factor);

        // Invert factors
        let a_inv = self.invert_matrix(&a_damped)?;
        let g_inv = self.invert_matrix(&g_damped)?;

        // Compute G⁻¹ ∇W A⁻¹
        // First: ∇W A⁻¹
        let mut grad_a_inv = vec![vec![0.0; input_dim]; output_dim];
        for i in 0..output_dim {
            for j in 0..input_dim {
                for k in 0..input_dim {
                    grad_a_inv[i][j] += weight_grad[i][k] * a_inv[k][j];
                }
            }
        }

        // Then: G⁻¹ (∇W A⁻¹)
        let mut nat_grad = vec![vec![0.0; input_dim]; output_dim];
        for i in 0..output_dim {
            for j in 0..input_dim {
                for k in 0..output_dim {
                    nat_grad[i][j] += g_inv[i][k] * grad_a_inv[k][j];
                }
            }
        }

        Ok(nat_grad)
    }

    /// Add damping to diagonal of matrix
    fn add_damping(&self, matrix: &[Vec<f64>]) -> Vec<Vec<f64>> {
        let n = matrix.len();
        let mut damped = matrix.to_vec();

        // Add π-damping (Tikhonov + trace normalization)
        let trace: f64 = (0..n).map(|i| matrix[i][i]).sum();
        let pi_damping = (self.damping * trace / n as f64).max(EPS);

        for i in 0..n {
            damped[i][i] += pi_damping;
        }

        damped
    }

    /// Invert matrix using Cholesky decomposition
    fn invert_matrix(&self, matrix: &[Vec<f64>]) -> Result<Vec<Vec<f64>>> {
        let n = matrix.len();

        // Cholesky: A = LLᵀ
        let mut l = vec![vec![0.0; n]; n];

        for i in 0..n {
            for j in 0..=i {
                let mut sum = matrix[i][j];
                for k in 0..j {
                    sum -= l[i][k] * l[j][k];
                }

                if i == j {
                    if sum <= 0.0 {
                        return Err(MathError::numerical_instability(
                            "Matrix not positive definite in K-FAC",
                        ));
                    }
                    l[i][j] = sum.sqrt();
                } else {
                    l[i][j] = sum / l[j][j];
                }
            }
        }

        // L⁻¹ via forward substitution
        let mut l_inv = vec![vec![0.0; n]; n];
        for i in 0..n {
            l_inv[i][i] = 1.0 / l[i][i];
            for j in (i + 1)..n {
                let mut sum = 0.0;
                for k in i..j {
                    sum -= l[j][k] * l_inv[k][i];
                }
                l_inv[j][i] = sum / l[j][j];
            }
        }

        // A⁻¹ = L⁻ᵀL⁻¹
        let mut inv = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                for k in 0..n {
                    inv[i][j] += l_inv[k][i] * l_inv[k][j];
                }
            }
        }

        Ok(inv)
    }

    /// Reset factor estimates
    pub fn reset(&mut self) {
        let input_dim = self.a_factor.len();
        let output_dim = self.g_factor.len();

        self.a_factor = vec![vec![0.0; input_dim]; input_dim];
        self.g_factor = vec![vec![0.0; output_dim]; output_dim];
        self.num_updates = 0;
    }
}

/// K-FAC approximation for full network
#[derive(Debug, Clone)]
pub struct KFACApproximation {
    /// Per-layer K-FAC factors
    layers: Vec<KFACLayer>,
    /// Learning rate
    learning_rate: f64,
    /// Global damping
    damping: f64,
}

impl KFACApproximation {
    /// Create K-FAC optimizer for a network
    ///
    /// # Arguments
    /// * `layer_dims` - List of (input_dim, output_dim) for each layer
    pub fn new(layer_dims: &[(usize, usize)]) -> Self {
        let layers = layer_dims
            .iter()
            .map(|&(input, output)| KFACLayer::new(input, output))
            .collect();

        Self {
            layers,
            learning_rate: 0.01,
            damping: 1e-3,
        }
    }

    /// Set learning rate
    pub fn with_learning_rate(mut self, lr: f64) -> Self {
        self.learning_rate = lr.max(EPS);
        self
    }

    /// Set damping
    pub fn with_damping(mut self, damping: f64) -> Self {
        self.damping = damping.max(EPS);
        for layer in &mut self.layers {
            layer.damping = damping;
        }
        self
    }

    /// Update factors for a layer
    pub fn update_layer(
        &mut self,
        layer_idx: usize,
        activations: &[Vec<f64>],
        gradients: &[Vec<f64>],
    ) -> Result<()> {
        if layer_idx >= self.layers.len() {
            return Err(MathError::invalid_parameter(
                "layer_idx",
                "index out of bounds",
            ));
        }

        self.layers[layer_idx].update(activations, gradients)
    }

    /// Compute natural gradient for a layer's weights
    pub fn natural_gradient_layer(
        &self,
        layer_idx: usize,
        weight_grad: &[Vec<f64>],
    ) -> Result<Vec<Vec<f64>>> {
        if layer_idx >= self.layers.len() {
            return Err(MathError::invalid_parameter(
                "layer_idx",
                "index out of bounds",
            ));
        }

        let mut nat_grad = self.layers[layer_idx].natural_gradient(weight_grad)?;

        // Scale by learning rate
        for row in &mut nat_grad {
            for val in row {
                *val *= -self.learning_rate;
            }
        }

        Ok(nat_grad)
    }

    /// Get number of layers
    pub fn num_layers(&self) -> usize {
        self.layers.len()
    }

    /// Reset all layer estimates
    pub fn reset(&mut self) {
        for layer in &mut self.layers {
            layer.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kfac_layer_creation() {
        let layer = KFACLayer::new(10, 5);

        assert_eq!(layer.a_factor.len(), 10);
        assert_eq!(layer.g_factor.len(), 5);
    }

    #[test]
    fn test_kfac_layer_update() {
        let mut layer = KFACLayer::new(3, 2);

        let activations = vec![vec![1.0, 0.0, 1.0], vec![0.0, 1.0, 1.0]];

        let gradients = vec![vec![0.5, 0.5], vec![0.3, 0.7]];

        layer.update(&activations, &gradients).unwrap();

        // Factors should be updated
        assert!(layer.a_factor[0][0] > 0.0);
        assert!(layer.g_factor[0][0] > 0.0);
    }

    #[test]
    fn test_kfac_natural_gradient() {
        let mut layer = KFACLayer::new(2, 2).with_damping(0.1);

        // Initialize with identity-like factors
        let activations = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let gradients = vec![vec![1.0, 0.0], vec![0.0, 1.0]];

        layer.update(&activations, &gradients).unwrap();

        let weight_grad = vec![vec![0.1, 0.2], vec![0.3, 0.4]];

        let nat_grad = layer.natural_gradient(&weight_grad).unwrap();

        assert_eq!(nat_grad.len(), 2);
        assert_eq!(nat_grad[0].len(), 2);
    }

    #[test]
    fn test_kfac_full_network() {
        let kfac = KFACApproximation::new(&[(10, 20), (20, 5)])
            .with_learning_rate(0.01)
            .with_damping(0.001);

        assert_eq!(kfac.num_layers(), 2);
    }
}
