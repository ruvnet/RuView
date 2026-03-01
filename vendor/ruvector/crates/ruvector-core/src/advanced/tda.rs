//! # Topological Data Analysis (TDA)
//!
//! Basic topological analysis for embedding quality assessment.
//! Detects mode collapse, degeneracy, and topological structure.

use crate::error::{Result, RuvectorError};
use ndarray::Array2;
use serde::{Deserialize, Serialize};

/// Topological analyzer for embeddings
pub struct TopologicalAnalyzer {
    /// k for k-nearest neighbors graph
    k_neighbors: usize,
    /// Distance threshold for edge creation
    epsilon: f32,
}

impl TopologicalAnalyzer {
    /// Create a new topological analyzer
    pub fn new(k_neighbors: usize, epsilon: f32) -> Self {
        Self {
            k_neighbors,
            epsilon,
        }
    }

    /// Analyze embedding quality
    pub fn analyze(&self, embeddings: &[Vec<f32>]) -> Result<EmbeddingQuality> {
        if embeddings.is_empty() {
            return Err(RuvectorError::InvalidInput("Empty embeddings".into()));
        }

        let n = embeddings.len();
        let dim = embeddings[0].len();

        // Build k-NN graph
        let graph = self.build_knn_graph(embeddings);

        // Compute topological features
        let connected_components = self.count_connected_components(&graph, n);
        let clustering_coefficient = self.compute_clustering_coefficient(&graph);
        let degree_stats = self.compute_degree_statistics(&graph, n);

        // Detect mode collapse
        let mode_collapse_score = self.detect_mode_collapse(embeddings);

        // Compute embedding spread
        let spread = self.compute_spread(embeddings);

        // Detect degeneracy (vectors collapsing to a lower-dimensional manifold)
        let degeneracy_score = self.detect_degeneracy(embeddings);

        // Compute persistence features (simplified)
        let persistence_score = self.compute_persistence(&graph, embeddings);

        // Overall quality score (0-1, higher is better)
        let quality_score = self.compute_quality_score(
            mode_collapse_score,
            degeneracy_score,
            connected_components,
            clustering_coefficient,
            spread,
        );

        Ok(EmbeddingQuality {
            dimensions: dim,
            num_vectors: n,
            connected_components,
            clustering_coefficient,
            avg_degree: degree_stats.0,
            degree_std: degree_stats.1,
            mode_collapse_score,
            degeneracy_score,
            spread,
            persistence_score,
            quality_score,
        })
    }

    fn build_knn_graph(&self, embeddings: &[Vec<f32>]) -> Vec<Vec<usize>> {
        let n = embeddings.len();
        let mut graph = vec![Vec::new(); n];

        for i in 0..n {
            let mut distances: Vec<(usize, f32)> = (0..n)
                .filter(|&j| i != j)
                .map(|j| {
                    let dist = euclidean_distance(&embeddings[i], &embeddings[j]);
                    (j, dist)
                })
                .collect();

            distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            // Add k nearest neighbors
            for (j, dist) in distances.iter().take(self.k_neighbors) {
                if *dist <= self.epsilon {
                    graph[i].push(*j);
                }
            }
        }

        graph
    }

    fn count_connected_components(&self, graph: &[Vec<usize>], n: usize) -> usize {
        let mut visited = vec![false; n];
        let mut components = 0;

        for i in 0..n {
            if !visited[i] {
                components += 1;
                self.dfs(i, graph, &mut visited);
            }
        }

        components
    }

    #[allow(clippy::only_used_in_recursion)]
    fn dfs(&self, node: usize, graph: &[Vec<usize>], visited: &mut [bool]) {
        visited[node] = true;
        for &neighbor in &graph[node] {
            if !visited[neighbor] {
                self.dfs(neighbor, graph, visited);
            }
        }
    }

    fn compute_clustering_coefficient(&self, graph: &[Vec<usize>]) -> f32 {
        let mut total_coeff = 0.0;
        let mut count = 0;

        for neighbors in graph {
            if neighbors.len() < 2 {
                continue;
            }

            let k = neighbors.len();
            let mut triangles = 0;

            // Count triangles
            for i in 0..k {
                for j in i + 1..k {
                    let ni = neighbors[i];
                    let nj = neighbors[j];

                    if graph[ni].contains(&nj) {
                        triangles += 1;
                    }
                }
            }

            let possible_triangles = k * (k - 1) / 2;
            if possible_triangles > 0 {
                total_coeff += triangles as f32 / possible_triangles as f32;
                count += 1;
            }
        }

        if count > 0 {
            total_coeff / count as f32
        } else {
            0.0
        }
    }

    fn compute_degree_statistics(&self, graph: &[Vec<usize>], n: usize) -> (f32, f32) {
        let degrees: Vec<f32> = graph
            .iter()
            .map(|neighbors| neighbors.len() as f32)
            .collect();

        let avg = degrees.iter().sum::<f32>() / n as f32;
        let variance = degrees.iter().map(|&d| (d - avg).powi(2)).sum::<f32>() / n as f32;
        let std = variance.sqrt();

        (avg, std)
    }

    fn detect_mode_collapse(&self, embeddings: &[Vec<f32>]) -> f32 {
        // Compute pairwise distances
        let n = embeddings.len();
        let mut distances = Vec::new();

        for i in 0..n {
            for j in i + 1..n {
                let dist = euclidean_distance(&embeddings[i], &embeddings[j]);
                distances.push(dist);
            }
        }

        if distances.is_empty() {
            return 0.0;
        }

        // Compute coefficient of variation
        let mean = distances.iter().sum::<f32>() / distances.len() as f32;
        let variance =
            distances.iter().map(|&d| (d - mean).powi(2)).sum::<f32>() / distances.len() as f32;
        let std = variance.sqrt();

        // High CV indicates good separation, low CV indicates collapse
        let cv = if mean > 0.0 { std / mean } else { 0.0 };

        // Normalize to 0-1, where 0 is collapsed, 1 is good
        (cv * 2.0).min(1.0)
    }

    fn compute_spread(&self, embeddings: &[Vec<f32>]) -> f32 {
        if embeddings.is_empty() {
            return 0.0;
        }

        let dim = embeddings[0].len();

        // Compute mean
        let mut mean = vec![0.0; dim];
        for emb in embeddings {
            for (i, &val) in emb.iter().enumerate() {
                mean[i] += val;
            }
        }
        for val in mean.iter_mut() {
            *val /= embeddings.len() as f32;
        }

        // Compute average distance from mean
        let mut total_dist = 0.0;
        for emb in embeddings {
            let dist = euclidean_distance(emb, &mean);
            total_dist += dist;
        }

        total_dist / embeddings.len() as f32
    }

    fn detect_degeneracy(&self, embeddings: &[Vec<f32>]) -> f32 {
        if embeddings.is_empty() || embeddings[0].is_empty() {
            return 1.0; // Fully degenerate
        }

        let n = embeddings.len();
        let dim = embeddings[0].len();

        if n < dim {
            return 0.0; // Cannot determine
        }

        // Compute covariance matrix
        let cov = self.compute_covariance_matrix(embeddings);

        // Estimate rank by counting significant singular values
        let singular_values = self.approximate_singular_values(&cov);

        let significant = singular_values.iter().filter(|&&sv| sv > 1e-6).count();

        // Degeneracy score: 0 = full rank, 1 = rank-1 (collapsed)
        1.0 - (significant as f32 / dim as f32)
    }

    fn compute_covariance_matrix(&self, embeddings: &[Vec<f32>]) -> Array2<f32> {
        let n = embeddings.len();
        let dim = embeddings[0].len();

        // Compute mean
        let mut mean = vec![0.0; dim];
        for emb in embeddings {
            for (i, &val) in emb.iter().enumerate() {
                mean[i] += val;
            }
        }
        for val in mean.iter_mut() {
            *val /= n as f32;
        }

        // Compute covariance
        let mut cov = Array2::zeros((dim, dim));
        for emb in embeddings {
            for i in 0..dim {
                for j in 0..dim {
                    cov[[i, j]] += (emb[i] - mean[i]) * (emb[j] - mean[j]);
                }
            }
        }

        cov.mapv(|x| x / (n - 1) as f32);
        cov
    }

    fn approximate_singular_values(&self, matrix: &Array2<f32>) -> Vec<f32> {
        // Power iteration for largest singular values (simplified)
        let dim = matrix.nrows();
        let mut values = Vec::new();

        // Just return diagonal for approximation
        for i in 0..dim {
            values.push(matrix[[i, i]].abs());
        }

        values.sort_by(|a, b| b.partial_cmp(a).unwrap());
        values
    }

    fn compute_persistence(&self, _graph: &[Vec<usize>], embeddings: &[Vec<f32>]) -> f32 {
        // Simplified persistence: measure how graph structure changes with distance threshold
        let scales = vec![0.1, 0.5, 1.0, 2.0, 5.0];
        let mut component_counts = Vec::new();

        for &scale in &scales {
            let scaled_analyzer = TopologicalAnalyzer::new(self.k_neighbors, scale);
            let scaled_graph = scaled_analyzer.build_knn_graph(embeddings);
            let components =
                scaled_analyzer.count_connected_components(&scaled_graph, embeddings.len());
            component_counts.push(components);
        }

        // Persistence is the variation in component count across scales
        let max_components = *component_counts.iter().max().unwrap_or(&1);
        let min_components = *component_counts.iter().min().unwrap_or(&1);

        (max_components - min_components) as f32 / max_components as f32
    }

    fn compute_quality_score(
        &self,
        mode_collapse: f32,
        degeneracy: f32,
        components: usize,
        clustering: f32,
        spread: f32,
    ) -> f32 {
        // Weighted combination of metrics
        let collapse_score = mode_collapse; // Higher is better
        let degeneracy_score = 1.0 - degeneracy; // Lower degeneracy is better
        let component_score = if components == 1 { 1.0 } else { 0.5 }; // Single component is good
        let clustering_score = clustering; // Higher clustering is good
        let spread_score = (spread / 10.0).min(1.0); // Reasonable spread

        (collapse_score * 0.3
            + degeneracy_score * 0.3
            + component_score * 0.2
            + clustering_score * 0.1
            + spread_score * 0.1)
            .clamp(0.0, 1.0)
    }
}

/// Embedding quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingQuality {
    /// Embedding dimensions
    pub dimensions: usize,
    /// Number of vectors
    pub num_vectors: usize,
    /// Number of connected components
    pub connected_components: usize,
    /// Clustering coefficient (0-1)
    pub clustering_coefficient: f32,
    /// Average node degree
    pub avg_degree: f32,
    /// Degree standard deviation
    pub degree_std: f32,
    /// Mode collapse score (0=collapsed, 1=good)
    pub mode_collapse_score: f32,
    /// Degeneracy score (0=full rank, 1=degenerate)
    pub degeneracy_score: f32,
    /// Average spread from centroid
    pub spread: f32,
    /// Topological persistence score
    pub persistence_score: f32,
    /// Overall quality (0-1, higher is better)
    pub quality_score: f32,
}

impl EmbeddingQuality {
    /// Check if embeddings show signs of mode collapse
    pub fn has_mode_collapse(&self) -> bool {
        self.mode_collapse_score < 0.3
    }

    /// Check if embeddings are degenerate
    pub fn is_degenerate(&self) -> bool {
        self.degeneracy_score > 0.7
    }

    /// Check if embeddings are well-structured
    pub fn is_good_quality(&self) -> bool {
        self.quality_score > 0.7
    }

    /// Get quality assessment
    pub fn assessment(&self) -> &str {
        if self.quality_score > 0.8 {
            "Excellent"
        } else if self.quality_score > 0.6 {
            "Good"
        } else if self.quality_score > 0.4 {
            "Fair"
        } else {
            "Poor"
        }
    }
}

fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_analysis() {
        let analyzer = TopologicalAnalyzer::new(3, 5.0);

        // Create well-separated embeddings
        let embeddings = vec![
            vec![0.0, 0.0],
            vec![0.1, 0.1],
            vec![0.2, 0.2],
            vec![5.0, 5.0],
            vec![5.1, 5.1],
        ];

        let quality = analyzer.analyze(&embeddings).unwrap();

        assert_eq!(quality.dimensions, 2);
        assert_eq!(quality.num_vectors, 5);
        assert!(quality.quality_score > 0.0);
    }

    #[test]
    fn test_mode_collapse_detection() {
        let analyzer = TopologicalAnalyzer::new(2, 10.0);

        // Well-separated embeddings (high CV should give high score)
        let good = vec![vec![0.0, 0.0], vec![5.0, 5.0], vec![10.0, 10.0]];
        let score_good = analyzer.detect_mode_collapse(&good);

        // Collapsed embeddings (all identical, CV = 0)
        let collapsed = vec![vec![1.0, 1.0], vec![1.0, 1.0], vec![1.0, 1.0]];
        let score_collapsed = analyzer.detect_mode_collapse(&collapsed);

        // Identical vectors should have score 0 (distances all same = CV 0)
        assert_eq!(score_collapsed, 0.0);

        // Well-separated should have higher score
        assert!(score_good > score_collapsed);
    }

    #[test]
    fn test_connected_components() {
        let analyzer = TopologicalAnalyzer::new(1, 1.0);

        // Two separate clusters
        let embeddings = vec![
            vec![0.0, 0.0],
            vec![0.5, 0.5],
            vec![10.0, 10.0],
            vec![10.5, 10.5],
        ];

        let graph = analyzer.build_knn_graph(&embeddings);
        let components = analyzer.count_connected_components(&graph, embeddings.len());

        assert!(components >= 2); // Should have at least 2 components
    }

    #[test]
    fn test_quality_assessment() {
        let quality = EmbeddingQuality {
            dimensions: 128,
            num_vectors: 1000,
            connected_components: 1,
            clustering_coefficient: 0.6,
            avg_degree: 5.0,
            degree_std: 1.2,
            mode_collapse_score: 0.8,
            degeneracy_score: 0.2,
            spread: 3.5,
            persistence_score: 0.4,
            quality_score: 0.75,
        };

        assert!(!quality.has_mode_collapse());
        assert!(!quality.is_degenerate());
        assert!(quality.is_good_quality());
        assert_eq!(quality.assessment(), "Good");
    }
}
