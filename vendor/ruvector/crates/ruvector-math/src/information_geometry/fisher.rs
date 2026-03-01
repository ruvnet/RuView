//! Fisher Information Matrix
//!
//! The Fisher Information Matrix (FIM) captures the curvature of the log-likelihood
//! surface and defines the natural metric on statistical manifolds.
//!
//! ## Definition
//!
//! F(θ) = E[∇log p(x|θ) ∇log p(x|θ)^T]
//!
//! For Gaussian distributions with fixed variance:
//! F(μ) = I/σ² (identity scaled by inverse variance)
//!
//! ## Use Cases
//!
//! - Natural gradient computation
//! - Information-theoretic regularization
//! - Model uncertainty quantification

use crate::error::{MathError, Result};
use crate::utils::EPS;

/// Fisher Information Matrix calculator
#[derive(Debug, Clone)]
pub struct FisherInformation {
    /// Damping factor for numerical stability
    damping: f64,
    /// Number of samples for empirical estimation
    num_samples: usize,
}

impl FisherInformation {
    /// Create a new FIM calculator
    pub fn new() -> Self {
        Self {
            damping: 1e-4,
            num_samples: 100,
        }
    }

    /// Set damping factor (for matrix inversion stability)
    pub fn with_damping(mut self, damping: f64) -> Self {
        self.damping = damping.max(EPS);
        self
    }

    /// Set number of samples for empirical FIM
    pub fn with_samples(mut self, num_samples: usize) -> Self {
        self.num_samples = num_samples.max(1);
        self
    }

    /// Compute empirical FIM from gradient samples
    ///
    /// F ≈ (1/N) Σᵢ ∇log p(xᵢ|θ) ∇log p(xᵢ|θ)^T
    ///
    /// # Arguments
    /// * `gradients` - Sample gradients, each of length d
    pub fn empirical_fim(&self, gradients: &[Vec<f64>]) -> Result<Vec<Vec<f64>>> {
        if gradients.is_empty() {
            return Err(MathError::empty_input("gradients"));
        }

        let d = gradients[0].len();
        if d == 0 {
            return Err(MathError::empty_input("gradient dimension"));
        }

        let n = gradients.len() as f64;

        // F = (1/n) Σ g gᵀ
        let mut fim = vec![vec![0.0; d]; d];

        for grad in gradients {
            if grad.len() != d {
                return Err(MathError::dimension_mismatch(d, grad.len()));
            }

            for i in 0..d {
                for j in 0..d {
                    fim[i][j] += grad[i] * grad[j] / n;
                }
            }
        }

        // Add damping for stability
        for i in 0..d {
            fim[i][i] += self.damping;
        }

        Ok(fim)
    }

    /// Compute diagonal FIM approximation (much faster)
    ///
    /// Only computes diagonal: F_ii ≈ (1/N) Σₙ (∂log p / ∂θᵢ)²
    pub fn diagonal_fim(&self, gradients: &[Vec<f64>]) -> Result<Vec<f64>> {
        if gradients.is_empty() {
            return Err(MathError::empty_input("gradients"));
        }

        let d = gradients[0].len();
        let n = gradients.len() as f64;

        let mut diag = vec![0.0; d];

        for grad in gradients {
            if grad.len() != d {
                return Err(MathError::dimension_mismatch(d, grad.len()));
            }

            for (i, &g) in grad.iter().enumerate() {
                diag[i] += g * g / n;
            }
        }

        // Add damping
        for d_i in &mut diag {
            *d_i += self.damping;
        }

        Ok(diag)
    }

    /// Compute FIM for Gaussian distribution with known variance
    ///
    /// For N(μ, σ²I): F(μ) = I/σ²
    pub fn gaussian_fim(&self, dim: usize, variance: f64) -> Vec<Vec<f64>> {
        let scale = 1.0 / (variance + self.damping);
        let mut fim = vec![vec![0.0; dim]; dim];
        for i in 0..dim {
            fim[i][i] = scale;
        }
        fim
    }

    /// Compute FIM for categorical distribution
    ///
    /// For categorical p = (p₁, ..., pₖ): F_ij = δᵢⱼ/pᵢ - 1
    pub fn categorical_fim(&self, probabilities: &[f64]) -> Result<Vec<Vec<f64>>> {
        let k = probabilities.len();
        if k == 0 {
            return Err(MathError::empty_input("probabilities"));
        }

        let mut fim = vec![vec![-1.0; k]; k]; // Off-diagonal = -1

        for (i, &pi) in probabilities.iter().enumerate() {
            let safe_pi = pi.max(EPS);
            fim[i][i] = 1.0 / safe_pi - 1.0 + self.damping;
        }

        Ok(fim)
    }

    /// Invert FIM using Cholesky decomposition
    ///
    /// Returns F⁻¹ for natural gradient computation
    pub fn invert_fim(&self, fim: &[Vec<f64>]) -> Result<Vec<Vec<f64>>> {
        let n = fim.len();
        if n == 0 {
            return Err(MathError::empty_input("FIM"));
        }

        // Cholesky decomposition: F = LLᵀ
        let mut l = vec![vec![0.0; n]; n];

        for i in 0..n {
            for j in 0..=i {
                let mut sum = fim[i][j];

                for k in 0..j {
                    sum -= l[i][k] * l[j][k];
                }

                if i == j {
                    if sum <= 0.0 {
                        // Matrix not positive definite
                        return Err(MathError::numerical_instability(
                            "FIM not positive definite",
                        ));
                    }
                    l[i][j] = sum.sqrt();
                } else {
                    l[i][j] = sum / l[j][j];
                }
            }
        }

        // Forward substitution to get L⁻¹
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

        // F⁻¹ = (LLᵀ)⁻¹ = L⁻ᵀ L⁻¹
        let mut fim_inv = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                for k in 0..n {
                    fim_inv[i][j] += l_inv[k][i] * l_inv[k][j];
                }
            }
        }

        Ok(fim_inv)
    }

    /// Compute natural gradient: F⁻¹ ∇L
    pub fn natural_gradient(&self, fim: &[Vec<f64>], gradient: &[f64]) -> Result<Vec<f64>> {
        let fim_inv = self.invert_fim(fim)?;
        let n = gradient.len();

        if fim_inv.len() != n {
            return Err(MathError::dimension_mismatch(n, fim_inv.len()));
        }

        let mut nat_grad = vec![0.0; n];
        for i in 0..n {
            for j in 0..n {
                nat_grad[i] += fim_inv[i][j] * gradient[j];
            }
        }

        Ok(nat_grad)
    }
}

impl Default for FisherInformation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empirical_fim() {
        let fisher = FisherInformation::new().with_damping(0.0);

        // Simple gradients
        let grads = vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![1.0, 1.0]];

        let fim = fisher.empirical_fim(&grads).unwrap();

        // Expected: [[2/3, 1/3], [1/3, 2/3]] + small damping
        assert!((fim[0][0] - 2.0 / 3.0).abs() < 1e-6);
        assert!((fim[1][1] - 2.0 / 3.0).abs() < 1e-6);
        assert!((fim[0][1] - 1.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_gaussian_fim() {
        let fisher = FisherInformation::new().with_damping(0.0);
        let fim = fisher.gaussian_fim(3, 0.5);

        // F = I / 0.5 = 2I (plus small damping on diagonal)
        assert!((fim[0][0] - 2.0).abs() < 1e-6);
        assert!((fim[1][1] - 2.0).abs() < 1e-6);
        assert!(fim[0][1].abs() < 1e-6);
    }

    #[test]
    fn test_fim_inversion() {
        let fisher = FisherInformation::new();

        // Identity matrix
        let fim = vec![vec![1.0, 0.0], vec![0.0, 1.0]];

        let fim_inv = fisher.invert_fim(&fim).unwrap();

        // Inverse of identity is identity
        assert!((fim_inv[0][0] - 1.0).abs() < 1e-6);
        assert!((fim_inv[1][1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_natural_gradient() {
        let fisher = FisherInformation::new().with_damping(0.0);

        // F = 2I
        let fim = vec![vec![2.0, 0.0], vec![0.0, 2.0]];
        let grad = vec![4.0, 6.0];

        let nat_grad = fisher.natural_gradient(&fim, &grad).unwrap();

        // nat_grad = F⁻¹ grad = (1/2) grad
        assert!((nat_grad[0] - 2.0).abs() < 1e-6);
        assert!((nat_grad[1] - 3.0).abs() < 1e-6);
    }
}
