//! Training utilities for attention-based graph neural networks
//!
//! This module provides training infrastructure including:
//! - Loss functions (InfoNCE, contrastive, spectral regularization)
//! - Optimizers (SGD, Adam, AdamW)
//! - Curriculum learning schedulers
//! - Hard negative mining strategies

pub mod curriculum;
pub mod loss;
pub mod mining;
pub mod optimizer;

pub use curriculum::{CurriculumScheduler, CurriculumStage, DecayType, TemperatureAnnealing};
pub use loss::{InfoNCELoss, LocalContrastiveLoss, Loss, Reduction, SpectralRegularization};
pub use mining::{HardNegativeMiner, MiningStrategy, NegativeMiner};
pub use optimizer::{Adam, AdamW, Optimizer, SGD};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_training_components_integration() {
        // Test optimizer with loss
        let mut optimizer = Adam::new(128, 0.001);
        let loss = InfoNCELoss::new(0.07);

        let mut params = vec![0.5; 128];
        let anchor = vec![1.0; 128];
        let positive = vec![0.9; 128];
        let negatives: Vec<Vec<f32>> = (0..5).map(|_| vec![0.1; 128]).collect();
        let neg_refs: Vec<&[f32]> = negatives.iter().map(|v| v.as_slice()).collect();

        let (loss_val, gradients) = loss.compute_with_gradients(&anchor, &positive, &neg_refs);

        assert!(loss_val >= 0.0);
        assert_eq!(gradients.len(), anchor.len());

        optimizer.step(&mut params, &gradients);
    }
}
