//! Value objects for the Analysis bounded context.
//!
//! Value objects are immutable objects that represent concepts without identity.
//! They are defined by their attributes rather than a unique identifier.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::entities::ClusterId;

/// Method used for clustering embeddings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClusteringMethod {
    /// HDBSCAN (Hierarchical Density-Based Spatial Clustering).
    /// Good for discovering clusters of varying densities and shapes.
    HDBSCAN,

    /// K-Means clustering with fixed number of clusters.
    KMeans {
        /// Number of clusters to create.
        k: usize,
    },

    /// Spectral clustering using eigenvalues of similarity matrix.
    Spectral {
        /// Number of clusters to create.
        n_clusters: usize,
    },

    /// Agglomerative hierarchical clustering.
    Agglomerative {
        /// Number of clusters to create.
        n_clusters: usize,
        /// Linkage criterion (ward, complete, average, single).
        linkage: LinkageMethod,
    },
}

impl Default for ClusteringMethod {
    fn default() -> Self {
        Self::HDBSCAN
    }
}

impl std::fmt::Display for ClusteringMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClusteringMethod::HDBSCAN => write!(f, "HDBSCAN"),
            ClusteringMethod::KMeans { k } => write!(f, "K-Means (k={})", k),
            ClusteringMethod::Spectral { n_clusters } => {
                write!(f, "Spectral (n={})", n_clusters)
            }
            ClusteringMethod::Agglomerative { n_clusters, linkage } => {
                write!(f, "Agglomerative (n={}, {:?})", n_clusters, linkage)
            }
        }
    }
}

/// Linkage method for agglomerative clustering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkageMethod {
    /// Ward's minimum variance method.
    Ward,
    /// Complete linkage (maximum distance).
    Complete,
    /// Average linkage (mean distance).
    Average,
    /// Single linkage (minimum distance).
    Single,
}

impl Default for LinkageMethod {
    fn default() -> Self {
        Self::Ward
    }
}

/// Distance metric for clustering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    /// Euclidean distance (L2 norm).
    Euclidean,
    /// Cosine distance (1 - cosine similarity).
    Cosine,
    /// Manhattan distance (L1 norm).
    Manhattan,
    /// Poincare distance (hyperbolic space).
    Poincare,
}

impl Default for DistanceMetric {
    fn default() -> Self {
        Self::Cosine
    }
}

impl std::fmt::Display for DistanceMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DistanceMetric::Euclidean => write!(f, "Euclidean"),
            DistanceMetric::Cosine => write!(f, "Cosine"),
            DistanceMetric::Manhattan => write!(f, "Manhattan"),
            DistanceMetric::Poincare => write!(f, "Poincare"),
        }
    }
}

/// Parameters for clustering algorithms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusteringParameters {
    /// Minimum number of points to form a cluster (HDBSCAN).
    pub min_cluster_size: usize,

    /// Minimum number of samples in neighborhood (HDBSCAN).
    pub min_samples: usize,

    /// Epsilon for DBSCAN-like algorithms (optional distance threshold).
    pub epsilon: Option<f32>,

    /// Distance metric to use.
    pub metric: DistanceMetric,

    /// Maximum number of clusters (optional limit).
    pub max_clusters: Option<usize>,

    /// Whether to allow single-point clusters.
    pub allow_single_cluster: bool,
}

impl Default for ClusteringParameters {
    fn default() -> Self {
        Self {
            min_cluster_size: 5,
            min_samples: 3,
            epsilon: None,
            metric: DistanceMetric::Cosine,
            max_clusters: None,
            allow_single_cluster: false,
        }
    }
}

impl ClusteringParameters {
    /// Create parameters for HDBSCAN.
    #[must_use]
    pub fn hdbscan(min_cluster_size: usize, min_samples: usize) -> Self {
        Self {
            min_cluster_size,
            min_samples,
            ..Default::default()
        }
    }

    /// Create parameters for K-means.
    #[must_use]
    pub fn kmeans() -> Self {
        Self {
            min_cluster_size: 1,
            min_samples: 1,
            allow_single_cluster: true,
            ..Default::default()
        }
    }

    /// Set the distance metric.
    #[must_use]
    pub fn with_metric(mut self, metric: DistanceMetric) -> Self {
        self.metric = metric;
        self
    }

    /// Set the epsilon threshold.
    #[must_use]
    pub fn with_epsilon(mut self, epsilon: f32) -> Self {
        self.epsilon = Some(epsilon);
        self
    }
}

/// Configuration for clustering operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusteringConfig {
    /// The clustering method to use.
    pub method: ClusteringMethod,

    /// Parameters for the clustering algorithm.
    pub parameters: ClusteringParameters,

    /// Whether to compute cluster prototypes.
    pub compute_prototypes: bool,

    /// Number of prototypes to compute per cluster.
    pub prototypes_per_cluster: usize,

    /// Whether to compute silhouette scores.
    pub compute_silhouette: bool,

    /// Random seed for reproducibility.
    pub random_seed: Option<u64>,
}

impl Default for ClusteringConfig {
    fn default() -> Self {
        Self {
            method: ClusteringMethod::HDBSCAN,
            parameters: ClusteringParameters::default(),
            compute_prototypes: true,
            prototypes_per_cluster: 3,
            compute_silhouette: true,
            random_seed: None,
        }
    }
}

impl ClusteringConfig {
    /// Create a HDBSCAN configuration.
    #[must_use]
    pub fn hdbscan(min_cluster_size: usize, min_samples: usize) -> Self {
        Self {
            method: ClusteringMethod::HDBSCAN,
            parameters: ClusteringParameters::hdbscan(min_cluster_size, min_samples),
            ..Default::default()
        }
    }

    /// Create a K-means configuration.
    #[must_use]
    pub fn kmeans(k: usize) -> Self {
        Self {
            method: ClusteringMethod::KMeans { k },
            parameters: ClusteringParameters::kmeans(),
            ..Default::default()
        }
    }

    /// Set a random seed for reproducibility.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.random_seed = Some(seed);
        self
    }
}

/// Configuration for motif detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotifConfig {
    /// Minimum length of motifs to detect.
    pub min_length: usize,

    /// Maximum length of motifs to detect.
    pub max_length: usize,

    /// Minimum number of occurrences for a motif.
    pub min_occurrences: usize,

    /// Minimum confidence threshold for motifs.
    pub min_confidence: f32,

    /// Whether to allow overlapping occurrences.
    pub allow_overlap: bool,

    /// Maximum gap (in clusters) between motif elements.
    pub max_gap: usize,
}

impl Default for MotifConfig {
    fn default() -> Self {
        Self {
            min_length: 2,
            max_length: 10,
            min_occurrences: 3,
            min_confidence: 0.5,
            allow_overlap: false,
            max_gap: 0,
        }
    }
}

impl MotifConfig {
    /// Create a strict motif configuration (no gaps, no overlap).
    #[must_use]
    pub fn strict() -> Self {
        Self {
            min_length: 3,
            max_length: 8,
            min_occurrences: 5,
            min_confidence: 0.7,
            allow_overlap: false,
            max_gap: 0,
        }
    }

    /// Create a relaxed motif configuration (allows gaps).
    #[must_use]
    pub fn relaxed() -> Self {
        Self {
            min_length: 2,
            max_length: 15,
            min_occurrences: 2,
            min_confidence: 0.3,
            allow_overlap: true,
            max_gap: 2,
        }
    }

    /// Set the length range.
    #[must_use]
    pub fn with_length_range(mut self, min: usize, max: usize) -> Self {
        self.min_length = min;
        self.max_length = max;
        self
    }
}

/// Metrics computed from sequence analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceMetrics {
    /// Shannon entropy of the sequence.
    pub entropy: f32,

    /// Normalized entropy (entropy / max_entropy).
    pub normalized_entropy: f32,

    /// Stereotypy score (1 - normalized_entropy).
    pub stereotypy: f32,

    /// Number of unique clusters in the sequence.
    pub unique_clusters: usize,

    /// Number of unique transitions in the sequence.
    pub unique_transitions: usize,

    /// Total number of transitions.
    pub total_transitions: usize,

    /// Most common transition and its probability.
    pub dominant_transition: Option<(ClusterId, ClusterId, f32)>,

    /// Repetition rate (self-transitions / total).
    pub repetition_rate: f32,
}

impl Default for SequenceMetrics {
    fn default() -> Self {
        Self {
            entropy: 0.0,
            normalized_entropy: 0.0,
            stereotypy: 1.0,
            unique_clusters: 0,
            unique_transitions: 0,
            total_transitions: 0,
            dominant_transition: None,
            repetition_rate: 0.0,
        }
    }
}

/// Transition matrix for Markov chain analysis.
///
/// Represents the probabilities of transitioning from one cluster to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionMatrix {
    /// Ordered list of cluster IDs (defines row/column indices).
    pub cluster_ids: Vec<ClusterId>,

    /// Transition probabilities (row = source, column = target).
    /// Values are probabilities (0.0 to 1.0, rows sum to 1.0).
    pub probabilities: Vec<Vec<f32>>,

    /// Raw observation counts (row = source, column = target).
    pub observations: Vec<Vec<u32>>,

    /// Mapping from ClusterId to matrix index.
    #[serde(skip)]
    index_map: HashMap<ClusterId, usize>,
}

impl TransitionMatrix {
    /// Create a new transition matrix for the given clusters.
    #[must_use]
    pub fn new(cluster_ids: Vec<ClusterId>) -> Self {
        let n = cluster_ids.len();
        let index_map: HashMap<ClusterId, usize> = cluster_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();

        Self {
            cluster_ids,
            probabilities: vec![vec![0.0; n]; n],
            observations: vec![vec![0; n]; n],
            index_map,
        }
    }

    /// Get the number of clusters (states) in the matrix.
    #[must_use]
    pub fn size(&self) -> usize {
        self.cluster_ids.len()
    }

    /// Get the index for a cluster ID.
    #[must_use]
    pub fn index_of(&self, cluster_id: &ClusterId) -> Option<usize> {
        self.index_map.get(cluster_id).copied()
    }

    /// Record an observed transition.
    pub fn record_transition(&mut self, from: &ClusterId, to: &ClusterId) {
        if let (Some(i), Some(j)) = (self.index_of(from), self.index_of(to)) {
            self.observations[i][j] += 1;
        }
    }

    /// Compute probabilities from observation counts.
    pub fn compute_probabilities(&mut self) {
        for i in 0..self.size() {
            let row_sum: u32 = self.observations[i].iter().sum();
            if row_sum > 0 {
                for j in 0..self.size() {
                    self.probabilities[i][j] = self.observations[i][j] as f32 / row_sum as f32;
                }
            }
        }
    }

    /// Get the transition probability from one cluster to another.
    #[must_use]
    pub fn probability(&self, from: &ClusterId, to: &ClusterId) -> Option<f32> {
        match (self.index_of(from), self.index_of(to)) {
            (Some(i), Some(j)) => Some(self.probabilities[i][j]),
            _ => None,
        }
    }

    /// Get the observation count for a transition.
    #[must_use]
    pub fn observation_count(&self, from: &ClusterId, to: &ClusterId) -> Option<u32> {
        match (self.index_of(from), self.index_of(to)) {
            (Some(i), Some(j)) => Some(self.observations[i][j]),
            _ => None,
        }
    }

    /// Get all non-zero transitions as (from, to, probability) tuples.
    #[must_use]
    pub fn non_zero_transitions(&self) -> Vec<(ClusterId, ClusterId, f32)> {
        let mut transitions = Vec::new();
        for (i, from) in self.cluster_ids.iter().enumerate() {
            for (j, to) in self.cluster_ids.iter().enumerate() {
                let prob = self.probabilities[i][j];
                if prob > 0.0 {
                    transitions.push((*from, *to, prob));
                }
            }
        }
        transitions
    }

    /// Get the stationary distribution (eigenvector of eigenvalue 1).
    /// Returns None if the matrix is not ergodic.
    #[must_use]
    pub fn stationary_distribution(&self) -> Option<Vec<f32>> {
        // Power iteration method for finding stationary distribution
        let n = self.size();
        if n == 0 {
            return None;
        }

        let mut dist = vec![1.0 / n as f32; n];
        let max_iterations = 1000;
        let tolerance = 1e-8;

        for _ in 0..max_iterations {
            let mut new_dist = vec![0.0; n];

            // Matrix-vector multiplication: new_dist = dist * P^T
            for j in 0..n {
                for i in 0..n {
                    new_dist[j] += dist[i] * self.probabilities[i][j];
                }
            }

            // Check convergence
            let diff: f32 = dist
                .iter()
                .zip(new_dist.iter())
                .map(|(a, b)| (a - b).abs())
                .sum();

            dist = new_dist;

            if diff < tolerance {
                return Some(dist);
            }
        }

        Some(dist)
    }

    /// Rebuild the index map (needed after deserialization).
    pub fn rebuild_index_map(&mut self) {
        self.index_map = self
            .cluster_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (*id, i))
            .collect();
    }
}

/// Result of a clustering operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusteringResult {
    /// The clusters discovered.
    pub clusters: Vec<super::entities::Cluster>,

    /// Embeddings classified as noise (HDBSCAN).
    pub noise: Vec<super::entities::EmbeddingId>,

    /// Silhouette score (if computed).
    pub silhouette_score: Option<f32>,

    /// V-measure score (if ground truth available).
    pub v_measure: Option<f32>,

    /// Prototypes for each cluster.
    pub prototypes: Vec<super::entities::Prototype>,

    /// Parameters used for clustering.
    pub parameters: ClusteringParameters,

    /// Method used for clustering.
    pub method: ClusteringMethod,
}

impl ClusteringResult {
    /// Get the number of clusters (excluding noise).
    #[must_use]
    pub fn cluster_count(&self) -> usize {
        self.clusters.len()
    }

    /// Get the noise rate (proportion of points in noise).
    #[must_use]
    pub fn noise_rate(&self) -> f32 {
        let total = self
            .clusters
            .iter()
            .map(|c| c.member_count())
            .sum::<usize>()
            + self.noise.len();
        if total == 0 {
            0.0
        } else {
            self.noise.len() as f32 / total as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clustering_config_creation() {
        let config = ClusteringConfig::hdbscan(10, 5);
        assert!(matches!(config.method, ClusteringMethod::HDBSCAN));
        assert_eq!(config.parameters.min_cluster_size, 10);
        assert_eq!(config.parameters.min_samples, 5);
    }

    #[test]
    fn test_transition_matrix() {
        let c1 = ClusterId::new();
        let c2 = ClusterId::new();
        let c3 = ClusterId::new();

        let mut matrix = TransitionMatrix::new(vec![c1, c2, c3]);

        // Record some transitions
        matrix.record_transition(&c1, &c2);
        matrix.record_transition(&c1, &c2);
        matrix.record_transition(&c1, &c3);
        matrix.record_transition(&c2, &c1);

        matrix.compute_probabilities();

        // c1 -> c2 should be 2/3
        assert!((matrix.probability(&c1, &c2).unwrap() - 2.0 / 3.0).abs() < 0.001);
        // c1 -> c3 should be 1/3
        assert!((matrix.probability(&c1, &c3).unwrap() - 1.0 / 3.0).abs() < 0.001);
        // c2 -> c1 should be 1.0
        assert!((matrix.probability(&c2, &c1).unwrap() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_motif_config() {
        let config = MotifConfig::strict();
        assert_eq!(config.min_length, 3);
        assert_eq!(config.min_occurrences, 5);
        assert!(!config.allow_overlap);

        let relaxed = MotifConfig::relaxed();
        assert!(relaxed.allow_overlap);
        assert_eq!(relaxed.max_gap, 2);
    }

    #[test]
    fn test_distance_metric_display() {
        assert_eq!(format!("{}", DistanceMetric::Cosine), "Cosine");
        assert_eq!(format!("{}", DistanceMetric::Euclidean), "Euclidean");
    }
}
