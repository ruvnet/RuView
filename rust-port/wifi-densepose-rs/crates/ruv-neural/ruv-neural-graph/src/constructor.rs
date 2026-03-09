//! Graph construction from connectivity matrices and multi-channel time series.
//!
//! The [`BrainGraphConstructor`] converts pairwise connectivity values into
//! [`BrainGraph`] instances, with optional thresholding to remove weak edges.
//! It also supports sliding-window construction from raw time series via the
//! signal crate's connectivity metrics.

use ruv_neural_core::brain::Parcellation;
use ruv_neural_core::error::{Result, RuvNeuralError};
use ruv_neural_core::graph::{BrainEdge, BrainGraph, BrainGraphSequence, ConnectivityMetric};
use ruv_neural_core::signal::{FrequencyBand, MultiChannelTimeSeries};
use ruv_neural_core::traits::GraphConstructor;

use crate::atlas::{AtlasType, load_atlas};

/// Constructs brain connectivity graphs from matrices or time series data.
pub struct BrainGraphConstructor {
    parcellation: Parcellation,
    metric: ConnectivityMetric,
    band: FrequencyBand,
    /// Edge weight threshold: edges below this value are dropped.
    threshold: f64,
    /// Sliding window duration in seconds.
    window_duration_s: f64,
    /// Sliding window step in seconds.
    window_step_s: f64,
}

impl BrainGraphConstructor {
    /// Create a new constructor with default window parameters.
    pub fn new(atlas: AtlasType, metric: ConnectivityMetric, band: FrequencyBand) -> Self {
        Self {
            parcellation: load_atlas(atlas),
            metric,
            band,
            threshold: 0.0,
            window_duration_s: 1.0,
            window_step_s: 0.5,
        }
    }

    /// Set the edge weight threshold. Edges with weight below this are excluded.
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Set the sliding window duration in seconds.
    pub fn with_window_duration(mut self, duration_s: f64) -> Self {
        self.window_duration_s = duration_s;
        self
    }

    /// Set the sliding window step in seconds.
    pub fn with_window_step(mut self, step_s: f64) -> Self {
        self.window_step_s = step_s;
        self
    }

    /// Construct a brain graph from a pre-computed connectivity matrix.
    ///
    /// The matrix should be `n x n` where `n` matches the number of atlas regions.
    /// The matrix is treated as symmetric; only the upper triangle is read.
    pub fn construct_from_matrix(
        &self,
        connectivity: &[Vec<f64>],
        timestamp: f64,
    ) -> BrainGraph {
        let n = self.parcellation.num_regions();
        let mut edges = Vec::new();

        for i in 0..n.min(connectivity.len()) {
            for j in (i + 1)..n.min(connectivity[i].len()) {
                let weight = connectivity[i][j];
                if weight.abs() > self.threshold {
                    edges.push(BrainEdge {
                        source: i,
                        target: j,
                        weight,
                        metric: self.metric,
                        frequency_band: self.band,
                    });
                }
            }
        }

        BrainGraph {
            num_nodes: n,
            edges,
            timestamp,
            window_duration_s: self.window_duration_s,
            atlas: self.parcellation.atlas,
        }
    }

    /// Construct a sequence of brain graphs from multi-channel time series
    /// using a sliding window approach.
    ///
    /// For each window, computes pairwise Pearson correlation as connectivity,
    /// then builds a graph with thresholding applied.
    pub fn construct_sequence(
        &self,
        data: &MultiChannelTimeSeries,
    ) -> BrainGraphSequence {
        let n_samples = data.num_samples;
        let sr = data.sample_rate_hz;

        let window_samples = (self.window_duration_s * sr) as usize;
        let step_samples = (self.window_step_s * sr) as usize;

        if window_samples == 0 || step_samples == 0 || n_samples < window_samples {
            return BrainGraphSequence {
                graphs: Vec::new(),
                window_step_s: self.window_step_s,
            };
        }

        let mut graphs = Vec::new();
        let mut offset = 0;

        while offset + window_samples <= n_samples {
            let timestamp = data.timestamp_start + offset as f64 / sr;

            // Extract windowed data for each channel
            let windowed: Vec<&[f64]> = data
                .data
                .iter()
                .map(|ch| &ch[offset..offset + window_samples])
                .collect();

            // Compute pairwise Pearson correlation matrix
            let connectivity = compute_correlation_matrix(&windowed);

            let graph = self.construct_from_matrix(&connectivity, timestamp);
            graphs.push(graph);

            offset += step_samples;
        }

        BrainGraphSequence {
            graphs,
            window_step_s: self.window_step_s,
        }
    }
}

impl GraphConstructor for BrainGraphConstructor {
    fn construct(&self, signals: &MultiChannelTimeSeries) -> Result<BrainGraph> {
        let n_channels = signals.num_channels;
        let expected = self.parcellation.num_regions();
        if n_channels != expected {
            return Err(RuvNeuralError::DimensionMismatch {
                expected,
                got: n_channels,
            });
        }

        let windowed: Vec<&[f64]> = signals.data.iter().map(|ch| ch.as_slice()).collect();
        let connectivity = compute_correlation_matrix(&windowed);
        Ok(self.construct_from_matrix(&connectivity, signals.timestamp_start))
    }
}

/// Compute pairwise Pearson correlation matrix for a set of channels.
fn compute_correlation_matrix(channels: &[&[f64]]) -> Vec<Vec<f64>> {
    let n = channels.len();
    let mut matrix = vec![vec![0.0; n]; n];

    // Pre-compute means and standard deviations
    let stats: Vec<(f64, f64)> = channels
        .iter()
        .map(|ch| {
            let len = ch.len() as f64;
            if len == 0.0 {
                return (0.0, 0.0);
            }
            let mean = ch.iter().sum::<f64>() / len;
            let var = ch.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / len;
            (mean, var.sqrt())
        })
        .collect();

    for i in 0..n {
        matrix[i][i] = 1.0;
        for j in (i + 1)..n {
            let (mean_i, std_i) = stats[i];
            let (mean_j, std_j) = stats[j];

            if std_i == 0.0 || std_j == 0.0 {
                matrix[i][j] = 0.0;
                matrix[j][i] = 0.0;
                continue;
            }

            let len = channels[i].len().min(channels[j].len());
            let cov: f64 = channels[i][..len]
                .iter()
                .zip(channels[j][..len].iter())
                .map(|(a, b)| (a - mean_i) * (b - mean_j))
                .sum::<f64>()
                / len as f64;

            let r = cov / (std_i * std_j);
            matrix[i][j] = r;
            matrix[j][i] = r;
        }
    }

    matrix
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::graph::ConnectivityMetric;
    use ruv_neural_core::signal::FrequencyBand;

    fn make_constructor() -> BrainGraphConstructor {
        BrainGraphConstructor::new(
            AtlasType::DesikanKilliany,
            ConnectivityMetric::PhaseLockingValue,
            FrequencyBand::Alpha,
        )
    }

    #[test]
    fn identity_matrix_fully_disconnected() {
        let ctor = make_constructor().with_threshold(0.01);
        let n = 68;
        // Identity matrix: diagonal = 1, off-diagonal = 0
        let identity: Vec<Vec<f64>> = (0..n)
            .map(|i| {
                let mut row = vec![0.0; n];
                row[i] = 1.0;
                row
            })
            .collect();

        let graph = ctor.construct_from_matrix(&identity, 0.0);
        assert_eq!(graph.num_nodes, 68);
        assert_eq!(graph.edges.len(), 0, "Identity matrix should produce no edges");
    }

    #[test]
    fn ones_matrix_fully_connected() {
        let ctor = make_constructor().with_threshold(0.01);
        let n = 68;
        let ones: Vec<Vec<f64>> = vec![vec![1.0; n]; n];

        let graph = ctor.construct_from_matrix(&ones, 0.0);
        let expected_edges = n * (n - 1) / 2;
        assert_eq!(graph.edges.len(), expected_edges);
    }

    #[test]
    fn threshold_filters_weak_edges() {
        let ctor = make_constructor().with_threshold(0.5);
        let n = 68;
        let mut matrix = vec![vec![0.0; n]; n];
        // Set a few strong edges
        matrix[0][1] = 0.8;
        matrix[1][0] = 0.8;
        // Set a weak edge
        matrix[2][3] = 0.3;
        matrix[3][2] = 0.3;

        let graph = ctor.construct_from_matrix(&matrix, 0.0);
        assert_eq!(graph.edges.len(), 1, "Only edge above threshold should survive");
        assert_eq!(graph.edges[0].source, 0);
        assert_eq!(graph.edges[0].target, 1);
    }

    #[test]
    fn construct_sequence_produces_graphs() {
        let ctor = BrainGraphConstructor::new(
            AtlasType::DesikanKilliany,
            ConnectivityMetric::PhaseLockingValue,
            FrequencyBand::Alpha,
        )
        .with_window_duration(0.5)
        .with_window_step(0.25);

        // 68 channels, 256 samples at 256 Hz = 1 second of data
        let n_ch = 68;
        let n_samples = 256;
        let data: Vec<Vec<f64>> = (0..n_ch)
            .map(|i| {
                (0..n_samples)
                    .map(|j| ((j as f64 + i as f64) * 0.1).sin())
                    .collect()
            })
            .collect();

        let ts = MultiChannelTimeSeries::new(data, 256.0, 0.0).unwrap();
        let seq = ctor.construct_sequence(&ts);

        // 1.0s data, 0.5s window, 0.25s step => 3 windows: [0,0.5], [0.25,0.75], [0.5,1.0]
        assert!(seq.len() >= 2, "Should produce at least 2 graphs, got {}", seq.len());
    }
}
