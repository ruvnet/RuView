//! # Domain Events
//!
//! Domain events represent things that have happened in the system.
//! They are used for event sourcing, audit logging, and inter-service communication.
//!
//! ## Event Categories
//!
//! - **Recording Events**: Lifecycle of audio recordings
//! - **Segment Events**: Audio segment detection and processing
//! - **Embedding Events**: Vector embedding generation
//! - **Cluster Events**: Species/sound clustering operations
//! - **Analysis Events**: Interpretation and analysis results

use serde::{Deserialize, Serialize};

use super::entities::{
    ClusterId, Confidence, EmbeddingId, GeoLocation, RecordingId, SegmentId, TaxonId, TimeRange,
    Timestamp,
};

/// Unique identifier for a domain event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(uuid::Uuid);

impl EventId {
    /// Creates a new random `EventId`.
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Returns the inner UUID value.
    #[must_use]
    pub const fn inner(&self) -> uuid::Uuid {
        self.0
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Metadata common to all domain events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Unique identifier for this event.
    pub event_id: EventId,
    /// When this event occurred.
    pub timestamp: Timestamp,
    /// Correlation ID for tracing related events.
    pub correlation_id: Option<String>,
    /// Causation ID (the event that caused this event).
    pub causation_id: Option<EventId>,
    /// Version of the event schema.
    pub schema_version: u32,
}

impl EventMetadata {
    /// Creates new event metadata with default values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            event_id: EventId::new(),
            timestamp: Timestamp::now(),
            correlation_id: None,
            causation_id: None,
            schema_version: 1,
        }
    }

    /// Creates new event metadata with a correlation ID.
    #[must_use]
    pub fn with_correlation(correlation_id: impl Into<String>) -> Self {
        Self {
            correlation_id: Some(correlation_id.into()),
            ..Self::new()
        }
    }

    /// Sets the causation ID.
    #[must_use]
    pub fn with_causation(mut self, causation_id: EventId) -> Self {
        self.causation_id = Some(causation_id);
        self
    }
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// All domain events in the system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DomainEvent {
    // =========================================================================
    // Recording Events
    // =========================================================================
    /// A new recording has been uploaded to the system.
    RecordingUploaded(RecordingUploadedEvent),

    /// A recording has been validated and is ready for processing.
    RecordingValidated(RecordingValidatedEvent),

    /// A recording has failed validation.
    RecordingValidationFailed(RecordingValidationFailedEvent),

    /// A recording has been fully processed.
    RecordingProcessed(RecordingProcessedEvent),

    /// A recording has been archived.
    RecordingArchived(RecordingArchivedEvent),

    /// A recording has been deleted.
    RecordingDeleted(RecordingDeletedEvent),

    // =========================================================================
    // Segment Events
    // =========================================================================
    /// A segment has been detected within a recording.
    SegmentDetected(SegmentDetectedEvent),

    /// A segment has been classified.
    SegmentClassified(SegmentClassifiedEvent),

    /// A segment has been rejected (e.g., noise, artifact).
    SegmentRejected(SegmentRejectedEvent),

    /// A segment has been manually verified by a user.
    SegmentVerified(SegmentVerifiedEvent),

    // =========================================================================
    // Embedding Events
    // =========================================================================
    /// An embedding has been generated for a segment.
    EmbeddingGenerated(EmbeddingGeneratedEvent),

    /// An embedding has been indexed in the vector database.
    EmbeddingIndexed(EmbeddingIndexedEvent),

    /// Similar embeddings have been found.
    SimilarEmbeddingsFound(SimilarEmbeddingsFoundEvent),

    // =========================================================================
    // Cluster Events
    // =========================================================================
    /// A new cluster has been created.
    ClusterCreated(ClusterCreatedEvent),

    /// An embedding has been added to a cluster.
    ClusterMemberAdded(ClusterMemberAddedEvent),

    /// A cluster has been merged with another.
    ClusterMerged(ClusterMergedEvent),

    /// A cluster has been identified as a species.
    ClusterIdentified(ClusterIdentifiedEvent),

    /// A cluster has been split into multiple clusters.
    ClusterSplit(ClusterSplitEvent),

    // =========================================================================
    // Analysis Events
    // =========================================================================
    /// Analysis has been requested for a recording or region.
    AnalysisRequested(AnalysisRequestedEvent),

    /// Analysis has completed.
    AnalysisCompleted(AnalysisCompletedEvent),

    /// A species has been detected.
    SpeciesDetected(SpeciesDetectedEvent),

    /// A biodiversity report has been generated.
    BiodiversityReportGenerated(BiodiversityReportGeneratedEvent),
}

impl DomainEvent {
    /// Returns the event type name.
    #[must_use]
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::RecordingUploaded(_) => "recording_uploaded",
            Self::RecordingValidated(_) => "recording_validated",
            Self::RecordingValidationFailed(_) => "recording_validation_failed",
            Self::RecordingProcessed(_) => "recording_processed",
            Self::RecordingArchived(_) => "recording_archived",
            Self::RecordingDeleted(_) => "recording_deleted",
            Self::SegmentDetected(_) => "segment_detected",
            Self::SegmentClassified(_) => "segment_classified",
            Self::SegmentRejected(_) => "segment_rejected",
            Self::SegmentVerified(_) => "segment_verified",
            Self::EmbeddingGenerated(_) => "embedding_generated",
            Self::EmbeddingIndexed(_) => "embedding_indexed",
            Self::SimilarEmbeddingsFound(_) => "similar_embeddings_found",
            Self::ClusterCreated(_) => "cluster_created",
            Self::ClusterMemberAdded(_) => "cluster_member_added",
            Self::ClusterMerged(_) => "cluster_merged",
            Self::ClusterIdentified(_) => "cluster_identified",
            Self::ClusterSplit(_) => "cluster_split",
            Self::AnalysisRequested(_) => "analysis_requested",
            Self::AnalysisCompleted(_) => "analysis_completed",
            Self::SpeciesDetected(_) => "species_detected",
            Self::BiodiversityReportGenerated(_) => "biodiversity_report_generated",
        }
    }

    /// Returns the event metadata.
    #[must_use]
    pub fn metadata(&self) -> &EventMetadata {
        match self {
            Self::RecordingUploaded(e) => &e.metadata,
            Self::RecordingValidated(e) => &e.metadata,
            Self::RecordingValidationFailed(e) => &e.metadata,
            Self::RecordingProcessed(e) => &e.metadata,
            Self::RecordingArchived(e) => &e.metadata,
            Self::RecordingDeleted(e) => &e.metadata,
            Self::SegmentDetected(e) => &e.metadata,
            Self::SegmentClassified(e) => &e.metadata,
            Self::SegmentRejected(e) => &e.metadata,
            Self::SegmentVerified(e) => &e.metadata,
            Self::EmbeddingGenerated(e) => &e.metadata,
            Self::EmbeddingIndexed(e) => &e.metadata,
            Self::SimilarEmbeddingsFound(e) => &e.metadata,
            Self::ClusterCreated(e) => &e.metadata,
            Self::ClusterMemberAdded(e) => &e.metadata,
            Self::ClusterMerged(e) => &e.metadata,
            Self::ClusterIdentified(e) => &e.metadata,
            Self::ClusterSplit(e) => &e.metadata,
            Self::AnalysisRequested(e) => &e.metadata,
            Self::AnalysisCompleted(e) => &e.metadata,
            Self::SpeciesDetected(e) => &e.metadata,
            Self::BiodiversityReportGenerated(e) => &e.metadata,
        }
    }
}

// =============================================================================
// Recording Events
// =============================================================================

/// Event emitted when a new recording is uploaded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordingUploadedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The recording ID.
    pub recording_id: RecordingId,
    /// Original filename.
    pub filename: String,
    /// File size in bytes.
    pub file_size_bytes: u64,
    /// MIME type of the uploaded file.
    pub mime_type: String,
    /// Geographic location where the recording was made.
    pub location: Option<GeoLocation>,
    /// When the recording was captured.
    pub recorded_at: Option<Timestamp>,
}

/// Event emitted when a recording passes validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordingValidatedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The recording ID.
    pub recording_id: RecordingId,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Number of channels.
    pub channels: u8,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// Event emitted when a recording fails validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordingValidationFailedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The recording ID.
    pub recording_id: RecordingId,
    /// Reason for validation failure.
    pub reason: String,
    /// Error code for programmatic handling.
    pub error_code: String,
}

/// Event emitted when a recording has been fully processed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordingProcessedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The recording ID.
    pub recording_id: RecordingId,
    /// Number of segments detected.
    pub segment_count: u32,
    /// Number of unique species detected.
    pub species_count: u32,
    /// Processing time in milliseconds.
    pub processing_time_ms: u64,
}

/// Event emitted when a recording is archived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordingArchivedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The recording ID.
    pub recording_id: RecordingId,
    /// Archive location (e.g., S3 URI).
    pub archive_location: String,
}

/// Event emitted when a recording is deleted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordingDeletedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The recording ID.
    pub recording_id: RecordingId,
    /// Reason for deletion.
    pub reason: Option<String>,
}

// =============================================================================
// Segment Events
// =============================================================================

/// Event emitted when a segment is detected in a recording.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentDetectedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The segment ID.
    pub segment_id: SegmentId,
    /// Parent recording ID.
    pub recording_id: RecordingId,
    /// Time range within the recording.
    pub time_range: TimeRange,
    /// Frequency range in Hz (low, high).
    pub frequency_range: Option<(f32, f32)>,
    /// Detection confidence.
    pub confidence: Confidence,
}

/// Event emitted when a segment is classified.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentClassifiedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The segment ID.
    pub segment_id: SegmentId,
    /// Predicted taxon.
    pub taxon_id: TaxonId,
    /// Classification confidence.
    pub confidence: Confidence,
    /// Top alternative predictions with confidences.
    pub alternatives: Vec<(TaxonId, Confidence)>,
}

/// Event emitted when a segment is rejected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentRejectedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The segment ID.
    pub segment_id: SegmentId,
    /// Reason for rejection.
    pub reason: SegmentRejectionReason,
}

/// Reasons why a segment might be rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentRejectionReason {
    /// Background noise or environmental sounds.
    Noise,
    /// Recording artifact (clipping, distortion).
    Artifact,
    /// Human speech or activity.
    HumanActivity,
    /// Mechanical or artificial sound.
    Mechanical,
    /// Too short to analyze.
    TooShort,
    /// Below confidence threshold.
    LowConfidence,
    /// Other reason with description.
    Other(String),
}

/// Event emitted when a segment is manually verified.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentVerifiedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The segment ID.
    pub segment_id: SegmentId,
    /// Verified taxon (may differ from prediction).
    pub verified_taxon: TaxonId,
    /// User who verified.
    pub verified_by: String,
    /// Whether the prediction was correct.
    pub prediction_correct: bool,
}

// =============================================================================
// Embedding Events
// =============================================================================

/// Event emitted when an embedding is generated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingGeneratedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The embedding ID.
    pub embedding_id: EmbeddingId,
    /// Source segment ID.
    pub segment_id: SegmentId,
    /// Embedding model used.
    pub model_name: String,
    /// Model version.
    pub model_version: String,
    /// Embedding dimensionality.
    pub dimensions: u32,
}

/// Event emitted when an embedding is indexed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingIndexedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The embedding ID.
    pub embedding_id: EmbeddingId,
    /// Vector database collection name.
    pub collection_name: String,
    /// Point ID in the vector database.
    pub point_id: String,
}

/// Event emitted when similar embeddings are found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimilarEmbeddingsFoundEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Query embedding ID.
    pub query_embedding_id: EmbeddingId,
    /// Similar embeddings with scores.
    pub similar: Vec<SimilarEmbedding>,
    /// Search parameters used.
    pub search_params: SearchParams,
}

/// A similar embedding result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimilarEmbedding {
    /// The embedding ID.
    pub embedding_id: EmbeddingId,
    /// Similarity score (0.0 to 1.0).
    pub score: f32,
    /// Associated taxon if known.
    pub taxon_id: Option<TaxonId>,
}

/// Parameters for similarity search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchParams {
    /// Number of results to return.
    pub limit: u32,
    /// Minimum similarity threshold.
    pub min_score: f32,
    /// Whether to use approximate search.
    pub approximate: bool,
}

// =============================================================================
// Cluster Events
// =============================================================================

/// Event emitted when a new cluster is created.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterCreatedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The cluster ID.
    pub cluster_id: ClusterId,
    /// Initial centroid embedding ID.
    pub centroid_embedding_id: EmbeddingId,
    /// Clustering algorithm used.
    pub algorithm: String,
}

/// Event emitted when an embedding joins a cluster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterMemberAddedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The cluster ID.
    pub cluster_id: ClusterId,
    /// The embedding ID.
    pub embedding_id: EmbeddingId,
    /// Distance to cluster centroid.
    pub distance_to_centroid: f32,
}

/// Event emitted when clusters are merged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterMergedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The resulting cluster ID.
    pub target_cluster_id: ClusterId,
    /// Clusters that were merged in.
    pub source_cluster_ids: Vec<ClusterId>,
    /// Number of members in merged cluster.
    pub member_count: u32,
}

/// Event emitted when a cluster is identified as a species.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterIdentifiedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The cluster ID.
    pub cluster_id: ClusterId,
    /// Identified taxon.
    pub taxon_id: TaxonId,
    /// Identification confidence.
    pub confidence: Confidence,
    /// Method used for identification.
    pub identification_method: IdentificationMethod,
}

/// Methods for cluster identification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentificationMethod {
    /// Automatic classification by model.
    Automatic,
    /// Manual identification by expert.
    Manual,
    /// Consensus from multiple verifications.
    Consensus,
    /// Reference library match.
    ReferenceMatch,
}

/// Event emitted when a cluster is split.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterSplitEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Original cluster ID.
    pub source_cluster_id: ClusterId,
    /// New cluster IDs created from split.
    pub new_cluster_ids: Vec<ClusterId>,
    /// Reason for split.
    pub reason: String,
}

// =============================================================================
// Analysis Events
// =============================================================================

/// Event emitted when analysis is requested.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisRequestedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Analysis request ID.
    pub request_id: String,
    /// Type of analysis requested.
    pub analysis_type: AnalysisType,
    /// Target (recording ID or location).
    pub target: AnalysisTarget,
}

/// Types of analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisType {
    /// Species detection.
    SpeciesDetection,
    /// Biodiversity assessment.
    BiodiversityAssessment,
    /// Temporal activity patterns.
    ActivityPattern,
    /// Acoustic index calculation.
    AcousticIndices,
    /// Custom analysis.
    Custom(String),
}

/// Target of analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisTarget {
    /// Single recording.
    Recording(RecordingId),
    /// Geographic region.
    Region {
        /// Center location.
        center: GeoLocation,
        /// Radius in meters.
        radius_m: f64,
    },
    /// Time period.
    TimePeriod {
        /// Start time.
        start: Timestamp,
        /// End time.
        end: Timestamp,
    },
}

/// Event emitted when analysis completes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisCompletedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Analysis request ID.
    pub request_id: String,
    /// Time taken in milliseconds.
    pub duration_ms: u64,
    /// Summary of results.
    pub summary: String,
    /// Location of full results.
    pub results_location: String,
}

/// Event emitted when a species is detected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeciesDetectedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// The recording ID.
    pub recording_id: RecordingId,
    /// Detected taxon.
    pub taxon_id: TaxonId,
    /// Detection confidence.
    pub confidence: Confidence,
    /// Time ranges where species was detected.
    pub time_ranges: Vec<TimeRange>,
    /// Location of detection.
    pub location: Option<GeoLocation>,
}

/// Event emitted when a biodiversity report is generated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiodiversityReportGeneratedEvent {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Report ID.
    pub report_id: String,
    /// Geographic region covered.
    pub region: GeoLocation,
    /// Radius in meters.
    pub radius_m: f64,
    /// Time period covered.
    pub time_period: (Timestamp, Timestamp),
    /// Number of species detected.
    pub species_count: u32,
    /// Shannon diversity index.
    pub shannon_index: f64,
    /// Location of full report.
    pub report_location: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_metadata_creation() {
        let meta = EventMetadata::new();
        assert!(meta.correlation_id.is_none());
        assert!(meta.causation_id.is_none());
        assert_eq!(meta.schema_version, 1);
    }

    #[test]
    fn test_event_metadata_with_correlation() {
        let meta = EventMetadata::with_correlation("test-correlation");
        assert_eq!(meta.correlation_id, Some("test-correlation".to_string()));
    }

    #[test]
    fn test_domain_event_type() {
        let event = DomainEvent::RecordingUploaded(RecordingUploadedEvent {
            metadata: EventMetadata::new(),
            recording_id: RecordingId::new(),
            filename: "test.wav".to_string(),
            file_size_bytes: 1024,
            mime_type: "audio/wav".to_string(),
            location: None,
            recorded_at: None,
        });

        assert_eq!(event.event_type(), "recording_uploaded");
    }

    #[test]
    fn test_event_serialization() {
        let event = DomainEvent::SegmentDetected(SegmentDetectedEvent {
            metadata: EventMetadata::new(),
            segment_id: SegmentId::new(),
            recording_id: RecordingId::new(),
            time_range: TimeRange::new(1000, 5000),
            frequency_range: Some((200.0, 8000.0)),
            confidence: Confidence::new(0.95),
        });

        let json = serde_json::to_string(&event).unwrap();
        let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.event_type(), deserialized.event_type());
    }
}
