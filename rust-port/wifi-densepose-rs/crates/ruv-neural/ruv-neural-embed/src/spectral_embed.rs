//! Spectral graph embedding using Laplacian eigenvectors.
//!
//! Computes a positional encoding for each node using the first `k` eigenvectors
//! of the normalized graph Laplacian. The graph-level embedding is formed by
//! concatenating summary statistics of the per-node spectral coordinates.

use ruv_neural_core::embedding::NeuralEmbedding;
use ruv_neural_core::error::{Result, RuvNeuralError};
use ruv_neural_core::graph::BrainGraph;
use ruv_neural_core::traits::EmbeddingGenerator;

use crate::default_metadata;

/// Spectral embedding via Laplacian eigenvectors.
pub struct SpectralEmbedder {
    /// Number of eigenvectors (spectral dimensions) to extract.
    pub dimension: usize,
    /// Number of power iteration steps for eigenvalue approximation.
    pub power_iterations: usize,
}

impl SpectralEmbedder {
    /// Create a new spectral embedder.
    ///
    /// `dimension` is the number of Laplacian eigenvectors to use.
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            power_iterations: 100,
        }
    }

    /// Compute the normalized Laplacian matrix: L_norm = I - D^{-1/2} A D^{-1/2}.
    fn normalized_laplacian(adj: &[Vec<f64>], n: usize) -> Vec<Vec<f64>> {
        let degrees: Vec<f64> = (0..n).map(|i| adj[i].iter().sum::<f64>()).collect();

        let inv_sqrt_deg: Vec<f64> = degrees
            .iter()
            .map(|d| if *d > 1e-12 { 1.0 / d.sqrt() } else { 0.0 })
            .collect();

        let mut laplacian = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    if degrees[i] > 1e-12 {
                        laplacian[i][j] = 1.0;
                    }
                } else {
                    laplacian[i][j] = -adj[i][j] * inv_sqrt_deg[i] * inv_sqrt_deg[j];
                }
            }
        }
        laplacian
    }

    /// Extract the k smallest eigenvectors using deflated power iteration on (max_eig*I - L).
    /// Returns eigenvectors as columns: result[eigenvector_index][node_index].
    fn smallest_eigenvectors(
        laplacian: &[Vec<f64>],
        n: usize,
        k: usize,
        iterations: usize,
    ) -> Vec<Vec<f64>> {
        if n == 0 || k == 0 {
            return vec![];
        }
        let k = k.min(n);

        // Gershgorin bound for max eigenvalue
        let max_eig: f64 = (0..n)
            .map(|i| {
                let diag = laplacian[i][i];
                let off: f64 = (0..n)
                    .filter(|&j| j != i)
                    .map(|j| laplacian[i][j].abs())
                    .sum();
                diag + off
            })
            .fold(0.0_f64, f64::max);

        // Shifted matrix: M = max_eig * I - L
        let shifted: Vec<Vec<f64>> = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| {
                        if i == j {
                            max_eig - laplacian[i][j]
                        } else {
                            -laplacian[i][j]
                        }
                    })
                    .collect()
            })
            .collect();

        let mut eigenvectors: Vec<Vec<f64>> = Vec::with_capacity(k);

        for _ev in 0..k {
            let mut v: Vec<f64> = (0..n).map(|i| ((i + 1) as f64).sin()).collect();
            let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > 1e-12 {
                for x in &mut v {
                    *x /= norm;
                }
            }

            // Deflate against already-found eigenvectors
            for prev in &eigenvectors {
                let dot: f64 = v.iter().zip(prev.iter()).map(|(a, b)| a * b).sum();
                for i in 0..n {
                    v[i] -= dot * prev[i];
                }
            }

            for _ in 0..iterations {
                let mut w = vec![0.0; n];
                for i in 0..n {
                    for j in 0..n {
                        w[i] += shifted[i][j] * v[j];
                    }
                }

                for prev in &eigenvectors {
                    let dot: f64 = w.iter().zip(prev.iter()).map(|(a, b)| a * b).sum();
                    for i in 0..n {
                        w[i] -= dot * prev[i];
                    }
                }

                let norm = w.iter().map(|x| x * x).sum::<f64>().sqrt();
                if norm < 1e-12 {
                    break;
                }
                for x in &mut w {
                    *x /= norm;
                }
                v = w;
            }

            eigenvectors.push(v);
        }

        eigenvectors
    }

    /// Embed a brain graph using spectral decomposition.
    pub fn embed_graph(&self, graph: &BrainGraph) -> Result<NeuralEmbedding> {
        let n = graph.num_nodes;
        if n < 2 {
            return Err(RuvNeuralError::Embedding(
                "Spectral embedding requires at least 2 nodes".into(),
            ));
        }

        let adj = graph.adjacency_matrix();
        let laplacian = Self::normalized_laplacian(&adj, n);

        // Skip the trivial first eigenvector and take the next `dimension`
        let num_to_extract = (self.dimension + 1).min(n);
        let eigvecs =
            Self::smallest_eigenvectors(&laplacian, n, num_to_extract, self.power_iterations);

        let useful: Vec<&Vec<f64>> = eigvecs.iter().skip(1).take(self.dimension).collect();

        // Build graph-level embedding: [mean, std, min, max] per eigenvector
        let mut values = Vec::with_capacity(self.dimension * 4);
        for ev in &useful {
            let mean = ev.iter().sum::<f64>() / n as f64;
            let variance = ev.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
            let std = variance.sqrt();
            let min = ev.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = ev.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            values.push(mean);
            values.push(std);
            values.push(min);
            values.push(max);
        }

        // Pad if fewer eigenvectors than requested
        while values.len() < self.dimension * 4 {
            values.push(0.0);
        }

        let meta = default_metadata("spectral", graph.atlas);
        NeuralEmbedding::new(values, graph.timestamp, meta)
    }
}

impl EmbeddingGenerator for SpectralEmbedder {
    fn embedding_dim(&self) -> usize {
        self.dimension * 4
    }

    fn embed(&self, graph: &BrainGraph) -> Result<NeuralEmbedding> {
        self.embed_graph(graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, ConnectivityMetric};
    use ruv_neural_core::signal::FrequencyBand;

    fn make_complete_graph(n: usize) -> BrainGraph {
        let mut edges = Vec::new();
        for i in 0..n {
            for j in (i + 1)..n {
                edges.push(BrainEdge {
                    source: i,
                    target: j,
                    weight: 1.0,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                });
            }
        }
        BrainGraph {
            num_nodes: n,
            edges,
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(n),
        }
    }

    fn make_two_cluster_graph() -> BrainGraph {
        let mut edges = Vec::new();
        // Cluster A: nodes 0-3 (fully connected)
        for i in 0..4 {
            for j in (i + 1)..4 {
                edges.push(BrainEdge {
                    source: i,
                    target: j,
                    weight: 1.0,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                });
            }
        }
        // Cluster B: nodes 4-7 (fully connected)
        for i in 4..8 {
            for j in (i + 1)..8 {
                edges.push(BrainEdge {
                    source: i,
                    target: j,
                    weight: 1.0,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                });
            }
        }
        // Weak bridge
        edges.push(BrainEdge {
            source: 3,
            target: 4,
            weight: 0.1,
            metric: ConnectivityMetric::Coherence,
            frequency_band: FrequencyBand::Alpha,
        });
        BrainGraph {
            num_nodes: 8,
            edges,
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(8),
        }
    }

    #[test]
    fn test_spectral_complete_graph() {
        let graph = make_complete_graph(6);
        let embedder = SpectralEmbedder::new(3);
        let emb = embedder.embed(&graph).unwrap();
        assert_eq!(emb.dimension, 3 * 4);
    }

    #[test]
    fn test_spectral_two_cluster_separation() {
        let graph = make_two_cluster_graph();
        let embedder = SpectralEmbedder::new(2);
        let emb = embedder.embed(&graph).unwrap();
        // Fiedler vector std (index 1) should show cluster separation
        let fiedler_std = emb.vector[1];
        assert!(
            fiedler_std > 0.01,
            "Fiedler eigenvector should show cluster separation, got std={}",
            fiedler_std
        );
    }

    #[test]
    fn test_spectral_too_small() {
        let graph = BrainGraph {
            num_nodes: 1,
            edges: vec![],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(1),
        };
        let embedder = SpectralEmbedder::new(2);
        assert!(embedder.embed(&graph).is_err());
    }
}
