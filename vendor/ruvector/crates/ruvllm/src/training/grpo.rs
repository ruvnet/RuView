//! # GRPO (Group Relative Policy Optimization) Implementation
//!
//! GRPO is a reinforcement learning algorithm that improves tool calling
//! by computing relative advantages within groups without requiring a critic network.
//!
//! ## Algorithm Overview
//!
//! GRPO uses the following update rule:
//!
//! ```text
//! L = -E[A_rel * log(π(a|s))] + β * KL(π || π_ref)
//! ```
//!
//! Where:
//! - `A_rel` is the relative advantage within a group
//! - `β` is the KL penalty coefficient
//! - `π_ref` is the reference policy
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::training::{GrpoOptimizer, GrpoConfig};
//!
//! let config = GrpoConfig::default();
//! let mut optimizer = GrpoOptimizer::new(config);
//!
//! // Compute group advantages
//! let rewards = vec![0.8, 0.6, 0.9, 0.5];
//! let advantages = optimizer.compute_relative_advantages(&rewards);
//!
//! // Perform policy update
//! let update = optimizer.grpo_update(&log_probs, &advantages, &ref_log_probs)?;
//! ```

use crate::error::{Result, RuvLLMError};
use ndarray::{Array1, Array2};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

/// Configuration for GRPO optimizer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpoConfig {
    /// Number of samples per group for relative advantage computation
    pub group_size: usize,
    /// Learning rate for policy updates
    pub learning_rate: f32,
    /// KL divergence penalty coefficient (β)
    pub kl_coefficient: f32,
    /// Minimum KL coefficient (adaptive)
    pub kl_min: f32,
    /// Maximum KL coefficient (adaptive)
    pub kl_max: f32,
    /// Target KL divergence for adaptive coefficient
    pub kl_target: f32,
    /// Entropy bonus coefficient
    pub entropy_coefficient: f32,
    /// Gradient clipping norm
    pub max_grad_norm: f32,
    /// Discount factor for rewards
    pub gamma: f32,
    /// GAE lambda for advantage estimation
    pub gae_lambda: f32,
    /// Value function coefficient in combined loss
    pub value_coef: f32,
    /// Enable adaptive KL coefficient
    pub adaptive_kl: bool,
    /// Number of update steps
    pub update_epochs: usize,
    /// Mini-batch size for updates
    pub mini_batch_size: usize,
    /// Clip range for policy ratio
    pub clip_range: f32,
    /// Enable reward normalization
    pub normalize_rewards: bool,
    /// Enable advantage normalization
    pub normalize_advantages: bool,
}

impl Default for GrpoConfig {
    fn default() -> Self {
        Self {
            group_size: 8,
            learning_rate: 1e-5,
            kl_coefficient: 0.02,
            kl_min: 0.001,
            kl_max: 0.1,
            kl_target: 0.01,
            entropy_coefficient: 0.01,
            max_grad_norm: 1.0,
            gamma: 0.99,
            gae_lambda: 0.95,
            value_coef: 0.5,
            adaptive_kl: true,
            update_epochs: 4,
            mini_batch_size: 32,
            clip_range: 0.2,
            normalize_rewards: true,
            normalize_advantages: true,
        }
    }
}

impl GrpoConfig {
    /// Create config optimized for tool use fine-tuning
    pub fn for_tool_use() -> Self {
        Self {
            group_size: 4,
            learning_rate: 5e-6,
            kl_coefficient: 0.05,
            kl_target: 0.02,
            entropy_coefficient: 0.005,
            update_epochs: 2,
            mini_batch_size: 16,
            clip_range: 0.15,
            ..Default::default()
        }
    }

    /// Create config for aggressive exploration
    pub fn exploration() -> Self {
        Self {
            entropy_coefficient: 0.05,
            kl_coefficient: 0.01,
            clip_range: 0.3,
            ..Default::default()
        }
    }

    /// Create config for stable fine-tuning
    pub fn stable() -> Self {
        Self {
            learning_rate: 1e-6,
            kl_coefficient: 0.1,
            clip_range: 0.1,
            update_epochs: 2,
            ..Default::default()
        }
    }
}

/// Experience sample for GRPO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpoSample {
    /// State representation (embedding)
    pub state: Vec<f32>,
    /// Action index (tool selection)
    pub action: usize,
    /// Log probability of the action
    pub log_prob: f32,
    /// Reference policy log probability
    pub ref_log_prob: f32,
    /// Reward received
    pub reward: f32,
    /// Whether this is a terminal state
    pub done: bool,
    /// Value estimate (optional)
    pub value: Option<f32>,
    /// Tool name for this action
    pub tool_name: String,
    /// Parameters used
    pub parameters: Option<serde_json::Value>,
}

/// Group of samples for relative advantage computation
#[derive(Debug, Clone)]
pub struct SampleGroup {
    /// Samples in this group
    pub samples: Vec<GrpoSample>,
    /// Group identifier
    pub group_id: u64,
    /// Task context for this group
    pub task_context: String,
}

impl SampleGroup {
    /// Create a new sample group
    pub fn new(samples: Vec<GrpoSample>, group_id: u64, task_context: String) -> Self {
        Self {
            samples,
            group_id,
            task_context,
        }
    }

    /// Get the number of samples in this group
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Check if the group is empty
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// GRPO policy update result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpoUpdateResult {
    /// Policy loss
    pub policy_loss: f32,
    /// KL divergence from reference policy
    pub kl_divergence: f32,
    /// Entropy of the policy
    pub entropy: f32,
    /// Combined loss
    pub total_loss: f32,
    /// Gradient norm
    pub grad_norm: f32,
    /// Number of samples processed
    pub num_samples: usize,
    /// Average advantage
    pub avg_advantage: f32,
    /// Clip fraction (how often clipping occurred)
    pub clip_fraction: f32,
    /// Updated KL coefficient (if adaptive)
    pub kl_coef: f32,
}

/// Statistics for GRPO training
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GrpoStats {
    /// Total updates performed
    pub total_updates: u64,
    /// Total samples processed
    pub total_samples: u64,
    /// Average reward
    pub avg_reward: f32,
    /// Average policy loss
    pub avg_policy_loss: f32,
    /// Average KL divergence
    pub avg_kl_divergence: f32,
    /// Average entropy
    pub avg_entropy: f32,
    /// Current KL coefficient
    pub current_kl_coef: f32,
    /// Recent rewards (for tracking)
    pub reward_history: Vec<f32>,
}

/// GRPO Optimizer for tool use fine-tuning
pub struct GrpoOptimizer {
    /// Configuration
    config: GrpoConfig,
    /// Current KL coefficient (adaptive)
    kl_coef: f32,
    /// Experience buffer
    experience_buffer: RwLock<VecDeque<GrpoSample>>,
    /// Group buffer for computing relative advantages
    group_buffer: RwLock<Vec<SampleGroup>>,
    /// Update counter
    update_count: AtomicU64,
    /// Training statistics
    stats: RwLock<GrpoStats>,
    /// Running mean of rewards
    reward_mean: f32,
    /// Running std of rewards
    reward_std: f32,
    /// Running mean of advantages
    advantage_mean: f32,
    /// Running std of advantages
    advantage_std: f32,
}

impl GrpoOptimizer {
    /// Create a new GRPO optimizer
    pub fn new(config: GrpoConfig) -> Self {
        let kl_coef = config.kl_coefficient;
        Self {
            config,
            kl_coef,
            experience_buffer: RwLock::new(VecDeque::with_capacity(10000)),
            group_buffer: RwLock::new(Vec::new()),
            update_count: AtomicU64::new(0),
            stats: RwLock::new(GrpoStats::default()),
            reward_mean: 0.0,
            reward_std: 1.0,
            advantage_mean: 0.0,
            advantage_std: 1.0,
        }
    }

    /// Compute relative advantages within a group
    ///
    /// This is the key insight of GRPO: instead of using absolute advantages,
    /// we compute advantages relative to the mean within each group.
    pub fn compute_relative_advantages(&self, rewards: &[f32]) -> Vec<f32> {
        if rewards.is_empty() {
            return Vec::new();
        }

        // Compute group mean
        let mean = rewards.iter().sum::<f32>() / rewards.len() as f32;

        // Compute group std
        let variance =
            rewards.iter().map(|r| (r - mean).powi(2)).sum::<f32>() / rewards.len() as f32;
        let std = variance.sqrt().max(1e-8);

        // Compute relative advantages
        rewards.iter().map(|r| (r - mean) / std).collect()
    }

    /// Compute generalized advantage estimation (GAE)
    pub fn compute_gae(
        &self,
        rewards: &[f32],
        values: &[f32],
        dones: &[bool],
        next_value: f32,
    ) -> Vec<f32> {
        let n = rewards.len();
        if n == 0 {
            return Vec::new();
        }

        let mut advantages = vec![0.0f32; n];
        let mut last_gae = 0.0f32;

        for t in (0..n).rev() {
            let next_val = if t == n - 1 {
                next_value
            } else {
                values[t + 1]
            };

            let mask = if dones[t] { 0.0 } else { 1.0 };

            let delta = rewards[t] + self.config.gamma * next_val * mask - values[t];
            last_gae = delta + self.config.gamma * self.config.gae_lambda * mask * last_gae;
            advantages[t] = last_gae;
        }

        advantages
    }

    /// Perform GRPO policy update
    ///
    /// # Arguments
    ///
    /// * `log_probs` - Log probabilities under current policy
    /// * `advantages` - Relative advantages for each sample
    /// * `ref_log_probs` - Log probabilities under reference policy
    ///
    /// # Returns
    ///
    /// Update result with loss and statistics
    pub fn grpo_update(
        &mut self,
        log_probs: &[f32],
        advantages: &[f32],
        ref_log_probs: &[f32],
    ) -> Result<GrpoUpdateResult> {
        if log_probs.len() != advantages.len() || log_probs.len() != ref_log_probs.len() {
            return Err(RuvLLMError::InvalidOperation(
                "GRPO update: array lengths must match".to_string(),
            ));
        }

        let n = log_probs.len();
        if n == 0 {
            return Err(RuvLLMError::InvalidOperation(
                "GRPO update: no samples provided".to_string(),
            ));
        }

        // Normalize advantages if configured
        let normalized_advantages = if self.config.normalize_advantages {
            self.normalize_advantages(advantages)
        } else {
            advantages.to_vec()
        };

        // Compute policy ratio
        let ratios: Vec<f32> = log_probs
            .iter()
            .zip(ref_log_probs.iter())
            .map(|(lp, rlp)| (lp - rlp).exp())
            .collect();

        // Compute clipped surrogate loss (PPO-style clipping)
        let mut policy_loss = 0.0f32;
        let mut clip_count = 0;
        for (ratio, adv) in ratios.iter().zip(normalized_advantages.iter()) {
            let surr1 = ratio * adv;
            let surr2 =
                ratio.clamp(1.0 - self.config.clip_range, 1.0 + self.config.clip_range) * adv;

            policy_loss -= surr1.min(surr2);

            // Count clips
            if *ratio < 1.0 - self.config.clip_range || *ratio > 1.0 + self.config.clip_range {
                clip_count += 1;
            }
        }
        policy_loss /= n as f32;

        // Compute KL divergence: D_KL(π || π_ref) = E[log(π/π_ref)]
        let kl_divergence: f32 = log_probs
            .iter()
            .zip(ref_log_probs.iter())
            .map(|(lp, rlp)| lp - rlp)
            .sum::<f32>()
            / n as f32;

        // Compute entropy: H(π) = -E[log π]
        let entropy = -log_probs.iter().sum::<f32>() / n as f32;

        // Compute total loss
        let kl_penalty = self.kl_coef * kl_divergence;
        let entropy_bonus = self.config.entropy_coefficient * entropy;
        let total_loss = policy_loss + kl_penalty - entropy_bonus;

        // Adaptive KL coefficient
        if self.config.adaptive_kl {
            self.adapt_kl_coefficient(kl_divergence);
        }

        // Compute gradient norm (simplified - actual gradient computation would be different)
        let grad_norm = total_loss.abs().sqrt();

        // Update statistics
        let update_count = self.update_count.fetch_add(1, Ordering::SeqCst);
        {
            let mut stats = self.stats.write();
            stats.total_updates = update_count + 1;
            stats.total_samples += n as u64;
            stats.avg_policy_loss = (stats.avg_policy_loss * 0.99) + (policy_loss * 0.01);
            stats.avg_kl_divergence = (stats.avg_kl_divergence * 0.99) + (kl_divergence * 0.01);
            stats.avg_entropy = (stats.avg_entropy * 0.99) + (entropy * 0.01);
            stats.current_kl_coef = self.kl_coef;
        }

        Ok(GrpoUpdateResult {
            policy_loss,
            kl_divergence,
            entropy,
            total_loss,
            grad_norm,
            num_samples: n,
            avg_advantage: normalized_advantages.iter().sum::<f32>() / n as f32,
            clip_fraction: clip_count as f32 / n as f32,
            kl_coef: self.kl_coef,
        })
    }

    /// Adapt KL coefficient based on observed KL divergence
    fn adapt_kl_coefficient(&mut self, observed_kl: f32) {
        if observed_kl > self.config.kl_target * 1.5 {
            // KL too high, increase penalty
            self.kl_coef = (self.kl_coef * 1.5).min(self.config.kl_max);
        } else if observed_kl < self.config.kl_target * 0.5 {
            // KL too low, decrease penalty (allow more exploration)
            self.kl_coef = (self.kl_coef / 1.5).max(self.config.kl_min);
        }
    }

    /// Normalize advantages using running statistics
    fn normalize_advantages(&self, advantages: &[f32]) -> Vec<f32> {
        if advantages.is_empty() {
            return Vec::new();
        }

        let mean = advantages.iter().sum::<f32>() / advantages.len() as f32;
        let variance =
            advantages.iter().map(|a| (a - mean).powi(2)).sum::<f32>() / advantages.len() as f32;
        let std = variance.sqrt().max(1e-8);

        advantages.iter().map(|a| (a - mean) / std).collect()
    }

    /// Add experience sample to buffer
    pub fn add_experience(&self, sample: GrpoSample) {
        let mut buffer = self.experience_buffer.write();
        if buffer.len() >= 10000 {
            buffer.pop_front();
        }
        buffer.push_back(sample);
    }

    /// Add a group of samples
    pub fn add_group(&self, group: SampleGroup) {
        let mut groups = self.group_buffer.write();
        groups.push(group);
    }

    /// Process buffered groups and compute updates
    pub fn process_groups(&mut self) -> Result<Vec<GrpoUpdateResult>> {
        let groups = {
            let mut buffer = self.group_buffer.write();
            std::mem::take(&mut *buffer)
        };

        let mut results = Vec::new();

        for group in groups {
            if group.samples.is_empty() {
                continue;
            }

            // Extract data from group
            let rewards: Vec<f32> = group.samples.iter().map(|s| s.reward).collect();
            let log_probs: Vec<f32> = group.samples.iter().map(|s| s.log_prob).collect();
            let ref_log_probs: Vec<f32> = group.samples.iter().map(|s| s.ref_log_prob).collect();

            // Compute relative advantages
            let advantages = self.compute_relative_advantages(&rewards);

            // Perform update
            let result = self.grpo_update(&log_probs, &advantages, &ref_log_probs)?;
            results.push(result);
        }

        Ok(results)
    }

    /// Get current statistics
    pub fn stats(&self) -> GrpoStats {
        self.stats.read().clone()
    }

    /// Get configuration
    pub fn config(&self) -> &GrpoConfig {
        &self.config
    }

    /// Get current KL coefficient
    pub fn kl_coefficient(&self) -> f32 {
        self.kl_coef
    }

    /// Reset the optimizer state
    pub fn reset(&mut self) {
        self.kl_coef = self.config.kl_coefficient;
        self.experience_buffer.write().clear();
        self.group_buffer.write().clear();
        self.update_count.store(0, Ordering::SeqCst);
        *self.stats.write() = GrpoStats::default();
        self.reward_mean = 0.0;
        self.reward_std = 1.0;
        self.advantage_mean = 0.0;
        self.advantage_std = 1.0;
    }

    /// Compute returns from rewards
    pub fn compute_returns(&self, rewards: &[f32], dones: &[bool]) -> Vec<f32> {
        let n = rewards.len();
        if n == 0 {
            return Vec::new();
        }

        let mut returns = vec![0.0f32; n];
        let mut running_return = 0.0f32;

        for t in (0..n).rev() {
            if dones[t] {
                running_return = 0.0;
            }
            running_return = rewards[t] + self.config.gamma * running_return;
            returns[t] = running_return;
        }

        returns
    }
}

/// Batch of samples for mini-batch training
#[derive(Debug, Clone)]
pub struct GrpoBatch {
    /// States (embeddings)
    pub states: Array2<f32>,
    /// Actions (tool indices)
    pub actions: Vec<usize>,
    /// Log probabilities
    pub log_probs: Array1<f32>,
    /// Reference log probabilities
    pub ref_log_probs: Array1<f32>,
    /// Advantages
    pub advantages: Array1<f32>,
    /// Returns
    pub returns: Array1<f32>,
    /// Values
    pub values: Array1<f32>,
}

impl GrpoBatch {
    /// Create a new batch from samples
    pub fn from_samples(samples: &[GrpoSample], embedding_dim: usize) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }

        let n = samples.len();

        // Build state matrix
        let mut states = Array2::zeros((n, embedding_dim));
        for (i, sample) in samples.iter().enumerate() {
            for (j, &val) in sample.state.iter().enumerate().take(embedding_dim) {
                states[[i, j]] = val;
            }
        }

        // Build other arrays
        let actions: Vec<usize> = samples.iter().map(|s| s.action).collect();
        let log_probs = Array1::from_vec(samples.iter().map(|s| s.log_prob).collect());
        let ref_log_probs = Array1::from_vec(samples.iter().map(|s| s.ref_log_prob).collect());

        // Placeholder advantages and returns (would be computed)
        let advantages = Array1::zeros(n);
        let returns = Array1::zeros(n);
        let values = Array1::from_vec(samples.iter().map(|s| s.value.unwrap_or(0.0)).collect());

        Some(Self {
            states,
            actions,
            log_probs,
            ref_log_probs,
            advantages,
            returns,
            values,
        })
    }

    /// Get batch size
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Split into mini-batches
    pub fn into_mini_batches(self, mini_batch_size: usize) -> Vec<GrpoBatch> {
        let n = self.len();
        if n <= mini_batch_size {
            return vec![self];
        }

        let num_batches = (n + mini_batch_size - 1) / mini_batch_size;
        let mut batches = Vec::with_capacity(num_batches);

        for i in 0..num_batches {
            let start = i * mini_batch_size;
            let end = (start + mini_batch_size).min(n);

            let states = self.states.slice(ndarray::s![start..end, ..]).to_owned();
            let actions = self.actions[start..end].to_vec();
            let log_probs = self.log_probs.slice(ndarray::s![start..end]).to_owned();
            let ref_log_probs = self.ref_log_probs.slice(ndarray::s![start..end]).to_owned();
            let advantages = self.advantages.slice(ndarray::s![start..end]).to_owned();
            let returns = self.returns.slice(ndarray::s![start..end]).to_owned();
            let values = self.values.slice(ndarray::s![start..end]).to_owned();

            batches.push(GrpoBatch {
                states,
                actions,
                log_probs,
                ref_log_probs,
                advantages,
                returns,
                values,
            });
        }

        batches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grpo_config_default() {
        let config = GrpoConfig::default();
        assert_eq!(config.group_size, 8);
        assert!((config.learning_rate - 1e-5).abs() < 1e-10);
    }

    #[test]
    fn test_compute_relative_advantages() {
        let optimizer = GrpoOptimizer::new(GrpoConfig::default());

        let rewards = vec![0.8, 0.6, 0.9, 0.5];
        let advantages = optimizer.compute_relative_advantages(&rewards);

        assert_eq!(advantages.len(), 4);

        // Mean should be approximately 0 after normalization
        let mean: f32 = advantages.iter().sum::<f32>() / advantages.len() as f32;
        assert!(mean.abs() < 1e-5);

        // Highest reward should have highest advantage
        let max_reward_idx = rewards
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        let max_advantage_idx = advantages
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(max_reward_idx, max_advantage_idx);
    }

    #[test]
    fn test_grpo_update() {
        let mut optimizer = GrpoOptimizer::new(GrpoConfig::default());

        let log_probs = vec![-0.5, -0.3, -0.7, -0.4];
        let advantages = vec![0.5, 0.2, -0.3, 0.1];
        let ref_log_probs = vec![-0.5, -0.3, -0.7, -0.4]; // Same as current

        let result = optimizer
            .grpo_update(&log_probs, &advantages, &ref_log_probs)
            .unwrap();

        assert_eq!(result.num_samples, 4);
        assert!(result.kl_divergence.abs() < 1e-5); // No KL when same policy
    }

    #[test]
    fn test_compute_gae() {
        let optimizer = GrpoOptimizer::new(GrpoConfig::default());

        let rewards = vec![1.0, 0.0, 1.0, 0.0];
        let values = vec![0.5, 0.5, 0.5, 0.5];
        let dones = vec![false, false, false, true];
        let next_value = 0.5;

        let advantages = optimizer.compute_gae(&rewards, &values, &dones, next_value);

        assert_eq!(advantages.len(), 4);
        // Last advantage should be simple TD error since it's terminal
        let expected_last = rewards[3] + 0.0 - values[3]; // 0.0 - 0.5 = -0.5
        assert!((advantages[3] - expected_last).abs() < 1e-5);
    }

    #[test]
    fn test_compute_returns() {
        let optimizer = GrpoOptimizer::new(GrpoConfig {
            gamma: 0.9,
            ..Default::default()
        });

        let rewards = vec![1.0, 1.0, 1.0];
        let dones = vec![false, false, true];

        let returns = optimizer.compute_returns(&rewards, &dones);

        assert_eq!(returns.len(), 3);
        // G_2 = r_2 = 1.0 (terminal)
        assert!((returns[2] - 1.0).abs() < 1e-5);
        // G_1 = r_1 + gamma * G_2 = 1.0 + 0.9 * 1.0 = 1.9
        assert!((returns[1] - 1.9).abs() < 1e-5);
        // G_0 = r_0 + gamma * G_1 = 1.0 + 0.9 * 1.9 = 2.71
        assert!((returns[0] - 2.71).abs() < 1e-5);
    }

    #[test]
    fn test_adaptive_kl() {
        let mut optimizer = GrpoOptimizer::new(GrpoConfig {
            adaptive_kl: true,
            kl_coefficient: 0.02,
            kl_target: 0.01,
            kl_min: 0.001,
            kl_max: 0.1,
            ..Default::default()
        });

        // High KL should increase coefficient
        optimizer.adapt_kl_coefficient(0.05); // > 1.5 * target
        assert!(optimizer.kl_coef > 0.02);

        // Reset
        optimizer.kl_coef = 0.02;

        // Low KL should decrease coefficient
        optimizer.adapt_kl_coefficient(0.001); // < 0.5 * target
        assert!(optimizer.kl_coef < 0.02);
    }

    #[test]
    fn test_grpo_sample() {
        let sample = GrpoSample {
            state: vec![0.1, 0.2, 0.3],
            action: 5,
            log_prob: -0.5,
            ref_log_prob: -0.5,
            reward: 0.8,
            done: false,
            value: Some(0.7),
            tool_name: "agent_spawn".to_string(),
            parameters: None,
        };

        assert_eq!(sample.action, 5);
        assert_eq!(sample.tool_name, "agent_spawn");
    }

    #[test]
    fn test_sample_group() {
        let samples = vec![
            GrpoSample {
                state: vec![0.1, 0.2],
                action: 0,
                log_prob: -0.5,
                ref_log_prob: -0.5,
                reward: 0.8,
                done: false,
                value: None,
                tool_name: "memory_store".to_string(),
                parameters: None,
            },
            GrpoSample {
                state: vec![0.3, 0.4],
                action: 1,
                log_prob: -0.3,
                ref_log_prob: -0.3,
                reward: 0.6,
                done: false,
                value: None,
                tool_name: "memory_search".to_string(),
                parameters: None,
            },
        ];

        let group = SampleGroup::new(samples, 1, "test task".to_string());
        assert_eq!(group.len(), 2);
        assert_eq!(group.group_id, 1);
        assert!(!group.is_empty());
    }

    #[test]
    fn test_batch_creation() {
        let samples = vec![
            GrpoSample {
                state: vec![0.1, 0.2, 0.3, 0.4],
                action: 0,
                log_prob: -0.5,
                ref_log_prob: -0.5,
                reward: 0.8,
                done: false,
                value: Some(0.7),
                tool_name: "test".to_string(),
                parameters: None,
            },
            GrpoSample {
                state: vec![0.5, 0.6, 0.7, 0.8],
                action: 1,
                log_prob: -0.3,
                ref_log_prob: -0.3,
                reward: 0.6,
                done: true,
                value: Some(0.5),
                tool_name: "test2".to_string(),
                parameters: None,
            },
        ];

        let batch = GrpoBatch::from_samples(&samples, 4).unwrap();
        assert_eq!(batch.len(), 2);
        assert_eq!(batch.states.shape(), &[2, 4]);
    }

    #[test]
    fn test_mini_batches() {
        let samples: Vec<GrpoSample> = (0..10)
            .map(|i| GrpoSample {
                state: vec![i as f32; 4],
                action: i,
                log_prob: -(i as f32) * 0.1,
                ref_log_prob: -(i as f32) * 0.1,
                reward: i as f32 * 0.1,
                done: false,
                value: None,
                tool_name: format!("tool_{}", i),
                parameters: None,
            })
            .collect();

        let batch = GrpoBatch::from_samples(&samples, 4).unwrap();
        let mini_batches = batch.into_mini_batches(3);

        assert_eq!(mini_batches.len(), 4); // ceil(10/3) = 4
        assert_eq!(mini_batches[0].len(), 3);
        assert_eq!(mini_batches[1].len(), 3);
        assert_eq!(mini_batches[2].len(), 3);
        assert_eq!(mini_batches[3].len(), 1);
    }
}
