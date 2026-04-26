//! Threshold-based topology decoder for cognitive state classification.

use std::collections::HashMap;

use ruv_neural_core::topology::{CognitiveState, TopologyMetrics};
use serde::{Deserialize, Serialize};

/// Decode cognitive states from topology metrics using learned thresholds.
///
/// Each cognitive state is associated with expected ranges for key topology
/// metrics (mincut, modularity, efficiency, entropy). The decoder scores
/// each candidate state by how well the input metrics fall within the
/// expected ranges.
pub struct ThresholdDecoder {
    thresholds: HashMap<CognitiveState, TopologyThreshold>,
}

/// Threshold ranges for topology metrics associated with a cognitive state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyThreshold {
    /// Expected range for global minimum cut value.
    pub mincut_range: (f64, f64),
    /// Expected range for modularity.
    pub modularity_range: (f64, f64),
    /// Expected range for global efficiency.
    pub efficiency_range: (f64, f64),
    /// Expected range for graph entropy.
    pub entropy_range: (f64, f64),
}

impl TopologyThreshold {
    /// Score how well a set of metrics matches this threshold.
    ///
    /// Returns a value in `[0, 1]` where 1.0 means all metrics fall within
    /// the expected ranges.
    fn score(&self, metrics: &TopologyMetrics) -> f64 {
        let scores = [
            range_score(metrics.global_mincut, self.mincut_range),
            range_score(metrics.modularity, self.modularity_range),
            range_score(metrics.global_efficiency, self.efficiency_range),
            range_score(metrics.graph_entropy, self.entropy_range),
        ];
        scores.iter().sum::<f64>() / scores.len() as f64
    }
}

impl ThresholdDecoder {
    /// Create a new threshold decoder with no thresholds defined.
    pub fn new() -> Self {
        Self {
            thresholds: HashMap::new(),
        }
    }

    /// Set the threshold for a specific cognitive state.
    pub fn set_threshold(&mut self, state: CognitiveState, threshold: TopologyThreshold) {
        self.thresholds.insert(state, threshold);
    }

    /// Learn thresholds from labeled topology data.
    ///
    /// For each cognitive state present in the data, computes the min/max
    /// range of each metric with a 10% margin.
    pub fn learn_thresholds(&mut self, labeled_data: &[(TopologyMetrics, CognitiveState)]) {
        // Group metrics by state.
        let mut grouped: HashMap<CognitiveState, Vec<&TopologyMetrics>> = HashMap::new();
        for (metrics, state) in labeled_data {
            grouped.entry(*state).or_default().push(metrics);
        }

        for (state, metrics_vec) in grouped {
            if metrics_vec.is_empty() {
                continue;
            }

            let mincut_range = compute_range(metrics_vec.iter().map(|m| m.global_mincut));
            let modularity_range = compute_range(metrics_vec.iter().map(|m| m.modularity));
            let efficiency_range =
                compute_range(metrics_vec.iter().map(|m| m.global_efficiency));
            let entropy_range = compute_range(metrics_vec.iter().map(|m| m.graph_entropy));

            self.thresholds.insert(
                state,
                TopologyThreshold {
                    mincut_range,
                    modularity_range,
                    efficiency_range,
                    entropy_range,
                },
            );
        }
    }

    /// Decode the cognitive state from topology metrics.
    ///
    /// Returns the best-matching state and a confidence score in `[0, 1]`.
    /// If no thresholds are defined, returns `(Unknown, 0.0)`.
    pub fn decode(&self, metrics: &TopologyMetrics) -> (CognitiveState, f64) {
        if self.thresholds.is_empty() {
            return (CognitiveState::Unknown, 0.0);
        }

        let mut best_state = CognitiveState::Unknown;
        let mut best_score = -1.0_f64;

        for (state, threshold) in &self.thresholds {
            let score = threshold.score(metrics);
            if score > best_score {
                best_score = score;
                best_state = *state;
            }
        }

        (best_state, best_score.clamp(0.0, 1.0))
    }

    /// Number of states with defined thresholds.
    pub fn num_states(&self) -> usize {
        self.thresholds.len()
    }
}

impl Default for ThresholdDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the range (min, max) from an iterator of values, with a 10% margin.
fn compute_range(values: impl Iterator<Item = f64>) -> (f64, f64) {
    let vals: Vec<f64> = values.collect();
    if vals.is_empty() {
        return (0.0, 0.0);
    }

    let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let margin = (max - min).abs() * 0.1;

    (min - margin, max + margin)
}

/// Score how well a value falls within a range.
///
/// Returns 1.0 if within range, decays toward 0.0 as the value moves
/// further outside.
fn range_score(value: f64, (lo, hi): (f64, f64)) -> f64 {
    if value >= lo && value <= hi {
        return 1.0;
    }
    let range_width = (hi - lo).abs().max(1e-10);
    if value < lo {
        let distance = lo - value;
        (-distance / range_width).exp()
    } else {
        let distance = value - hi;
        (-distance / range_width).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metrics(mincut: f64, modularity: f64, efficiency: f64, entropy: f64) -> TopologyMetrics {
        TopologyMetrics {
            global_mincut: mincut,
            modularity,
            global_efficiency: efficiency,
            local_efficiency: 0.0,
            graph_entropy: entropy,
            fiedler_value: 0.0,
            num_modules: 4,
            timestamp: 0.0,
        }
    }

    #[test]
    fn test_learn_thresholds() {
        let mut decoder = ThresholdDecoder::new();
        let data = vec![
            (make_metrics(5.0, 0.4, 0.3, 2.0), CognitiveState::Rest),
            (make_metrics(5.5, 0.45, 0.32, 2.1), CognitiveState::Rest),
            (make_metrics(5.2, 0.42, 0.31, 2.05), CognitiveState::Rest),
            (make_metrics(8.0, 0.6, 0.5, 3.0), CognitiveState::Focused),
            (make_metrics(8.5, 0.65, 0.52, 3.1), CognitiveState::Focused),
        ];

        decoder.learn_thresholds(&data);
        assert_eq!(decoder.num_states(), 2);

        // Query with Rest-like metrics.
        let (state, confidence) = decoder.decode(&make_metrics(5.1, 0.41, 0.31, 2.03));
        assert_eq!(state, CognitiveState::Rest);
        assert!(confidence > 0.5);
    }

    #[test]
    fn test_set_threshold() {
        let mut decoder = ThresholdDecoder::new();
        decoder.set_threshold(
            CognitiveState::Rest,
            TopologyThreshold {
                mincut_range: (4.0, 6.0),
                modularity_range: (0.3, 0.5),
                efficiency_range: (0.2, 0.4),
                entropy_range: (1.5, 2.5),
            },
        );

        let (state, confidence) = decoder.decode(&make_metrics(5.0, 0.4, 0.3, 2.0));
        assert_eq!(state, CognitiveState::Rest);
        assert!((confidence - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_empty_decoder_returns_unknown() {
        let decoder = ThresholdDecoder::new();
        let (state, confidence) = decoder.decode(&make_metrics(5.0, 0.4, 0.3, 2.0));
        assert_eq!(state, CognitiveState::Unknown);
        assert!((confidence - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_confidence_in_range() {
        let mut decoder = ThresholdDecoder::new();
        decoder.set_threshold(
            CognitiveState::Focused,
            TopologyThreshold {
                mincut_range: (7.0, 9.0),
                modularity_range: (0.5, 0.7),
                efficiency_range: (0.4, 0.6),
                entropy_range: (2.5, 3.5),
            },
        );
        // Query outside all ranges.
        let (_, confidence) = decoder.decode(&make_metrics(0.0, 0.0, 0.0, 0.0));
        assert!(confidence >= 0.0 && confidence <= 1.0);
    }
}
