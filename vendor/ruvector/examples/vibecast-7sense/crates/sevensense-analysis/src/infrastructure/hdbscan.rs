//! HDBSCAN clustering implementation.
//!
//! Hierarchical Density-Based Spatial Clustering of Applications with Noise.
//! This implementation uses core distance and mutual reachability distance
//! to build a minimum spanning tree and extract clusters.

use ndarray::{Array2, ArrayView1};
use petgraph::graph::{NodeIndex, UnGraph};
use petgraph::algo::min_spanning_tree;
use petgraph::data::FromElements;
use std::collections::{HashMap, HashSet};
use tracing::{debug, instrument};

use crate::application::services::AnalysisError;
use crate::domain::value_objects::DistanceMetric;

/// HDBSCAN clustering algorithm.
pub struct HdbscanClusterer {
    /// Minimum cluster size.
    min_cluster_size: usize,
    /// Minimum samples for core point determination.
    min_samples: usize,
    /// Distance metric to use.
    metric: DistanceMetric,
}

impl HdbscanClusterer {
    /// Create a new HDBSCAN clusterer.
    #[must_use]
    pub fn new(min_cluster_size: usize, min_samples: usize, metric: DistanceMetric) -> Self {
        Self {
            min_cluster_size,
            min_samples,
            metric,
        }
    }

    /// Fit HDBSCAN to the data and return cluster labels.
    ///
    /// # Arguments
    ///
    /// * `data` - 2D array where rows are samples and columns are features
    ///
    /// # Returns
    ///
    /// Vector of cluster labels (-1 for noise).
    #[instrument(skip(self, data), fields(n_samples = data.nrows(), n_features = data.ncols()))]
    pub fn fit(&self, data: &Array2<f32>) -> Result<Vec<i32>, AnalysisError> {
        let n = data.nrows();
        if n < self.min_cluster_size {
            return Err(AnalysisError::InsufficientData(format!(
                "Need at least {} samples, got {}",
                self.min_cluster_size, n
            )));
        }

        debug!(
            n_samples = n,
            min_cluster_size = self.min_cluster_size,
            min_samples = self.min_samples,
            "Starting HDBSCAN fit"
        );

        // Step 1: Compute pairwise distances
        let distances = self.compute_pairwise_distances(data);

        // Step 2: Compute core distances
        let core_distances = self.compute_core_distances(&distances);

        // Step 3: Compute mutual reachability distances
        let mrd = self.compute_mutual_reachability(&distances, &core_distances);

        // Step 4: Build minimum spanning tree
        let mst = self.build_mst(&mrd);

        // Step 5: Build cluster hierarchy
        let labels = self.extract_clusters(&mst, n);

        debug!(
            n_clusters = labels.iter().filter(|&&l| l >= 0).collect::<HashSet<_>>().len(),
            n_noise = labels.iter().filter(|&&l| l < 0).count(),
            "HDBSCAN fit completed"
        );

        Ok(labels)
    }

    /// Compute pairwise distance matrix.
    fn compute_pairwise_distances(&self, data: &Array2<f32>) -> Array2<f32> {
        let n = data.nrows();
        let mut distances = Array2::<f32>::zeros((n, n));

        for i in 0..n {
            for j in (i + 1)..n {
                let dist = self.distance(data.row(i), data.row(j));
                distances[[i, j]] = dist;
                distances[[j, i]] = dist;
            }
        }

        distances
    }

    /// Compute core distance for each point (k-th nearest neighbor distance).
    fn compute_core_distances(&self, distances: &Array2<f32>) -> Vec<f32> {
        let n = distances.nrows();
        let k = self.min_samples.min(n - 1);

        let mut core_distances = Vec::with_capacity(n);

        for i in 0..n {
            let mut row_distances: Vec<f32> = distances.row(i).to_vec();
            row_distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            // k-th nearest neighbor (index k because index 0 is self with distance 0)
            let core_dist = row_distances.get(k).copied().unwrap_or(f32::MAX);
            core_distances.push(core_dist);
        }

        core_distances
    }

    /// Compute mutual reachability distance matrix.
    fn compute_mutual_reachability(
        &self,
        distances: &Array2<f32>,
        core_distances: &[f32],
    ) -> Array2<f32> {
        let n = distances.nrows();
        let mut mrd = Array2::<f32>::zeros((n, n));

        for i in 0..n {
            for j in (i + 1)..n {
                let d = distances[[i, j]];
                let mr = core_distances[i].max(core_distances[j]).max(d);
                mrd[[i, j]] = mr;
                mrd[[j, i]] = mr;
            }
        }

        mrd
    }

    /// Build minimum spanning tree from mutual reachability distances.
    fn build_mst(&self, mrd: &Array2<f32>) -> Vec<(usize, usize, f32)> {
        let n = mrd.nrows();

        // Build graph with all edges
        let mut graph = UnGraph::<usize, f32>::new_undirected();

        // Add nodes
        let nodes: Vec<NodeIndex> = (0..n).map(|i| graph.add_node(i)).collect();

        // Add edges (only upper triangle to avoid duplicates)
        for i in 0..n {
            for j in (i + 1)..n {
                let weight = mrd[[i, j]];
                if weight < f32::MAX {
                    graph.add_edge(nodes[i], nodes[j], weight);
                }
            }
        }

        // Compute MST using Prim's algorithm via petgraph
        let mst_graph = UnGraph::<usize, f32>::from_elements(min_spanning_tree(&graph));

        // Extract edges from MST
        let mut edges: Vec<(usize, usize, f32)> = mst_graph
            .edge_indices()
            .filter_map(|e| {
                let (a, b) = mst_graph.edge_endpoints(e)?;
                let weight = *mst_graph.edge_weight(e)?;
                let a_val = *mst_graph.node_weight(a)?;
                let b_val = *mst_graph.node_weight(b)?;
                Some((a_val, b_val, weight))
            })
            .collect();

        // Sort by weight descending for cluster extraction
        edges.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        edges
    }

    /// Extract flat clusters from MST using HDBSCAN* algorithm.
    fn extract_clusters(&self, mst: &[(usize, usize, f32)], n: usize) -> Vec<i32> {
        // Use simplified cluster extraction based on edge cutting
        // This is a simplified version - full HDBSCAN uses condensed tree

        let mut labels = vec![-1i32; n];
        let mut current_cluster = 0i32;

        // Build adjacency from MST
        let mut adj: HashMap<usize, Vec<(usize, f32)>> = HashMap::new();
        for &(a, b, w) in mst {
            adj.entry(a).or_default().push((b, w));
            adj.entry(b).or_default().push((a, w));
        }

        // Find connected components, removing edges above threshold
        // Use adaptive threshold based on edge weight distribution
        let threshold = self.compute_threshold(mst);

        let mut visited = vec![false; n];

        for start in 0..n {
            if visited[start] {
                continue;
            }

            // BFS to find connected component
            let mut component = Vec::new();
            let mut queue = vec![start];

            while let Some(node) = queue.pop() {
                if visited[node] {
                    continue;
                }
                visited[node] = true;
                component.push(node);

                if let Some(neighbors) = adj.get(&node) {
                    for &(neighbor, weight) in neighbors {
                        if !visited[neighbor] && weight < threshold {
                            queue.push(neighbor);
                        }
                    }
                }
            }

            // Only assign cluster label if component is large enough
            if component.len() >= self.min_cluster_size {
                for &node in &component {
                    labels[node] = current_cluster;
                }
                current_cluster += 1;
            }
        }

        labels
    }

    /// Compute adaptive threshold for edge cutting.
    fn compute_threshold(&self, mst: &[(usize, usize, f32)]) -> f32 {
        if mst.is_empty() {
            return f32::MAX;
        }

        let weights: Vec<f32> = mst.iter().map(|&(_, _, w)| w).collect();
        let n = weights.len();

        // Use median + IQR method for threshold
        let mut sorted = weights.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let _median = sorted[n / 2];
        let q1 = sorted[n / 4];
        let q3 = sorted[3 * n / 4];
        let iqr = q3 - q1;

        // Threshold at Q3 + 1.5 * IQR (outlier boundary)
        q3 + 1.5 * iqr
    }

    /// Compute distance between two vectors.
    fn distance(&self, a: ArrayView1<f32>, b: ArrayView1<f32>) -> f32 {
        match self.metric {
            DistanceMetric::Euclidean => {
                a.iter()
                    .zip(b.iter())
                    .map(|(x, y)| (x - y).powi(2))
                    .sum::<f32>()
                    .sqrt()
            }
            DistanceMetric::Cosine => {
                let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
                let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
                let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm_a == 0.0 || norm_b == 0.0 {
                    1.0
                } else {
                    1.0 - (dot / (norm_a * norm_b))
                }
            }
            DistanceMetric::Manhattan => a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum(),
            DistanceMetric::Poincare => {
                // Simplified - would need proper hyperbolic distance
                a.iter()
                    .zip(b.iter())
                    .map(|(x, y)| (x - y).powi(2))
                    .sum::<f32>()
                    .sqrt()
            }
        }
    }
}

/// Single linkage tree node for cluster hierarchy.
#[derive(Debug, Clone)]
struct SingleLinkageNode {
    left: Option<usize>,
    right: Option<usize>,
    distance: f32,
    size: usize,
}

/// HDBSCAN condensed tree for cluster extraction.
#[derive(Debug)]
pub struct CondensedTree {
    nodes: Vec<CondensedNode>,
}

#[derive(Debug, Clone)]
struct CondensedNode {
    parent: Option<usize>,
    children: Vec<usize>,
    lambda_birth: f32,
    lambda_death: f32,
    stability: f32,
    points: HashSet<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    fn create_clustered_data() -> Array2<f32> {
        // Create 3 clear clusters with deterministic variation
        let mut data = Array2::<f32>::zeros((30, 2));

        // Cluster 1: around (0, 0)
        for i in 0..10 {
            data[[i, 0]] = rand_offset(0.0, i);
            data[[i, 1]] = rand_offset(0.0, i + 1);
        }

        // Cluster 2: around (5, 5)
        for i in 10..20 {
            data[[i, 0]] = rand_offset(5.0, i);
            data[[i, 1]] = rand_offset(5.0, i + 1);
        }

        // Cluster 3: around (10, 0)
        for i in 20..30 {
            data[[i, 0]] = rand_offset(10.0, i);
            data[[i, 1]] = rand_offset(0.0, i + 1);
        }

        data
    }

    fn rand_offset(center: f32, seed: usize) -> f32 {
        // Deterministic "random" offset using seed for variation
        let variation = ((seed as f32 * 1.618) % 1.0 - 0.5) * 0.5;
        center + variation
    }

    #[test]
    fn test_hdbscan_basic() {
        let clusterer = HdbscanClusterer::new(3, 2, DistanceMetric::Euclidean);
        let data = create_clustered_data();

        let labels = clusterer.fit(&data).unwrap();
        assert_eq!(labels.len(), 30);

        // Should have at least one cluster
        let n_clusters = labels.iter().filter(|&&l| l >= 0).collect::<HashSet<_>>().len();
        assert!(n_clusters >= 1);
    }

    #[test]
    fn test_hdbscan_insufficient_data() {
        let clusterer = HdbscanClusterer::new(10, 5, DistanceMetric::Euclidean);
        let data = Array2::<f32>::zeros((5, 2));

        let result = clusterer.fit(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_distance_euclidean() {
        let clusterer = HdbscanClusterer::new(5, 3, DistanceMetric::Euclidean);
        let a = Array1::from_vec(vec![0.0, 0.0]);
        let b = Array1::from_vec(vec![3.0, 4.0]);

        let dist = clusterer.distance(a.view(), b.view());
        assert!((dist - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_distance_cosine() {
        let clusterer = HdbscanClusterer::new(5, 3, DistanceMetric::Cosine);
        let a = Array1::from_vec(vec![1.0, 0.0]);
        let b = Array1::from_vec(vec![1.0, 0.0]);

        let dist = clusterer.distance(a.view(), b.view());
        assert!(dist.abs() < 0.001); // Same vector = 0 distance

        let c = Array1::from_vec(vec![0.0, 1.0]);
        let dist2 = clusterer.distance(a.view(), c.view());
        assert!((dist2 - 1.0).abs() < 0.001); // Orthogonal = 1 distance
    }
}
