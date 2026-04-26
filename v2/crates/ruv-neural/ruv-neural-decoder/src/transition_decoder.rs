//! Transition decoder for detecting cognitive state changes from topology dynamics.

use std::collections::HashMap;

use ruv_neural_core::topology::{CognitiveState, TopologyMetrics};
use serde::{Deserialize, Serialize};

/// Detect cognitive state transitions from topology change patterns.
///
/// Monitors a sliding window of topology metrics and compares observed
/// deltas against registered transition patterns to detect state changes.
pub struct TransitionDecoder {
    current_state: CognitiveState,
    transition_patterns: HashMap<(CognitiveState, CognitiveState), TransitionPattern>,
    history: Vec<TopologyMetrics>,
    window_size: usize,
}

/// A pattern describing the expected topology change during a state transition.
#[derive(Debug, Clone)]
pub struct TransitionPattern {
    /// Expected change in global minimum cut value.
    pub mincut_delta: f64,
    /// Expected change in modularity.
    pub modularity_delta: f64,
    /// Expected duration of the transition in seconds.
    pub duration_s: f64,
}

/// A detected state transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    /// State before the transition.
    pub from: CognitiveState,
    /// State after the transition.
    pub to: CognitiveState,
    /// Confidence of the detection in `[0, 1]`.
    pub confidence: f64,
    /// Timestamp when the transition was detected.
    pub timestamp: f64,
}

impl TransitionDecoder {
    /// Create a new transition decoder with a given sliding window size.
    ///
    /// The window size determines how many recent topology snapshots are
    /// retained for computing deltas.
    pub fn new(window_size: usize) -> Self {
        let window_size = if window_size < 2 { 2 } else { window_size };
        Self {
            current_state: CognitiveState::Unknown,
            transition_patterns: HashMap::new(),
            history: Vec::new(),
            window_size,
        }
    }

    /// Register a transition pattern between two states.
    pub fn register_pattern(
        &mut self,
        from: CognitiveState,
        to: CognitiveState,
        pattern: TransitionPattern,
    ) {
        self.transition_patterns.insert((from, to), pattern);
    }

    /// Get the current estimated cognitive state.
    pub fn current_state(&self) -> CognitiveState {
        self.current_state
    }

    /// Set the current state explicitly (e.g., from an external decoder).
    pub fn set_current_state(&mut self, state: CognitiveState) {
        self.current_state = state;
    }

    /// Push a new topology snapshot and check for state transitions.
    ///
    /// Returns `Some(StateTransition)` if a transition is detected,
    /// `None` otherwise.
    pub fn update(&mut self, metrics: TopologyMetrics) -> Option<StateTransition> {
        self.history.push(metrics);

        // Trim history to window size.
        if self.history.len() > self.window_size {
            let excess = self.history.len() - self.window_size;
            self.history.drain(..excess);
        }

        // Need at least 2 samples to compute deltas.
        if self.history.len() < 2 {
            return None;
        }

        let oldest = &self.history[0];
        let newest = self.history.last().unwrap();

        let observed_mincut_delta = newest.global_mincut - oldest.global_mincut;
        let observed_modularity_delta = newest.modularity - oldest.modularity;
        let observed_duration = newest.timestamp - oldest.timestamp;

        // Score each registered pattern.
        let mut best_match: Option<(CognitiveState, f64)> = None;

        for (&(from, to), pattern) in &self.transition_patterns {
            // Only consider patterns starting from the current state.
            if from != self.current_state {
                continue;
            }

            let score = pattern_match_score(
                observed_mincut_delta,
                observed_modularity_delta,
                observed_duration,
                pattern,
            );

            if score > 0.5 {
                if let Some((_, best_score)) = &best_match {
                    if score > *best_score {
                        best_match = Some((to, score));
                    }
                } else {
                    best_match = Some((to, score));
                }
            }
        }

        if let Some((to_state, confidence)) = best_match {
            let transition = StateTransition {
                from: self.current_state,
                to: to_state,
                confidence: confidence.clamp(0.0, 1.0),
                timestamp: newest.timestamp,
            };
            self.current_state = to_state;
            Some(transition)
        } else {
            None
        }
    }

    /// Number of registered transition patterns.
    pub fn num_patterns(&self) -> usize {
        self.transition_patterns.len()
    }

    /// Number of topology snapshots in the history buffer.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }
}

/// Compute a similarity score between observed deltas and a transition pattern.
///
/// Returns a value in `[0, 1]` where 1.0 means a perfect match.
fn pattern_match_score(
    observed_mincut_delta: f64,
    observed_modularity_delta: f64,
    observed_duration: f64,
    pattern: &TransitionPattern,
) -> f64 {
    let mincut_score = if pattern.mincut_delta.abs() < 1e-10 {
        if observed_mincut_delta.abs() < 0.5 {
            1.0
        } else {
            0.5
        }
    } else {
        let ratio = observed_mincut_delta / pattern.mincut_delta;
        gaussian_score(ratio, 1.0, 0.5)
    };

    let modularity_score = if pattern.modularity_delta.abs() < 1e-10 {
        if observed_modularity_delta.abs() < 0.05 {
            1.0
        } else {
            0.5
        }
    } else {
        let ratio = observed_modularity_delta / pattern.modularity_delta;
        gaussian_score(ratio, 1.0, 0.5)
    };

    let duration_score = if pattern.duration_s.abs() < 1e-10 {
        1.0
    } else {
        let ratio = observed_duration / pattern.duration_s;
        gaussian_score(ratio, 1.0, 0.5)
    };

    (mincut_score + modularity_score + duration_score) / 3.0
}

/// Gaussian-shaped score centered at `center` with width `sigma`.
fn gaussian_score(value: f64, center: f64, sigma: f64) -> f64 {
    let diff = value - center;
    (-0.5 * (diff / sigma).powi(2)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metrics(
        mincut: f64,
        modularity: f64,
        timestamp: f64,
    ) -> TopologyMetrics {
        TopologyMetrics {
            global_mincut: mincut,
            modularity,
            global_efficiency: 0.3,
            local_efficiency: 0.0,
            graph_entropy: 2.0,
            fiedler_value: 0.0,
            num_modules: 4,
            timestamp,
        }
    }

    #[test]
    fn test_detect_state_transition() {
        let mut decoder = TransitionDecoder::new(5);
        decoder.set_current_state(CognitiveState::Rest);

        // Register a pattern: Rest -> Focused causes mincut increase and modularity increase.
        decoder.register_pattern(
            CognitiveState::Rest,
            CognitiveState::Focused,
            TransitionPattern {
                mincut_delta: 3.0,
                modularity_delta: 0.2,
                duration_s: 2.0,
            },
        );

        // Feed metrics that progressively match the pattern.
        // The transition may fire on any update once deltas are large enough.
        let updates = vec![
            make_metrics(5.0, 0.4, 0.0),
            make_metrics(6.0, 0.45, 0.5),
            make_metrics(7.0, 0.5, 1.0),
            make_metrics(8.0, 0.6, 2.0),
        ];

        let mut detected: Option<StateTransition> = None;
        for m in updates {
            if let Some(t) = decoder.update(m) {
                detected = Some(t);
            }
        }

        assert!(detected.is_some(), "Expected a transition to be detected");
        let transition = detected.unwrap();
        assert_eq!(transition.from, CognitiveState::Rest);
        assert_eq!(transition.to, CognitiveState::Focused);
        assert!(transition.confidence > 0.0 && transition.confidence <= 1.0);
    }

    #[test]
    fn test_no_transition_without_pattern() {
        let mut decoder = TransitionDecoder::new(3);
        decoder.set_current_state(CognitiveState::Rest);

        let result = decoder.update(make_metrics(5.0, 0.4, 0.0));
        assert!(result.is_none());
        let result = decoder.update(make_metrics(8.0, 0.6, 2.0));
        assert!(result.is_none());
    }

    #[test]
    fn test_window_trimming() {
        let mut decoder = TransitionDecoder::new(3);
        for i in 0..10 {
            decoder.update(make_metrics(5.0, 0.4, i as f64));
        }
        assert_eq!(decoder.history_len(), 3);
    }

    #[test]
    fn test_single_sample_no_transition() {
        let mut decoder = TransitionDecoder::new(5);
        decoder.register_pattern(
            CognitiveState::Rest,
            CognitiveState::Focused,
            TransitionPattern {
                mincut_delta: 3.0,
                modularity_delta: 0.2,
                duration_s: 2.0,
            },
        );
        decoder.set_current_state(CognitiveState::Rest);
        let result = decoder.update(make_metrics(5.0, 0.4, 0.0));
        assert!(result.is_none());
    }
}
