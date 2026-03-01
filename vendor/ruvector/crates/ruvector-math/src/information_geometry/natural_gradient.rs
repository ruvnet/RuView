//! Natural Gradient Descent
//!
//! Natural gradient descent rescales gradient updates to account for the
//! curvature of the parameter space, leading to faster convergence.
//!
//! ## Algorithm
//!
//! θ_{t+1} = θ_t - η F(θ_t)⁻¹ ∇L(θ_t)
//!
//! where F is the Fisher Information Matrix.
//!
//! ## Benefits
//!
//! - **Invariant to reparameterization**: Same trajectory regardless of parameterization
//! - **Faster convergence**: 3-5x fewer iterations than SGD/Adam on well-conditioned problems
//! - **Better generalization**: Follows geodesics in probability space

use super::FisherInformation;
use crate::error::{MathError, Result};
use crate::utils::EPS;

/// Natural gradient optimizer state
#[derive(Debug, Clone)]
pub struct NaturalGradient {
    /// Learning rate
    learning_rate: f64,
    /// Damping factor for FIM
    damping: f64,
    /// Whether to use diagonal approximation
    use_diagonal: bool,
    /// Exponential moving average factor for FIM
    ema_factor: f64,
    /// Running FIM estimate
    fim_estimate: Option<FimEstimate>,
}

#[derive(Debug, Clone)]
enum FimEstimate {
    Full(Vec<Vec<f64>>),
    Diagonal(Vec<f64>),
}

impl NaturalGradient {
    /// Create a new natural gradient optimizer
    ///
    /// # Arguments
    /// * `learning_rate` - Step size (0.01-0.1 typical)
    pub fn new(learning_rate: f64) -> Self {
        Self {
            learning_rate: learning_rate.max(EPS),
            damping: 1e-4,
            use_diagonal: false,
            ema_factor: 0.9,
            fim_estimate: None,
        }
    }

    /// Set damping factor
    pub fn with_damping(mut self, damping: f64) -> Self {
        self.damping = damping.max(EPS);
        self
    }

    /// Use diagonal FIM approximation (faster, less memory)
    pub fn with_diagonal(mut self, use_diagonal: bool) -> Self {
        self.use_diagonal = use_diagonal;
        self
    }

    /// Set EMA factor for FIM smoothing
    pub fn with_ema(mut self, ema: f64) -> Self {
        self.ema_factor = ema.clamp(0.0, 1.0);
        self
    }

    /// Compute natural gradient step
    ///
    /// # Arguments
    /// * `gradient` - Standard gradient ∇L
    /// * `gradient_samples` - Optional gradient samples for FIM estimation
    pub fn step(
        &mut self,
        gradient: &[f64],
        gradient_samples: Option<&[Vec<f64>]>,
    ) -> Result<Vec<f64>> {
        // Update FIM estimate if samples provided
        if let Some(samples) = gradient_samples {
            self.update_fim(samples)?;
        }

        // Compute natural gradient
        let nat_grad = match &self.fim_estimate {
            Some(FimEstimate::Full(fim)) => {
                let fisher = FisherInformation::new().with_damping(self.damping);
                fisher.natural_gradient(fim, gradient)?
            }
            Some(FimEstimate::Diagonal(diag)) => {
                // Element-wise: nat_grad = grad / diag
                gradient
                    .iter()
                    .zip(diag.iter())
                    .map(|(&g, &d)| g / (d + self.damping))
                    .collect()
            }
            None => {
                // No FIM estimate, use gradient as-is
                gradient.to_vec()
            }
        };

        // Scale by learning rate
        Ok(nat_grad.iter().map(|&g| -self.learning_rate * g).collect())
    }

    /// Update running FIM estimate
    fn update_fim(&mut self, gradient_samples: &[Vec<f64>]) -> Result<()> {
        let fisher = FisherInformation::new().with_damping(0.0);

        if self.use_diagonal {
            let new_diag = fisher.diagonal_fim(gradient_samples)?;

            self.fim_estimate = Some(FimEstimate::Diagonal(match &self.fim_estimate {
                Some(FimEstimate::Diagonal(old)) => {
                    // EMA update
                    old.iter()
                        .zip(new_diag.iter())
                        .map(|(&o, &n)| self.ema_factor * o + (1.0 - self.ema_factor) * n)
                        .collect()
                }
                _ => new_diag,
            }));
        } else {
            let new_fim = fisher.empirical_fim(gradient_samples)?;
            let dim = new_fim.len();

            self.fim_estimate = Some(FimEstimate::Full(match &self.fim_estimate {
                Some(FimEstimate::Full(old)) if old.len() == dim => {
                    // EMA update
                    (0..dim)
                        .map(|i| {
                            (0..dim)
                                .map(|j| {
                                    self.ema_factor * old[i][j]
                                        + (1.0 - self.ema_factor) * new_fim[i][j]
                                })
                                .collect()
                        })
                        .collect()
                }
                _ => new_fim,
            }));
        }

        Ok(())
    }

    /// Apply update to parameters
    pub fn apply_update(parameters: &mut [f64], update: &[f64]) -> Result<()> {
        if parameters.len() != update.len() {
            return Err(MathError::dimension_mismatch(
                parameters.len(),
                update.len(),
            ));
        }

        for (p, &u) in parameters.iter_mut().zip(update.iter()) {
            *p += u;
        }

        Ok(())
    }

    /// Full optimization step: compute and apply update
    pub fn optimize_step(
        &mut self,
        parameters: &mut [f64],
        gradient: &[f64],
        gradient_samples: Option<&[Vec<f64>]>,
    ) -> Result<f64> {
        let update = self.step(gradient, gradient_samples)?;

        let update_norm: f64 = update.iter().map(|&u| u * u).sum::<f64>().sqrt();

        Self::apply_update(parameters, &update)?;

        Ok(update_norm)
    }

    /// Reset optimizer state
    pub fn reset(&mut self) {
        self.fim_estimate = None;
    }
}

/// Natural gradient with diagonal preconditioning (AdaGrad-like)
#[derive(Debug, Clone)]
pub struct DiagonalNaturalGradient {
    /// Learning rate
    learning_rate: f64,
    /// Damping factor
    damping: f64,
    /// Accumulated squared gradients
    accumulator: Vec<f64>,
}

impl DiagonalNaturalGradient {
    /// Create new diagonal natural gradient optimizer
    pub fn new(learning_rate: f64, dim: usize) -> Self {
        Self {
            learning_rate: learning_rate.max(EPS),
            damping: 1e-8,
            accumulator: vec![0.0; dim],
        }
    }

    /// Set damping factor
    pub fn with_damping(mut self, damping: f64) -> Self {
        self.damping = damping.max(EPS);
        self
    }

    /// Compute and apply update
    pub fn step(&mut self, parameters: &mut [f64], gradient: &[f64]) -> Result<f64> {
        if parameters.len() != gradient.len() || parameters.len() != self.accumulator.len() {
            return Err(MathError::dimension_mismatch(
                parameters.len(),
                gradient.len(),
            ));
        }

        let mut update_norm_sq = 0.0;

        for (i, (p, &g)) in parameters.iter_mut().zip(gradient.iter()).enumerate() {
            // Accumulate squared gradient (Fisher diagonal approximation)
            self.accumulator[i] += g * g;

            // Natural gradient step
            let update = -self.learning_rate * g / (self.accumulator[i].sqrt() + self.damping);
            *p += update;
            update_norm_sq += update * update;
        }

        Ok(update_norm_sq.sqrt())
    }

    /// Reset accumulator
    pub fn reset(&mut self) {
        self.accumulator.fill(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_natural_gradient_step() {
        let mut ng = NaturalGradient::new(0.1).with_diagonal(true);

        let gradient = vec![1.0, 2.0, 3.0];

        // First step without FIM estimate uses gradient directly
        let update = ng.step(&gradient, None).unwrap();

        assert_eq!(update.len(), 3);
        // Should be -lr * gradient
        assert!((update[0] + 0.1).abs() < 1e-10);
    }

    #[test]
    fn test_natural_gradient_with_fim() {
        let mut ng = NaturalGradient::new(0.1)
            .with_diagonal(true)
            .with_damping(0.0);

        let gradient = vec![2.0, 4.0];

        // Provide gradient samples for FIM estimation
        let samples = vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![1.0, 1.0]];

        let update = ng.step(&gradient, Some(&samples)).unwrap();

        // With FIM, update should be preconditioned
        assert_eq!(update.len(), 2);
    }

    #[test]
    fn test_diagonal_natural_gradient() {
        let mut dng = DiagonalNaturalGradient::new(1.0, 2);

        let mut params = vec![0.0, 0.0];
        let gradient = vec![1.0, 2.0];

        let norm = dng.step(&mut params, &gradient).unwrap();

        assert!(norm > 0.0);
        // Parameters should have moved
        assert!(params[0] < 0.0); // Moved in negative gradient direction
    }

    #[test]
    fn test_optimizer_reset() {
        let mut ng = NaturalGradient::new(0.1);

        let samples = vec![vec![1.0, 2.0]];
        let _ = ng.step(&[1.0, 1.0], Some(&samples));

        ng.reset();
        assert!(ng.fim_estimate.is_none());
    }
}
