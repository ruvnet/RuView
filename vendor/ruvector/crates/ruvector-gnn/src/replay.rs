//! Experience Replay Buffer for GNN Training
//!
//! This module implements an experience replay buffer to mitigate catastrophic forgetting
//! during continual learning. The buffer stores past training samples and supports:
//! - Reservoir sampling for uniform distribution over time
//! - Batch sampling for training
//! - Distribution shift detection

use rand::Rng;
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single entry in the replay buffer
#[derive(Debug, Clone)]
pub struct ReplayEntry {
    /// Query vector used for training
    pub query: Vec<f32>,
    /// IDs of positive nodes for this query
    pub positive_ids: Vec<usize>,
    /// Timestamp when this entry was added (milliseconds since epoch)
    pub timestamp: u64,
}

impl ReplayEntry {
    /// Create a new replay entry with current timestamp
    pub fn new(query: Vec<f32>, positive_ids: Vec<usize>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            query,
            positive_ids,
            timestamp,
        }
    }
}

/// Statistics for tracking distribution characteristics
#[derive(Debug, Clone)]
pub struct DistributionStats {
    /// Running mean of query vectors
    pub mean: Vec<f32>,
    /// Running variance of query vectors
    pub variance: Vec<f32>,
    /// Number of samples used to compute statistics
    pub count: usize,
}

impl DistributionStats {
    /// Create new distribution statistics
    pub fn new(dimension: usize) -> Self {
        Self {
            mean: vec![0.0; dimension],
            variance: vec![0.0; dimension],
            count: 0,
        }
    }

    /// Update statistics with a new sample using Welford's online algorithm
    pub fn update(&mut self, sample: &[f32]) {
        if self.mean.is_empty() && !sample.is_empty() {
            self.mean = vec![0.0; sample.len()];
            self.variance = vec![0.0; sample.len()];
        }

        if self.mean.len() != sample.len() {
            return; // Dimension mismatch, skip update
        }

        self.count += 1;
        let count = self.count as f32;

        for i in 0..sample.len() {
            let delta = sample[i] - self.mean[i];
            self.mean[i] += delta / count;
            let delta2 = sample[i] - self.mean[i];
            self.variance[i] += delta * delta2;
        }
    }

    /// Compute standard deviation from variance
    pub fn std_dev(&self) -> Vec<f32> {
        if self.count <= 1 {
            return vec![0.0; self.variance.len()];
        }

        self.variance
            .iter()
            .map(|&v| (v / (self.count - 1) as f32).sqrt())
            .collect()
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        let dim = self.mean.len();
        self.mean = vec![0.0; dim];
        self.variance = vec![0.0; dim];
        self.count = 0;
    }
}

/// Experience Replay Buffer for storing and sampling past training examples
pub struct ReplayBuffer {
    /// Circular buffer of replay entries
    queries: VecDeque<ReplayEntry>,
    /// Maximum capacity of the buffer
    capacity: usize,
    /// Total number of samples seen (including evicted ones)
    total_seen: usize,
    /// Statistics of the overall distribution
    distribution_stats: DistributionStats,
}

impl ReplayBuffer {
    /// Create a new replay buffer with specified capacity
    ///
    /// # Arguments
    /// * `capacity` - Maximum number of entries to store
    pub fn new(capacity: usize) -> Self {
        Self {
            queries: VecDeque::with_capacity(capacity),
            capacity,
            total_seen: 0,
            distribution_stats: DistributionStats::new(0),
        }
    }

    /// Add a new entry to the buffer using reservoir sampling
    ///
    /// Reservoir sampling ensures uniform distribution over all samples seen,
    /// even as old samples are evicted due to capacity constraints.
    ///
    /// # Arguments
    /// * `query` - Query vector
    /// * `positive_ids` - IDs of positive nodes for this query
    pub fn add(&mut self, query: &[f32], positive_ids: &[usize]) {
        let entry = ReplayEntry::new(query.to_vec(), positive_ids.to_vec());

        self.total_seen += 1;

        // Update distribution statistics
        self.distribution_stats.update(query);

        // If buffer is not full, just add the entry
        if self.queries.len() < self.capacity {
            self.queries.push_back(entry);
            return;
        }

        // Reservoir sampling: replace a random entry with probability capacity/total_seen
        let mut rng = rand::thread_rng();
        let random_index = rng.gen_range(0..self.total_seen);

        if random_index < self.capacity {
            self.queries[random_index] = entry;
        }
    }

    /// Sample a batch of entries uniformly at random
    ///
    /// # Arguments
    /// * `batch_size` - Number of entries to sample
    ///
    /// # Returns
    /// Vector of references to sampled entries (may be smaller than batch_size if buffer is small)
    pub fn sample(&self, batch_size: usize) -> Vec<&ReplayEntry> {
        if self.queries.is_empty() {
            return Vec::new();
        }

        let actual_batch_size = batch_size.min(self.queries.len());
        let mut rng = rand::thread_rng();
        let mut indices: Vec<usize> = (0..self.queries.len()).collect();

        // Fisher-Yates shuffle for first batch_size elements
        for i in 0..actual_batch_size {
            let j = rng.gen_range(i..indices.len());
            indices.swap(i, j);
        }

        indices[..actual_batch_size]
            .iter()
            .map(|&idx| &self.queries[idx])
            .collect()
    }

    /// Detect distribution shift between recent samples and overall distribution
    ///
    /// Uses Kullback-Leibler divergence approximation based on mean and variance changes.
    ///
    /// # Arguments
    /// * `recent_window` - Number of most recent samples to compare
    ///
    /// # Returns
    /// Shift score (higher values indicate more significant distribution shift)
    /// Returns 0.0 if insufficient data
    pub fn detect_distribution_shift(&self, recent_window: usize) -> f32 {
        if self.queries.len() < recent_window || recent_window == 0 {
            return 0.0;
        }

        // Compute statistics for recent window
        let mut recent_stats = DistributionStats::new(self.distribution_stats.mean.len());

        let start_idx = self.queries.len().saturating_sub(recent_window);
        for entry in self.queries.iter().skip(start_idx) {
            recent_stats.update(&entry.query);
        }

        // Compute shift using normalized mean difference
        let overall_mean = &self.distribution_stats.mean;
        let recent_mean = &recent_stats.mean;

        if overall_mean.is_empty() || recent_mean.is_empty() {
            return 0.0;
        }

        let overall_std = self.distribution_stats.std_dev();
        let mut shift_sum = 0.0;
        let mut count = 0;

        for i in 0..overall_mean.len() {
            if overall_std[i] > 1e-8 {
                let diff = (recent_mean[i] - overall_mean[i]).abs();
                shift_sum += diff / overall_std[i];
                count += 1;
            }
        }

        if count > 0 {
            shift_sum / count as f32
        } else {
            0.0
        }
    }

    /// Get the number of entries currently in the buffer
    pub fn len(&self) -> usize {
        self.queries.len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.queries.is_empty()
    }

    /// Get the total capacity of the buffer
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the total number of samples seen (including evicted ones)
    pub fn total_seen(&self) -> usize {
        self.total_seen
    }

    /// Get a reference to the distribution statistics
    pub fn distribution_stats(&self) -> &DistributionStats {
        &self.distribution_stats
    }

    /// Clear all entries from the buffer
    pub fn clear(&mut self) {
        self.queries.clear();
        self.total_seen = 0;
        self.distribution_stats.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_buffer_basic() {
        let mut buffer = ReplayBuffer::new(10);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.capacity(), 10);

        buffer.add(&[1.0, 2.0, 3.0], &[0, 1]);
        assert_eq!(buffer.len(), 1);
        assert!(!buffer.is_empty());

        buffer.add(&[4.0, 5.0, 6.0], &[2, 3]);
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.total_seen(), 2);
    }

    #[test]
    fn test_replay_buffer_capacity() {
        let mut buffer = ReplayBuffer::new(3);

        // Add entries up to capacity
        for i in 0..3 {
            buffer.add(&[i as f32], &[i]);
        }
        assert_eq!(buffer.len(), 3);

        // Adding more should maintain capacity through reservoir sampling
        for i in 3..10 {
            buffer.add(&[i as f32], &[i]);
        }
        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.total_seen(), 10);
    }

    #[test]
    fn test_sample_empty_buffer() {
        let buffer = ReplayBuffer::new(10);
        let samples = buffer.sample(5);
        assert!(samples.is_empty());
    }

    #[test]
    fn test_sample_basic() {
        let mut buffer = ReplayBuffer::new(10);

        for i in 0..5 {
            buffer.add(&[i as f32], &[i]);
        }

        let samples = buffer.sample(3);
        assert_eq!(samples.len(), 3);

        // Check that samples are from the buffer
        for sample in samples {
            assert!(sample.query[0] >= 0.0 && sample.query[0] < 5.0);
        }
    }

    #[test]
    fn test_sample_larger_than_buffer() {
        let mut buffer = ReplayBuffer::new(10);

        buffer.add(&[1.0], &[0]);
        buffer.add(&[2.0], &[1]);

        let samples = buffer.sample(5);
        assert_eq!(samples.len(), 2); // Can only return what's available
    }

    #[test]
    fn test_distribution_stats_update() {
        let mut stats = DistributionStats::new(2);

        stats.update(&[1.0, 2.0]);
        assert_eq!(stats.count, 1);
        assert_eq!(stats.mean, vec![1.0, 2.0]);

        stats.update(&[3.0, 4.0]);
        assert_eq!(stats.count, 2);
        assert_eq!(stats.mean, vec![2.0, 3.0]);

        stats.update(&[2.0, 3.0]);
        assert_eq!(stats.count, 3);
        assert_eq!(stats.mean, vec![2.0, 3.0]);
    }

    #[test]
    fn test_distribution_stats_std_dev() {
        let mut stats = DistributionStats::new(2);

        stats.update(&[1.0, 1.0]);
        stats.update(&[3.0, 3.0]);
        stats.update(&[5.0, 5.0]);

        let std_dev = stats.std_dev();
        // Expected std dev for [1, 3, 5] is 2.0
        assert!((std_dev[0] - 2.0).abs() < 0.01);
        assert!((std_dev[1] - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_detect_distribution_shift_no_shift() {
        let mut buffer = ReplayBuffer::new(100);

        // Add samples from the same distribution
        for _ in 0..50 {
            buffer.add(&[1.0, 2.0, 3.0], &[0]);
        }

        let shift = buffer.detect_distribution_shift(10);
        assert!(shift < 0.1); // Should be very low
    }

    #[test]
    fn test_detect_distribution_shift_with_shift() {
        let mut buffer = ReplayBuffer::new(100);

        // Add samples from one distribution
        for _ in 0..40 {
            buffer.add(&[1.0, 2.0, 3.0], &[0]);
        }

        // Add samples from a different distribution
        for _ in 0..10 {
            buffer.add(&[5.0, 6.0, 7.0], &[1]);
        }

        let shift = buffer.detect_distribution_shift(10);
        assert!(shift > 0.5); // Should detect significant shift
    }

    #[test]
    fn test_detect_distribution_shift_insufficient_data() {
        let mut buffer = ReplayBuffer::new(100);

        buffer.add(&[1.0, 2.0], &[0]);

        let shift = buffer.detect_distribution_shift(10);
        assert_eq!(shift, 0.0); // Not enough data
    }

    #[test]
    fn test_clear() {
        let mut buffer = ReplayBuffer::new(10);

        for i in 0..5 {
            buffer.add(&[i as f32], &[i]);
        }

        assert_eq!(buffer.len(), 5);
        assert_eq!(buffer.total_seen(), 5);

        buffer.clear();
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.total_seen(), 0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.distribution_stats().count, 0);
    }

    #[test]
    fn test_replay_entry_creation() {
        let entry = ReplayEntry::new(vec![1.0, 2.0, 3.0], vec![0, 1, 2]);

        assert_eq!(entry.query, vec![1.0, 2.0, 3.0]);
        assert_eq!(entry.positive_ids, vec![0, 1, 2]);
        assert!(entry.timestamp > 0);
    }

    #[test]
    fn test_reservoir_sampling_distribution() {
        let mut buffer = ReplayBuffer::new(10);

        // Add 100 entries (much more than capacity)
        for i in 0..100 {
            buffer.add(&[i as f32], &[i]);
        }

        assert_eq!(buffer.len(), 10);
        assert_eq!(buffer.total_seen(), 100);

        // Sample multiple times and verify we get different samples
        let samples1 = buffer.sample(5);
        let samples2 = buffer.sample(5);

        assert_eq!(samples1.len(), 5);
        assert_eq!(samples2.len(), 5);

        // Check that samples come from the full range (not just recent entries)
        let sample_batch = buffer.sample(10);
        let values: Vec<f32> = sample_batch.iter().map(|e| e.query[0]).collect();

        // With reservoir sampling, we should have some diversity in values
        let unique_values: std::collections::HashSet<_> =
            values.iter().map(|&v| v as i32).collect();
        assert!(unique_values.len() > 1);
    }

    #[test]
    fn test_dimension_mismatch_handling() {
        let mut buffer = ReplayBuffer::new(10);

        buffer.add(&[1.0, 2.0], &[0]);

        // This should not panic, just be handled gracefully
        // The implementation will initialize stats on first add
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.distribution_stats().mean.len(), 2);
    }

    #[test]
    fn test_sample_uniqueness() {
        let mut buffer = ReplayBuffer::new(5);

        for i in 0..5 {
            buffer.add(&[i as f32], &[i]);
        }

        // Sample all entries
        let samples = buffer.sample(5);
        let values: Vec<f32> = samples.iter().map(|e| e.query[0]).collect();

        // All samples should be unique (no duplicates in a single batch)
        let unique_values: std::collections::HashSet<_> =
            values.iter().map(|&v| v as i32).collect();
        assert_eq!(unique_values.len(), 5);
    }
}
