//! Reduced supergraph from worker tile summaries

use crate::receipt::WitnessSummary;
use crate::{TileId, WitnessFragment};
use std::collections::HashMap;

/// Reduced graph maintained by TileZero
pub struct ReducedGraph {
    /// Coherence scores per tile
    tile_coherence: HashMap<TileId, f32>,
    /// Global cut value
    global_cut_value: f64,
    /// Aggregated e-value
    aggregated_e_value: f64,
    /// Shift pressure
    shift_pressure: f64,
    /// Boundary edge count
    boundary_edges: usize,
}

impl ReducedGraph {
    /// Create a new reduced graph
    pub fn new() -> Self {
        Self {
            tile_coherence: HashMap::new(),
            global_cut_value: 100.0,   // Start with high coherence
            aggregated_e_value: 100.0, // Start with high evidence
            shift_pressure: 0.0,
            boundary_edges: 0,
        }
    }

    /// Update from a witness fragment
    pub fn update_from_fragment(&mut self, fragment: &WitnessFragment) {
        self.boundary_edges = fragment.boundary_edges.len();
        // Update global cut based on local cuts
        self.global_cut_value = self.global_cut_value.min(fragment.cut_value as f64);
    }

    /// Update coherence for a tile
    pub fn update_coherence(&mut self, tile_id: TileId, coherence: f32) {
        self.tile_coherence.insert(tile_id, coherence);

        // Recompute aggregates
        if !self.tile_coherence.is_empty() {
            let sum: f32 = self.tile_coherence.values().sum();
            let avg = sum / self.tile_coherence.len() as f32;

            // Use average coherence to influence e-value
            self.aggregated_e_value = (avg as f64) * 100.0;
        }
    }

    /// Get the global cut value
    pub fn global_cut(&self) -> f64 {
        self.global_cut_value
    }

    /// Aggregate shift pressure across tiles
    pub fn aggregate_shift_pressure(&self) -> f64 {
        self.shift_pressure
    }

    /// Aggregate evidence across tiles
    pub fn aggregate_evidence(&self) -> f64 {
        self.aggregated_e_value
    }

    /// Generate witness summary
    pub fn witness_summary(&self) -> WitnessSummary {
        use crate::receipt::{EvidentialWitness, PredictiveWitness, StructuralWitness};

        let partition = if self.global_cut_value >= 10.0 {
            "stable"
        } else if self.global_cut_value >= 5.0 {
            "marginal"
        } else {
            "fragile"
        };

        let verdict = if self.aggregated_e_value >= 100.0 {
            "accept"
        } else if self.aggregated_e_value >= 0.01 {
            "continue"
        } else {
            "reject"
        };

        WitnessSummary {
            structural: StructuralWitness {
                cut_value: self.global_cut_value,
                partition: partition.to_string(),
                critical_edges: self.boundary_edges,
                boundary: vec![],
            },
            predictive: PredictiveWitness {
                set_size: 1, // Simplified
                coverage: 0.95,
            },
            evidential: EvidentialWitness {
                e_value: self.aggregated_e_value,
                verdict: verdict.to_string(),
            },
        }
    }

    /// Set shift pressure (for testing or external updates)
    pub fn set_shift_pressure(&mut self, pressure: f64) {
        self.shift_pressure = pressure;
    }

    /// Set global cut value (for testing or external updates)
    pub fn set_global_cut(&mut self, cut: f64) {
        self.global_cut_value = cut;
    }

    /// Set aggregated evidence (for testing or external updates)
    pub fn set_evidence(&mut self, evidence: f64) {
        self.aggregated_e_value = evidence;
    }
}

impl Default for ReducedGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Structural filter for graph-based decisions
pub struct StructuralFilter {
    /// Minimum cut threshold
    min_cut: f64,
}

impl StructuralFilter {
    /// Create a new structural filter
    pub fn new(min_cut: f64) -> Self {
        Self { min_cut }
    }

    /// Evaluate if structure is stable
    pub fn is_stable(&self, graph: &ReducedGraph) -> bool {
        graph.global_cut() >= self.min_cut
    }
}

/// Shift pressure tracking
pub struct ShiftPressure {
    /// Current pressure
    current: f64,
    /// Threshold for deferral
    threshold: f64,
}

impl ShiftPressure {
    /// Create new shift pressure tracker
    pub fn new(threshold: f64) -> Self {
        Self {
            current: 0.0,
            threshold,
        }
    }

    /// Update with new observation
    pub fn update(&mut self, value: f64) {
        // Exponential moving average
        self.current = 0.9 * self.current + 0.1 * value;
    }

    /// Check if shift is detected
    pub fn is_shifting(&self) -> bool {
        self.current >= self.threshold
    }

    /// Get current pressure
    pub fn current(&self) -> f64 {
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reduced_graph() {
        let mut graph = ReducedGraph::new();
        assert!(graph.global_cut() >= 100.0);

        graph.update_coherence(1, 0.9);
        graph.update_coherence(2, 0.8);

        let summary = graph.witness_summary();
        assert_eq!(summary.structural.partition, "stable");
    }

    #[test]
    fn test_structural_filter() {
        let filter = StructuralFilter::new(5.0);
        let mut graph = ReducedGraph::new();

        assert!(filter.is_stable(&graph));

        graph.set_global_cut(3.0);
        assert!(!filter.is_stable(&graph));
    }

    #[test]
    fn test_shift_pressure() {
        let mut pressure = ShiftPressure::new(0.5);

        for _ in 0..20 {
            pressure.update(0.8);
        }

        assert!(pressure.is_shifting());
    }
}
