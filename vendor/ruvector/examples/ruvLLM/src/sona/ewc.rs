//! EWC++ (Enhanced Elastic Weight Consolidation) for SONA
//!
//! Prevents catastrophic forgetting with:
//! - Online Fisher information estimation
//! - Multi-task memory with circular buffer
//! - Automatic task boundary detection
//! - Adaptive lambda scheduling

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// EWC++ configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EwcConfig {
    /// Number of parameters
    pub param_count: usize,
    /// Maximum tasks to remember
    pub max_tasks: usize,
    /// Initial lambda
    pub initial_lambda: f32,
    /// Minimum lambda
    pub min_lambda: f32,
    /// Maximum lambda
    pub max_lambda: f32,
    /// Fisher EMA decay factor
    pub fisher_ema_decay: f32,
    /// Task boundary detection threshold
    pub boundary_threshold: f32,
    /// Gradient history for boundary detection
    pub gradient_history_size: usize,
}

impl Default for EwcConfig {
    fn default() -> Self {
        // OPTIMIZED DEFAULTS based on @ruvector/sona v0.1.1 benchmarks:
        // - Lambda 2000 optimal for catastrophic forgetting prevention
        // - Higher max_lambda (15000) for aggressive protection when needed
        Self {
            param_count: 1000,
            max_tasks: 10,
            initial_lambda: 2000.0, // OPTIMIZED: Better forgetting prevention
            min_lambda: 100.0,
            max_lambda: 15000.0, // OPTIMIZED: Higher ceiling for multi-task
            fisher_ema_decay: 0.999,
            boundary_threshold: 2.0,
            gradient_history_size: 100,
        }
    }
}

/// Task-specific Fisher information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskFisher {
    /// Task ID
    pub task_id: usize,
    /// Fisher diagonal
    pub fisher: Vec<f32>,
    /// Optimal weights for this task
    pub optimal_weights: Vec<f32>,
    /// Task importance (for weighted consolidation)
    pub importance: f32,
}

/// EWC++ implementation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EwcPlusPlus {
    /// Configuration
    config: EwcConfig,
    /// Current Fisher information (online estimate)
    current_fisher: Vec<f32>,
    /// Current optimal weights
    current_weights: Vec<f32>,
    /// Task memory (circular buffer)
    task_memory: VecDeque<TaskFisher>,
    /// Current task ID
    current_task_id: usize,
    /// Current lambda
    lambda: f32,
    /// Gradient history for boundary detection
    gradient_history: VecDeque<Vec<f32>>,
    /// Running gradient mean
    gradient_mean: Vec<f32>,
    /// Running gradient variance
    gradient_var: Vec<f32>,
    /// Samples seen for current task
    samples_seen: u64,
}

impl EwcPlusPlus {
    /// Create new EWC++
    pub fn new(config: EwcConfig) -> Self {
        let param_count = config.param_count;
        let initial_lambda = config.initial_lambda;

        Self {
            config: config.clone(),
            current_fisher: vec![0.0; param_count],
            current_weights: vec![0.0; param_count],
            task_memory: VecDeque::with_capacity(config.max_tasks),
            current_task_id: 0,
            lambda: initial_lambda,
            gradient_history: VecDeque::with_capacity(config.gradient_history_size),
            gradient_mean: vec![0.0; param_count],
            gradient_var: vec![1.0; param_count],
            samples_seen: 0,
        }
    }

    /// Update Fisher information online using EMA
    pub fn update_fisher(&mut self, gradients: &[f32]) {
        if gradients.len() != self.config.param_count {
            return;
        }

        let decay = self.config.fisher_ema_decay;

        // Online Fisher update: F_t = decay * F_{t-1} + (1 - decay) * g^2
        for (i, &g) in gradients.iter().enumerate() {
            self.current_fisher[i] = decay * self.current_fisher[i] + (1.0 - decay) * g * g;
        }

        // Update gradient statistics for boundary detection
        self.update_gradient_stats(gradients);
        self.samples_seen += 1;
    }

    /// Update gradient statistics for boundary detection
    fn update_gradient_stats(&mut self, gradients: &[f32]) {
        // Store in history
        if self.gradient_history.len() >= self.config.gradient_history_size {
            self.gradient_history.pop_front();
        }
        self.gradient_history.push_back(gradients.to_vec());

        // Update running mean and variance (Welford's algorithm)
        let n = self.samples_seen as f32 + 1.0;

        for (i, &g) in gradients.iter().enumerate() {
            let delta = g - self.gradient_mean[i];
            self.gradient_mean[i] += delta / n;
            let delta2 = g - self.gradient_mean[i];
            self.gradient_var[i] += delta * delta2;
        }
    }

    /// Detect task boundary using distribution shift
    pub fn detect_task_boundary(&self, gradients: &[f32]) -> bool {
        if self.samples_seen < 50 || gradients.len() != self.config.param_count {
            return false;
        }

        // Compute z-score of current gradients vs running stats
        let mut z_score_sum = 0.0f32;
        let mut count = 0;

        for (i, &g) in gradients.iter().enumerate() {
            let var = self.gradient_var[i] / self.samples_seen as f32;
            if var > 1e-8 {
                let std = var.sqrt();
                let z = (g - self.gradient_mean[i]).abs() / std;
                z_score_sum += z;
                count += 1;
            }
        }

        if count == 0 {
            return false;
        }

        let avg_z = z_score_sum / count as f32;
        avg_z > self.config.boundary_threshold
    }

    /// Start new task - saves current Fisher to memory
    pub fn start_new_task(&mut self) {
        // Save current task's Fisher
        let task_fisher = TaskFisher {
            task_id: self.current_task_id,
            fisher: self.current_fisher.clone(),
            optimal_weights: self.current_weights.clone(),
            importance: 1.0,
        };

        // Add to circular buffer
        if self.task_memory.len() >= self.config.max_tasks {
            self.task_memory.pop_front();
        }
        self.task_memory.push_back(task_fisher);

        // Reset for new task
        self.current_task_id += 1;
        self.current_fisher.fill(0.0);
        self.gradient_history.clear();
        self.gradient_mean.fill(0.0);
        self.gradient_var.fill(1.0);
        self.samples_seen = 0;

        // Adapt lambda based on task count
        self.adapt_lambda();
    }

    /// Adapt lambda based on accumulated tasks
    fn adapt_lambda(&mut self) {
        let task_count = self.task_memory.len();
        if task_count == 0 {
            return;
        }

        // Increase lambda as more tasks accumulate (more to protect)
        let scale = 1.0 + 0.1 * task_count as f32;
        self.lambda = (self.config.initial_lambda * scale)
            .clamp(self.config.min_lambda, self.config.max_lambda);
    }

    /// Apply EWC++ constraints to gradients
    pub fn apply_constraints(&self, gradients: &[f32]) -> Vec<f32> {
        if gradients.len() != self.config.param_count {
            return gradients.to_vec();
        }

        let mut constrained = gradients.to_vec();

        // Apply constraint from each remembered task
        for task in &self.task_memory {
            for (i, g) in constrained.iter_mut().enumerate() {
                // Penalty: lambda * F_i * (w_i - w*_i)
                // Gradient of penalty: lambda * F_i
                // Project gradient to preserve important weights
                let importance = task.fisher[i] * task.importance;
                if importance > 1e-8 {
                    let penalty_grad = self.lambda * importance;
                    // Reduce gradient magnitude for important parameters
                    *g *= 1.0 / (1.0 + penalty_grad);
                }
            }
        }

        // Also apply current task's Fisher (online)
        for (i, g) in constrained.iter_mut().enumerate() {
            if self.current_fisher[i] > 1e-8 {
                let penalty_grad = self.lambda * self.current_fisher[i] * 0.1; // Lower weight for current
                *g *= 1.0 / (1.0 + penalty_grad);
            }
        }

        constrained
    }

    /// Compute EWC regularization loss
    pub fn regularization_loss(&self, current_weights: &[f32]) -> f32 {
        if current_weights.len() != self.config.param_count {
            return 0.0;
        }

        let mut loss = 0.0f32;

        for task in &self.task_memory {
            for i in 0..self.config.param_count {
                let diff = current_weights[i] - task.optimal_weights[i];
                loss += task.fisher[i] * diff * diff * task.importance;
            }
        }

        self.lambda * loss / 2.0
    }

    /// Update optimal weights reference
    pub fn set_optimal_weights(&mut self, weights: &[f32]) {
        if weights.len() == self.config.param_count {
            self.current_weights.copy_from_slice(weights);
        }
    }

    /// Consolidate all tasks (merge Fisher information)
    pub fn consolidate_all_tasks(&mut self) {
        if self.task_memory.is_empty() {
            return;
        }

        // Compute weighted average of Fisher matrices
        let mut consolidated_fisher = vec![0.0f32; self.config.param_count];
        let mut total_importance = 0.0f32;

        for task in &self.task_memory {
            for (i, &f) in task.fisher.iter().enumerate() {
                consolidated_fisher[i] += f * task.importance;
            }
            total_importance += task.importance;
        }

        if total_importance > 0.0 {
            for f in &mut consolidated_fisher {
                *f /= total_importance;
            }
        }

        // Store as single consolidated task
        let consolidated = TaskFisher {
            task_id: 0,
            fisher: consolidated_fisher,
            optimal_weights: self.current_weights.clone(),
            importance: total_importance,
        };

        self.task_memory.clear();
        self.task_memory.push_back(consolidated);
    }

    /// Get current lambda
    pub fn lambda(&self) -> f32 {
        self.lambda
    }

    /// Set lambda manually
    pub fn set_lambda(&mut self, lambda: f32) {
        self.lambda = lambda.clamp(self.config.min_lambda, self.config.max_lambda);
    }

    /// Get task count
    pub fn task_count(&self) -> usize {
        self.task_memory.len()
    }

    /// Get current task ID
    pub fn current_task_id(&self) -> usize {
        self.current_task_id
    }

    /// Get samples seen for current task
    pub fn samples_seen(&self) -> u64 {
        self.samples_seen
    }

    /// Get parameter importance scores
    pub fn importance_scores(&self) -> Vec<f32> {
        let mut scores = self.current_fisher.clone();

        for task in &self.task_memory {
            for (i, &f) in task.fisher.iter().enumerate() {
                scores[i] += f * task.importance;
            }
        }

        scores
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ewc_creation() {
        let config = EwcConfig {
            param_count: 100,
            ..Default::default()
        };
        let ewc = EwcPlusPlus::new(config);

        assert_eq!(ewc.task_count(), 0);
        assert_eq!(ewc.current_task_id(), 0);
    }

    #[test]
    fn test_fisher_update() {
        let config = EwcConfig {
            param_count: 10,
            ..Default::default()
        };
        let mut ewc = EwcPlusPlus::new(config);

        let gradients = vec![0.5; 10];
        ewc.update_fisher(&gradients);

        assert!(ewc.samples_seen() > 0);
        assert!(ewc.current_fisher.iter().any(|&f| f > 0.0));
    }

    #[test]
    fn test_task_boundary() {
        let config = EwcConfig {
            param_count: 10,
            gradient_history_size: 10,
            boundary_threshold: 2.0,
            ..Default::default()
        };
        let mut ewc = EwcPlusPlus::new(config);

        // Train on consistent gradients
        for _ in 0..60 {
            let gradients = vec![0.1; 10];
            ewc.update_fisher(&gradients);
        }

        // Normal gradient should not trigger boundary
        let normal = vec![0.1; 10];
        assert!(!ewc.detect_task_boundary(&normal));

        // Very different gradient might trigger boundary
        let different = vec![10.0; 10];
        // May or may not trigger depending on variance
    }

    #[test]
    fn test_constraint_application() {
        let config = EwcConfig {
            param_count: 5,
            ..Default::default()
        };
        let mut ewc = EwcPlusPlus::new(config);

        // Build up some Fisher information
        for _ in 0..10 {
            ewc.update_fisher(&vec![1.0; 5]);
        }
        ewc.start_new_task();

        // Apply constraints
        let gradients = vec![1.0; 5];
        let constrained = ewc.apply_constraints(&gradients);

        // Constrained gradients should be smaller
        let orig_mag: f32 = gradients.iter().map(|x| x.abs()).sum();
        let const_mag: f32 = constrained.iter().map(|x| x.abs()).sum();
        assert!(const_mag <= orig_mag);
    }

    #[test]
    fn test_regularization_loss() {
        let config = EwcConfig {
            param_count: 5,
            initial_lambda: 100.0,
            ..Default::default()
        };
        let mut ewc = EwcPlusPlus::new(config);

        // Set up optimal weights and Fisher
        ewc.set_optimal_weights(&vec![0.0; 5]);
        for _ in 0..10 {
            ewc.update_fisher(&vec![1.0; 5]);
        }
        ewc.start_new_task();

        // Loss should be zero when at optimal
        let at_optimal = ewc.regularization_loss(&vec![0.0; 5]);

        // Loss should be positive when deviated
        let deviated = ewc.regularization_loss(&vec![1.0; 5]);
        assert!(deviated > at_optimal);
    }

    #[test]
    fn test_task_consolidation() {
        let config = EwcConfig {
            param_count: 5,
            max_tasks: 5,
            ..Default::default()
        };
        let mut ewc = EwcPlusPlus::new(config);

        // Create multiple tasks
        for _ in 0..3 {
            for _ in 0..10 {
                ewc.update_fisher(&vec![1.0; 5]);
            }
            ewc.start_new_task();
        }

        assert_eq!(ewc.task_count(), 3);

        ewc.consolidate_all_tasks();
        assert_eq!(ewc.task_count(), 1);
    }

    #[test]
    fn test_lambda_adaptation() {
        let config = EwcConfig {
            param_count: 5,
            initial_lambda: 1000.0,
            ..Default::default()
        };
        let mut ewc = EwcPlusPlus::new(config);

        let initial_lambda = ewc.lambda();

        // Add tasks
        for _ in 0..5 {
            ewc.start_new_task();
        }

        // Lambda should have increased
        assert!(ewc.lambda() >= initial_lambda);
    }
}
