//! Temporal graph dynamics: tracking topology metrics over time.
//!
//! The [`TopologyTracker`] accumulates brain graphs and computes time series
//! of graph-theoretic metrics to detect state transitions and measure
//! the rate of topological change.

use ruv_neural_core::graph::BrainGraph;

use crate::metrics::{clustering_coefficient, global_efficiency};
use crate::spectral::fiedler_value;

/// A timestamped snapshot of graph topology metrics.
#[derive(Debug, Clone)]
pub struct TopologySnapshot {
    /// Timestamp of the graph.
    pub timestamp: f64,
    /// Global efficiency.
    pub global_efficiency: f64,
    /// Clustering coefficient.
    pub clustering: f64,
    /// Fiedler value (algebraic connectivity).
    pub fiedler: f64,
    /// Graph density.
    pub density: f64,
    /// Total edge weight (proxy for minimum cut in dense graphs).
    pub total_weight: f64,
}

/// Tracks graph topology metrics over time and detects transitions.
pub struct TopologyTracker {
    /// History of topology snapshots.
    history: Vec<TopologySnapshot>,
}

impl TopologyTracker {
    /// Create an empty tracker.
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }

    /// Track a new brain graph, computing and storing its topology metrics.
    pub fn track(&mut self, graph: &BrainGraph) {
        let snapshot = TopologySnapshot {
            timestamp: graph.timestamp,
            global_efficiency: global_efficiency(graph),
            clustering: clustering_coefficient(graph),
            fiedler: fiedler_value(graph),
            density: graph.density(),
            total_weight: graph.total_weight(),
        };
        self.history.push(snapshot);
    }

    /// Number of tracked time points.
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Returns true if no graphs have been tracked.
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// Get the full history of snapshots.
    pub fn snapshots(&self) -> &[TopologySnapshot] {
        &self.history
    }

    /// Return a time series of (timestamp, total_weight) as a proxy for minimum cut.
    ///
    /// The total weight correlates with overall connectivity strength.
    pub fn mincut_timeseries(&self) -> Vec<(f64, f64)> {
        self.history
            .iter()
            .map(|s| (s.timestamp, s.total_weight))
            .collect()
    }

    /// Return a time series of (timestamp, fiedler_value).
    ///
    /// The Fiedler value tracks algebraic connectivity over time.
    pub fn fiedler_timeseries(&self) -> Vec<(f64, f64)> {
        self.history
            .iter()
            .map(|s| (s.timestamp, s.fiedler))
            .collect()
    }

    /// Return a time series of (timestamp, global_efficiency).
    pub fn efficiency_timeseries(&self) -> Vec<(f64, f64)> {
        self.history
            .iter()
            .map(|s| (s.timestamp, s.global_efficiency))
            .collect()
    }

    /// Return a time series of (timestamp, clustering_coefficient).
    pub fn clustering_timeseries(&self) -> Vec<(f64, f64)> {
        self.history
            .iter()
            .map(|s| (s.timestamp, s.clustering))
            .collect()
    }

    /// Detect timestamps where significant topology changes occur.
    ///
    /// A transition is detected when the absolute change in global efficiency
    /// between consecutive snapshots exceeds the given threshold.
    pub fn detect_transitions(&self, threshold: f64) -> Vec<f64> {
        if self.history.len() < 2 {
            return Vec::new();
        }

        let mut transitions = Vec::new();
        for i in 1..self.history.len() {
            let delta = (self.history[i].global_efficiency
                - self.history[i - 1].global_efficiency)
                .abs();
            if delta > threshold {
                transitions.push(self.history[i].timestamp);
            }
        }

        transitions
    }

    /// Compute the rate of change of global efficiency over time.
    ///
    /// Returns (timestamp, d_efficiency/dt) for each consecutive pair.
    pub fn rate_of_change(&self) -> Vec<(f64, f64)> {
        if self.history.len() < 2 {
            return Vec::new();
        }

        self.history
            .windows(2)
            .map(|pair| {
                let dt = pair[1].timestamp - pair[0].timestamp;
                let de = pair[1].global_efficiency - pair[0].global_efficiency;
                let rate = if dt.abs() > 1e-15 { de / dt } else { 0.0 };
                (pair[1].timestamp, rate)
            })
            .collect()
    }
}

impl Default for TopologyTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, BrainGraph, ConnectivityMetric};
    use ruv_neural_core::signal::FrequencyBand;

    fn make_edge(s: usize, t: usize, w: f64) -> BrainEdge {
        BrainEdge {
            source: s,
            target: t,
            weight: w,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        }
    }

    fn make_graph(timestamp: f64, edges: Vec<BrainEdge>) -> BrainGraph {
        BrainGraph {
            num_nodes: 4,
            edges,
            timestamp,
            window_duration_s: 0.5,
            atlas: Atlas::Custom(4),
        }
    }

    #[test]
    fn tracker_stores_history() {
        let mut tracker = TopologyTracker::new();
        assert!(tracker.is_empty());

        let g1 = make_graph(0.0, vec![make_edge(0, 1, 1.0), make_edge(2, 3, 1.0)]);
        let g2 = make_graph(1.0, vec![
            make_edge(0, 1, 1.0),
            make_edge(1, 2, 1.0),
            make_edge(2, 3, 1.0),
        ]);

        tracker.track(&g1);
        tracker.track(&g2);

        assert_eq!(tracker.len(), 2);
        assert!(!tracker.is_empty());
    }

    #[test]
    fn mincut_timeseries_correct_length() {
        let mut tracker = TopologyTracker::new();
        for i in 0..5 {
            let g = make_graph(
                i as f64,
                vec![make_edge(0, 1, 1.0), make_edge(2, 3, i as f64 * 0.5)],
            );
            tracker.track(&g);
        }

        let ts = tracker.mincut_timeseries();
        assert_eq!(ts.len(), 5);
        assert_eq!(ts[0].0, 0.0);
        assert_eq!(ts[4].0, 4.0);
    }

    #[test]
    fn detect_transitions_returns_correct_timestamps() {
        let mut tracker = TopologyTracker::new();

        // Stable phase: few edges
        for i in 0..3 {
            let g = make_graph(
                i as f64,
                vec![make_edge(0, 1, 0.5)],
            );
            tracker.track(&g);
        }

        // Sudden change: fully connected
        let g = make_graph(3.0, vec![
            make_edge(0, 1, 1.0),
            make_edge(0, 2, 1.0),
            make_edge(0, 3, 1.0),
            make_edge(1, 2, 1.0),
            make_edge(1, 3, 1.0),
            make_edge(2, 3, 1.0),
        ]);
        tracker.track(&g);

        // With a small threshold, we should detect the transition at t=3.0
        let transitions = tracker.detect_transitions(0.01);
        assert!(
            transitions.contains(&3.0),
            "Should detect transition at t=3.0, got {:?}",
            transitions
        );
    }

    #[test]
    fn rate_of_change_correct_length() {
        let mut tracker = TopologyTracker::new();
        for i in 0..4 {
            let g = make_graph(i as f64, vec![make_edge(0, 1, 1.0)]);
            tracker.track(&g);
        }

        let roc = tracker.rate_of_change();
        assert_eq!(roc.len(), 3); // n-1 rates for n points
    }
}
