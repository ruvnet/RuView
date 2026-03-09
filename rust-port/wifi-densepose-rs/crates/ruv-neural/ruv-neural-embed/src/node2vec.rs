//! Node2Vec-inspired random walk embedding.
//!
//! Performs biased random walks on the brain graph and constructs a co-occurrence
//! matrix. The graph-level embedding is obtained via SVD of the co-occurrence
//! matrix (a simplified skip-gram approximation).

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use ruv_neural_core::embedding::NeuralEmbedding;
use ruv_neural_core::error::{Result, RuvNeuralError};
use ruv_neural_core::graph::BrainGraph;
use ruv_neural_core::traits::EmbeddingGenerator;

use crate::default_metadata;

/// Node2Vec-style graph embedder using biased random walks.
pub struct Node2VecEmbedder {
    /// Length of each random walk.
    pub walk_length: usize,
    /// Number of walks per node.
    pub num_walks: usize,
    /// Output embedding dimension.
    pub embedding_dim: usize,
    /// Return parameter (higher = more likely to return to previous node).
    pub p: f64,
    /// In-out parameter (higher = more likely to explore outward).
    pub q: f64,
    /// Random seed for reproducibility.
    pub seed: u64,
}

impl Node2VecEmbedder {
    /// Create a new Node2Vec embedder with default parameters.
    pub fn new(embedding_dim: usize) -> Self {
        Self {
            walk_length: 20,
            num_walks: 10,
            embedding_dim,
            p: 1.0,
            q: 1.0,
            seed: 42,
        }
    }

    /// Perform a single biased random walk starting from `start`.
    fn random_walk(
        &self,
        adj: &[Vec<f64>],
        n: usize,
        start: usize,
        rng: &mut StdRng,
    ) -> Vec<usize> {
        let mut walk = Vec::with_capacity(self.walk_length);
        walk.push(start);

        if self.walk_length <= 1 || n <= 1 {
            return walk;
        }

        // First step: weighted over neighbors
        let neighbors: Vec<(usize, f64)> = (0..n)
            .filter(|&j| adj[start][j] > 1e-12)
            .map(|j| (j, adj[start][j]))
            .collect();

        if neighbors.is_empty() {
            return walk;
        }

        let total: f64 = neighbors.iter().map(|(_, w)| w).sum();
        let r: f64 = rng.gen::<f64>() * total;
        let mut cum = 0.0;
        let mut chosen = neighbors[0].0;
        for &(j, w) in &neighbors {
            cum += w;
            if r <= cum {
                chosen = j;
                break;
            }
        }
        walk.push(chosen);

        // Subsequent steps: biased by p and q
        for _ in 2..self.walk_length {
            let current = *walk.last().unwrap();
            let prev = walk[walk.len() - 2];

            let neighbors: Vec<(usize, f64)> = (0..n)
                .filter(|&j| adj[current][j] > 1e-12)
                .map(|j| (j, adj[current][j]))
                .collect();

            if neighbors.is_empty() {
                break;
            }

            let biased: Vec<(usize, f64)> = neighbors
                .iter()
                .map(|&(j, w)| {
                    let bias = if j == prev {
                        1.0 / self.p
                    } else if adj[prev][j] > 1e-12 {
                        1.0
                    } else {
                        1.0 / self.q
                    };
                    (j, w * bias)
                })
                .collect();

            let total: f64 = biased.iter().map(|(_, w)| w).sum();
            if total < 1e-12 {
                break;
            }
            let r: f64 = rng.gen::<f64>() * total;
            let mut cum = 0.0;
            let mut chosen = biased[0].0;
            for &(j, w) in &biased {
                cum += w;
                if r <= cum {
                    chosen = j;
                    break;
                }
            }
            walk.push(chosen);
        }

        walk
    }

    /// Generate all random walks from all nodes.
    fn generate_walks(&self, adj: &[Vec<f64>], n: usize) -> Vec<Vec<usize>> {
        let mut rng = StdRng::seed_from_u64(self.seed);
        let mut all_walks = Vec::with_capacity(n * self.num_walks);
        for _ in 0..self.num_walks {
            for node in 0..n {
                all_walks.push(self.random_walk(adj, n, node, &mut rng));
            }
        }
        all_walks
    }

    /// Build co-occurrence matrix from walks using a skip-gram window.
    fn build_cooccurrence(walks: &[Vec<usize>], n: usize, window: usize) -> Vec<Vec<f64>> {
        let mut cooc = vec![vec![0.0; n]; n];
        for walk in walks {
            for (i, &center) in walk.iter().enumerate() {
                let start = if i >= window { i - window } else { 0 };
                let end = (i + window + 1).min(walk.len());
                for j in start..end {
                    if j != i {
                        cooc[center][walk[j]] += 1.0;
                    }
                }
            }
        }
        cooc
    }

    /// Simplified SVD via power iteration: extract top-k left singular vectors scaled by sigma.
    fn truncated_svd(matrix: &[Vec<f64>], n: usize, k: usize) -> Vec<Vec<f64>> {
        let k = k.min(n);
        if k == 0 || n == 0 {
            return vec![];
        }

        let mut result: Vec<Vec<f64>> = Vec::with_capacity(k);

        for col in 0..k {
            let mut v: Vec<f64> = (0..n).map(|i| ((i + col + 1) as f64).sin()).collect();
            let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > 1e-12 {
                for x in &mut v {
                    *x /= norm;
                }
            }

            // Deflate
            for prev in &result {
                let prev_norm: f64 = prev.iter().map(|x| x * x).sum::<f64>().sqrt();
                if prev_norm > 1e-12 {
                    let prev_unit: Vec<f64> = prev.iter().map(|x| x / prev_norm).collect();
                    let dot: f64 = v.iter().zip(prev_unit.iter()).map(|(a, b)| a * b).sum();
                    for i in 0..n {
                        v[i] -= dot * prev_unit[i];
                    }
                }
            }

            // Power iteration on M^T M
            for _ in 0..100 {
                let mut u = vec![0.0; n];
                for i in 0..n {
                    for j in 0..n {
                        u[i] += matrix[i][j] * v[j];
                    }
                }
                let mut new_v = vec![0.0; n];
                for j in 0..n {
                    for i in 0..n {
                        new_v[j] += matrix[i][j] * u[i];
                    }
                }

                // Deflate
                for prev in &result {
                    let prev_norm: f64 = prev.iter().map(|x| x * x).sum::<f64>().sqrt();
                    if prev_norm > 1e-12 {
                        let prev_unit: Vec<f64> = prev.iter().map(|x| x / prev_norm).collect();
                        let dot: f64 = new_v
                            .iter()
                            .zip(prev_unit.iter())
                            .map(|(a, b)| a * b)
                            .sum();
                        for i in 0..n {
                            new_v[i] -= dot * prev_unit[i];
                        }
                    }
                }

                let norm = new_v.iter().map(|x| x * x).sum::<f64>().sqrt();
                if norm < 1e-12 {
                    break;
                }
                for x in &mut new_v {
                    *x /= norm;
                }
                v = new_v;
            }

            // sigma * u = M * v
            let mut mv = vec![0.0; n];
            for i in 0..n {
                for j in 0..n {
                    mv[i] += matrix[i][j] * v[j];
                }
            }

            result.push(mv);
        }

        result
    }

    /// Generate the Node2Vec embedding for a brain graph.
    pub fn embed_graph(&self, graph: &BrainGraph) -> Result<NeuralEmbedding> {
        let n = graph.num_nodes;
        if n < 2 {
            return Err(RuvNeuralError::Embedding(
                "Node2Vec requires at least 2 nodes".into(),
            ));
        }

        let adj = graph.adjacency_matrix();
        let walks = self.generate_walks(&adj, n);
        let cooc = Self::build_cooccurrence(&walks, n, 5);

        // Log transform (PPMI-like)
        let log_cooc: Vec<Vec<f64>> = cooc
            .iter()
            .map(|row| row.iter().map(|&v| (1.0 + v).ln()).collect())
            .collect();

        let dim = self.embedding_dim.min(n);
        let node_embeddings = Self::truncated_svd(&log_cooc, n, dim);

        // Aggregate: [mean, std] per SVD component
        let mut values = Vec::with_capacity(dim * 2);
        for component in &node_embeddings {
            let mean = component.iter().sum::<f64>() / n as f64;
            let var = component.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
            values.push(mean);
            values.push(var.sqrt());
        }

        while values.len() < self.embedding_dim * 2 {
            values.push(0.0);
        }

        let meta = default_metadata("node2vec", graph.atlas);
        NeuralEmbedding::new(values, graph.timestamp, meta)
    }
}

impl EmbeddingGenerator for Node2VecEmbedder {
    fn embedding_dim(&self) -> usize {
        self.embedding_dim * 2
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

    fn make_connected_graph() -> BrainGraph {
        let edges: Vec<BrainEdge> = (0..4)
            .map(|i| BrainEdge {
                source: i,
                target: i + 1,
                weight: 1.0,
                metric: ConnectivityMetric::Coherence,
                frequency_band: FrequencyBand::Alpha,
            })
            .collect();
        BrainGraph {
            num_nodes: 5,
            edges,
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(5),
        }
    }

    #[test]
    fn test_node2vec_walks_visit_all_nodes() {
        let graph = make_connected_graph();
        let embedder = Node2VecEmbedder {
            walk_length: 50,
            num_walks: 20,
            embedding_dim: 4,
            p: 1.0,
            q: 1.0,
            seed: 42,
        };

        let adj = graph.adjacency_matrix();
        let walks = embedder.generate_walks(&adj, graph.num_nodes);

        let mut visited = std::collections::HashSet::new();
        for walk in &walks {
            for &node in walk {
                visited.insert(node);
            }
        }

        assert_eq!(visited.len(), 5, "All nodes should be visited");
    }

    #[test]
    fn test_node2vec_embed() {
        let graph = make_connected_graph();
        let embedder = Node2VecEmbedder::new(3);
        let emb = embedder.embed(&graph).unwrap();
        assert_eq!(emb.dimension, 3 * 2);
        assert_eq!(emb.metadata.embedding_method, "node2vec");
    }

    #[test]
    fn test_node2vec_too_small() {
        let graph = BrainGraph {
            num_nodes: 1,
            edges: vec![],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(1),
        };
        let embedder = Node2VecEmbedder::new(4);
        assert!(embedder.embed(&graph).is_err());
    }
}
