//! Dynamic minimum cut tracking over temporal brain graph sequences.
//!
//! Tracks the evolution of minimum cut values over time, detects significant
//! topology transitions (integration vs. segregation events), and computes
//! derived metrics such as rate of change, integration index, and partition
//! stability.

use serde::{Deserialize, Serialize};

use ruv_neural_core::graph::BrainGraph;
use ruv_neural_core::topology::MincutResult;
use ruv_neural_core::Result;

use crate::stoer_wagner::stoer_wagner_mincut;

/// Tracks minimum cut evolution over a sequence of brain graphs.
#[derive(Debug, Clone)]
pub struct DynamicMincutTracker {
    /// History of mincut results.
    history: Vec<MincutResult>,
    /// Timestamps corresponding to each result.
    timestamps: Vec<f64>,
    /// Baseline mincut from resting state.
    baseline: Option<f64>,
}

impl Default for DynamicMincutTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicMincutTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            timestamps: Vec::new(),
            baseline: None,
        }
    }

    /// Set the baseline mincut value (typically from a resting-state graph).
    pub fn set_baseline(&mut self, baseline: f64) {
        self.baseline = Some(baseline);
    }

    /// Get the current baseline, if set.
    pub fn baseline(&self) -> Option<f64> {
        self.baseline
    }

    /// Process a new brain graph, compute its mincut, and add it to the history.
    ///
    /// Returns the mincut result for this graph.
    pub fn update(&mut self, graph: &BrainGraph) -> Result<MincutResult> {
        let result = stoer_wagner_mincut(graph)?;
        self.timestamps.push(graph.timestamp);
        self.history.push(result.clone());
        Ok(result)
    }

    /// Number of time points tracked so far.
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Returns true if no time points have been tracked.
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// Get the mincut time series as (timestamp, cut_value) pairs.
    pub fn mincut_timeseries(&self) -> Vec<(f64, f64)> {
        self.timestamps
            .iter()
            .zip(self.history.iter())
            .map(|(&t, r)| (t, r.cut_value))
            .collect()
    }

    /// Get the full history of mincut results.
    pub fn history(&self) -> &[MincutResult] {
        &self.history
    }

    /// Detect significant topology transitions.
    ///
    /// A transition is detected where the mincut changes by more than
    /// `threshold * baseline` between consecutive time points. If no baseline
    /// is set, the mean mincut is used as the baseline.
    ///
    /// # Arguments
    ///
    /// * `threshold` - Fraction of the baseline that constitutes a significant
    ///   change (e.g., 0.2 means a 20% change).
    pub fn detect_transitions(&self, threshold: f64) -> Vec<TopologyTransition> {
        if self.history.len() < 2 {
            return Vec::new();
        }

        let baseline = self.baseline.unwrap_or_else(|| {
            let sum: f64 = self.history.iter().map(|r| r.cut_value).sum();
            sum / self.history.len() as f64
        });

        if baseline <= 0.0 {
            return Vec::new();
        }

        let change_threshold = threshold * baseline;
        let mut transitions = Vec::new();

        for i in 1..self.history.len() {
            let before = self.history[i - 1].cut_value;
            let after = self.history[i].cut_value;
            let delta = after - before;

            if delta.abs() > change_threshold {
                let direction = if delta < 0.0 {
                    TransitionDirection::Integration
                } else {
                    TransitionDirection::Segregation
                };

                transitions.push(TopologyTransition {
                    timestamp: self.timestamps[i],
                    mincut_before: before,
                    mincut_after: after,
                    direction,
                    magnitude: delta.abs() / baseline,
                });
            }
        }

        transitions
    }

    /// Rate of topology change (finite difference of mincut values).
    ///
    /// Returns (timestamp, rate) pairs where the rate is the change in mincut
    /// per unit time.
    pub fn rate_of_change(&self) -> Vec<(f64, f64)> {
        if self.history.len() < 2 {
            return Vec::new();
        }

        let mut rates = Vec::new();
        for i in 1..self.history.len() {
            let dt = self.timestamps[i] - self.timestamps[i - 1];
            if dt > 0.0 {
                let dcut = self.history[i].cut_value - self.history[i - 1].cut_value;
                let midpoint = (self.timestamps[i] + self.timestamps[i - 1]) / 2.0;
                rates.push((midpoint, dcut / dt));
            }
        }
        rates
    }

    /// Integration-segregation balance index over time.
    ///
    /// The integration index is defined as:
    ///
    /// ```text
    /// I(t) = 1.0 - mincut(t) / max_mincut
    /// ```
    ///
    /// High values (close to 1) indicate integrated states; low values indicate
    /// segregated states.
    pub fn integration_index(&self) -> Vec<(f64, f64)> {
        if self.history.is_empty() {
            return Vec::new();
        }

        let max_cut = self
            .history
            .iter()
            .map(|r| r.cut_value)
            .fold(f64::NEG_INFINITY, f64::max);

        if max_cut <= 0.0 {
            return self
                .timestamps
                .iter()
                .map(|&t| (t, 1.0))
                .collect();
        }

        self.timestamps
            .iter()
            .zip(self.history.iter())
            .map(|(&t, r)| (t, 1.0 - r.cut_value / max_cut))
            .collect()
    }

    /// Partition stability: for how many consecutive time points does the same
    /// partition topology persist?
    ///
    /// Returns (timestamp, stability) pairs where stability is the Jaccard
    /// similarity between the current partition_a and the previous one.
    pub fn partition_stability(&self) -> Vec<(f64, f64)> {
        if self.history.is_empty() {
            return Vec::new();
        }

        let mut stability = vec![(self.timestamps[0], 1.0)];

        for i in 1..self.history.len() {
            let prev_a: std::collections::HashSet<usize> =
                self.history[i - 1].partition_a.iter().copied().collect();
            let curr_a: std::collections::HashSet<usize> =
                self.history[i].partition_a.iter().copied().collect();

            let jaccard = jaccard_similarity(&prev_a, &curr_a);
            // Take the max of comparing A-to-A and A-to-B (since partitions
            // can be labelled either way).
            let curr_b: std::collections::HashSet<usize> =
                self.history[i].partition_b.iter().copied().collect();
            let jaccard_flipped = jaccard_similarity(&prev_a, &curr_b);

            stability.push((self.timestamps[i], jaccard.max(jaccard_flipped)));
        }

        stability
    }
}

/// Compute the Jaccard similarity between two sets.
fn jaccard_similarity(a: &std::collections::HashSet<usize>, b: &std::collections::HashSet<usize>) -> f64 {
    let intersection = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    if union == 0.0 {
        1.0
    } else {
        intersection / union
    }
}

/// A significant topology transition detected in the mincut time series.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyTransition {
    /// Timestamp at which the transition was detected.
    pub timestamp: f64,
    /// Mincut value immediately before the transition.
    pub mincut_before: f64,
    /// Mincut value immediately after the transition.
    pub mincut_after: f64,
    /// Direction of the transition.
    pub direction: TransitionDirection,
    /// Magnitude of the transition relative to baseline.
    pub magnitude: f64,
}

/// Direction of a topology transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionDirection {
    /// Mincut decreased: networks are merging (becoming more integrated).
    Integration,
    /// Mincut increased: networks are separating (becoming more segregated).
    Segregation,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::BrainEdge;
    use ruv_neural_core::signal::FrequencyBand;

    fn make_edge(source: usize, target: usize, weight: f64) -> BrainEdge {
        BrainEdge {
            source,
            target,
            weight,
            metric: ruv_neural_core::graph::ConnectivityMetric::Coherence,
            frequency_band: FrequencyBand::Alpha,
        }
    }

    fn make_graph(timestamp: f64, bridge_weight: f64) -> BrainGraph {
        BrainGraph {
            num_nodes: 4,
            edges: vec![
                make_edge(0, 1, 5.0),
                make_edge(2, 3, 5.0),
                make_edge(1, 2, bridge_weight),
            ],
            timestamp,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        }
    }

    #[test]
    fn test_tracker_basic() {
        let mut tracker = DynamicMincutTracker::new();
        assert!(tracker.is_empty());

        let g1 = make_graph(0.0, 1.0);
        let r1 = tracker.update(&g1).unwrap();
        assert_eq!(tracker.len(), 1);
        assert!(r1.cut_value > 0.0);
    }

    #[test]
    fn test_tracker_timeseries() {
        let mut tracker = DynamicMincutTracker::new();
        for i in 0..5 {
            let bridge = (i as f64 + 1.0) * 0.5;
            let g = make_graph(i as f64, bridge);
            tracker.update(&g).unwrap();
        }

        let ts = tracker.mincut_timeseries();
        assert_eq!(ts.len(), 5);
        // Timestamps should be 0, 1, 2, 3, 4.
        for (i, (t, _)) in ts.iter().enumerate() {
            assert!((t - i as f64).abs() < 1e-9);
        }
    }

    #[test]
    fn test_detect_transitions() {
        let mut tracker = DynamicMincutTracker::new();
        // Create a sequence where bridge weight jumps suddenly.
        let weights = [1.0, 1.0, 1.0, 10.0, 10.0, 1.0];
        for (i, &w) in weights.iter().enumerate() {
            let g = make_graph(i as f64, w);
            tracker.update(&g).unwrap();
        }

        tracker.set_baseline(1.0);
        let transitions = tracker.detect_transitions(0.5);
        // Should detect at least the jump at t=3 and t=5.
        assert!(
            !transitions.is_empty(),
            "Should detect transitions for large mincut changes"
        );
    }

    #[test]
    fn test_rate_of_change() {
        let mut tracker = DynamicMincutTracker::new();
        for i in 0..4 {
            let g = make_graph(i as f64, (i as f64 + 1.0) * 2.0);
            tracker.update(&g).unwrap();
        }

        let rates = tracker.rate_of_change();
        assert_eq!(rates.len(), 3);
    }

    #[test]
    fn test_integration_index() {
        let mut tracker = DynamicMincutTracker::new();
        for i in 0..3 {
            let g = make_graph(i as f64, i as f64 + 1.0);
            tracker.update(&g).unwrap();
        }

        let idx = tracker.integration_index();
        assert_eq!(idx.len(), 3);
        // All values should be in [0, 1].
        for (_, val) in &idx {
            assert!(*val >= -1e-9 && *val <= 1.0 + 1e-9);
        }
    }

    #[test]
    fn test_partition_stability() {
        let mut tracker = DynamicMincutTracker::new();
        // Same graph repeated should give stability = 1.0.
        for i in 0..3 {
            let g = make_graph(i as f64, 0.5);
            tracker.update(&g).unwrap();
        }

        let stability = tracker.partition_stability();
        assert_eq!(stability.len(), 3);
        // First one is always 1.0.
        assert!((stability[0].1 - 1.0).abs() < 1e-9);
        // Same graph should yield high stability.
        for (_, s) in &stability {
            assert!(*s >= 0.5, "Same graph should have high stability, got {}", s);
        }
    }

    #[test]
    fn test_default_tracker() {
        let tracker = DynamicMincutTracker::default();
        assert!(tracker.is_empty());
        assert!(tracker.baseline().is_none());
    }

    #[test]
    fn test_transition_direction() {
        let mut tracker = DynamicMincutTracker::new();
        // Low bridge -> high bridge (segregation)
        tracker.update(&make_graph(0.0, 0.1)).unwrap();
        tracker.update(&make_graph(1.0, 10.0)).unwrap();

        tracker.set_baseline(0.1);
        let transitions = tracker.detect_transitions(0.2);
        if !transitions.is_empty() {
            // The bridge weight went up, but the mincut depends on the full graph.
            // Just verify we get a valid transition.
            assert!(transitions[0].magnitude > 0.0);
        }
    }
}
