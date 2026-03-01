//! Domain entities for the Analysis bounded context.
//!
//! This module contains the core domain entities representing clusters,
//! prototypes, motifs, sequences, and anomalies in bioacoustic analysis.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Unique identifier for a cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClusterId(Uuid);

impl ClusterId {
    /// Create a new random cluster ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a cluster ID from a UUID.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID.
    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Noise cluster ID (used for HDBSCAN noise points).
    #[must_use]
    pub fn noise() -> Self {
        Self(Uuid::nil())
    }

    /// Check if this is the noise cluster.
    #[must_use]
    pub fn is_noise(&self) -> bool {
        self.0.is_nil()
    }
}

impl Default for ClusterId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ClusterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for ClusterId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Unique identifier for an embedding (from sevensense-embedding context).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmbeddingId(Uuid);

impl EmbeddingId {
    /// Create a new random embedding ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from UUID.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID.
    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for EmbeddingId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EmbeddingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for EmbeddingId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Unique identifier for a recording (from sevensense-audio context).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecordingId(Uuid);

impl RecordingId {
    /// Create a new random recording ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from UUID.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID.
    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for RecordingId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RecordingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for RecordingId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Unique identifier for a segment (from sevensense-audio context).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SegmentId(Uuid);

impl SegmentId {
    /// Create a new random segment ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from UUID.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID.
    #[must_use]
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for SegmentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SegmentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for SegmentId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// A cluster of acoustically similar call segments.
///
/// Clusters group embeddings that represent similar vocalizations,
/// enabling pattern discovery and call type identification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    /// Unique identifier for this cluster.
    pub id: ClusterId,

    /// The prototype (representative) embedding ID for this cluster.
    pub prototype_id: EmbeddingId,

    /// IDs of all embeddings belonging to this cluster.
    pub member_ids: Vec<EmbeddingId>,

    /// Centroid vector (mean of all member embeddings).
    pub centroid: Vec<f32>,

    /// Variance within the cluster (measure of spread).
    pub variance: f32,

    /// Optional human-readable label for the cluster.
    pub label: Option<String>,

    /// Timestamp when the cluster was created.
    pub created_at: DateTime<Utc>,

    /// Timestamp when the cluster was last updated.
    pub updated_at: DateTime<Utc>,
}

impl Cluster {
    /// Create a new cluster with the given parameters.
    #[must_use]
    pub fn new(
        prototype_id: EmbeddingId,
        member_ids: Vec<EmbeddingId>,
        centroid: Vec<f32>,
        variance: f32,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: ClusterId::new(),
            prototype_id,
            member_ids,
            centroid,
            variance,
            label: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Get the number of members in this cluster.
    #[must_use]
    pub fn member_count(&self) -> usize {
        self.member_ids.len()
    }

    /// Check if an embedding is a member of this cluster.
    #[must_use]
    pub fn contains(&self, embedding_id: &EmbeddingId) -> bool {
        self.member_ids.contains(embedding_id)
    }

    /// Add a member to the cluster.
    pub fn add_member(&mut self, embedding_id: EmbeddingId) {
        if !self.member_ids.contains(&embedding_id) {
            self.member_ids.push(embedding_id);
            self.updated_at = Utc::now();
        }
    }

    /// Remove a member from the cluster.
    pub fn remove_member(&mut self, embedding_id: &EmbeddingId) -> bool {
        if let Some(pos) = self.member_ids.iter().position(|id| id == embedding_id) {
            self.member_ids.remove(pos);
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }

    /// Update the centroid vector.
    pub fn update_centroid(&mut self, centroid: Vec<f32>, variance: f32) {
        self.centroid = centroid;
        self.variance = variance;
        self.updated_at = Utc::now();
    }

    /// Set a human-readable label for this cluster.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = Some(label.into());
        self.updated_at = Utc::now();
    }
}

/// A prototype (exemplar) embedding that best represents a cluster.
///
/// Prototypes are actual call segments that serve as the most representative
/// examples of their cluster, useful for visualization and interpretation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prototype {
    /// The embedding ID of this prototype.
    pub id: EmbeddingId,

    /// The cluster this prototype represents.
    pub cluster_id: ClusterId,

    /// Score indicating how well this exemplar represents the cluster.
    /// Higher scores indicate better representation.
    pub exemplar_score: f32,

    /// Optional path to the spectrogram image for visualization.
    pub spectrogram_path: Option<PathBuf>,

    /// Timestamp when this prototype was identified.
    pub created_at: DateTime<Utc>,
}

impl Prototype {
    /// Create a new prototype.
    #[must_use]
    pub fn new(
        id: EmbeddingId,
        cluster_id: ClusterId,
        exemplar_score: f32,
    ) -> Self {
        Self {
            id,
            cluster_id,
            exemplar_score,
            spectrogram_path: None,
            created_at: Utc::now(),
        }
    }

    /// Set the spectrogram path for this prototype.
    pub fn set_spectrogram_path(&mut self, path: impl Into<PathBuf>) {
        self.spectrogram_path = Some(path.into());
    }
}

/// A motif (recurring pattern) in vocalization sequences.
///
/// Motifs represent frequently occurring sequences of cluster assignments,
/// indicating repeated vocal phrases or behavioral patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Motif {
    /// Unique identifier for this motif.
    pub id: String,

    /// The sequence of cluster IDs that define this motif.
    pub sequence: Vec<ClusterId>,

    /// Number of times this motif occurs in the analyzed data.
    pub occurrences: usize,

    /// Average duration of this motif in milliseconds.
    pub avg_duration_ms: f64,

    /// Confidence score for this motif (0.0 to 1.0).
    pub confidence: f32,

    /// All occurrences of this motif.
    pub occurrence_instances: Vec<MotifOccurrence>,

    /// Timestamp when this motif was discovered.
    pub discovered_at: DateTime<Utc>,
}

impl Motif {
    /// Create a new motif.
    #[must_use]
    pub fn new(
        sequence: Vec<ClusterId>,
        occurrences: usize,
        avg_duration_ms: f64,
        confidence: f32,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            sequence,
            occurrences,
            avg_duration_ms,
            confidence,
            occurrence_instances: Vec::new(),
            discovered_at: Utc::now(),
        }
    }

    /// Get the length of this motif (number of clusters).
    #[must_use]
    pub fn length(&self) -> usize {
        self.sequence.len()
    }

    /// Add an occurrence instance to this motif.
    pub fn add_occurrence(&mut self, occurrence: MotifOccurrence) {
        self.occurrence_instances.push(occurrence);
        self.occurrences = self.occurrence_instances.len();
    }

    /// Check if this motif contains a specific cluster.
    #[must_use]
    pub fn contains_cluster(&self, cluster_id: &ClusterId) -> bool {
        self.sequence.contains(cluster_id)
    }
}

/// A specific occurrence of a motif in a recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotifOccurrence {
    /// The recording where this occurrence was found.
    pub recording_id: RecordingId,

    /// The segment IDs that make up this occurrence.
    pub segment_ids: Vec<SegmentId>,

    /// Start time within the recording (milliseconds).
    pub start_time_ms: u64,

    /// End time within the recording (milliseconds).
    pub end_time_ms: u64,

    /// Similarity score to the motif template.
    pub similarity: f32,
}

impl MotifOccurrence {
    /// Create a new motif occurrence.
    #[must_use]
    pub fn new(
        recording_id: RecordingId,
        segment_ids: Vec<SegmentId>,
        start_time_ms: u64,
        end_time_ms: u64,
        similarity: f32,
    ) -> Self {
        Self {
            recording_id,
            segment_ids,
            start_time_ms,
            end_time_ms,
            similarity,
        }
    }

    /// Get the duration of this occurrence in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        self.end_time_ms.saturating_sub(self.start_time_ms)
    }
}

/// Analysis of a vocalization sequence from a recording.
///
/// Contains transition information, entropy metrics, and stereotypy scores
/// for understanding sequential patterns in bird vocalizations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceAnalysis {
    /// The recording this analysis pertains to.
    pub recording_id: RecordingId,

    /// Transitions between clusters with weights (probabilities).
    /// Format: (source_cluster, target_cluster, probability)
    pub transitions: Vec<(ClusterId, ClusterId, f32)>,

    /// Shannon entropy of the transition distribution.
    /// Higher values indicate more unpredictable sequences.
    pub entropy: f32,

    /// Stereotypy score (0.0 to 1.0).
    /// Higher values indicate more repetitive/stereotyped sequences.
    pub stereotypy_score: f32,

    /// The sequence of cluster IDs in order.
    pub cluster_sequence: Vec<ClusterId>,

    /// The segment IDs corresponding to the cluster sequence.
    pub segment_ids: Vec<SegmentId>,

    /// Timestamp when this analysis was performed.
    pub analyzed_at: DateTime<Utc>,
}

impl SequenceAnalysis {
    /// Create a new sequence analysis.
    #[must_use]
    pub fn new(
        recording_id: RecordingId,
        transitions: Vec<(ClusterId, ClusterId, f32)>,
        entropy: f32,
        stereotypy_score: f32,
    ) -> Self {
        Self {
            recording_id,
            transitions,
            entropy,
            stereotypy_score,
            cluster_sequence: Vec::new(),
            segment_ids: Vec::new(),
            analyzed_at: Utc::now(),
        }
    }

    /// Get the number of unique transitions.
    #[must_use]
    pub fn unique_transition_count(&self) -> usize {
        self.transitions.len()
    }

    /// Get all clusters involved in the sequence.
    #[must_use]
    pub fn unique_clusters(&self) -> Vec<ClusterId> {
        let mut clusters: Vec<ClusterId> = self.cluster_sequence.clone();
        clusters.sort_by_key(|c| c.as_uuid());
        clusters.dedup();
        clusters
    }

    /// Set the cluster sequence and corresponding segment IDs.
    pub fn set_sequence(&mut self, clusters: Vec<ClusterId>, segments: Vec<SegmentId>) {
        self.cluster_sequence = clusters;
        self.segment_ids = segments;
    }
}

/// Type of anomaly detected in the analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnomalyType {
    /// Rare vocalization (low occurrence count).
    Rare,
    /// Novel vocalization (doesn't fit any cluster well).
    Novel,
    /// Artifact (likely noise or recording issue).
    Artifact,
    /// Outlier within a cluster.
    Outlier,
    /// Unknown anomaly type.
    Unknown,
}

impl std::fmt::Display for AnomalyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnomalyType::Rare => write!(f, "Rare"),
            AnomalyType::Novel => write!(f, "Novel"),
            AnomalyType::Artifact => write!(f, "Artifact"),
            AnomalyType::Outlier => write!(f, "Outlier"),
            AnomalyType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// An anomalous embedding that doesn't fit well into any cluster.
///
/// Anomalies can represent rare vocalizations, novel sounds, or artifacts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    /// The embedding that is anomalous.
    pub embedding_id: EmbeddingId,

    /// Anomaly score (higher = more anomalous).
    pub anomaly_score: f32,

    /// The nearest cluster to this anomaly.
    pub nearest_cluster: ClusterId,

    /// Distance from the anomaly to the nearest cluster's centroid.
    pub distance_to_centroid: f32,

    /// Type of anomaly detected.
    pub anomaly_type: AnomalyType,

    /// Local outlier factor (if computed).
    pub local_outlier_factor: Option<f32>,

    /// Timestamp when this anomaly was detected.
    pub detected_at: DateTime<Utc>,
}

impl Anomaly {
    /// Create a new anomaly.
    #[must_use]
    pub fn new(
        embedding_id: EmbeddingId,
        anomaly_score: f32,
        nearest_cluster: ClusterId,
        distance_to_centroid: f32,
    ) -> Self {
        Self {
            embedding_id,
            anomaly_score,
            nearest_cluster,
            distance_to_centroid,
            anomaly_type: AnomalyType::Unknown,
            local_outlier_factor: None,
            detected_at: Utc::now(),
        }
    }

    /// Set the anomaly type.
    pub fn set_type(&mut self, anomaly_type: AnomalyType) {
        self.anomaly_type = anomaly_type;
    }

    /// Set the local outlier factor.
    pub fn set_lof(&mut self, lof: f32) {
        self.local_outlier_factor = Some(lof);
    }

    /// Check if this is a severe anomaly (score > threshold).
    #[must_use]
    pub fn is_severe(&self, threshold: f32) -> bool {
        self.anomaly_score > threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_id_creation() {
        let id1 = ClusterId::new();
        let id2 = ClusterId::new();
        assert_ne!(id1, id2);

        let noise = ClusterId::noise();
        assert!(noise.is_noise());
        assert!(!id1.is_noise());
    }

    #[test]
    fn test_cluster_member_operations() {
        let mut cluster = Cluster::new(
            EmbeddingId::new(),
            vec![EmbeddingId::new()],
            vec![0.0; 1536],
            0.1,
        );

        let new_member = EmbeddingId::new();
        cluster.add_member(new_member);
        assert_eq!(cluster.member_count(), 2);
        assert!(cluster.contains(&new_member));

        cluster.remove_member(&new_member);
        assert_eq!(cluster.member_count(), 1);
        assert!(!cluster.contains(&new_member));
    }

    #[test]
    fn test_motif_length() {
        let motif = Motif::new(
            vec![ClusterId::new(), ClusterId::new(), ClusterId::new()],
            5,
            1500.0,
            0.85,
        );
        assert_eq!(motif.length(), 3);
        assert_eq!(motif.occurrences, 5);
    }

    #[test]
    fn test_sequence_analysis_unique_clusters() {
        let c1 = ClusterId::new();
        let c2 = ClusterId::new();

        let mut analysis = SequenceAnalysis::new(
            RecordingId::new(),
            vec![],
            1.5,
            0.3,
        );
        analysis.set_sequence(
            vec![c1, c2, c1, c2, c1],
            vec![SegmentId::new(); 5],
        );

        let unique = analysis.unique_clusters();
        assert_eq!(unique.len(), 2);
    }

    #[test]
    fn test_anomaly_severity() {
        let mut anomaly = Anomaly::new(
            EmbeddingId::new(),
            0.8,
            ClusterId::new(),
            2.5,
        );

        assert!(anomaly.is_severe(0.5));
        assert!(!anomaly.is_severe(0.9));

        anomaly.set_type(AnomalyType::Novel);
        assert_eq!(anomaly.anomaly_type, AnomalyType::Novel);
    }
}
