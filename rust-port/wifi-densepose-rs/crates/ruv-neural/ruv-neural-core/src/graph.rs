//! Brain connectivity graph types.

use serde::{Deserialize, Serialize};

use crate::brain::Atlas;
use crate::error::{Result, RuvNeuralError};
use crate::signal::FrequencyBand;

/// Connectivity metric used to compute edge weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectivityMetric {
    /// Phase locking value.
    PhaseLockingValue,
    /// Amplitude envelope correlation.
    AmplitudeEnvelopeCorrelation,
    /// Weighted phase lag index.
    WeightedPhaseLagIndex,
    /// Coherence.
    Coherence,
    /// Granger causality.
    GrangerCausality,
    /// Transfer entropy.
    TransferEntropy,
    /// Mutual information.
    MutualInformation,
}

/// An edge in the brain connectivity graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainEdge {
    /// Source node index.
    pub source: usize,
    /// Target node index.
    pub target: usize,
    /// Edge weight (connectivity strength).
    pub weight: f64,
    /// Metric used to compute this edge.
    pub metric: ConnectivityMetric,
    /// Frequency band for this connectivity estimate.
    pub frequency_band: FrequencyBand,
}

/// Brain connectivity graph at a single time window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainGraph {
    /// Number of nodes (brain regions).
    pub num_nodes: usize,
    /// Edges with connectivity weights.
    pub edges: Vec<BrainEdge>,
    /// Timestamp of this graph window (Unix time).
    pub timestamp: f64,
    /// Duration of the analysis window in seconds.
    pub window_duration_s: f64,
    /// Atlas used for parcellation.
    pub atlas: Atlas,
}

impl BrainGraph {
    /// Validate graph integrity: edge bounds, weight finiteness, no self-loops.
    pub fn validate(&self) -> Result<()> {
        for (i, edge) in self.edges.iter().enumerate() {
            if edge.source >= self.num_nodes {
                return Err(RuvNeuralError::Graph(format!(
                    "Edge {i}: source {} out of bounds (num_nodes={})",
                    edge.source, self.num_nodes
                )));
            }
            if edge.target >= self.num_nodes {
                return Err(RuvNeuralError::Graph(format!(
                    "Edge {i}: target {} out of bounds (num_nodes={})",
                    edge.target, self.num_nodes
                )));
            }
            if edge.source == edge.target {
                return Err(RuvNeuralError::Graph(format!(
                    "Edge {i}: self-loop on node {}",
                    edge.source
                )));
            }
            if !edge.weight.is_finite() {
                return Err(RuvNeuralError::Graph(format!(
                    "Edge {i}: non-finite weight {}",
                    edge.weight
                )));
            }
        }
        Ok(())
    }

    /// Build a dense adjacency matrix (num_nodes x num_nodes).
    /// For duplicate edges, the last one wins.
    pub fn adjacency_matrix(&self) -> Vec<Vec<f64>> {
        let n = self.num_nodes;
        let mut mat = vec![vec![0.0; n]; n];
        for edge in &self.edges {
            if edge.source < n && edge.target < n {
                mat[edge.source][edge.target] = edge.weight;
                mat[edge.target][edge.source] = edge.weight;
            }
        }
        mat
    }

    /// Get the weight of the edge between source and target, if it exists.
    pub fn edge_weight(&self, source: usize, target: usize) -> Option<f64> {
        self.edges
            .iter()
            .find(|e| {
                (e.source == source && e.target == target)
                    || (e.source == target && e.target == source)
            })
            .map(|e| e.weight)
    }

    /// Weighted degree of a node (sum of incident edge weights).
    pub fn node_degree(&self, node: usize) -> f64 {
        self.edges
            .iter()
            .filter(|e| e.source == node || e.target == node)
            .map(|e| e.weight)
            .sum()
    }

    /// Graph density: ratio of actual edges to possible edges.
    pub fn density(&self) -> f64 {
        if self.num_nodes < 2 {
            return 0.0;
        }
        let max_edges = self.num_nodes * (self.num_nodes - 1) / 2;
        if max_edges == 0 {
            return 0.0;
        }
        self.edges.len() as f64 / max_edges as f64
    }

    /// Total weight of all edges.
    pub fn total_weight(&self) -> f64 {
        self.edges.iter().map(|e| e.weight).sum()
    }
}

/// Temporal sequence of brain graphs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainGraphSequence {
    /// Ordered sequence of graphs.
    pub graphs: Vec<BrainGraph>,
    /// Step between successive windows in seconds.
    pub window_step_s: f64,
}

impl BrainGraphSequence {
    /// Number of time points.
    pub fn len(&self) -> usize {
        self.graphs.len()
    }

    /// Returns true if the sequence is empty.
    pub fn is_empty(&self) -> bool {
        self.graphs.is_empty()
    }

    /// Total duration covered by the sequence in seconds.
    pub fn duration_s(&self) -> f64 {
        if self.graphs.is_empty() {
            return 0.0;
        }
        let first = self.graphs.first().unwrap();
        let last = self.graphs.last().unwrap();
        (last.timestamp - first.timestamp) + last.window_duration_s
    }
}
