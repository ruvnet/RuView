//! Neural coherence detection via minimum cut analysis.
//!
//! Detects when brain networks become coherent (strongly coupled) or decouple,
//! by monitoring the minimum cut over a temporal graph sequence. Significant
//! changes in mincut topology correspond to network formation, dissolution,
//! merger, and split events.

use serde::{Deserialize, Serialize};

use crate::dynamic::DynamicMincutTracker;

/// Type of coherence event detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoherenceEventType {
    /// A new coherent module forms (integration event).
    NetworkFormation,
    /// A coherent module breaks apart (segregation event).
    NetworkDissolution,
    /// Two modules merge into one.
    NetworkMerger,
    /// One module splits into two.
    NetworkSplit,
}

/// A coherence event detected in the brain network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceEvent {
    /// Start time of the event.
    pub start_time: f64,
    /// End time of the event.
    pub end_time: f64,
    /// Type of coherence event.
    pub event_type: CoherenceEventType,
    /// Brain region indices involved in the event.
    pub involved_regions: Vec<usize>,
    /// Peak coherence magnitude during the event.
    pub peak_coherence: f64,
}

/// Detects coherence events in temporal brain graph sequences.
#[derive(Debug, Clone)]
pub struct CoherenceDetector {
    /// Internal tracker for mincut evolution.
    tracker: DynamicMincutTracker,
    /// Threshold (fraction of baseline) for integration detection.
    threshold_integration: f64,
    /// Threshold (fraction of baseline) for segregation detection.
    threshold_segregation: f64,
}

impl CoherenceDetector {
    /// Create a new coherence detector.
    ///
    /// # Arguments
    ///
    /// * `threshold_integration` - Fraction of baseline for integration detection
    ///   (e.g., 0.3 means a 30% decrease in mincut triggers an integration event).
    /// * `threshold_segregation` - Fraction of baseline for segregation detection.
    pub fn new(threshold_integration: f64, threshold_segregation: f64) -> Self {
        Self {
            tracker: DynamicMincutTracker::new(),
            threshold_integration,
            threshold_segregation,
        }
    }

    /// Set the baseline mincut value from resting-state data.
    pub fn set_baseline(&mut self, baseline: f64) {
        self.tracker.set_baseline(baseline);
    }

    /// Get a reference to the internal tracker.
    pub fn tracker(&self) -> &DynamicMincutTracker {
        &self.tracker
    }

    /// Detect coherence events from a mincut time series.
    ///
    /// Processes each `(timestamp, mincut_value)` pair, detects transitions,
    /// and classifies them into coherence events.
    pub fn detect_from_timeseries(
        &self,
        mincut_series: &[(f64, f64)],
    ) -> Vec<CoherenceEvent> {
        if mincut_series.len() < 2 {
            return Vec::new();
        }

        // Compute baseline as mean if not set.
        let baseline = self.tracker.baseline().unwrap_or_else(|| {
            let sum: f64 = mincut_series.iter().map(|(_, v)| v).sum();
            sum / mincut_series.len() as f64
        });

        if baseline <= 0.0 {
            return Vec::new();
        }

        let threshold = self.threshold_integration.min(self.threshold_segregation);
        let change_threshold = threshold * baseline;

        let mut events = Vec::new();
        let mut i = 1;

        while i < mincut_series.len() {
            let (_t_prev, v_prev) = mincut_series[i - 1];
            let (t_curr, v_curr) = mincut_series[i];
            let delta = v_curr - v_prev;

            if delta.abs() > change_threshold {
                let magnitude = delta.abs() / baseline;

                if delta < 0.0 && magnitude >= self.threshold_integration {
                    // Integration: mincut decreased -> networks merging.
                    let end_time =
                        find_recovery_time_in_series(mincut_series, i, v_prev, baseline);

                    events.push(CoherenceEvent {
                        start_time: t_curr,
                        end_time,
                        event_type: CoherenceEventType::NetworkFormation,
                        involved_regions: Vec::new(),
                        peak_coherence: magnitude,
                    });
                } else if delta > 0.0 && magnitude >= self.threshold_segregation {
                    // Segregation: mincut increased -> networks separating.
                    let end_time =
                        find_recovery_time_in_series(mincut_series, i, v_prev, baseline);

                    events.push(CoherenceEvent {
                        start_time: t_curr,
                        end_time,
                        event_type: CoherenceEventType::NetworkDissolution,
                        involved_regions: Vec::new(),
                        peak_coherence: magnitude,
                    });
                }

                // Check for merger/split patterns (opposing transitions close together).
                if i + 1 < mincut_series.len() {
                    let (t_next, v_next) = mincut_series[i + 1];
                    let dt = t_next - t_curr;
                    let delta_next = v_next - v_curr;

                    if dt < 2.0 && delta_next.abs() > change_threshold {
                        if delta < 0.0 && delta_next > 0.0 {
                            events.push(CoherenceEvent {
                                start_time: t_curr,
                                end_time: t_next,
                                event_type: CoherenceEventType::NetworkSplit,
                                involved_regions: Vec::new(),
                                peak_coherence: magnitude.max(delta_next.abs() / baseline),
                            });
                            i += 1;
                        } else if delta > 0.0 && delta_next < 0.0 {
                            events.push(CoherenceEvent {
                                start_time: t_curr,
                                end_time: t_next,
                                event_type: CoherenceEventType::NetworkMerger,
                                involved_regions: Vec::new(),
                                peak_coherence: magnitude.max(delta_next.abs() / baseline),
                            });
                            i += 1;
                        }
                    }
                }
            }

            i += 1;
        }

        events
    }

    /// Detect coherence events by processing a brain graph sequence.
    ///
    /// Updates the internal tracker with each graph and then analyzes the
    /// resulting mincut time series.
    pub fn detect_coherence_events(
        &mut self,
        sequence: &ruv_neural_core::graph::BrainGraphSequence,
    ) -> ruv_neural_core::Result<Vec<CoherenceEvent>> {
        for graph in &sequence.graphs {
            self.tracker.update(graph)?;
        }

        let timeseries = self.tracker.mincut_timeseries();
        Ok(self.detect_from_timeseries(&timeseries))
    }
}

/// Find the time when the mincut recovers to near the original value.
fn find_recovery_time_in_series(
    series: &[(f64, f64)],
    start_idx: usize,
    original_value: f64,
    baseline: f64,
) -> f64 {
    let recovery_threshold = 0.1 * baseline;

    for &(t, v) in series.iter().skip(start_idx + 1) {
        if (v - original_value).abs() < recovery_threshold {
            return t;
        }
    }

    // No recovery found; return last timestamp.
    series.last().map_or(series[start_idx].0, |&(t, _)| t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coherence_event_types_serialization() {
        for event_type in [
            CoherenceEventType::NetworkFormation,
            CoherenceEventType::NetworkDissolution,
            CoherenceEventType::NetworkMerger,
            CoherenceEventType::NetworkSplit,
        ] {
            let json = serde_json::to_string(&event_type).unwrap();
            let back: CoherenceEventType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, event_type);
        }
    }

    #[test]
    fn test_coherence_event_serialization() {
        let event = CoherenceEvent {
            start_time: 0.0,
            end_time: 1.0,
            event_type: CoherenceEventType::NetworkFormation,
            involved_regions: vec![0, 1, 2],
            peak_coherence: 0.8,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: CoherenceEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.event_type, CoherenceEventType::NetworkFormation);
        assert!((back.peak_coherence - 0.8).abs() < 1e-9);
    }

    #[test]
    fn test_detect_no_events_for_constant_series() {
        let detector = CoherenceDetector::new(0.3, 0.3);
        let series: Vec<(f64, f64)> = (0..10)
            .map(|i| (i as f64, 5.0))
            .collect();
        let events = detector.detect_from_timeseries(&series);
        assert!(events.is_empty());
    }

    #[test]
    fn test_detect_formation_event() {
        let mut detector = CoherenceDetector::new(0.2, 0.2);
        detector.set_baseline(5.0);

        // Constant, then a sudden drop in mincut (integration).
        let series = vec![
            (0.0, 5.0),
            (1.0, 5.0),
            (2.0, 5.0),
            (3.0, 1.0), // big drop
            (4.0, 1.0),
            (5.0, 5.0), // recovery
        ];

        let events = detector.detect_from_timeseries(&series);
        assert!(
            !events.is_empty(),
            "Should detect a formation event from a large mincut decrease"
        );
        // First event should be a formation (integration).
        assert_eq!(events[0].event_type, CoherenceEventType::NetworkFormation);
    }

    #[test]
    fn test_detect_dissolution_event() {
        let mut detector = CoherenceDetector::new(0.2, 0.2);
        detector.set_baseline(5.0);

        // Sudden increase in mincut (segregation).
        let series = vec![
            (0.0, 5.0),
            (1.0, 5.0),
            (2.0, 15.0), // big jump
            (3.0, 15.0),
        ];

        let events = detector.detect_from_timeseries(&series);
        let dissolution_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == CoherenceEventType::NetworkDissolution)
            .collect();
        assert!(
            !dissolution_events.is_empty(),
            "Should detect a dissolution event from a large mincut increase"
        );
    }

    #[test]
    fn test_detector_empty_series() {
        let detector = CoherenceDetector::new(0.3, 0.3);
        let events = detector.detect_from_timeseries(&[]);
        assert!(events.is_empty());
    }

    #[test]
    fn test_detector_single_point() {
        let detector = CoherenceDetector::new(0.3, 0.3);
        let events = detector.detect_from_timeseries(&[(0.0, 5.0)]);
        assert!(events.is_empty());
    }
}
