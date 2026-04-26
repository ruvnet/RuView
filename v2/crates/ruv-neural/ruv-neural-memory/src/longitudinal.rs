//! Longitudinal tracking and drift detection for neural topology changes
//! over extended observation periods.

use ruv_neural_core::embedding::NeuralEmbedding;

/// Direction of observed trend in neural embeddings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendDirection {
    /// No significant change from baseline.
    Stable,
    /// Embedding distances are decreasing (closer to baseline).
    Improving,
    /// Embedding distances are increasing (drifting from baseline).
    Degrading,
    /// Embeddings alternate between improving and degrading.
    Oscillating,
}

/// Tracks neural topology changes over extended periods, detecting drift
/// from an established baseline.
pub struct LongitudinalTracker {
    /// Baseline embeddings representing the reference state.
    baseline_embeddings: Vec<NeuralEmbedding>,
    /// Current trajectory of observations.
    current_trajectory: Vec<NeuralEmbedding>,
    /// Threshold above which drift is considered significant.
    drift_threshold: f64,
}

impl LongitudinalTracker {
    /// Create a new tracker with the given drift threshold.
    pub fn new(drift_threshold: f64) -> Self {
        Self {
            baseline_embeddings: Vec::new(),
            current_trajectory: Vec::new(),
            drift_threshold,
        }
    }

    /// Set the baseline embeddings (the reference state).
    pub fn set_baseline(&mut self, embeddings: Vec<NeuralEmbedding>) {
        self.baseline_embeddings = embeddings;
    }

    /// Add a new observation to the current trajectory.
    pub fn add_observation(&mut self, embedding: NeuralEmbedding) {
        self.current_trajectory.push(embedding);
    }

    /// Number of observations in the current trajectory.
    pub fn num_observations(&self) -> usize {
        self.current_trajectory.len()
    }

    /// Compute the mean drift from baseline.
    ///
    /// Returns the average Euclidean distance from each trajectory embedding
    /// to the nearest baseline embedding. Returns 0.0 if either baseline or
    /// trajectory is empty.
    pub fn compute_drift(&self) -> f64 {
        if self.baseline_embeddings.is_empty() || self.current_trajectory.is_empty() {
            return 0.0;
        }

        let total_drift: f64 = self
            .current_trajectory
            .iter()
            .map(|obs| self.min_distance_to_baseline(obs))
            .sum();

        total_drift / self.current_trajectory.len() as f64
    }

    /// Detect the overall trend direction from the trajectory.
    ///
    /// Compares drift of the first half vs second half of the trajectory.
    pub fn detect_trend(&self) -> TrendDirection {
        if self.current_trajectory.len() < 4 || self.baseline_embeddings.is_empty() {
            return TrendDirection::Stable;
        }

        let mid = self.current_trajectory.len() / 2;
        let first_half: Vec<f64> = self.current_trajectory[..mid]
            .iter()
            .map(|obs| self.min_distance_to_baseline(obs))
            .collect();
        let second_half: Vec<f64> = self.current_trajectory[mid..]
            .iter()
            .map(|obs| self.min_distance_to_baseline(obs))
            .collect();

        let first_mean = mean(&first_half);
        let second_mean = mean(&second_half);

        let diff = second_mean - first_mean;

        if diff.abs() < self.drift_threshold * 0.1 {
            // Check for oscillation by looking at alternating signs
            let diffs: Vec<f64> = self
                .current_trajectory
                .windows(2)
                .map(|w| {
                    self.min_distance_to_baseline(&w[1])
                        - self.min_distance_to_baseline(&w[0])
                })
                .collect();

            let sign_changes = diffs
                .windows(2)
                .filter(|w| w[0].signum() != w[1].signum())
                .count();

            if sign_changes > diffs.len() / 2 {
                return TrendDirection::Oscillating;
            }

            TrendDirection::Stable
        } else if diff > 0.0 {
            TrendDirection::Degrading
        } else {
            TrendDirection::Improving
        }
    }

    /// Compute an anomaly score for a single embedding.
    ///
    /// Returns a score in [0, 1] where 1 means highly anomalous relative
    /// to the baseline. Based on how far the embedding is from the baseline
    /// relative to the drift threshold.
    pub fn anomaly_score(&self, embedding: &NeuralEmbedding) -> f64 {
        if self.baseline_embeddings.is_empty() {
            return 0.0;
        }

        let dist = self.min_distance_to_baseline(embedding);
        // Sigmoid-like mapping: score = 1 - exp(-dist / threshold)
        1.0 - (-dist / self.drift_threshold).exp()
    }

    /// Minimum Euclidean distance from an embedding to any baseline embedding.
    fn min_distance_to_baseline(&self, embedding: &NeuralEmbedding) -> f64 {
        self.baseline_embeddings
            .iter()
            .filter_map(|base| base.euclidean_distance(embedding).ok())
            .fold(f64::MAX, f64::min)
    }
}

/// Compute the arithmetic mean of a slice.
fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::embedding::EmbeddingMetadata;
    use ruv_neural_core::topology::CognitiveState;

    fn make_embedding(vector: Vec<f64>, timestamp: f64) -> NeuralEmbedding {
        NeuralEmbedding::new(
            vector,
            timestamp,
            EmbeddingMetadata {
                subject_id: Some("subj1".to_string()),
                session_id: None,
                cognitive_state: Some(CognitiveState::Rest),
                source_atlas: Atlas::Schaefer100,
                embedding_method: "test".to_string(),
            },
        )
        .unwrap()
    }

    #[test]
    fn empty_tracker_returns_zero_drift() {
        let tracker = LongitudinalTracker::new(1.0);
        assert_eq!(tracker.compute_drift(), 0.0);
    }

    #[test]
    fn no_drift_when_same_as_baseline() {
        let mut tracker = LongitudinalTracker::new(1.0);
        tracker.set_baseline(vec![make_embedding(vec![0.0, 0.0], 0.0)]);
        tracker.add_observation(make_embedding(vec![0.0, 0.0], 1.0));

        assert!(tracker.compute_drift() < 1e-10);
    }

    #[test]
    fn detects_known_drift() {
        let mut tracker = LongitudinalTracker::new(1.0);
        tracker.set_baseline(vec![make_embedding(vec![0.0, 0.0, 0.0], 0.0)]);

        // Add observations that progressively drift
        for i in 1..=10 {
            let offset = i as f64;
            tracker.add_observation(make_embedding(vec![offset, 0.0, 0.0], i as f64));
        }

        let drift = tracker.compute_drift();
        assert!(drift > 1.0, "Expected significant drift, got {}", drift);
    }

    #[test]
    fn degrading_trend_detected() {
        let mut tracker = LongitudinalTracker::new(1.0);
        tracker.set_baseline(vec![make_embedding(vec![0.0, 0.0], 0.0)]);

        // First half: close to baseline
        for i in 1..=5 {
            tracker.add_observation(make_embedding(vec![0.1 * i as f64, 0.0], i as f64));
        }
        // Second half: far from baseline
        for i in 6..=10 {
            tracker.add_observation(make_embedding(vec![2.0 * i as f64, 0.0], i as f64));
        }

        assert_eq!(tracker.detect_trend(), TrendDirection::Degrading);
    }

    #[test]
    fn improving_trend_detected() {
        let mut tracker = LongitudinalTracker::new(1.0);
        tracker.set_baseline(vec![make_embedding(vec![0.0, 0.0], 0.0)]);

        // First half: far from baseline
        for i in 1..=5 {
            tracker.add_observation(make_embedding(
                vec![10.0 - i as f64 * 1.5, 0.0],
                i as f64,
            ));
        }
        // Second half: close to baseline
        for i in 6..=10 {
            tracker.add_observation(make_embedding(vec![0.1, 0.0], i as f64));
        }

        assert_eq!(tracker.detect_trend(), TrendDirection::Improving);
    }

    #[test]
    fn anomaly_score_increases_with_distance() {
        let mut tracker = LongitudinalTracker::new(2.0);
        tracker.set_baseline(vec![make_embedding(vec![0.0, 0.0], 0.0)]);

        let near = make_embedding(vec![0.1, 0.0], 1.0);
        let far = make_embedding(vec![10.0, 10.0], 2.0);

        let score_near = tracker.anomaly_score(&near);
        let score_far = tracker.anomaly_score(&far);

        assert!(score_near < score_far);
        assert!(score_near >= 0.0 && score_near <= 1.0);
        assert!(score_far >= 0.0 && score_far <= 1.0);
    }

    #[test]
    fn anomaly_score_zero_without_baseline() {
        let tracker = LongitudinalTracker::new(1.0);
        let emb = make_embedding(vec![5.0, 5.0], 1.0);
        assert_eq!(tracker.anomaly_score(&emb), 0.0);
    }
}
