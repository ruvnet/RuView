//! Elastic Weight Consolidation (EWC) and Complementary Learning Systems
//!
//! Based on Kirkpatrick et al. 2017: "Overcoming catastrophic forgetting in neural networks"
//! - Protects task-important weights via Fisher Information diagonal
//! - Loss: L = L_new + (λ/2)Σ F_i(θ_i - θ*_i)²
//! - 45% reduction in forgetting with only 2× parameter overhead

use crate::{NervousSystemError, Result};
use parking_lot::RwLock;
use std::sync::Arc;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Experience sample for replay learning
#[derive(Debug, Clone)]
pub struct Experience {
    /// Input vector
    pub input: Vec<f32>,
    /// Target output vector
    pub target: Vec<f32>,
    /// Importance weight for prioritized replay
    pub importance: f32,
}

impl Experience {
    /// Create a new experience sample
    pub fn new(input: Vec<f32>, target: Vec<f32>, importance: f32) -> Self {
        Self {
            input,
            target,
            importance,
        }
    }
}

/// Elastic Weight Consolidation (EWC)
///
/// Prevents catastrophic forgetting by adding a quadratic penalty on weight changes,
/// weighted by the Fisher Information Matrix diagonal.
///
/// # Algorithm
///
/// 1. After learning task A, compute Fisher Information diagonal F_i = E[(∂L/∂θ_i)²]
/// 2. Store optimal parameters θ* from task A
/// 3. When learning task B, add EWC loss: L_EWC = (λ/2)Σ F_i(θ_i - θ*_i)²
/// 4. This protects important weights from task A while allowing task B learning
///
/// # Performance
///
/// - Fisher computation: O(n × m) for n parameters, m gradient samples
/// - EWC loss: O(n) for n parameters
/// - Memory: 2× parameter count (Fisher diagonal + optimal params)
#[derive(Debug, Clone)]
pub struct EWC {
    /// Fisher Information Matrix diagonal
    pub(crate) fisher_diag: Vec<f32>,
    /// Optimal parameters from previous task
    optimal_params: Vec<f32>,
    /// Regularization strength (λ)
    lambda: f32,
    /// Number of samples used for Fisher estimation
    num_samples: usize,
}

impl EWC {
    /// Create a new EWC instance
    ///
    /// # Arguments
    ///
    /// * `lambda` - Regularization strength. Higher values protect old tasks more strongly.
    ///              Typical range: 100-10000
    ///
    /// # Example
    ///
    /// ```
    /// use ruvector_nervous_system::plasticity::consolidate::EWC;
    ///
    /// let ewc = EWC::new(1000.0);
    /// ```
    pub fn new(lambda: f32) -> Self {
        Self {
            fisher_diag: Vec::new(),
            optimal_params: Vec::new(),
            lambda,
            num_samples: 0,
        }
    }

    /// Compute Fisher Information Matrix diagonal approximation
    ///
    /// Fisher Information: F_i = E[(∂L/∂θ_i)²]
    /// We approximate the expectation using empirical gradient samples.
    ///
    /// # Arguments
    ///
    /// * `params` - Current optimal parameters from completed task
    /// * `gradients` - Collection of gradient samples (outer vec = samples, inner vec = parameter gradients)
    ///
    /// # Performance
    ///
    /// - Time: O(n × m) for n parameters, m samples
    /// - Target: <100ms for 1M parameters with 50 samples
    ///
    /// # Example
    ///
    /// ```
    /// use ruvector_nervous_system::plasticity::consolidate::EWC;
    ///
    /// let mut ewc = EWC::new(1000.0);
    /// let params = vec![0.5; 100];
    /// let gradients: Vec<Vec<f32>> = vec![vec![0.1; 100]; 50];
    /// ewc.compute_fisher(&params, &gradients).unwrap();
    /// ```
    pub fn compute_fisher(&mut self, params: &[f32], gradients: &[Vec<f32>]) -> Result<()> {
        if gradients.is_empty() {
            return Err(NervousSystemError::InvalidGradients(
                "No gradient samples provided".to_string(),
            ));
        }

        let num_params = params.len();
        let num_samples = gradients.len();

        // Validate gradient dimensions
        for (_i, grad) in gradients.iter().enumerate() {
            if grad.len() != num_params {
                return Err(NervousSystemError::DimensionMismatch {
                    expected: num_params,
                    actual: grad.len(),
                });
            }
        }

        // Initialize Fisher diagonal
        self.fisher_diag = vec![0.0; num_params];
        self.num_samples = num_samples;

        // Compute diagonal Fisher Information: F_i = E[(∂L/∂θ_i)²]
        #[cfg(feature = "parallel")]
        {
            self.fisher_diag = (0..num_params)
                .into_par_iter()
                .map(|i| {
                    let sum_sq: f32 = gradients.iter().map(|g| g[i] * g[i]).sum();
                    sum_sq / num_samples as f32
                })
                .collect();
        }

        #[cfg(not(feature = "parallel"))]
        {
            for i in 0..num_params {
                let sum_sq: f32 = gradients.iter().map(|g| g[i] * g[i]).sum();
                self.fisher_diag[i] = sum_sq / num_samples as f32;
            }
        }

        // Store optimal parameters
        self.optimal_params = params.to_vec();

        Ok(())
    }

    /// Compute EWC regularization loss
    ///
    /// L_EWC = (λ/2)Σ F_i(θ_i - θ*_i)²
    ///
    /// # Arguments
    ///
    /// * `current_params` - Current parameter values during new task training
    ///
    /// # Returns
    ///
    /// Scalar EWC loss to add to the new task loss
    ///
    /// # Performance
    ///
    /// - Time: O(n) for n parameters
    /// - Target: <1ms for 1M parameters
    pub fn ewc_loss(&self, current_params: &[f32]) -> f32 {
        if self.fisher_diag.is_empty() {
            return 0.0; // No previous task, no penalty
        }

        #[cfg(feature = "parallel")]
        {
            let sum: f32 = current_params
                .par_iter()
                .zip(self.optimal_params.par_iter())
                .zip(self.fisher_diag.par_iter())
                .map(|((curr, opt), fisher)| {
                    let diff = curr - opt;
                    fisher * diff * diff
                })
                .sum();
            (self.lambda / 2.0) * sum
        }

        #[cfg(not(feature = "parallel"))]
        {
            let sum: f32 = current_params
                .iter()
                .zip(self.optimal_params.iter())
                .zip(self.fisher_diag.iter())
                .map(|((curr, opt), fisher)| {
                    let diff = curr - opt;
                    fisher * diff * diff
                })
                .sum();
            (self.lambda / 2.0) * sum
        }
    }

    /// Compute EWC gradient for backpropagation
    ///
    /// ∂L_EWC/∂θ_i = λ F_i (θ_i - θ*_i)
    ///
    /// # Arguments
    ///
    /// * `current_params` - Current parameter values
    ///
    /// # Returns
    ///
    /// Gradient vector to add to the new task gradient
    ///
    /// # Performance
    ///
    /// - Time: O(n) for n parameters
    /// - Target: <1ms for 1M parameters
    pub fn ewc_gradient(&self, current_params: &[f32]) -> Vec<f32> {
        if self.fisher_diag.is_empty() {
            return vec![0.0; current_params.len()];
        }

        #[cfg(feature = "parallel")]
        {
            current_params
                .par_iter()
                .zip(self.optimal_params.par_iter())
                .zip(self.fisher_diag.par_iter())
                .map(|((curr, opt), fisher)| self.lambda * fisher * (curr - opt))
                .collect()
        }

        #[cfg(not(feature = "parallel"))]
        {
            current_params
                .iter()
                .zip(self.optimal_params.iter())
                .zip(self.fisher_diag.iter())
                .map(|((curr, opt), fisher)| self.lambda * fisher * (curr - opt))
                .collect()
        }
    }

    /// Get the number of parameters
    pub fn num_params(&self) -> usize {
        self.fisher_diag.len()
    }

    /// Get the regularization strength
    pub fn lambda(&self) -> f32 {
        self.lambda
    }

    /// Get the number of samples used for Fisher estimation
    pub fn num_samples(&self) -> usize {
        self.num_samples
    }

    /// Check if EWC has been initialized with a previous task
    pub fn is_initialized(&self) -> bool {
        !self.fisher_diag.is_empty()
    }
}

/// Ring buffer for experience replay
#[derive(Debug)]
struct RingBuffer<T> {
    buffer: Vec<Option<T>>,
    capacity: usize,
    head: usize,
    size: usize,
}

impl<T> RingBuffer<T> {
    fn new(capacity: usize) -> Self {
        Self {
            buffer: (0..capacity).map(|_| None).collect(),
            capacity,
            head: 0,
            size: 0,
        }
    }

    fn push(&mut self, item: T) {
        self.buffer[self.head] = Some(item);
        self.head = (self.head + 1) % self.capacity;
        if self.size < self.capacity {
            self.size += 1;
        }
    }

    fn sample(&self, n: usize) -> Vec<&T> {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();

        let valid_items: Vec<&T> = self.buffer.iter().filter_map(|opt| opt.as_ref()).collect();

        valid_items
            .choose_multiple(&mut rng, n.min(valid_items.len()))
            .copied()
            .collect()
    }

    fn len(&self) -> usize {
        self.size
    }

    fn is_empty(&self) -> bool {
        self.size == 0
    }

    fn clear(&mut self) {
        self.buffer = (0..self.capacity).map(|_| None).collect();
        self.head = 0;
        self.size = 0;
    }
}

/// Complementary Learning Systems (CLS)
///
/// Implements the dual-system architecture inspired by hippocampus and neocortex:
/// - Hippocampus: Fast learning, temporary storage (ring buffer)
/// - Neocortex: Slow learning, permanent storage (parameters with EWC protection)
///
/// # Algorithm
///
/// 1. New experiences stored in hippocampal buffer (fast)
/// 2. Periodic consolidation: replay hippocampal memories to train neocortex (slow)
/// 3. EWC protects previously consolidated knowledge
/// 4. Interleaved training balances new and old task performance
///
/// # References
///
/// - McClelland et al. 1995: "Why there are complementary learning systems"
/// - Kumaran et al. 2016: "What learning systems do intelligent agents need?"
#[derive(Debug)]
pub struct ComplementaryLearning {
    /// Hippocampal buffer for fast learning
    hippocampus: Arc<RwLock<RingBuffer<Experience>>>,
    /// Neocortical parameters for slow consolidation
    neocortex_params: Vec<f32>,
    /// EWC for protecting consolidated knowledge
    ewc: EWC,
    /// Batch size for replay
    replay_batch_size: usize,
}

impl ComplementaryLearning {
    /// Create a new complementary learning system
    ///
    /// # Arguments
    ///
    /// * `param_size` - Number of parameters in the model
    /// * `buffer_size` - Hippocampal buffer capacity
    /// * `lambda` - EWC regularization strength
    ///
    /// # Example
    ///
    /// ```
    /// use ruvector_nervous_system::plasticity::consolidate::ComplementaryLearning;
    ///
    /// let cls = ComplementaryLearning::new(1000, 10000, 1000.0);
    /// ```
    pub fn new(param_size: usize, buffer_size: usize, lambda: f32) -> Self {
        Self {
            hippocampus: Arc::new(RwLock::new(RingBuffer::new(buffer_size))),
            neocortex_params: vec![0.0; param_size],
            ewc: EWC::new(lambda),
            replay_batch_size: 32,
        }
    }

    /// Store a new experience in hippocampal buffer
    ///
    /// # Arguments
    ///
    /// * `exp` - Experience to store
    pub fn store_experience(&self, exp: Experience) {
        self.hippocampus.write().push(exp);
    }

    /// Consolidate hippocampal memories into neocortex
    ///
    /// Replays experiences from hippocampus to train neocortical parameters
    /// with EWC protection of previously consolidated knowledge.
    ///
    /// # Arguments
    ///
    /// * `iterations` - Number of consolidation iterations
    /// * `lr` - Learning rate for consolidation
    ///
    /// # Returns
    ///
    /// Average loss over consolidation iterations
    pub fn consolidate(&mut self, iterations: usize, lr: f32) -> Result<f32> {
        let mut total_loss = 0.0;

        for _ in 0..iterations {
            // Sample from hippocampus
            let num_experiences = {
                let hippo = self.hippocampus.read();
                hippo.len().min(self.replay_batch_size)
            };

            if num_experiences == 0 {
                continue;
            }

            // Get experiences in separate scope to avoid borrow conflicts
            let sampled_experiences: Vec<Experience> = {
                let hippo = self.hippocampus.read();
                hippo
                    .sample(self.replay_batch_size)
                    .into_iter()
                    .map(|e| e.clone())
                    .collect()
            };

            // Compute gradients and update (simplified placeholder)
            // In practice, this would involve forward pass, loss computation, and backprop
            let mut batch_loss = 0.0;

            for exp in &sampled_experiences {
                // Placeholder: compute simple MSE loss
                let prediction = &self.neocortex_params[0..exp.target.len()];
                let loss: f32 = prediction
                    .iter()
                    .zip(exp.target.iter())
                    .map(|(p, t)| (p - t).powi(2))
                    .sum::<f32>()
                    / exp.target.len() as f32;

                batch_loss += loss * exp.importance;

                // Simple gradient descent update (placeholder)
                for i in 0..exp.target.len().min(self.neocortex_params.len()) {
                    let grad =
                        2.0 * (self.neocortex_params[i] - exp.target[i]) / exp.target.len() as f32;
                    let ewc_grad = if self.ewc.is_initialized() {
                        self.ewc.ewc_gradient(&self.neocortex_params)[i]
                    } else {
                        0.0
                    };
                    self.neocortex_params[i] -= lr * (grad + ewc_grad);
                }
            }

            total_loss += batch_loss / sampled_experiences.len() as f32;
        }

        Ok(total_loss / iterations as f32)
    }

    /// Interleaved training with new data and replay
    ///
    /// Balances learning new task with maintaining old task performance
    /// by interleaving new data with hippocampal replay.
    ///
    /// # Arguments
    ///
    /// * `new_data` - New experiences to learn
    /// * `lr` - Learning rate
    pub fn interleaved_training(&mut self, new_data: &[Experience], lr: f32) -> Result<()> {
        // Store new experiences in hippocampus
        for exp in new_data {
            self.store_experience(exp.clone());
        }

        // Interleave new data with replay
        let replay_ratio = 0.5; // 50% replay, 50% new data
        let num_replay = (new_data.len() as f32 * replay_ratio) as usize;

        if num_replay > 0 {
            self.consolidate(num_replay, lr)?;
        }

        Ok(())
    }

    /// Clear hippocampal buffer
    pub fn clear_hippocampus(&self) {
        self.hippocampus.write().clear();
    }

    /// Get number of experiences in hippocampus
    pub fn hippocampus_size(&self) -> usize {
        self.hippocampus.read().len()
    }

    /// Get neocortical parameters
    pub fn neocortex_params(&self) -> &[f32] {
        &self.neocortex_params
    }

    /// Update EWC Fisher information after task completion
    ///
    /// Call this after completing a task to protect learned weights
    pub fn update_ewc(&mut self, gradients: &[Vec<f32>]) -> Result<()> {
        self.ewc.compute_fisher(&self.neocortex_params, gradients)
    }
}

/// Reward-modulated consolidation
///
/// Implements biologically-inspired reward-gated memory consolidation.
/// High-reward experiences trigger stronger consolidation.
///
/// # Algorithm
///
/// 1. Track reward with exponential moving average: r(t+1) = (1-α)r(t) + αR
/// 2. Consolidate when reward exceeds threshold
/// 3. Modulate EWC lambda by reward magnitude
///
/// # References
///
/// - Gruber & Ranganath 2019: "How context affects memory consolidation"
/// - Murty et al. 2016: "Selective updating of working memory content"
#[derive(Debug)]
pub struct RewardConsolidation {
    /// EWC instance
    ewc: EWC,
    /// Reward trace (exponential moving average)
    reward_trace: f32,
    /// Time constant for reward decay
    tau_reward: f32,
    /// Consolidation threshold
    threshold: f32,
    /// Base lambda for EWC
    base_lambda: f32,
}

impl RewardConsolidation {
    /// Create a new reward-modulated consolidation system
    ///
    /// # Arguments
    ///
    /// * `base_lambda` - Base EWC regularization strength
    /// * `tau_reward` - Time constant for reward trace decay
    /// * `threshold` - Reward threshold for consolidation trigger
    pub fn new(base_lambda: f32, tau_reward: f32, threshold: f32) -> Self {
        Self {
            ewc: EWC::new(base_lambda),
            reward_trace: 0.0,
            tau_reward,
            threshold,
            base_lambda,
        }
    }

    /// Update reward trace with new reward signal
    ///
    /// # Arguments
    ///
    /// * `reward` - New reward value
    /// * `dt` - Time step
    pub fn modulate(&mut self, reward: f32, dt: f32) {
        let alpha = 1.0 - (-dt / self.tau_reward).exp();
        self.reward_trace = (1.0 - alpha) * self.reward_trace + alpha * reward;

        // Modulate lambda by reward magnitude
        let lambda_scale = 1.0 + (self.reward_trace / self.threshold).max(0.0);
        self.ewc.lambda = self.base_lambda * lambda_scale;
    }

    /// Check if reward exceeds consolidation threshold
    pub fn should_consolidate(&self) -> bool {
        self.reward_trace >= self.threshold
    }

    /// Get current reward trace
    pub fn reward_trace(&self) -> f32 {
        self.reward_trace
    }

    /// Get EWC instance
    pub fn ewc(&self) -> &EWC {
        &self.ewc
    }

    /// Get mutable EWC instance
    pub fn ewc_mut(&mut self) -> &mut EWC {
        &mut self.ewc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ewc_creation() {
        let ewc = EWC::new(1000.0);
        assert_eq!(ewc.lambda(), 1000.0);
        assert!(!ewc.is_initialized());
    }

    #[test]
    fn test_ewc_fisher_computation() {
        let mut ewc = EWC::new(1000.0);
        let params = vec![0.5; 10];
        let gradients: Vec<Vec<f32>> = vec![vec![0.1; 10]; 5];

        ewc.compute_fisher(&params, &gradients).unwrap();

        assert!(ewc.is_initialized());
        assert_eq!(ewc.num_params(), 10);
        assert_eq!(ewc.num_samples(), 5);
    }

    #[test]
    fn test_ewc_loss_gradient() {
        let mut ewc = EWC::new(1000.0);
        let params = vec![0.5; 10];
        let gradients: Vec<Vec<f32>> = vec![vec![0.1; 10]; 5];

        ewc.compute_fisher(&params, &gradients).unwrap();

        let new_params = vec![0.6; 10];
        let loss = ewc.ewc_loss(&new_params);
        let grad = ewc.ewc_gradient(&new_params);

        assert!(loss > 0.0);
        assert_eq!(grad.len(), 10);
        assert!(grad.iter().all(|&g| g > 0.0)); // All gradients should push towards optimal
    }

    #[test]
    fn test_complementary_learning() {
        let mut cls = ComplementaryLearning::new(10, 100, 1000.0);

        let exp = Experience::new(vec![1.0; 5], vec![0.5; 5], 1.0);
        cls.store_experience(exp);

        assert_eq!(cls.hippocampus_size(), 1);

        let result = cls.consolidate(10, 0.01);
        assert!(result.is_ok());
    }

    #[test]
    fn test_reward_consolidation() {
        let mut rc = RewardConsolidation::new(1000.0, 1.0, 0.5);

        assert!(!rc.should_consolidate());

        // Apply high reward
        rc.modulate(1.0, 0.1);
        assert!(rc.reward_trace() > 0.0);

        // Multiple high rewards should trigger consolidation
        for _ in 0..10 {
            rc.modulate(1.0, 0.1);
        }
        assert!(rc.should_consolidate());
    }

    #[test]
    fn test_ring_buffer() {
        let mut buffer: RingBuffer<i32> = RingBuffer::new(3);

        buffer.push(1);
        buffer.push(2);
        buffer.push(3);
        assert_eq!(buffer.len(), 3);

        buffer.push(4); // Should overwrite first
        assert_eq!(buffer.len(), 3);

        let samples = buffer.sample(2);
        assert_eq!(samples.len(), 2);
    }

    #[test]
    fn test_interleaved_training() {
        let mut cls = ComplementaryLearning::new(10, 100, 1000.0);

        let new_data = vec![
            Experience::new(vec![1.0; 5], vec![0.5; 5], 1.0),
            Experience::new(vec![0.8; 5], vec![0.4; 5], 1.0),
        ];

        let result = cls.interleaved_training(&new_data, 0.01);
        assert!(result.is_ok());
        assert!(cls.hippocampus_size() > 0);
    }
}
