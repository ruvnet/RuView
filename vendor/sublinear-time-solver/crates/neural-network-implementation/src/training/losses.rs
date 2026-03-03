//! Loss functions for training temporal neural networks

use crate::error::{Result, TemporalNeuralError};
use nalgebra::DVector;

/// Trait for loss functions
pub trait LossFunction: Send + Sync {
    /// Compute loss between prediction and target
    fn compute_loss(&self, prediction: &DVector<f64>, target: &DVector<f64>) -> Result<f64>;

    /// Compute gradient of loss with respect to prediction
    fn compute_gradient(&self, prediction: &DVector<f64>, target: &DVector<f64>) -> Result<DVector<f64>>;
}

/// Mean Squared Error loss with optional smoothness penalty
pub struct MseLoss {
    smoothness_weight: f64,
}

impl MseLoss {
    pub fn new(smoothness_weight: f64) -> Self {
        Self { smoothness_weight }
    }
}

impl LossFunction for MseLoss {
    fn compute_loss(&self, prediction: &DVector<f64>, target: &DVector<f64>) -> Result<f64> {
        if prediction.len() != target.len() {
            return Err(TemporalNeuralError::TrainingError {
                epoch: 0,
                message: "Prediction and target dimension mismatch".to_string(),
                metrics: None,
            });
        }

        let diff = prediction - target;
        let mse = diff.norm_squared() / prediction.len() as f64;

        // Add smoothness penalty if enabled
        let smoothness_penalty = if self.smoothness_weight > 0.0 && prediction.len() >= 2 {
            // Penalize large velocities (assuming prediction is [x, y])
            let velocity_penalty = prediction[0].powi(2) + prediction[1].powi(2);
            self.smoothness_weight * velocity_penalty
        } else {
            0.0
        };

        Ok(mse + smoothness_penalty)
    }

    fn compute_gradient(&self, prediction: &DVector<f64>, target: &DVector<f64>) -> Result<DVector<f64>> {
        if prediction.len() != target.len() {
            return Err(TemporalNeuralError::TrainingError {
                epoch: 0,
                message: "Prediction and target dimension mismatch".to_string(),
                metrics: None,
            });
        }

        let mut grad = 2.0 * (prediction - target) / prediction.len() as f64;

        // Add smoothness gradient
        if self.smoothness_weight > 0.0 && prediction.len() >= 2 {
            grad[0] += 2.0 * self.smoothness_weight * prediction[0];
            grad[1] += 2.0 * self.smoothness_weight * prediction[1];
        }

        Ok(grad)
    }
}

/// Smoothness penalty for temporal predictions
pub struct SmoothnessPenalty {
    weight: f64,
}

impl SmoothnessPenalty {
    pub fn new(weight: f64) -> Self {
        Self { weight }
    }

    pub fn compute_penalty(&self, prediction: &DVector<f64>) -> f64 {
        if prediction.len() < 2 {
            return 0.0;
        }

        // Penalize large magnitudes (velocity penalty)
        self.weight * prediction.norm_squared()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mse_loss() {
        let loss_fn = MseLoss::new(0.0);
        let prediction = DVector::from_vec(vec![1.0, 2.0]);
        let target = DVector::from_vec(vec![1.5, 1.5]);

        let loss = loss_fn.compute_loss(&prediction, &target).unwrap();
        assert!(loss > 0.0);

        let grad = loss_fn.compute_gradient(&prediction, &target).unwrap();
        assert_eq!(grad.len(), 2);
    }

    #[test]
    fn test_mse_with_smoothness() {
        let loss_fn = MseLoss::new(0.1);
        let prediction = DVector::from_vec(vec![1.0, 2.0]);
        let target = DVector::from_vec(vec![1.0, 2.0]);

        // Even with perfect prediction, smoothness penalty should add to loss
        let loss = loss_fn.compute_loss(&prediction, &target).unwrap();
        assert!(loss > 0.0);
    }
}