//! Topology analysis result types (mincut, partition, metrics).

use serde::{Deserialize, Serialize};

/// Result of a minimum cut computation on a brain graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MincutResult {
    /// Value of the minimum cut.
    pub cut_value: f64,
    /// Node indices in partition A.
    pub partition_a: Vec<usize>,
    /// Node indices in partition B.
    pub partition_b: Vec<usize>,
    /// Cut edges: (source, target, weight).
    pub cut_edges: Vec<(usize, usize, f64)>,
    /// Timestamp of the source graph.
    pub timestamp: f64,
}

impl MincutResult {
    /// Total number of nodes across both partitions.
    pub fn num_nodes(&self) -> usize {
        self.partition_a.len() + self.partition_b.len()
    }

    /// Number of edges crossing the cut.
    pub fn num_cut_edges(&self) -> usize {
        self.cut_edges.len()
    }

    /// Balance ratio: min(|A|, |B|) / max(|A|, |B|).
    pub fn balance_ratio(&self) -> f64 {
        let a = self.partition_a.len() as f64;
        let b = self.partition_b.len() as f64;
        if a == 0.0 || b == 0.0 {
            return 0.0;
        }
        a.min(b) / a.max(b)
    }
}

/// Multi-way partition result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiPartition {
    /// Each inner vec is a set of node indices forming one partition.
    pub partitions: Vec<Vec<usize>>,
    /// Total cut value.
    pub cut_value: f64,
    /// Newman-Girvan modularity score.
    pub modularity: f64,
}

impl MultiPartition {
    /// Number of partitions (modules).
    pub fn num_partitions(&self) -> usize {
        self.partitions.len()
    }

    /// Total number of nodes.
    pub fn num_nodes(&self) -> usize {
        self.partitions.iter().map(|p| p.len()).sum()
    }
}

/// Cognitive state derived from brain topology analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CognitiveState {
    Rest,
    Focused,
    MotorPlanning,
    SpeechProcessing,
    MemoryEncoding,
    MemoryRetrieval,
    Creative,
    Stressed,
    Fatigued,
    Sleep(SleepStage),
    Unknown,
}

/// Sleep stage classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SleepStage {
    Wake,
    N1,
    N2,
    N3,
    Rem,
}

/// Topology metrics computed from a brain graph at a single time point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyMetrics {
    /// Global minimum cut value.
    pub global_mincut: f64,
    /// Newman-Girvan modularity.
    pub modularity: f64,
    /// Global efficiency (inverse path length).
    pub global_efficiency: f64,
    /// Mean local efficiency.
    pub local_efficiency: f64,
    /// Graph entropy (edge weight distribution).
    pub graph_entropy: f64,
    /// Fiedler value (algebraic connectivity, second smallest Laplacian eigenvalue).
    pub fiedler_value: f64,
    /// Number of detected modules.
    pub num_modules: usize,
    /// Timestamp of the source graph.
    pub timestamp: f64,
}
