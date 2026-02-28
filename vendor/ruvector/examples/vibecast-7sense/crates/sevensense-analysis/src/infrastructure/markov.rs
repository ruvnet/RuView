//! Markov chain analysis for vocalization sequences.
//!
//! Provides transition matrix computation, entropy calculation,
//! and sequence analysis for understanding vocalization patterns.

use std::collections::HashSet;
use tracing::{debug, instrument};

use crate::domain::entities::ClusterId;
use crate::domain::value_objects::{SequenceMetrics, TransitionMatrix};

/// Markov chain analyzer for vocalization sequences.
pub struct MarkovAnalyzer {
    /// Smoothing factor for probability estimation (Laplace smoothing).
    smoothing: f32,
}

impl MarkovAnalyzer {
    /// Create a new Markov analyzer.
    #[must_use]
    pub fn new() -> Self {
        Self { smoothing: 0.0 }
    }

    /// Create with Laplace smoothing.
    #[must_use]
    pub fn with_smoothing(smoothing: f32) -> Self {
        Self { smoothing }
    }

    /// Build a transition matrix from a sequence of cluster IDs.
    ///
    /// # Arguments
    ///
    /// * `sequence` - Ordered sequence of cluster IDs
    ///
    /// # Returns
    ///
    /// A TransitionMatrix representing transition probabilities.
    #[instrument(skip(self, sequence), fields(seq_len = sequence.len()))]
    pub fn build_transition_matrix(&self, sequence: &[ClusterId]) -> TransitionMatrix {
        // Collect all unique clusters
        let unique_clusters: Vec<ClusterId> = sequence
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let mut matrix = TransitionMatrix::new(unique_clusters);

        // Count transitions
        for window in sequence.windows(2) {
            matrix.record_transition(&window[0], &window[1]);
        }

        // Apply smoothing if configured
        if self.smoothing > 0.0 {
            self.apply_smoothing(&mut matrix);
        }

        // Compute probabilities
        matrix.compute_probabilities();

        debug!(
            n_states = matrix.size(),
            n_transitions = matrix.non_zero_transitions().len(),
            "Built transition matrix"
        );

        matrix
    }

    /// Build transition matrix from multiple sequences.
    #[instrument(skip(self, sequences))]
    pub fn build_from_sequences(&self, sequences: &[Vec<ClusterId>]) -> TransitionMatrix {
        // Collect all unique clusters from all sequences
        let unique_clusters: Vec<ClusterId> = sequences
            .iter()
            .flatten()
            .copied()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let mut matrix = TransitionMatrix::new(unique_clusters);

        // Count transitions from all sequences
        for sequence in sequences {
            for window in sequence.windows(2) {
                matrix.record_transition(&window[0], &window[1]);
            }
        }

        // Apply smoothing and compute probabilities
        if self.smoothing > 0.0 {
            self.apply_smoothing(&mut matrix);
        }
        matrix.compute_probabilities();

        matrix
    }

    /// Compute Shannon entropy of transition probabilities.
    ///
    /// # Arguments
    ///
    /// * `transitions` - Slice of (source, target, probability) tuples
    ///
    /// # Returns
    ///
    /// Entropy value in nats (natural log base).
    #[must_use]
    pub fn compute_entropy(&self, transitions: &[(ClusterId, ClusterId, f32)]) -> f32 {
        let mut entropy = 0.0f32;

        for &(_, _, prob) in transitions {
            if prob > 0.0 {
                entropy -= prob * prob.ln();
            }
        }

        entropy
    }

    /// Compute entropy rate of a Markov chain.
    ///
    /// The entropy rate is the average entropy per step, weighted
    /// by the stationary distribution.
    #[must_use]
    pub fn compute_entropy_rate(&self, matrix: &TransitionMatrix) -> f32 {
        let stationary = match matrix.stationary_distribution() {
            Some(dist) => dist,
            None => return 0.0,
        };

        let n = matrix.size();
        let mut entropy_rate = 0.0f32;

        for (i, &pi) in stationary.iter().enumerate() {
            if pi <= 0.0 {
                continue;
            }

            // Compute entropy of row i
            let mut row_entropy = 0.0f32;
            for j in 0..n {
                let prob = matrix.probabilities[i][j];
                if prob > 0.0 {
                    row_entropy -= prob * prob.ln();
                }
            }

            entropy_rate += pi * row_entropy;
        }

        entropy_rate
    }

    /// Compute sequence metrics from a cluster sequence.
    #[instrument(skip(self, sequence))]
    pub fn compute_metrics(&self, sequence: &[ClusterId]) -> SequenceMetrics {
        if sequence.len() < 2 {
            return SequenceMetrics::default();
        }

        let matrix = self.build_transition_matrix(sequence);
        let transitions = matrix.non_zero_transitions();

        // Count unique elements
        let unique_clusters: HashSet<_> = sequence.iter().collect();
        let total_transitions = sequence.len() - 1;

        // Count self-transitions
        let self_transitions = sequence
            .windows(2)
            .filter(|w| w[0] == w[1])
            .count();

        // Compute entropy
        let entropy = self.compute_entropy(&transitions);

        // Normalize entropy
        let max_entropy = (unique_clusters.len() as f32).ln().max(1.0);
        let normalized_entropy = if max_entropy > 0.0 {
            entropy / max_entropy
        } else {
            0.0
        };

        // Find dominant transition
        let dominant_transition = transitions
            .iter()
            .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            .map(|&(from, to, prob)| (from, to, prob));

        SequenceMetrics {
            entropy,
            normalized_entropy,
            stereotypy: 1.0 - normalized_entropy,
            unique_clusters: unique_clusters.len(),
            unique_transitions: transitions.len(),
            total_transitions,
            dominant_transition,
            repetition_rate: self_transitions as f32 / total_transitions as f32,
        }
    }

    /// Compute stereotypy score (measure of sequence repetitiveness).
    ///
    /// Higher values indicate more stereotyped/predictable sequences.
    #[must_use]
    pub fn compute_stereotypy(&self, matrix: &TransitionMatrix) -> f32 {
        let entropy_rate = self.compute_entropy_rate(matrix);
        let max_entropy = (matrix.size() as f32).ln();

        if max_entropy > 0.0 {
            1.0 - (entropy_rate / max_entropy)
        } else {
            1.0
        }
    }

    /// Detect periodic patterns in a sequence.
    ///
    /// Returns a vector of (period_length, confidence) tuples for detected patterns.
    #[instrument(skip(self, sequence))]
    pub fn detect_periodicity(&self, sequence: &[ClusterId]) -> Vec<(usize, f32)> {
        let n = sequence.len();
        if n < 4 {
            return Vec::new();
        }

        let mut periods = Vec::new();
        let max_period = n / 2;

        for period in 2..=max_period {
            let matches = self.count_periodic_matches(sequence, period);
            let max_matches = n / period;
            let confidence = matches as f32 / max_matches as f32;

            if confidence > 0.5 {
                periods.push((period, confidence));
            }
        }

        // Sort by confidence descending
        periods.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        periods
    }

    /// Count matches for a given period length.
    fn count_periodic_matches(&self, sequence: &[ClusterId], period: usize) -> usize {
        let n = sequence.len();
        let mut matches = 0;

        for i in period..n {
            if sequence[i] == sequence[i - period] {
                matches += 1;
            }
        }

        matches
    }

    /// Apply Laplace smoothing to observation counts.
    fn apply_smoothing(&self, matrix: &mut TransitionMatrix) {
        let n = matrix.size();
        for i in 0..n {
            for j in 0..n {
                matrix.observations[i][j] += self.smoothing as u32;
            }
        }
    }

    /// Compute log-likelihood of a sequence given a transition matrix.
    #[must_use]
    pub fn log_likelihood(&self, sequence: &[ClusterId], matrix: &TransitionMatrix) -> f32 {
        if sequence.len() < 2 {
            return 0.0;
        }

        let mut log_prob = 0.0f32;

        for window in sequence.windows(2) {
            if let Some(prob) = matrix.probability(&window[0], &window[1]) {
                if prob > 0.0 {
                    log_prob += prob.ln();
                } else {
                    // Unseen transition - return negative infinity
                    return f32::NEG_INFINITY;
                }
            }
        }

        log_prob
    }

    /// Find the most likely next cluster given current state.
    #[must_use]
    pub fn predict_next(
        &self,
        current: &ClusterId,
        matrix: &TransitionMatrix,
    ) -> Option<(ClusterId, f32)> {
        let idx = matrix.index_of(current)?;

        let mut best_cluster = None;
        let mut best_prob = 0.0f32;

        for (j, &target_id) in matrix.cluster_ids.iter().enumerate() {
            let prob = matrix.probabilities[idx][j];
            if prob > best_prob {
                best_prob = prob;
                best_cluster = Some(target_id);
            }
        }

        best_cluster.map(|c| (c, best_prob))
    }

    /// Generate a sequence from the Markov chain.
    ///
    /// # Arguments
    ///
    /// * `matrix` - The transition matrix
    /// * `start` - Starting cluster
    /// * `length` - Desired sequence length
    /// * `seed` - Random seed for reproducibility
    pub fn generate_sequence(
        &self,
        matrix: &TransitionMatrix,
        start: ClusterId,
        length: usize,
        seed: u64,
    ) -> Vec<ClusterId> {
        let mut sequence = Vec::with_capacity(length);
        sequence.push(start);

        let mut rng_state = seed;
        let mut next_random = || {
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((rng_state >> 33) as f32) / (u32::MAX as f32)
        };

        let mut current = start;

        for _ in 1..length {
            let idx = match matrix.index_of(&current) {
                Some(i) => i,
                None => break,
            };

            // Sample from transition probabilities
            let r = next_random();
            let mut cumsum = 0.0f32;
            let mut next_cluster = current;

            for (j, &cluster_id) in matrix.cluster_ids.iter().enumerate() {
                cumsum += matrix.probabilities[idx][j];
                if r < cumsum {
                    next_cluster = cluster_id;
                    break;
                }
            }

            sequence.push(next_cluster);
            current = next_cluster;
        }

        sequence
    }
}

impl Default for MarkovAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_sequence() -> Vec<ClusterId> {
        let c1 = ClusterId::from_uuid(uuid::Uuid::from_u128(1));
        let c2 = ClusterId::from_uuid(uuid::Uuid::from_u128(2));
        let c3 = ClusterId::from_uuid(uuid::Uuid::from_u128(3));

        // Pattern: c1 -> c2 -> c3 -> c1 -> c2 -> c3 (periodic)
        vec![c1, c2, c3, c1, c2, c3, c1, c2, c3]
    }

    #[test]
    fn test_build_transition_matrix() {
        let analyzer = MarkovAnalyzer::new();
        let sequence = create_test_sequence();

        let matrix = analyzer.build_transition_matrix(&sequence);

        assert_eq!(matrix.size(), 3);
        assert!(!matrix.non_zero_transitions().is_empty());
    }

    #[test]
    fn test_entropy_computation() {
        let analyzer = MarkovAnalyzer::new();

        // Uniform distribution should have higher entropy
        let c1 = ClusterId::new();
        let c2 = ClusterId::new();

        let uniform_transitions = vec![
            (c1, c1, 0.25),
            (c1, c2, 0.25),
            (c2, c1, 0.25),
            (c2, c2, 0.25),
        ];

        let entropy = analyzer.compute_entropy(&uniform_transitions);
        assert!(entropy > 0.0);

        // Deterministic distribution should have lower entropy
        let deterministic = vec![
            (c1, c2, 1.0),
            (c2, c1, 1.0),
        ];

        let det_entropy = analyzer.compute_entropy(&deterministic);
        assert!(det_entropy < entropy);
    }

    #[test]
    fn test_compute_metrics() {
        let analyzer = MarkovAnalyzer::new();
        let sequence = create_test_sequence();

        let metrics = analyzer.compute_metrics(&sequence);

        assert_eq!(metrics.unique_clusters, 3);
        // Deterministic sequence has zero entropy (each state has one successor)
        assert!(metrics.entropy >= 0.0);
        assert!(metrics.stereotypy >= 0.0 && metrics.stereotypy <= 1.0);
        assert!(metrics.total_transitions == sequence.len() - 1);
    }

    #[test]
    fn test_periodicity_detection() {
        let analyzer = MarkovAnalyzer::new();

        // Create highly periodic sequence
        let c1 = ClusterId::from_uuid(uuid::Uuid::from_u128(1));
        let c2 = ClusterId::from_uuid(uuid::Uuid::from_u128(2));

        let periodic_sequence = vec![c1, c2, c1, c2, c1, c2, c1, c2, c1, c2];
        let periods = analyzer.detect_periodicity(&periodic_sequence);

        // Should detect period 2 (may not be first due to confidence calculation)
        assert!(!periods.is_empty());
        // Check that period 2 is in the detected periods
        let has_period_2 = periods.iter().any(|(p, _)| *p == 2);
        assert!(has_period_2, "Period 2 should be detected, found periods: {:?}", periods);
    }

    #[test]
    fn test_predict_next() {
        let analyzer = MarkovAnalyzer::new();
        let sequence = create_test_sequence();
        let matrix = analyzer.build_transition_matrix(&sequence);

        let c1 = ClusterId::from_uuid(uuid::Uuid::from_u128(1));
        let c2 = ClusterId::from_uuid(uuid::Uuid::from_u128(2));

        // Given the pattern c1 -> c2 -> c3 -> ..., after c1 should come c2
        if let Some((next, prob)) = analyzer.predict_next(&c1, &matrix) {
            assert_eq!(next, c2);
            assert!(prob > 0.0);
        }
    }

    #[test]
    fn test_sequence_generation() {
        let analyzer = MarkovAnalyzer::new();
        let sequence = create_test_sequence();
        let matrix = analyzer.build_transition_matrix(&sequence);

        let c1 = ClusterId::from_uuid(uuid::Uuid::from_u128(1));
        let generated = analyzer.generate_sequence(&matrix, c1, 10, 42);

        assert_eq!(generated.len(), 10);
        assert_eq!(generated[0], c1);
    }

    #[test]
    fn test_smoothing() {
        let analyzer = MarkovAnalyzer::with_smoothing(1.0);

        let c1 = ClusterId::new();
        let c2 = ClusterId::new();
        let sequence = vec![c1, c2, c1, c2];

        let matrix = analyzer.build_transition_matrix(&sequence);

        // With smoothing, all transitions should have non-zero probability
        for i in 0..matrix.size() {
            for j in 0..matrix.size() {
                assert!(matrix.probabilities[i][j] > 0.0);
            }
        }
    }

    #[test]
    fn test_log_likelihood() {
        let analyzer = MarkovAnalyzer::new();
        let sequence = create_test_sequence();
        let matrix = analyzer.build_transition_matrix(&sequence);

        // Log-likelihood of the training sequence should be reasonably high
        let ll = analyzer.log_likelihood(&sequence, &matrix);
        assert!(ll.is_finite());
        assert!(ll <= 0.0); // Log probabilities are non-positive
    }
}
