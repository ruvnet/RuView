//! Training utilities for learned restriction maps.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// A batch of training examples.
#[derive(Debug, Clone)]
pub struct TrainingBatch {
    /// Source state vectors.
    pub sources: Vec<Vec<f32>>,
    /// Target state vectors.
    pub targets: Vec<Vec<f32>>,
    /// Expected residuals.
    pub expected_residuals: Vec<Vec<f32>>,
}

impl TrainingBatch {
    /// Create a new empty batch.
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            targets: Vec::new(),
            expected_residuals: Vec::new(),
        }
    }

    /// Create a batch with specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            sources: Vec::with_capacity(capacity),
            targets: Vec::with_capacity(capacity),
            expected_residuals: Vec::with_capacity(capacity),
        }
    }

    /// Add an example to the batch.
    pub fn add(&mut self, source: Vec<f32>, target: Vec<f32>, expected: Vec<f32>) {
        self.sources.push(source);
        self.targets.push(target);
        self.expected_residuals.push(expected);
    }

    /// Get batch size.
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// Clear the batch.
    pub fn clear(&mut self) {
        self.sources.clear();
        self.targets.clear();
        self.expected_residuals.clear();
    }
}

impl Default for TrainingBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Experience replay buffer for stable training.
#[derive(Debug)]
pub struct ReplayBuffer {
    /// Stored experiences.
    experiences: VecDeque<Experience>,
    /// Maximum capacity.
    capacity: usize,
}

impl ReplayBuffer {
    /// Create a new replay buffer.
    pub fn new(capacity: usize) -> Self {
        Self {
            experiences: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Add an experience.
    pub fn add(&mut self, source: Vec<f32>, target: Vec<f32>, expected: Vec<f32>) {
        if self.experiences.len() >= self.capacity {
            self.experiences.pop_front();
        }

        self.experiences.push_back(Experience {
            source,
            target,
            expected_residual: expected,
            timestamp_ms: current_time_ms(),
        });
    }

    /// Sample a batch of experiences.
    pub fn sample(&self, batch_size: usize) -> TrainingBatch {
        let mut batch = TrainingBatch::with_capacity(batch_size);

        if self.experiences.is_empty() {
            return batch;
        }

        // Simple random sampling using time-based seed
        let seed = current_time_ms();
        let n = self.experiences.len();

        for i in 0..batch_size.min(n) {
            // Simple LCG for pseudo-random selection
            let idx = ((seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(i as u64))
                % n as u64) as usize;
            let exp = &self.experiences[idx];
            batch.add(
                exp.source.clone(),
                exp.target.clone(),
                exp.expected_residual.clone(),
            );
        }

        batch
    }

    /// Get the number of stored experiences.
    pub fn len(&self) -> usize {
        self.experiences.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.experiences.is_empty()
    }

    /// Clear all experiences.
    pub fn clear(&mut self) {
        self.experiences.clear();
    }
}

/// A single experience.
#[derive(Debug, Clone)]
struct Experience {
    source: Vec<f32>,
    target: Vec<f32>,
    expected_residual: Vec<f32>,
    timestamp_ms: u64,
}

/// Training metrics from a training step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingMetrics {
    /// Loss value.
    pub loss: f32,
    /// EWC regularization loss.
    pub ewc_loss: f32,
    /// Total loss.
    pub total_loss: f32,
    /// Gradient norm.
    pub gradient_norm: f32,
    /// Current learning rate.
    pub learning_rate: f32,
    /// Batch size used.
    pub batch_size: usize,
    /// Training step number.
    pub step: usize,
}

impl TrainingMetrics {
    /// Create new training metrics.
    pub fn new(
        loss: f32,
        ewc_loss: f32,
        gradient_norm: f32,
        learning_rate: f32,
        batch_size: usize,
        step: usize,
    ) -> Self {
        Self {
            loss,
            ewc_loss,
            total_loss: loss + ewc_loss,
            gradient_norm,
            learning_rate,
            batch_size,
            step,
        }
    }
}

/// Result of a training epoch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingResult {
    /// Average loss over the epoch.
    pub avg_loss: f32,
    /// Average EWC loss.
    pub avg_ewc_loss: f32,
    /// Number of batches processed.
    pub batches: usize,
    /// Total samples processed.
    pub samples: usize,
    /// Epoch number.
    pub epoch: usize,
    /// Training duration in milliseconds.
    pub duration_ms: u64,
}

impl TrainingResult {
    /// Create from accumulated metrics.
    pub fn from_metrics(metrics: &[TrainingMetrics], epoch: usize, duration_ms: u64) -> Self {
        let n = metrics.len() as f32;
        Self {
            avg_loss: metrics.iter().map(|m| m.loss).sum::<f32>() / n.max(1.0),
            avg_ewc_loss: metrics.iter().map(|m| m.ewc_loss).sum::<f32>() / n.max(1.0),
            batches: metrics.len(),
            samples: metrics.iter().map(|m| m.batch_size).sum(),
            epoch,
            duration_ms,
        }
    }
}

/// Get current time in milliseconds.
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_training_batch() {
        let mut batch = TrainingBatch::new();
        batch.add(vec![1.0, 2.0], vec![3.0, 4.0], vec![0.1, 0.2]);

        assert_eq!(batch.len(), 1);
        assert!(!batch.is_empty());

        batch.clear();
        assert!(batch.is_empty());
    }

    #[test]
    fn test_replay_buffer() {
        let mut buffer = ReplayBuffer::new(100);

        for i in 0..50 {
            buffer.add(vec![i as f32], vec![i as f32 + 1.0], vec![0.1]);
        }

        assert_eq!(buffer.len(), 50);

        let batch = buffer.sample(10);
        assert_eq!(batch.len(), 10);
    }

    #[test]
    fn test_replay_buffer_overflow() {
        let mut buffer = ReplayBuffer::new(10);

        for i in 0..20 {
            buffer.add(vec![i as f32], vec![i as f32], vec![0.0]);
        }

        // Should only keep last 10
        assert_eq!(buffer.len(), 10);
    }
}
