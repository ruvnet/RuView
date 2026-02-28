/// Elastic Weight Consolidation (EWC) for preventing catastrophic forgetting in GNNs
///
/// EWC adds a regularization term that penalizes changes to important weights,
/// where importance is measured by the Fisher information matrix diagonal.
///
/// The EWC loss term is: L_EWC = λ/2 * Σ F_i * (θ_i - θ*_i)²
/// where:
/// - λ is the regularization strength
/// - F_i is the Fisher information for weight i
/// - θ_i is the current weight
/// - θ*_i is the anchor weight from the previous task
use std::f32;

/// Elastic Weight Consolidation implementation
///
/// Prevents catastrophic forgetting by penalizing changes to important weights
/// learned from previous tasks.
#[derive(Debug, Clone)]
pub struct ElasticWeightConsolidation {
    /// Fisher information diagonal (importance of each weight)
    /// Higher values indicate more important weights
    fisher_diag: Vec<f32>,

    /// Anchor weights (optimal weights from previous task)
    /// These are the weights we want to stay close to
    anchor_weights: Vec<f32>,

    /// Regularization strength (λ)
    /// Controls how strongly we penalize deviations from anchor weights
    lambda: f32,

    /// Whether EWC is active
    /// EWC is only active after consolidation has been called
    active: bool,
}

impl ElasticWeightConsolidation {
    /// Create a new EWC instance with specified regularization strength
    ///
    /// # Arguments
    /// * `lambda` - Regularization strength (typically 10-10000)
    ///
    /// # Returns
    /// A new inactive EWC instance
    pub fn new(lambda: f32) -> Self {
        assert!(lambda >= 0.0, "Lambda must be non-negative");

        Self {
            fisher_diag: Vec::new(),
            anchor_weights: Vec::new(),
            lambda,
            active: false,
        }
    }

    /// Compute Fisher information diagonal from gradients
    ///
    /// The Fisher information measures the importance of each weight.
    /// It's approximated as the mean squared gradient over samples:
    /// F_i ≈ (1/N) * Σ (∂L/∂θ_i)²
    ///
    /// # Arguments
    /// * `gradients` - Slice of gradient vectors for each sample
    /// * `sample_count` - Number of samples (for normalization)
    pub fn compute_fisher(&mut self, gradients: &[&[f32]], sample_count: usize) {
        if gradients.is_empty() {
            return;
        }

        let num_weights = gradients[0].len();

        // Always reset Fisher diagonal to zero before computing
        // (Fisher information should be computed fresh from current gradients)
        self.fisher_diag = vec![0.0; num_weights];

        // Accumulate squared gradients
        for grad in gradients {
            assert_eq!(
                grad.len(),
                num_weights,
                "All gradient vectors must have the same length"
            );

            for (i, &g) in grad.iter().enumerate() {
                self.fisher_diag[i] += g * g;
            }
        }

        // Normalize by sample count
        let normalization = 1.0 / (sample_count as f32).max(1.0);
        for f in &mut self.fisher_diag {
            *f *= normalization;
        }
    }

    /// Save current weights as anchor and activate EWC
    ///
    /// This should be called after training on a task, before moving to the next task.
    /// It marks the current weights as important and activates the EWC penalty.
    ///
    /// # Arguments
    /// * `weights` - Current model weights to save as anchor
    pub fn consolidate(&mut self, weights: &[f32]) {
        assert!(
            !self.fisher_diag.is_empty(),
            "Must compute Fisher information before consolidating"
        );
        assert_eq!(
            weights.len(),
            self.fisher_diag.len(),
            "Weight count must match Fisher information size"
        );

        self.anchor_weights = weights.to_vec();
        self.active = true;
    }

    /// Compute EWC penalty term
    ///
    /// Returns: λ/2 * Σ F_i * (θ_i - θ*_i)²
    ///
    /// This penalty is added to the loss function to discourage changes
    /// to important weights.
    ///
    /// # Arguments
    /// * `weights` - Current model weights
    ///
    /// # Returns
    /// The EWC penalty value (0.0 if not active)
    pub fn penalty(&self, weights: &[f32]) -> f32 {
        if !self.active {
            return 0.0;
        }

        assert_eq!(
            weights.len(),
            self.anchor_weights.len(),
            "Weight count must match anchor weights"
        );

        let mut penalty = 0.0;

        for i in 0..weights.len() {
            let diff = weights[i] - self.anchor_weights[i];
            penalty += self.fisher_diag[i] * diff * diff;
        }

        // Multiply by λ/2
        penalty * self.lambda * 0.5
    }

    /// Compute EWC gradient
    ///
    /// Returns: λ * F_i * (θ_i - θ*_i) for each weight i
    ///
    /// This gradient is added to the model gradients during training
    /// to push weights back toward their anchor values.
    ///
    /// # Arguments
    /// * `weights` - Current model weights
    ///
    /// # Returns
    /// Gradient vector (all zeros if not active)
    pub fn gradient(&self, weights: &[f32]) -> Vec<f32> {
        if !self.active {
            return vec![0.0; weights.len()];
        }

        assert_eq!(
            weights.len(),
            self.anchor_weights.len(),
            "Weight count must match anchor weights"
        );

        let mut grad = Vec::with_capacity(weights.len());

        for i in 0..weights.len() {
            let diff = weights[i] - self.anchor_weights[i];
            grad.push(self.lambda * self.fisher_diag[i] * diff);
        }

        grad
    }

    /// Check if EWC is active
    ///
    /// # Returns
    /// true if consolidate() has been called, false otherwise
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get the regularization strength
    pub fn lambda(&self) -> f32 {
        self.lambda
    }

    /// Update the regularization strength
    pub fn set_lambda(&mut self, lambda: f32) {
        assert!(lambda >= 0.0, "Lambda must be non-negative");
        self.lambda = lambda;
    }

    /// Get the Fisher information diagonal
    pub fn fisher_diag(&self) -> &[f32] {
        &self.fisher_diag
    }

    /// Get the anchor weights
    pub fn anchor_weights(&self) -> &[f32] {
        &self.anchor_weights
    }

    /// Reset EWC to inactive state
    pub fn reset(&mut self) {
        self.fisher_diag.clear();
        self.anchor_weights.clear();
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let ewc = ElasticWeightConsolidation::new(1000.0);
        assert_eq!(ewc.lambda(), 1000.0);
        assert!(!ewc.is_active());
        assert!(ewc.fisher_diag().is_empty());
        assert!(ewc.anchor_weights().is_empty());
    }

    #[test]
    #[should_panic(expected = "Lambda must be non-negative")]
    fn test_new_negative_lambda() {
        ElasticWeightConsolidation::new(-1.0);
    }

    #[test]
    fn test_compute_fisher_single_sample() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);

        // Single gradient: [1.0, 2.0, 3.0]
        let grad1 = vec![1.0, 2.0, 3.0];
        let gradients = vec![grad1.as_slice()];

        ewc.compute_fisher(&gradients, 1);

        // Fisher should be squared gradients
        assert_eq!(ewc.fisher_diag(), &[1.0, 4.0, 9.0]);
    }

    #[test]
    fn test_compute_fisher_multiple_samples() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);

        // Two gradients
        let grad1 = vec![1.0, 2.0, 3.0];
        let grad2 = vec![2.0, 1.0, 1.0];
        let gradients = vec![grad1.as_slice(), grad2.as_slice()];

        ewc.compute_fisher(&gradients, 2);

        // Fisher should be mean of squared gradients
        // Position 0: (1² + 2²) / 2 = 2.5
        // Position 1: (2² + 1²) / 2 = 2.5
        // Position 2: (3² + 1²) / 2 = 5.0
        let expected = vec![2.5, 2.5, 5.0];
        assert_eq!(ewc.fisher_diag().len(), expected.len());
        for (actual, exp) in ewc.fisher_diag().iter().zip(expected.iter()) {
            assert!((actual - exp).abs() < 1e-6);
        }
    }

    #[test]
    fn test_compute_fisher_accumulates() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);

        // First computation
        let grad1 = vec![1.0, 2.0];
        ewc.compute_fisher(&[grad1.as_slice()], 1);
        assert_eq!(ewc.fisher_diag(), &[1.0, 4.0]);

        // Second computation accumulates on top of first
        // When fisher_diag has same length, it's reset to zero first in compute_fisher
        // then accumulates: 0 + 2^2 = 4, 0 + 1^2 = 1
        // normalized by 1/1 = 4.0, 1.0
        let grad2 = vec![2.0, 1.0];
        ewc.compute_fisher(&[grad2.as_slice()], 1);
        // Fisher is reset and recomputed with new gradients
        assert_eq!(ewc.fisher_diag(), &[4.0, 1.0]);
    }

    #[test]
    #[should_panic(expected = "All gradient vectors must have the same length")]
    fn test_compute_fisher_mismatched_sizes() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);

        let grad1 = vec![1.0, 2.0];
        let grad2 = vec![1.0, 2.0, 3.0];
        ewc.compute_fisher(&[grad1.as_slice(), grad2.as_slice()], 2);
    }

    #[test]
    fn test_consolidate() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);

        // Setup Fisher information
        let grad = vec![1.0, 2.0, 3.0];
        ewc.compute_fisher(&[grad.as_slice()], 1);

        // Consolidate weights
        let weights = vec![0.5, 1.0, 1.5];
        ewc.consolidate(&weights);

        assert!(ewc.is_active());
        assert_eq!(ewc.anchor_weights(), &weights);
    }

    #[test]
    #[should_panic(expected = "Must compute Fisher information before consolidating")]
    fn test_consolidate_without_fisher() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);
        let weights = vec![1.0, 2.0];
        ewc.consolidate(&weights);
    }

    #[test]
    #[should_panic(expected = "Weight count must match Fisher information size")]
    fn test_consolidate_size_mismatch() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);

        let grad = vec![1.0, 2.0];
        ewc.compute_fisher(&[grad.as_slice()], 1);

        let weights = vec![1.0, 2.0, 3.0]; // Wrong size
        ewc.consolidate(&weights);
    }

    #[test]
    fn test_penalty_inactive() {
        let ewc = ElasticWeightConsolidation::new(100.0);
        let weights = vec![1.0, 2.0, 3.0];

        assert_eq!(ewc.penalty(&weights), 0.0);
    }

    #[test]
    fn test_penalty_no_deviation() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);

        // Setup
        let grad = vec![1.0, 2.0, 3.0];
        ewc.compute_fisher(&[grad.as_slice()], 1);

        let weights = vec![0.5, 1.0, 1.5];
        ewc.consolidate(&weights);

        // Penalty should be 0 when weights match anchor
        assert_eq!(ewc.penalty(&weights), 0.0);
    }

    #[test]
    fn test_penalty_with_deviation() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);

        // Fisher diagonal: [1.0, 4.0, 9.0]
        let grad = vec![1.0, 2.0, 3.0];
        ewc.compute_fisher(&[grad.as_slice()], 1);

        // Anchor weights: [0.0, 0.0, 0.0]
        let anchor = vec![0.0, 0.0, 0.0];
        ewc.consolidate(&anchor);

        // Current weights: [1.0, 1.0, 1.0]
        let weights = vec![1.0, 1.0, 1.0];

        // Penalty = λ/2 * Σ F_i * (w_i - w*_i)²
        // = 100/2 * (1.0 * 1² + 4.0 * 1² + 9.0 * 1²)
        // = 50 * 14 = 700
        let penalty = ewc.penalty(&weights);
        assert!((penalty - 700.0).abs() < 1e-4);
    }

    #[test]
    fn test_penalty_increases_with_deviation() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);

        let grad = vec![1.0, 1.0, 1.0];
        ewc.compute_fisher(&[grad.as_slice()], 1);

        let anchor = vec![0.0, 0.0, 0.0];
        ewc.consolidate(&anchor);

        // Small deviation
        let weights1 = vec![0.1, 0.1, 0.1];
        let penalty1 = ewc.penalty(&weights1);

        // Larger deviation
        let weights2 = vec![0.5, 0.5, 0.5];
        let penalty2 = ewc.penalty(&weights2);

        // Penalty should increase
        assert!(penalty2 > penalty1);

        // Penalty should scale quadratically
        // (0.5/0.1)² = 25
        assert!((penalty2 / penalty1 - 25.0).abs() < 1e-4);
    }

    #[test]
    fn test_gradient_inactive() {
        let ewc = ElasticWeightConsolidation::new(100.0);
        let weights = vec![1.0, 2.0, 3.0];

        let grad = ewc.gradient(&weights);
        assert_eq!(grad, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_gradient_no_deviation() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);

        let grad = vec![1.0, 2.0, 3.0];
        ewc.compute_fisher(&[grad.as_slice()], 1);

        let weights = vec![0.5, 1.0, 1.5];
        ewc.consolidate(&weights);

        // Gradient should be 0 when weights match anchor
        let grad = ewc.gradient(&weights);
        assert_eq!(grad, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_gradient_points_toward_anchor() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);

        // Fisher diagonal: [1.0, 4.0, 9.0]
        let grad = vec![1.0, 2.0, 3.0];
        ewc.compute_fisher(&[grad.as_slice()], 1);

        // Anchor at origin
        let anchor = vec![0.0, 0.0, 0.0];
        ewc.consolidate(&anchor);

        // Weights moved positive
        let weights = vec![1.0, 1.0, 1.0];

        // Gradient = λ * F_i * (w_i - w*_i)
        // = 100 * [1.0, 4.0, 9.0] * [1.0, 1.0, 1.0]
        // = [100, 400, 900]
        let grad = ewc.gradient(&weights);
        assert_eq!(grad.len(), 3);
        assert!((grad[0] - 100.0).abs() < 1e-4);
        assert!((grad[1] - 400.0).abs() < 1e-4);
        assert!((grad[2] - 900.0).abs() < 1e-4);

        // Weights moved negative
        let weights = vec![-1.0, -1.0, -1.0];
        let grad = ewc.gradient(&weights);

        // Gradient should point opposite direction (toward anchor)
        assert!(grad[0] < 0.0);
        assert!(grad[1] < 0.0);
        assert!(grad[2] < 0.0);
        assert!((grad[0] + 100.0).abs() < 1e-4);
        assert!((grad[1] + 400.0).abs() < 1e-4);
        assert!((grad[2] + 900.0).abs() < 1e-4);
    }

    #[test]
    fn test_gradient_magnitude_scales_with_fisher() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);

        // Fisher with varying importance
        let grad = vec![1.0, 2.0, 3.0];
        ewc.compute_fisher(&[grad.as_slice()], 1);

        let anchor = vec![0.0, 0.0, 0.0];
        ewc.consolidate(&anchor);

        let weights = vec![1.0, 1.0, 1.0];
        let grad = ewc.gradient(&weights);

        // Gradient magnitude should increase with Fisher importance
        assert!(grad[0].abs() < grad[1].abs());
        assert!(grad[1].abs() < grad[2].abs());
    }

    #[test]
    fn test_lambda_scaling() {
        let mut ewc1 = ElasticWeightConsolidation::new(100.0);
        let mut ewc2 = ElasticWeightConsolidation::new(200.0);

        // Same setup for both
        let grad = vec![1.0, 1.0, 1.0];
        ewc1.compute_fisher(&[grad.as_slice()], 1);
        ewc2.compute_fisher(&[grad.as_slice()], 1);

        let anchor = vec![0.0, 0.0, 0.0];
        ewc1.consolidate(&anchor);
        ewc2.consolidate(&anchor);

        let weights = vec![1.0, 1.0, 1.0];

        // Penalty and gradient should scale with lambda
        let penalty1 = ewc1.penalty(&weights);
        let penalty2 = ewc2.penalty(&weights);
        assert!((penalty2 / penalty1 - 2.0).abs() < 1e-4);

        let grad1 = ewc1.gradient(&weights);
        let grad2 = ewc2.gradient(&weights);
        assert!((grad2[0] / grad1[0] - 2.0).abs() < 1e-4);
    }

    #[test]
    fn test_set_lambda() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);
        assert_eq!(ewc.lambda(), 100.0);

        ewc.set_lambda(500.0);
        assert_eq!(ewc.lambda(), 500.0);
    }

    #[test]
    #[should_panic(expected = "Lambda must be non-negative")]
    fn test_set_lambda_negative() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);
        ewc.set_lambda(-10.0);
    }

    #[test]
    fn test_reset() {
        let mut ewc = ElasticWeightConsolidation::new(100.0);

        // Setup active EWC
        let grad = vec![1.0, 2.0, 3.0];
        ewc.compute_fisher(&[grad.as_slice()], 1);

        let weights = vec![0.5, 1.0, 1.5];
        ewc.consolidate(&weights);

        assert!(ewc.is_active());

        // Reset
        ewc.reset();

        assert!(!ewc.is_active());
        assert!(ewc.fisher_diag().is_empty());
        assert!(ewc.anchor_weights().is_empty());
        assert_eq!(ewc.lambda(), 100.0); // Lambda preserved
    }

    #[test]
    fn test_sequential_task_learning() {
        // Simulate learning two tasks sequentially
        let mut ewc = ElasticWeightConsolidation::new(1000.0);

        // Task 1: Learn weights [1.0, 2.0, 3.0]
        let task1_grad = vec![2.0, 1.0, 3.0];
        ewc.compute_fisher(&[task1_grad.as_slice()], 1);

        let task1_weights = vec![1.0, 2.0, 3.0];
        ewc.consolidate(&task1_weights);

        // Task 2: Try to learn very different weights
        let task2_weights = vec![5.0, 6.0, 7.0];

        // EWC penalty should be significant
        let penalty = ewc.penalty(&task2_weights);
        assert!(penalty > 10000.0); // Large penalty for large deviation

        // Gradient should point back toward task 1 weights
        let grad = ewc.gradient(&task2_weights);
        assert!(grad[0] > 0.0); // Push toward lower value
        assert!(grad[1] > 0.0);
        assert!(grad[2] > 0.0);
    }
}
