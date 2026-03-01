//! Spectral Clustering
//!
//! This module implements spectral clustering algorithms for graph partitioning.
//!
//! ## Algorithm Overview
//!
//! Spectral clustering works by:
//! 1. Computing the graph Laplacian (normalized or unnormalized)
//! 2. Finding the k smallest eigenvectors (spectral embedding)
//! 3. Clustering the embedded points using k-means or similar
//!
//! ## Theoretical Foundation
//!
//! - The first k eigenvectors of the Laplacian encode cluster structure
//! - Small eigenvalues correspond to "smooth" functions on the graph
//! - The Fiedler vector (2nd eigenvector) gives optimal 2-way cut (relaxed)

use super::analyzer::SpectralAnalyzer;
use super::types::{Graph, NodeId, Vector, EPS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cluster assignment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterAssignment {
    /// Cluster label for each node (0 to k-1)
    pub labels: Vec<usize>,
    /// Number of clusters
    pub k: usize,
    /// Nodes in each cluster
    pub clusters: Vec<Vec<NodeId>>,
    /// Quality metrics
    pub quality: ClusterQuality,
}

impl ClusterAssignment {
    /// Get cluster for a specific node
    pub fn cluster_of(&self, node: NodeId) -> usize {
        self.labels[node]
    }

    /// Get all nodes in a specific cluster
    pub fn nodes_in_cluster(&self, cluster: usize) -> &[NodeId] {
        &self.clusters[cluster]
    }

    /// Get cluster sizes
    pub fn cluster_sizes(&self) -> Vec<usize> {
        self.clusters.iter().map(|c| c.len()).collect()
    }

    /// Check if clustering is balanced (no cluster has < 10% or > 50% of nodes)
    pub fn is_balanced(&self) -> bool {
        let n = self.labels.len();
        let sizes = self.cluster_sizes();

        for size in sizes {
            let ratio = size as f64 / n as f64;
            if ratio < 0.1 || ratio > 0.5 {
                return false;
            }
        }
        true
    }
}

/// Quality metrics for clustering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterQuality {
    /// Normalized cut value
    pub normalized_cut: f64,
    /// Ratio cut value
    pub ratio_cut: f64,
    /// Modularity score
    pub modularity: f64,
    /// Average cluster conductance
    pub avg_conductance: f64,
    /// Silhouette score (spectral space)
    pub silhouette: f64,
}

/// Spectral clustering configuration
#[derive(Debug, Clone)]
pub struct ClusterConfig {
    /// Number of clusters
    pub k: usize,
    /// Use normalized Laplacian
    pub use_normalized: bool,
    /// K-means iterations
    pub kmeans_iter: usize,
    /// K-means restarts (for finding best clustering)
    pub kmeans_restarts: usize,
    /// Random seed for reproducibility
    pub seed: u64,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            k: 2,
            use_normalized: true,
            kmeans_iter: 100,
            kmeans_restarts: 10,
            seed: 42,
        }
    }
}

/// Spectral clusterer for graph partitioning
pub struct SpectralClusterer {
    config: ClusterConfig,
}

impl SpectralClusterer {
    /// Create a new spectral clusterer
    pub fn new(k: usize) -> Self {
        Self {
            config: ClusterConfig {
                k,
                ..Default::default()
            },
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: ClusterConfig) -> Self {
        Self { config }
    }

    /// Perform spectral clustering on a graph
    pub fn cluster(&self, graph: &Graph) -> ClusterAssignment {
        let n = graph.n;
        let k = self.config.k.min(n);

        if k == 0 || n == 0 {
            return ClusterAssignment {
                labels: Vec::new(),
                k: 0,
                clusters: Vec::new(),
                quality: ClusterQuality {
                    normalized_cut: 0.0,
                    ratio_cut: 0.0,
                    modularity: 0.0,
                    avg_conductance: 0.0,
                    silhouette: 0.0,
                },
            };
        }

        // Step 1: Compute spectral embedding
        let embedding = self.compute_spectral_embedding(graph, k);

        // Step 2: Run k-means clustering on embedding
        let labels = self.kmeans_cluster(&embedding, k);

        // Step 3: Build cluster assignments
        let mut clusters: Vec<Vec<NodeId>> = vec![Vec::new(); k];
        for (node, &label) in labels.iter().enumerate() {
            clusters[label].push(node);
        }

        // Step 4: Compute quality metrics
        let quality = self.compute_quality(graph, &labels, &clusters, &embedding);

        ClusterAssignment {
            labels,
            k,
            clusters,
            quality,
        }
    }

    /// Compute spectral embedding using first k eigenvectors
    fn compute_spectral_embedding(&self, graph: &Graph, k: usize) -> Vec<Vector> {
        let config = super::analyzer::SpectralConfig::builder()
            .num_eigenvalues(k + 1) // +1 to skip trivial eigenvector
            .normalized(self.config.use_normalized)
            .build();

        let mut analyzer = SpectralAnalyzer::with_config(graph.clone(), config);
        analyzer.compute_laplacian_spectrum();

        // Get spectral embedding
        analyzer.spectral_embedding(k)
    }

    /// K-means clustering on the spectral embedding
    fn kmeans_cluster(&self, embedding: &[Vector], k: usize) -> Vec<usize> {
        let n = embedding.len();
        if n == 0 || k == 0 {
            return Vec::new();
        }

        let dim = embedding[0].len();
        let mut best_labels = vec![0; n];
        let mut best_inertia = f64::INFINITY;

        for restart in 0..self.config.kmeans_restarts {
            let seed = self.config.seed + restart as u64;
            let (labels, inertia) = self.kmeans_single(embedding, k, dim, seed);

            if inertia < best_inertia {
                best_inertia = inertia;
                best_labels = labels;
            }
        }

        best_labels
    }

    /// Single run of k-means
    fn kmeans_single(
        &self,
        embedding: &[Vector],
        k: usize,
        dim: usize,
        seed: u64,
    ) -> (Vec<usize>, f64) {
        let n = embedding.len();

        // Initialize centroids using k-means++
        let mut centroids = self.kmeans_pp_init(embedding, k, seed);
        let mut labels = vec![0; n];

        for _ in 0..self.config.kmeans_iter {
            // Assignment step
            for (i, point) in embedding.iter().enumerate() {
                let mut min_dist = f64::INFINITY;
                for (c, centroid) in centroids.iter().enumerate() {
                    let dist = euclidean_distance_sq(point, centroid);
                    if dist < min_dist {
                        min_dist = dist;
                        labels[i] = c;
                    }
                }
            }

            // Update step
            let mut new_centroids = vec![vec![0.0; dim]; k];
            let mut counts = vec![0; k];

            for (i, point) in embedding.iter().enumerate() {
                let c = labels[i];
                counts[c] += 1;
                for (j, &val) in point.iter().enumerate() {
                    new_centroids[c][j] += val;
                }
            }

            // Normalize centroids
            for c in 0..k {
                if counts[c] > 0 {
                    for j in 0..dim {
                        new_centroids[c][j] /= counts[c] as f64;
                    }
                }
            }

            // Check convergence
            let mut converged = true;
            for (old, new) in centroids.iter().zip(new_centroids.iter()) {
                if euclidean_distance_sq(old, new) > EPS {
                    converged = false;
                    break;
                }
            }

            centroids = new_centroids;

            if converged {
                break;
            }
        }

        // Compute inertia (total within-cluster variance)
        let mut inertia = 0.0;
        for (i, point) in embedding.iter().enumerate() {
            let c = labels[i];
            inertia += euclidean_distance_sq(point, &centroids[c]);
        }

        (labels, inertia)
    }

    /// K-means++ initialization
    fn kmeans_pp_init(&self, embedding: &[Vector], k: usize, seed: u64) -> Vec<Vector> {
        let n = embedding.len();
        let dim = embedding[0].len();
        let mut centroids = Vec::with_capacity(k);
        let mut rng_state = seed;

        // Choose first centroid uniformly at random
        rng_state = lcg_next(rng_state);
        let first_idx = (rng_state % n as u64) as usize;
        centroids.push(embedding[first_idx].clone());

        // Choose remaining centroids
        for _ in 1..k {
            // Compute squared distances to nearest centroid
            let mut distances: Vec<f64> = embedding
                .iter()
                .map(|point| {
                    centroids
                        .iter()
                        .map(|c| euclidean_distance_sq(point, c))
                        .fold(f64::INFINITY, f64::min)
                })
                .collect();

            // Convert to probability distribution
            let total: f64 = distances.iter().sum();
            if total > EPS {
                for d in distances.iter_mut() {
                    *d /= total;
                }
            }

            // Sample next centroid
            rng_state = lcg_next(rng_state);
            let rand = (rng_state as f64) / (u64::MAX as f64);
            let mut cumsum = 0.0;
            let mut next_idx = 0;

            for (i, &p) in distances.iter().enumerate() {
                cumsum += p;
                if rand <= cumsum {
                    next_idx = i;
                    break;
                }
            }

            centroids.push(embedding[next_idx].clone());
        }

        centroids
    }

    /// Compute clustering quality metrics
    fn compute_quality(
        &self,
        graph: &Graph,
        labels: &[usize],
        clusters: &[Vec<NodeId>],
        embedding: &[Vector],
    ) -> ClusterQuality {
        let k = clusters.len();
        let n = graph.n;

        if k == 0 || n == 0 {
            return ClusterQuality {
                normalized_cut: 0.0,
                ratio_cut: 0.0,
                modularity: 0.0,
                avg_conductance: 0.0,
                silhouette: 0.0,
            };
        }

        // Compute cut values
        let (normalized_cut, ratio_cut) = self.compute_cut_values(graph, labels, clusters);

        // Compute modularity
        let modularity = self.compute_modularity(graph, labels, clusters);

        // Compute average conductance
        let avg_conductance = self.compute_avg_conductance(graph, clusters);

        // Compute silhouette score
        let silhouette = self.compute_silhouette(embedding, labels, k);

        ClusterQuality {
            normalized_cut,
            ratio_cut,
            modularity,
            avg_conductance,
            silhouette,
        }
    }

    /// Compute normalized cut and ratio cut
    fn compute_cut_values(
        &self,
        graph: &Graph,
        labels: &[usize],
        clusters: &[Vec<NodeId>],
    ) -> (f64, f64) {
        let k = clusters.len();
        let mut total_ncut = 0.0;
        let mut total_rcut = 0.0;

        for c in 0..k {
            let mut cut_weight = 0.0;
            let mut cluster_volume = 0.0;

            for &node in &clusters[c] {
                cluster_volume += graph.degree(node);

                for &(neighbor, weight) in &graph.adj[node] {
                    if labels[neighbor] != c {
                        cut_weight += weight;
                    }
                }
            }

            // Each cut edge counted twice
            cut_weight /= 2.0;

            if cluster_volume > EPS {
                total_ncut += cut_weight / cluster_volume;
            }

            if !clusters[c].is_empty() {
                total_rcut += cut_weight / clusters[c].len() as f64;
            }
        }

        (total_ncut, total_rcut)
    }

    /// Compute modularity
    fn compute_modularity(
        &self,
        graph: &Graph,
        labels: &[usize],
        clusters: &[Vec<NodeId>],
    ) -> f64 {
        let total_weight = graph.total_weight();
        if total_weight < EPS {
            return 0.0;
        }

        let mut modularity = 0.0;
        let two_m = 2.0 * total_weight;

        for cluster in clusters {
            let mut internal_edges = 0.0;
            let mut cluster_degree = 0.0;

            for &u in cluster {
                cluster_degree += graph.degree(u);

                for &(v, w) in &graph.adj[u] {
                    if labels[v] == labels[u] {
                        internal_edges += w;
                    }
                }
            }

            // Internal edges counted twice
            internal_edges /= 2.0;

            modularity += internal_edges / total_weight
                - (cluster_degree / two_m).powi(2);
        }

        modularity
    }

    /// Compute average conductance
    fn compute_avg_conductance(&self, graph: &Graph, clusters: &[Vec<NodeId>]) -> f64 {
        if clusters.is_empty() {
            return 0.0;
        }

        let total_volume: f64 = graph.degrees().iter().sum();
        let mut total_conductance = 0.0;

        for cluster in clusters {
            let node_set: std::collections::HashSet<NodeId> =
                cluster.iter().cloned().collect();

            let mut cut_weight = 0.0;
            let mut cluster_volume = 0.0;

            for &node in cluster {
                cluster_volume += graph.degree(node);

                for &(neighbor, weight) in &graph.adj[node] {
                    if !node_set.contains(&neighbor) {
                        cut_weight += weight;
                    }
                }
            }

            let complement_volume = total_volume - cluster_volume;
            let min_volume = cluster_volume.min(complement_volume);

            if min_volume > EPS {
                total_conductance += cut_weight / min_volume;
            }
        }

        total_conductance / clusters.len() as f64
    }

    /// Compute silhouette score in spectral space
    fn compute_silhouette(&self, embedding: &[Vector], labels: &[usize], k: usize) -> f64 {
        let n = embedding.len();
        if n < 2 || k < 2 {
            return 0.0;
        }

        let mut total_silhouette = 0.0;

        for i in 0..n {
            // Compute a(i): average distance to points in same cluster
            let mut same_cluster_dist = 0.0;
            let mut same_cluster_count = 0;

            for j in 0..n {
                if i != j && labels[j] == labels[i] {
                    same_cluster_dist += euclidean_distance_sq(&embedding[i], &embedding[j]).sqrt();
                    same_cluster_count += 1;
                }
            }

            let a_i = if same_cluster_count > 0 {
                same_cluster_dist / same_cluster_count as f64
            } else {
                0.0
            };

            // Compute b(i): minimum average distance to points in other clusters
            let mut b_i = f64::INFINITY;

            for c in 0..k {
                if c == labels[i] {
                    continue;
                }

                let mut other_cluster_dist = 0.0;
                let mut other_cluster_count = 0;

                for j in 0..n {
                    if labels[j] == c {
                        other_cluster_dist +=
                            euclidean_distance_sq(&embedding[i], &embedding[j]).sqrt();
                        other_cluster_count += 1;
                    }
                }

                if other_cluster_count > 0 {
                    let avg_dist = other_cluster_dist / other_cluster_count as f64;
                    b_i = b_i.min(avg_dist);
                }
            }

            // Silhouette coefficient for point i
            let max_ab = a_i.max(b_i);
            if max_ab > EPS {
                total_silhouette += (b_i - a_i) / max_ab;
            }
        }

        total_silhouette / n as f64
    }

    /// Estimate optimal number of clusters using eigengap heuristic
    pub fn estimate_k(&self, graph: &Graph, max_k: usize) -> usize {
        let config = super::analyzer::SpectralConfig::builder()
            .num_eigenvalues(max_k + 2)
            .normalized(self.config.use_normalized)
            .build();

        let mut analyzer = SpectralAnalyzer::with_config(graph.clone(), config);
        analyzer.compute_laplacian_spectrum();

        if analyzer.eigenvalues.len() < 2 {
            return 1;
        }

        // Find largest gap in eigenvalues
        let mut max_gap = 0.0;
        let mut best_k = 1;

        for i in 1..analyzer.eigenvalues.len().min(max_k) {
            let gap = analyzer.eigenvalues[i] - analyzer.eigenvalues[i - 1];
            if gap > max_gap {
                max_gap = gap;
                best_k = i;
            }
        }

        best_k.max(2).min(max_k)
    }
}

/// Squared Euclidean distance
fn euclidean_distance_sq(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum()
}

/// Simple LCG for reproducible random numbers
fn lcg_next(state: u64) -> u64 {
    state.wrapping_mul(6364136223846793005).wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_two_cliques(size1: usize, size2: usize, bridge_weight: f64) -> Graph {
        let n = size1 + size2;
        let mut g = Graph::new(n);

        // First clique
        for i in 0..size1 {
            for j in i + 1..size1 {
                g.add_edge(i, j, 1.0);
            }
        }

        // Second clique
        for i in size1..n {
            for j in i + 1..n {
                g.add_edge(i, j, 1.0);
            }
        }

        // Bridge
        g.add_edge(size1 - 1, size1, bridge_weight);

        g
    }

    #[test]
    fn test_spectral_clustering_two_cliques() {
        let g = create_two_cliques(5, 5, 0.1);
        let clusterer = SpectralClusterer::new(2);
        let assignment = clusterer.cluster(&g);

        assert_eq!(assignment.k, 2);
        assert_eq!(assignment.labels.len(), 10);

        // Check that most nodes in first clique have same label
        let first_clique_labels: Vec<usize> = (0..5).map(|i| assignment.labels[i]).collect();
        let most_common_first = *first_clique_labels.iter()
            .max_by_key(|&l| first_clique_labels.iter().filter(|&x| x == l).count())
            .unwrap();

        let first_cluster_correct = first_clique_labels.iter()
            .filter(|&&l| l == most_common_first)
            .count();

        assert!(first_cluster_correct >= 4, "First clique should be mostly in one cluster");
    }

    #[test]
    fn test_clustering_quality() {
        let g = create_two_cliques(4, 4, 0.1);
        let clusterer = SpectralClusterer::new(2);
        let assignment = clusterer.cluster(&g);

        // Should have positive modularity for good clustering
        assert!(assignment.quality.modularity > 0.0);

        // Silhouette should be positive for clear clusters
        assert!(assignment.quality.silhouette > 0.0);
    }

    #[test]
    fn test_estimate_k() {
        let g = create_two_cliques(5, 5, 0.01);
        let clusterer = SpectralClusterer::new(2);
        let estimated_k = clusterer.estimate_k(&g, 10);

        // Should estimate 2 clusters for two clear cliques
        assert!(estimated_k >= 2 && estimated_k <= 3);
    }

    #[test]
    fn test_single_cluster() {
        // Complete graph should be one cluster
        let mut g = Graph::new(5);
        for i in 0..5 {
            for j in i + 1..5 {
                g.add_edge(i, j, 1.0);
            }
        }

        let clusterer = SpectralClusterer::new(1);
        let assignment = clusterer.cluster(&g);

        assert_eq!(assignment.k, 1);
        assert!(assignment.labels.iter().all(|&l| l == 0));
    }

    #[test]
    fn test_balanced_clustering() {
        let g = create_two_cliques(5, 5, 0.1);
        let clusterer = SpectralClusterer::new(2);
        let assignment = clusterer.cluster(&g);

        assert!(assignment.is_balanced());
    }
}
