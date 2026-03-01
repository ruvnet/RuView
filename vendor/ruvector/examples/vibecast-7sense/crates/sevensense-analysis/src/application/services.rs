//! Application services for analysis operations.
//!
//! These services orchestrate domain operations and coordinate with
//! infrastructure components to perform clustering, motif detection,
//! sequence analysis, and anomaly detection.

use std::collections::HashMap;

use ndarray::Array2;
use thiserror::Error;
use tracing::{debug, info, instrument, warn};

use crate::domain::entities::{
    Anomaly, AnomalyType, Cluster, ClusterId, EmbeddingId, Motif, MotifOccurrence, Prototype,
    RecordingId, SegmentId, SequenceAnalysis,
};
use crate::domain::value_objects::{
    ClusteringConfig, ClusteringMethod, ClusteringResult, DistanceMetric, MotifConfig,
    SequenceMetrics, TransitionMatrix,
};
use crate::infrastructure::{HdbscanClusterer, KMeansClusterer, MarkovAnalyzer};
use crate::metrics::SilhouetteScore;

/// Errors that can occur in analysis services.
#[derive(Debug, Error)]
pub enum AnalysisError {
    /// Insufficient data for analysis.
    #[error("Insufficient data: {0}")]
    InsufficientData(String),

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Clustering failed.
    #[error("Clustering failed: {0}")]
    ClusteringFailed(String),

    /// Motif detection failed.
    #[error("Motif detection failed: {0}")]
    MotifDetectionFailed(String),

    /// Sequence analysis failed.
    #[error("Sequence analysis failed: {0}")]
    SequenceAnalysisFailed(String),

    /// Anomaly detection failed.
    #[error("Anomaly detection failed: {0}")]
    AnomalyDetectionFailed(String),

    /// Internal computation error.
    #[error("Computation error: {0}")]
    ComputationError(String),

    /// Infrastructure error.
    #[error("Infrastructure error: {0}")]
    Infrastructure(String),
}

/// Result type for analysis operations.
pub type Result<T> = std::result::Result<T, AnalysisError>;

/// Embedding data with ID.
pub type EmbeddingWithId = (EmbeddingId, Vec<f32>);

/// Service for clustering embeddings.
///
/// Supports HDBSCAN and K-means clustering algorithms with automatic
/// prototype computation and cluster quality metrics.
pub struct ClusteringService {
    /// Clustering configuration.
    config: ClusteringConfig,
}

impl ClusteringService {
    /// Create a new clustering service with the given configuration.
    #[must_use]
    pub fn new(config: ClusteringConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    #[must_use]
    pub fn default_service() -> Self {
        Self::new(ClusteringConfig::default())
    }

    /// Run HDBSCAN clustering on the provided embeddings.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - Slice of (EmbeddingId, vector) tuples
    ///
    /// # Returns
    ///
    /// A vector of discovered clusters.
    #[instrument(skip(self, embeddings), fields(n_embeddings = embeddings.len()))]
    pub async fn run_hdbscan(
        &self,
        embeddings: &[EmbeddingWithId],
    ) -> Result<Vec<Cluster>> {
        if embeddings.is_empty() {
            return Err(AnalysisError::InsufficientData(
                "Cannot cluster empty embedding set".to_string(),
            ));
        }

        if embeddings.len() < self.config.parameters.min_cluster_size {
            return Err(AnalysisError::InsufficientData(format!(
                "Need at least {} embeddings for HDBSCAN, got {}",
                self.config.parameters.min_cluster_size,
                embeddings.len()
            )));
        }

        info!(
            n_embeddings = embeddings.len(),
            min_cluster_size = self.config.parameters.min_cluster_size,
            min_samples = self.config.parameters.min_samples,
            "Starting HDBSCAN clustering"
        );

        // Build embedding matrix
        let dim = embeddings[0].1.len();
        let n = embeddings.len();
        let mut matrix = Array2::<f32>::zeros((n, dim));

        for (i, (_, vec)) in embeddings.iter().enumerate() {
            if vec.len() != dim {
                return Err(AnalysisError::InvalidConfig(format!(
                    "Embedding dimension mismatch: expected {}, got {}",
                    dim,
                    vec.len()
                )));
            }
            for (j, &val) in vec.iter().enumerate() {
                matrix[[i, j]] = val;
            }
        }

        // Run HDBSCAN
        let clusterer = HdbscanClusterer::new(
            self.config.parameters.min_cluster_size,
            self.config.parameters.min_samples,
            self.config.parameters.metric,
        );

        let labels = clusterer.fit(&matrix)?;

        // Convert labels to clusters
        let clusters = self.labels_to_clusters(embeddings, &labels)?;

        info!(
            n_clusters = clusters.len(),
            "HDBSCAN clustering completed"
        );

        Ok(clusters)
    }

    /// Run K-means clustering on the provided embeddings.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - Slice of (EmbeddingId, vector) tuples
    /// * `k` - Number of clusters
    ///
    /// # Returns
    ///
    /// A vector of k clusters.
    #[instrument(skip(self, embeddings), fields(n_embeddings = embeddings.len(), k = k))]
    pub async fn run_kmeans(
        &self,
        embeddings: &[EmbeddingWithId],
        k: usize,
    ) -> Result<Vec<Cluster>> {
        if embeddings.is_empty() {
            return Err(AnalysisError::InsufficientData(
                "Cannot cluster empty embedding set".to_string(),
            ));
        }

        if embeddings.len() < k {
            return Err(AnalysisError::InsufficientData(format!(
                "Need at least {} embeddings for K-means with k={}, got {}",
                k,
                k,
                embeddings.len()
            )));
        }

        info!(n_embeddings = embeddings.len(), k = k, "Starting K-means clustering");

        // Build embedding matrix
        let dim = embeddings[0].1.len();
        let n = embeddings.len();
        let mut matrix = Array2::<f32>::zeros((n, dim));

        for (i, (_, vec)) in embeddings.iter().enumerate() {
            for (j, &val) in vec.iter().enumerate() {
                matrix[[i, j]] = val;
            }
        }

        // Run K-means
        let clusterer = KMeansClusterer::new(k, self.config.random_seed);
        let (labels, centroids) = clusterer.fit(&matrix)?;

        // Convert labels to clusters with known centroids
        let clusters = self.labels_to_clusters_with_centroids(
            embeddings,
            &labels,
            &centroids,
        )?;

        info!(n_clusters = clusters.len(), "K-means clustering completed");

        Ok(clusters)
    }

    /// Assign an embedding to the nearest cluster.
    ///
    /// # Arguments
    ///
    /// * `embedding` - The embedding vector to assign
    /// * `clusters` - Available clusters to assign to
    ///
    /// # Returns
    ///
    /// The ID of the nearest cluster.
    #[instrument(skip(self, embedding, clusters), fields(n_clusters = clusters.len()))]
    pub async fn assign_to_nearest(
        &self,
        embedding: &[f32],
        clusters: &[Cluster],
    ) -> Result<ClusterId> {
        if clusters.is_empty() {
            return Err(AnalysisError::InsufficientData(
                "No clusters available for assignment".to_string(),
            ));
        }

        let mut min_distance = f32::MAX;
        let mut nearest_cluster = clusters[0].id;

        for cluster in clusters {
            let distance = self.compute_distance(embedding, &cluster.centroid);
            if distance < min_distance {
                min_distance = distance;
                nearest_cluster = cluster.id;
            }
        }

        debug!(
            cluster_id = %nearest_cluster,
            distance = min_distance,
            "Assigned embedding to nearest cluster"
        );

        Ok(nearest_cluster)
    }

    /// Compute prototypes (exemplars) for a cluster.
    ///
    /// # Arguments
    ///
    /// * `cluster` - The cluster to compute prototypes for
    /// * `embeddings` - All embeddings with their IDs
    ///
    /// # Returns
    ///
    /// A vector of prototypes for the cluster.
    #[instrument(skip(self, cluster, embeddings), fields(cluster_id = %cluster.id))]
    pub async fn compute_prototypes(
        &self,
        cluster: &Cluster,
        embeddings: &HashMap<EmbeddingId, Vec<f32>>,
    ) -> Result<Vec<Prototype>> {
        if cluster.member_ids.is_empty() {
            return Ok(Vec::new());
        }

        let n_prototypes = self.config.prototypes_per_cluster.min(cluster.member_ids.len());
        let mut scored_members: Vec<(EmbeddingId, f32)> = Vec::new();

        // Score each member by distance to centroid (lower = better exemplar)
        for &member_id in &cluster.member_ids {
            if let Some(vec) = embeddings.get(&member_id) {
                let distance = self.compute_distance(vec, &cluster.centroid);
                let score = 1.0 / (1.0 + distance); // Higher score = closer to centroid
                scored_members.push((member_id, score));
            }
        }

        // Sort by score descending
        scored_members.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top prototypes
        let prototypes: Vec<Prototype> = scored_members
            .into_iter()
            .take(n_prototypes)
            .map(|(id, score)| Prototype::new(id, cluster.id, score))
            .collect();

        debug!(
            cluster_id = %cluster.id,
            n_prototypes = prototypes.len(),
            "Computed cluster prototypes"
        );

        Ok(prototypes)
    }

    /// Run full clustering pipeline with metrics.
    #[instrument(skip(self, embeddings))]
    pub async fn cluster_with_metrics(
        &self,
        embeddings: &[EmbeddingWithId],
    ) -> Result<ClusteringResult> {
        let clusters = match &self.config.method {
            ClusteringMethod::HDBSCAN => self.run_hdbscan(embeddings).await?,
            ClusteringMethod::KMeans { k } => self.run_kmeans(embeddings, *k).await?,
            _ => {
                return Err(AnalysisError::InvalidConfig(
                    "Unsupported clustering method".to_string(),
                ))
            }
        };

        // Identify noise points (not in any cluster)
        let assigned: std::collections::HashSet<_> = clusters
            .iter()
            .flat_map(|c| c.member_ids.iter())
            .copied()
            .collect();

        let noise: Vec<EmbeddingId> = embeddings
            .iter()
            .map(|(id, _)| *id)
            .filter(|id| !assigned.contains(id))
            .collect();

        // Compute silhouette score if configured
        let silhouette_score = if self.config.compute_silhouette && !clusters.is_empty() {
            let labels = self.clusters_to_labels(&clusters, embeddings);
            let matrix = self.embeddings_to_matrix(embeddings);
            Some(SilhouetteScore::new().compute(&matrix, &labels))
        } else {
            None
        };

        // Compute prototypes
        let embedding_map: HashMap<EmbeddingId, Vec<f32>> = embeddings
            .iter()
            .map(|(id, vec)| (*id, vec.clone()))
            .collect();

        let mut prototypes = Vec::new();
        if self.config.compute_prototypes {
            for cluster in &clusters {
                let cluster_prototypes = self.compute_prototypes(cluster, &embedding_map).await?;
                prototypes.extend(cluster_prototypes);
            }
        }

        Ok(ClusteringResult {
            clusters,
            noise,
            silhouette_score,
            v_measure: None,
            prototypes,
            parameters: self.config.parameters.clone(),
            method: self.config.method.clone(),
        })
    }

    // Helper methods

    fn labels_to_clusters(
        &self,
        embeddings: &[EmbeddingWithId],
        labels: &[i32],
    ) -> Result<Vec<Cluster>> {
        let mut cluster_members: HashMap<i32, Vec<EmbeddingId>> = HashMap::new();
        let mut cluster_vectors: HashMap<i32, Vec<Vec<f32>>> = HashMap::new();

        for ((id, vec), &label) in embeddings.iter().zip(labels.iter()) {
            if label >= 0 {
                cluster_members.entry(label).or_default().push(*id);
                cluster_vectors.entry(label).or_default().push(vec.clone());
            }
        }

        let mut clusters = Vec::new();
        for (label, member_ids) in cluster_members {
            let vectors = &cluster_vectors[&label];
            let centroid = self.compute_centroid(vectors);
            let variance = self.compute_variance(vectors, &centroid);

            let prototype_id = member_ids
                .iter()
                .min_by(|a, b| {
                    let idx_a = embeddings.iter().position(|(id, _)| id == *a).unwrap();
                    let idx_b = embeddings.iter().position(|(id, _)| id == *b).unwrap();
                    let dist_a = self.compute_distance(&embeddings[idx_a].1, &centroid);
                    let dist_b = self.compute_distance(&embeddings[idx_b].1, &centroid);
                    dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied()
                .unwrap_or_else(EmbeddingId::new);

            clusters.push(Cluster::new(prototype_id, member_ids, centroid, variance));
        }

        Ok(clusters)
    }

    fn labels_to_clusters_with_centroids(
        &self,
        embeddings: &[EmbeddingWithId],
        labels: &[usize],
        centroids: &Array2<f32>,
    ) -> Result<Vec<Cluster>> {
        let k = centroids.nrows();
        let mut cluster_members: Vec<Vec<EmbeddingId>> = vec![Vec::new(); k];
        let mut cluster_vectors: Vec<Vec<Vec<f32>>> = vec![Vec::new(); k];

        for ((id, vec), &label) in embeddings.iter().zip(labels.iter()) {
            if label < k {
                cluster_members[label].push(*id);
                cluster_vectors[label].push(vec.clone());
            }
        }

        let mut clusters = Vec::new();
        for (i, member_ids) in cluster_members.into_iter().enumerate() {
            if member_ids.is_empty() {
                continue;
            }

            let centroid: Vec<f32> = centroids.row(i).to_vec();
            let variance = self.compute_variance(&cluster_vectors[i], &centroid);

            let prototype_id = member_ids
                .iter()
                .min_by(|a, b| {
                    let idx_a = embeddings.iter().position(|(id, _)| id == *a).unwrap();
                    let idx_b = embeddings.iter().position(|(id, _)| id == *b).unwrap();
                    let dist_a = self.compute_distance(&embeddings[idx_a].1, &centroid);
                    let dist_b = self.compute_distance(&embeddings[idx_b].1, &centroid);
                    dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
                })
                .copied()
                .unwrap_or_else(EmbeddingId::new);

            clusters.push(Cluster::new(prototype_id, member_ids, centroid, variance));
        }

        Ok(clusters)
    }

    fn compute_centroid(&self, vectors: &[Vec<f32>]) -> Vec<f32> {
        if vectors.is_empty() {
            return Vec::new();
        }

        let dim = vectors[0].len();
        let n = vectors.len() as f32;
        let mut centroid = vec![0.0; dim];

        for vec in vectors {
            for (i, &val) in vec.iter().enumerate() {
                centroid[i] += val;
            }
        }

        for val in &mut centroid {
            *val /= n;
        }

        centroid
    }

    fn compute_variance(&self, vectors: &[Vec<f32>], centroid: &[f32]) -> f32 {
        if vectors.is_empty() {
            return 0.0;
        }

        let mut total_variance = 0.0;
        for vec in vectors {
            let dist = self.compute_distance(vec, centroid);
            total_variance += dist * dist;
        }

        total_variance / vectors.len() as f32
    }

    fn compute_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.config.parameters.metric {
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
            DistanceMetric::Euclidean => {
                a.iter()
                    .zip(b.iter())
                    .map(|(x, y)| (x - y).powi(2))
                    .sum::<f32>()
                    .sqrt()
            }
            DistanceMetric::Manhattan => {
                a.iter()
                    .zip(b.iter())
                    .map(|(x, y)| (x - y).abs())
                    .sum()
            }
            DistanceMetric::Poincare => {
                // Simplified Poincare distance approximation
                let euclidean: f32 = a
                    .iter()
                    .zip(b.iter())
                    .map(|(x, y)| (x - y).powi(2))
                    .sum::<f32>()
                    .sqrt();
                euclidean // Placeholder - full implementation would use hyperbolic geometry
            }
        }
    }

    fn clusters_to_labels(
        &self,
        clusters: &[Cluster],
        embeddings: &[EmbeddingWithId],
    ) -> Vec<i32> {
        let mut labels = vec![-1i32; embeddings.len()];
        let id_to_idx: HashMap<EmbeddingId, usize> = embeddings
            .iter()
            .enumerate()
            .map(|(i, (id, _))| (*id, i))
            .collect();

        for (cluster_idx, cluster) in clusters.iter().enumerate() {
            for member_id in &cluster.member_ids {
                if let Some(&idx) = id_to_idx.get(member_id) {
                    labels[idx] = cluster_idx as i32;
                }
            }
        }

        labels
    }

    fn embeddings_to_matrix(&self, embeddings: &[EmbeddingWithId]) -> Array2<f32> {
        if embeddings.is_empty() {
            return Array2::zeros((0, 0));
        }

        let dim = embeddings[0].1.len();
        let n = embeddings.len();
        let mut matrix = Array2::<f32>::zeros((n, dim));

        for (i, (_, vec)) in embeddings.iter().enumerate() {
            for (j, &val) in vec.iter().enumerate() {
                matrix[[i, j]] = val;
            }
        }

        matrix
    }
}

/// Service for detecting motif patterns in vocalization sequences.
pub struct MotifDetectionService {
    /// Motif detection configuration.
    config: MotifConfig,
}

impl MotifDetectionService {
    /// Create a new motif detection service.
    #[must_use]
    pub fn new(config: MotifConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    #[must_use]
    pub fn default_service() -> Self {
        Self::new(MotifConfig::default())
    }

    /// Detect motifs in cluster sequences.
    ///
    /// # Arguments
    ///
    /// * `sequences` - Collection of cluster sequences to analyze
    /// * `min_length` - Minimum motif length (overrides config)
    ///
    /// # Returns
    ///
    /// A vector of detected motifs.
    #[instrument(skip(self, sequences), fields(n_sequences = sequences.len()))]
    pub async fn detect_motifs(
        &self,
        sequences: &[Vec<ClusterId>],
        min_length: usize,
    ) -> Result<Vec<Motif>> {
        if sequences.is_empty() {
            return Ok(Vec::new());
        }

        let effective_min_length = min_length.max(self.config.min_length);
        let effective_max_length = self.config.max_length;

        info!(
            n_sequences = sequences.len(),
            min_length = effective_min_length,
            max_length = effective_max_length,
            "Starting motif detection"
        );

        let mut all_motifs: HashMap<Vec<ClusterId>, Vec<(usize, usize)>> = HashMap::new();

        // Extract all subsequences of valid lengths
        for (seq_idx, sequence) in sequences.iter().enumerate() {
            for length in effective_min_length..=effective_max_length {
                if sequence.len() < length {
                    continue;
                }

                for start in 0..=(sequence.len() - length) {
                    let subsequence: Vec<ClusterId> = sequence[start..start + length].to_vec();
                    all_motifs
                        .entry(subsequence)
                        .or_default()
                        .push((seq_idx, start));
                }
            }
        }

        // Filter by minimum occurrences and confidence
        let motifs: Vec<Motif> = all_motifs
            .into_iter()
            .filter(|(_, occurrences)| occurrences.len() >= self.config.min_occurrences)
            .filter_map(|(sequence, occurrences)| {
                let n_occurrences = occurrences.len();
                let confidence = n_occurrences as f32 / sequences.len() as f32;

                if confidence < self.config.min_confidence {
                    return None;
                }

                // Estimate average duration (placeholder - would need segment timing data)
                let avg_duration_ms = (sequence.len() * 500) as f64; // Rough estimate

                let mut motif = Motif::new(sequence, n_occurrences, avg_duration_ms, confidence);

                // Add occurrence instances (simplified - would need segment IDs)
                for (_seq_idx, start) in occurrences {
                    motif.add_occurrence(MotifOccurrence::new(
                        RecordingId::new(),
                        Vec::new(),
                        (start * 500) as u64,
                        ((start + motif.length()) * 500) as u64,
                        1.0,
                    ));
                }

                Some(motif)
            })
            .collect();

        info!(n_motifs = motifs.len(), "Motif detection completed");

        Ok(motifs)
    }

    /// Compute a transition matrix from cluster sequences.
    ///
    /// # Arguments
    ///
    /// * `sequences` - Collection of cluster sequences
    ///
    /// # Returns
    ///
    /// A transition matrix representing transition probabilities.
    #[instrument(skip(self, sequences))]
    pub async fn compute_transition_matrix(
        &self,
        sequences: &[Vec<ClusterId>],
    ) -> Result<TransitionMatrix> {
        // Collect all unique clusters
        let mut all_clusters: std::collections::HashSet<ClusterId> = std::collections::HashSet::new();
        for sequence in sequences {
            for cluster in sequence {
                all_clusters.insert(*cluster);
            }
        }

        let cluster_ids: Vec<ClusterId> = all_clusters.into_iter().collect();
        let mut matrix = TransitionMatrix::new(cluster_ids);

        // Record transitions
        for sequence in sequences {
            for window in sequence.windows(2) {
                matrix.record_transition(&window[0], &window[1]);
            }
        }

        // Compute probabilities
        matrix.compute_probabilities();

        debug!(
            n_clusters = matrix.size(),
            n_transitions = matrix.non_zero_transitions().len(),
            "Computed transition matrix"
        );

        Ok(matrix)
    }

    /// Find occurrences of a specific motif in sequences.
    #[instrument(skip(self, motif, sequences))]
    pub async fn find_motif_occurrences(
        &self,
        motif: &Motif,
        sequences: &[(RecordingId, Vec<ClusterId>)],
    ) -> Result<Vec<MotifOccurrence>> {
        let pattern = &motif.sequence;
        let mut occurrences = Vec::new();

        for (recording_id, sequence) in sequences {
            if sequence.len() < pattern.len() {
                continue;
            }

            for start in 0..=(sequence.len() - pattern.len()) {
                let subsequence = &sequence[start..start + pattern.len()];
                if subsequence == pattern.as_slice() {
                    occurrences.push(MotifOccurrence::new(
                        *recording_id,
                        Vec::new(),
                        (start * 500) as u64,
                        ((start + pattern.len()) * 500) as u64,
                        1.0,
                    ));
                }
            }
        }

        Ok(occurrences)
    }
}

/// Service for analyzing vocalization sequences.
pub struct SequenceAnalysisService {
    /// Markov chain analyzer.
    analyzer: MarkovAnalyzer,
}

impl SequenceAnalysisService {
    /// Create a new sequence analysis service.
    #[must_use]
    pub fn new() -> Self {
        Self {
            analyzer: MarkovAnalyzer::new(),
        }
    }

    /// Analyze a sequence of segments with their cluster assignments.
    ///
    /// # Arguments
    ///
    /// * `segment_ids` - Ordered segment IDs from a recording
    /// * `cluster_assignments` - Mapping from segment ID to cluster ID
    /// * `recording_id` - The recording being analyzed
    ///
    /// # Returns
    ///
    /// A SequenceAnalysis with entropy and stereotypy metrics.
    #[instrument(skip(self, segment_ids, cluster_assignments))]
    pub async fn analyze_sequence(
        &self,
        segment_ids: &[SegmentId],
        cluster_assignments: &HashMap<SegmentId, ClusterId>,
        recording_id: RecordingId,
    ) -> Result<SequenceAnalysis> {
        if segment_ids.is_empty() {
            return Err(AnalysisError::InsufficientData(
                "Empty segment sequence".to_string(),
            ));
        }

        // Build cluster sequence
        let cluster_sequence: Vec<ClusterId> = segment_ids
            .iter()
            .filter_map(|seg_id| cluster_assignments.get(seg_id).copied())
            .collect();

        if cluster_sequence.len() < 2 {
            return Err(AnalysisError::InsufficientData(
                "Need at least 2 segments for sequence analysis".to_string(),
            ));
        }

        // Compute transition matrix
        let mut transitions: HashMap<(ClusterId, ClusterId), u32> = HashMap::new();
        for window in cluster_sequence.windows(2) {
            *transitions.entry((window[0], window[1])).or_insert(0) += 1;
        }

        // Convert to probability transitions
        let total_transitions = transitions.values().sum::<u32>() as f32;
        let transition_probs: Vec<(ClusterId, ClusterId, f32)> = transitions
            .into_iter()
            .map(|((from, to), count)| (from, to, count as f32 / total_transitions))
            .collect();

        // Compute entropy
        let entropy = self.compute_entropy(&transition_probs);

        // Compute stereotypy (inverse of normalized entropy)
        let n_unique_clusters = cluster_sequence
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len();
        let max_entropy = (n_unique_clusters as f32).ln();
        let normalized_entropy = if max_entropy > 0.0 {
            entropy / max_entropy
        } else {
            0.0
        };
        let stereotypy_score = 1.0 - normalized_entropy;

        let mut analysis = SequenceAnalysis::new(
            recording_id,
            transition_probs,
            entropy,
            stereotypy_score,
        );
        analysis.set_sequence(cluster_sequence, segment_ids.to_vec());

        info!(
            recording_id = %recording_id,
            entropy = entropy,
            stereotypy = stereotypy_score,
            "Sequence analysis completed"
        );

        Ok(analysis)
    }

    /// Compute Shannon entropy of transition probabilities.
    #[must_use]
    pub fn compute_entropy(&self, transitions: &[(ClusterId, ClusterId, f32)]) -> f32 {
        self.analyzer.compute_entropy(transitions)
    }

    /// Compute sequence metrics from a cluster sequence.
    #[instrument(skip(self, cluster_sequence))]
    pub async fn compute_metrics(
        &self,
        cluster_sequence: &[ClusterId],
    ) -> Result<SequenceMetrics> {
        if cluster_sequence.len() < 2 {
            return Ok(SequenceMetrics::default());
        }

        // Count transitions
        let mut transitions: HashMap<(ClusterId, ClusterId), u32> = HashMap::new();
        let mut self_transitions = 0u32;

        for window in cluster_sequence.windows(2) {
            *transitions.entry((window[0], window[1])).or_insert(0) += 1;
            if window[0] == window[1] {
                self_transitions += 1;
            }
        }

        let total_transitions = (cluster_sequence.len() - 1) as u32;
        let unique_clusters: std::collections::HashSet<_> = cluster_sequence.iter().collect();

        // Compute probabilities and entropy
        let transition_probs: Vec<(ClusterId, ClusterId, f32)> = transitions
            .iter()
            .map(|(&(from, to), &count)| (from, to, count as f32 / total_transitions as f32))
            .collect();

        let entropy = self.compute_entropy(&transition_probs);
        let max_entropy = (unique_clusters.len() as f32).ln().max(1.0);
        let normalized_entropy = entropy / max_entropy;

        // Find dominant transition
        let dominant_transition = transition_probs
            .iter()
            .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            .map(|&(from, to, prob)| (from, to, prob));

        Ok(SequenceMetrics {
            entropy,
            normalized_entropy,
            stereotypy: 1.0 - normalized_entropy,
            unique_clusters: unique_clusters.len(),
            unique_transitions: transitions.len(),
            total_transitions: total_transitions as usize,
            dominant_transition,
            repetition_rate: self_transitions as f32 / total_transitions as f32,
        })
    }
}

impl Default for SequenceAnalysisService {
    fn default() -> Self {
        Self::new()
    }
}

/// Service for detecting anomalous embeddings.
pub struct AnomalyDetectionService {
    /// Anomaly score threshold.
    threshold: f32,

    /// Number of neighbors to consider for local outlier factor.
    k_neighbors: usize,
}

impl AnomalyDetectionService {
    /// Create a new anomaly detection service.
    ///
    /// # Arguments
    ///
    /// * `threshold` - Score threshold above which points are considered anomalous
    /// * `k_neighbors` - Number of neighbors for LOF computation
    #[must_use]
    pub fn new(threshold: f32, k_neighbors: usize) -> Self {
        Self {
            threshold,
            k_neighbors,
        }
    }

    /// Create with default settings.
    #[must_use]
    pub fn default_service() -> Self {
        Self::new(0.5, 20)
    }

    /// Detect anomalies among embeddings based on cluster assignments.
    ///
    /// # Arguments
    ///
    /// * `embeddings` - All embeddings with IDs
    /// * `clusters` - Discovered clusters
    ///
    /// # Returns
    ///
    /// A vector of detected anomalies.
    #[instrument(skip(self, embeddings, clusters))]
    pub async fn detect_anomalies(
        &self,
        embeddings: &[EmbeddingWithId],
        clusters: &[Cluster],
    ) -> Result<Vec<Anomaly>> {
        if clusters.is_empty() {
            return Ok(Vec::new());
        }

        let mut anomalies = Vec::new();

        // Build a map of cluster members
        let assigned: std::collections::HashSet<EmbeddingId> = clusters
            .iter()
            .flat_map(|c| c.member_ids.iter())
            .copied()
            .collect();

        for (embedding_id, vector) in embeddings {
            // Find nearest cluster
            let (nearest_cluster, distance) = clusters
                .iter()
                .map(|c| {
                    let dist = self.cosine_distance(vector, &c.centroid);
                    (c.id, dist)
                })
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap();

            // Compute anomaly score based on distance and cluster assignment
            let in_cluster = assigned.contains(embedding_id);
            let anomaly_score = if in_cluster {
                // For assigned points, score based on distance to centroid
                (distance * 2.0).min(1.0)
            } else {
                // Unassigned points (noise) get higher base score
                (0.5 + distance).min(1.0)
            };

            if anomaly_score > self.threshold {
                let mut anomaly = Anomaly::new(
                    *embedding_id,
                    anomaly_score,
                    nearest_cluster,
                    distance,
                );

                // Classify anomaly type
                if !in_cluster {
                    anomaly.set_type(AnomalyType::Novel);
                } else if distance > 0.5 {
                    anomaly.set_type(AnomalyType::Outlier);
                } else {
                    anomaly.set_type(AnomalyType::Rare);
                }

                anomalies.push(anomaly);
            }
        }

        info!(
            n_anomalies = anomalies.len(),
            threshold = self.threshold,
            "Anomaly detection completed"
        );

        Ok(anomalies)
    }

    /// Compute local outlier factor for embeddings.
    #[instrument(skip(self, embeddings))]
    pub async fn compute_lof(
        &self,
        embeddings: &[EmbeddingWithId],
    ) -> Result<HashMap<EmbeddingId, f32>> {
        if embeddings.len() <= self.k_neighbors {
            warn!(
                n_embeddings = embeddings.len(),
                k = self.k_neighbors,
                "Not enough embeddings for LOF computation"
            );
            return Ok(HashMap::new());
        }

        let n = embeddings.len();
        let k = self.k_neighbors.min(n - 1);

        // Compute pairwise distances
        let mut distances: Vec<Vec<(usize, f32)>> = Vec::with_capacity(n);
        for i in 0..n {
            let mut row: Vec<(usize, f32)> = Vec::with_capacity(n - 1);
            for j in 0..n {
                if i != j {
                    let dist = self.cosine_distance(&embeddings[i].1, &embeddings[j].1);
                    row.push((j, dist));
                }
            }
            row.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            distances.push(row);
        }

        // Compute k-distance for each point
        let k_distances: Vec<f32> = distances
            .iter()
            .map(|d| d.get(k - 1).map_or(f32::MAX, |x| x.1))
            .collect();

        // Compute local reachability density
        let mut lrd: Vec<f32> = vec![0.0; n];
        for i in 0..n {
            let mut reach_dist_sum = 0.0;
            for &(j, dist) in distances[i].iter().take(k) {
                reach_dist_sum += k_distances[j].max(dist);
            }
            lrd[i] = if reach_dist_sum > 0.0 {
                k as f32 / reach_dist_sum
            } else {
                f32::MAX
            };
        }

        // Compute LOF
        let mut lof_scores: HashMap<EmbeddingId, f32> = HashMap::new();
        for i in 0..n {
            let mut lof_sum = 0.0;
            for &(j, _) in distances[i].iter().take(k) {
                if lrd[i] > 0.0 {
                    lof_sum += lrd[j] / lrd[i];
                }
            }
            let lof = lof_sum / k as f32;
            lof_scores.insert(embeddings[i].0, lof);
        }

        Ok(lof_scores)
    }

    /// Classify the type of anomaly based on context.
    #[must_use]
    pub fn classify_anomaly(
        &self,
        anomaly: &Anomaly,
        cluster_member_count: usize,
    ) -> AnomalyType {
        if anomaly.distance_to_centroid > 0.8 {
            AnomalyType::Artifact
        } else if cluster_member_count < 3 {
            AnomalyType::Rare
        } else if anomaly.local_outlier_factor.map_or(false, |lof| lof > 2.0) {
            AnomalyType::Outlier
        } else {
            AnomalyType::Novel
        }
    }

    fn cosine_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            1.0
        } else {
            1.0 - (dot / (norm_a * norm_b))
        }
    }
}

impl Default for AnomalyDetectionService {
    fn default() -> Self {
        Self::default_service()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_embeddings(n: usize, dim: usize) -> Vec<EmbeddingWithId> {
        (0..n)
            .map(|i| {
                let vec: Vec<f32> = (0..dim)
                    .map(|j| ((i * dim + j) as f32 * 0.01).sin())
                    .collect();
                (EmbeddingId::new(), vec)
            })
            .collect()
    }

    #[tokio::test]
    async fn test_clustering_service_insufficient_data() {
        let service = ClusteringService::default_service();
        let result = service.run_hdbscan(&[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_motif_detection_empty_sequences() {
        let service = MotifDetectionService::default_service();
        let motifs = service.detect_motifs(&[], 2).await.unwrap();
        assert!(motifs.is_empty());
    }

    #[tokio::test]
    async fn test_motif_detection_finds_patterns() {
        let service = MotifDetectionService::new(MotifConfig {
            min_length: 2,
            max_length: 5,
            min_occurrences: 2,
            min_confidence: 0.0,
            allow_overlap: false,
            max_gap: 0,
        });

        let c1 = ClusterId::new();
        let c2 = ClusterId::new();
        let c3 = ClusterId::new();

        let sequences = vec![
            vec![c1, c2, c3, c1, c2, c3],
            vec![c1, c2, c3, c2, c1],
            vec![c2, c1, c2, c3, c1, c2],
        ];

        let motifs = service.detect_motifs(&sequences, 2).await.unwrap();
        assert!(!motifs.is_empty());

        // Should find [c1, c2] as a common pattern
        let has_c1_c2 = motifs.iter().any(|m| m.sequence == vec![c1, c2]);
        assert!(has_c1_c2);
    }

    #[tokio::test]
    async fn test_sequence_analysis_computes_entropy() {
        let service = SequenceAnalysisService::new();

        let c1 = ClusterId::new();
        let c2 = ClusterId::new();

        let metrics = service
            .compute_metrics(&[c1, c2, c1, c2, c1, c2])
            .await
            .unwrap();

        assert!(metrics.entropy > 0.0);
        assert!(metrics.stereotypy >= 0.0 && metrics.stereotypy <= 1.0);
    }

    #[tokio::test]
    async fn test_anomaly_detection_empty_clusters() {
        let service = AnomalyDetectionService::default_service();
        let embeddings = create_test_embeddings(10, 16);
        let anomalies = service.detect_anomalies(&embeddings, &[]).await.unwrap();
        assert!(anomalies.is_empty());
    }

    #[test]
    fn test_cosine_distance() {
        let service = AnomalyDetectionService::default_service();

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((service.cosine_distance(&a, &b) - 0.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((service.cosine_distance(&a, &c) - 1.0).abs() < 0.001);
    }
}
