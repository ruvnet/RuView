//! Domain events for the Analysis bounded context.
//!
//! Domain events represent significant occurrences within the Analysis domain
//! that other parts of the system may need to react to.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::entities::{AnomalyType, ClusterId, EmbeddingId, RecordingId};
use super::value_objects::ClusteringMethod;

/// Base trait for analysis domain events.
pub trait AnalysisEvent: Send + Sync {
    /// Get the unique event ID.
    fn event_id(&self) -> Uuid;

    /// Get the timestamp when the event occurred.
    fn occurred_at(&self) -> DateTime<Utc>;

    /// Get the event type name.
    fn event_type(&self) -> &'static str;
}

/// Event emitted when clustering is completed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClustersDiscovered {
    /// Unique event ID.
    pub event_id: Uuid,

    /// When the event occurred.
    pub occurred_at: DateTime<Utc>,

    /// Number of clusters discovered.
    pub cluster_count: usize,

    /// Number of noise points (not assigned to any cluster).
    pub noise_count: usize,

    /// Clustering method used.
    pub method: ClusteringMethod,

    /// Silhouette score (if computed).
    pub silhouette_score: Option<f32>,

    /// Total number of embeddings processed.
    pub total_embeddings: usize,
}

impl ClustersDiscovered {
    /// Create a new ClustersDiscovered event.
    #[must_use]
    pub fn new(
        cluster_count: usize,
        noise_count: usize,
        method: ClusteringMethod,
        total_embeddings: usize,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            cluster_count,
            noise_count,
            method,
            silhouette_score: None,
            total_embeddings,
        }
    }

    /// Add silhouette score to the event.
    #[must_use]
    pub fn with_silhouette_score(mut self, score: f32) -> Self {
        self.silhouette_score = Some(score);
        self
    }
}

impl AnalysisEvent for ClustersDiscovered {
    fn event_id(&self) -> Uuid {
        self.event_id
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn event_type(&self) -> &'static str {
        "ClustersDiscovered"
    }
}

/// Event emitted when an embedding is assigned to a cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterAssigned {
    /// Unique event ID.
    pub event_id: Uuid,

    /// When the event occurred.
    pub occurred_at: DateTime<Utc>,

    /// The embedding that was assigned.
    pub embedding_id: EmbeddingId,

    /// The cluster it was assigned to.
    pub cluster_id: ClusterId,

    /// Confidence/probability of the assignment.
    pub confidence: f32,

    /// Distance to the cluster centroid.
    pub distance_to_centroid: f32,
}

impl ClusterAssigned {
    /// Create a new ClusterAssigned event.
    #[must_use]
    pub fn new(
        embedding_id: EmbeddingId,
        cluster_id: ClusterId,
        confidence: f32,
        distance_to_centroid: f32,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            embedding_id,
            cluster_id,
            confidence,
            distance_to_centroid,
        }
    }
}

impl AnalysisEvent for ClusterAssigned {
    fn event_id(&self) -> Uuid {
        self.event_id
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn event_type(&self) -> &'static str {
        "ClusterAssigned"
    }
}

/// Event emitted when a motif pattern is detected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotifDetected {
    /// Unique event ID.
    pub event_id: Uuid,

    /// When the event occurred.
    pub occurred_at: DateTime<Utc>,

    /// The motif ID.
    pub motif_id: String,

    /// The cluster sequence defining the motif.
    pub pattern: Vec<ClusterId>,

    /// Number of occurrences found.
    pub occurrences: usize,

    /// Confidence score for this motif.
    pub confidence: f32,

    /// Average duration in milliseconds.
    pub avg_duration_ms: f64,
}

impl MotifDetected {
    /// Create a new MotifDetected event.
    #[must_use]
    pub fn new(
        motif_id: String,
        pattern: Vec<ClusterId>,
        occurrences: usize,
        confidence: f32,
        avg_duration_ms: f64,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            motif_id,
            pattern,
            occurrences,
            confidence,
            avg_duration_ms,
        }
    }
}

impl AnalysisEvent for MotifDetected {
    fn event_id(&self) -> Uuid {
        self.event_id
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn event_type(&self) -> &'static str {
        "MotifDetected"
    }
}

/// Event emitted when a sequence is analyzed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceAnalyzed {
    /// Unique event ID.
    pub event_id: Uuid,

    /// When the event occurred.
    pub occurred_at: DateTime<Utc>,

    /// The recording that was analyzed.
    pub recording_id: RecordingId,

    /// Shannon entropy of the sequence.
    pub entropy: f32,

    /// Stereotypy score.
    pub stereotypy_score: f32,

    /// Number of unique clusters in the sequence.
    pub unique_clusters: usize,

    /// Number of unique transitions.
    pub unique_transitions: usize,

    /// Total sequence length.
    pub sequence_length: usize,
}

impl SequenceAnalyzed {
    /// Create a new SequenceAnalyzed event.
    #[must_use]
    pub fn new(
        recording_id: RecordingId,
        entropy: f32,
        stereotypy_score: f32,
        unique_clusters: usize,
        unique_transitions: usize,
        sequence_length: usize,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            recording_id,
            entropy,
            stereotypy_score,
            unique_clusters,
            unique_transitions,
            sequence_length,
        }
    }
}

impl AnalysisEvent for SequenceAnalyzed {
    fn event_id(&self) -> Uuid {
        self.event_id
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn event_type(&self) -> &'static str {
        "SequenceAnalyzed"
    }
}

/// Event emitted when an anomaly is detected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetected {
    /// Unique event ID.
    pub event_id: Uuid,

    /// When the event occurred.
    pub occurred_at: DateTime<Utc>,

    /// The embedding identified as anomalous.
    pub embedding_id: EmbeddingId,

    /// Anomaly score.
    pub anomaly_score: f32,

    /// Type of anomaly.
    pub anomaly_type: AnomalyType,

    /// The nearest cluster.
    pub nearest_cluster: ClusterId,

    /// Distance to the nearest cluster centroid.
    pub distance_to_centroid: f32,
}

impl AnomalyDetected {
    /// Create a new AnomalyDetected event.
    #[must_use]
    pub fn new(
        embedding_id: EmbeddingId,
        anomaly_score: f32,
        anomaly_type: AnomalyType,
        nearest_cluster: ClusterId,
        distance_to_centroid: f32,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            embedding_id,
            anomaly_score,
            anomaly_type,
            nearest_cluster,
            distance_to_centroid,
        }
    }
}

impl AnalysisEvent for AnomalyDetected {
    fn event_id(&self) -> Uuid {
        self.event_id
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn event_type(&self) -> &'static str {
        "AnomalyDetected"
    }
}

/// Event emitted when cluster prototypes are updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrototypesComputed {
    /// Unique event ID.
    pub event_id: Uuid,

    /// When the event occurred.
    pub occurred_at: DateTime<Utc>,

    /// The cluster for which prototypes were computed.
    pub cluster_id: ClusterId,

    /// Number of prototypes computed.
    pub prototype_count: usize,

    /// Best exemplar score.
    pub best_exemplar_score: f32,
}

impl PrototypesComputed {
    /// Create a new PrototypesComputed event.
    #[must_use]
    pub fn new(cluster_id: ClusterId, prototype_count: usize, best_exemplar_score: f32) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            cluster_id,
            prototype_count,
            best_exemplar_score,
        }
    }
}

impl AnalysisEvent for PrototypesComputed {
    fn event_id(&self) -> Uuid {
        self.event_id
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn event_type(&self) -> &'static str {
        "PrototypesComputed"
    }
}

/// Event emitted when a cluster label is updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterLabeled {
    /// Unique event ID.
    pub event_id: Uuid,

    /// When the event occurred.
    pub occurred_at: DateTime<Utc>,

    /// The cluster that was labeled.
    pub cluster_id: ClusterId,

    /// The new label (None if label was removed).
    pub label: Option<String>,

    /// Previous label (None if no previous label).
    pub previous_label: Option<String>,
}

impl ClusterLabeled {
    /// Create a new ClusterLabeled event.
    #[must_use]
    pub fn new(
        cluster_id: ClusterId,
        label: Option<String>,
        previous_label: Option<String>,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            occurred_at: Utc::now(),
            cluster_id,
            label,
            previous_label,
        }
    }
}

impl AnalysisEvent for ClusterLabeled {
    fn event_id(&self) -> Uuid {
        self.event_id
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    fn event_type(&self) -> &'static str {
        "ClusterLabeled"
    }
}

/// Event publisher trait for analysis events.
#[async_trait::async_trait]
pub trait AnalysisEventPublisher: Send + Sync {
    /// Publish an analysis event.
    async fn publish<E: AnalysisEvent + Serialize + 'static>(
        &self,
        event: E,
    ) -> Result<(), EventPublishError>;
}

/// Error type for event publishing.
#[derive(Debug, thiserror::Error)]
pub enum EventPublishError {
    /// Serialization failed.
    #[error("Failed to serialize event: {0}")]
    Serialization(String),

    /// Transport error.
    #[error("Failed to publish event: {0}")]
    Transport(String),

    /// Channel closed.
    #[error("Event channel closed")]
    ChannelClosed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clusters_discovered_event() {
        let event = ClustersDiscovered::new(
            10,
            5,
            ClusteringMethod::HDBSCAN,
            100,
        )
        .with_silhouette_score(0.75);

        assert_eq!(event.cluster_count, 10);
        assert_eq!(event.noise_count, 5);
        assert_eq!(event.silhouette_score, Some(0.75));
        assert_eq!(event.event_type(), "ClustersDiscovered");
    }

    #[test]
    fn test_cluster_assigned_event() {
        let event = ClusterAssigned::new(
            EmbeddingId::new(),
            ClusterId::new(),
            0.95,
            0.1,
        );

        assert_eq!(event.confidence, 0.95);
        assert_eq!(event.event_type(), "ClusterAssigned");
    }

    #[test]
    fn test_motif_detected_event() {
        let pattern = vec![ClusterId::new(), ClusterId::new()];
        let event = MotifDetected::new(
            "motif-1".to_string(),
            pattern.clone(),
            10,
            0.85,
            1500.0,
        );

        assert_eq!(event.pattern.len(), 2);
        assert_eq!(event.occurrences, 10);
        assert_eq!(event.event_type(), "MotifDetected");
    }

    #[test]
    fn test_anomaly_detected_event() {
        let event = AnomalyDetected::new(
            EmbeddingId::new(),
            0.9,
            AnomalyType::Novel,
            ClusterId::new(),
            2.5,
        );

        assert_eq!(event.anomaly_type, AnomalyType::Novel);
        assert_eq!(event.event_type(), "AnomalyDetected");
    }
}
