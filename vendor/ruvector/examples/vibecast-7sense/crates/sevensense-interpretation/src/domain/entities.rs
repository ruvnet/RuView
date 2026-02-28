//! Core domain entities for the Interpretation bounded context.
//!
//! These entities represent RAB (Retrieval-Augmented Bioacoustics) evidence packs
//! and their associated interpretations with cited claims.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type alias for timestamps
pub type Timestamp = DateTime<Utc>;

/// Unique identifier for embeddings
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmbeddingId(pub String);

impl EmbeddingId {
    /// Create a new embedding ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a new random embedding ID
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Get the inner string value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for EmbeddingId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for EmbeddingId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl std::fmt::Display for EmbeddingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for clusters
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClusterId(pub String);

impl ClusterId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ClusterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for audio segments
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SegmentId(pub String);

impl SegmentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Evidence pack containing all evidence for a bioacoustic query.
///
/// An evidence pack is the core artifact of RAB interpretation, bundling
/// together neighbor evidence, cluster context, sequence context, and
/// the generated interpretation with cited claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidencePack {
    /// Unique identifier for this evidence pack
    pub id: String,

    /// The query embedding that initiated this evidence pack
    pub query_embedding_id: EmbeddingId,

    /// Evidence from nearest neighbor search
    pub neighbors: Vec<NeighborEvidence>,

    /// Context from cluster analysis
    pub cluster_context: ClusterContext,

    /// Optional temporal sequence context
    pub sequence_context: Option<SequenceContext>,

    /// Generated interpretation with claims
    pub interpretation: Interpretation,

    /// When this evidence pack was created
    pub created_at: Timestamp,
}

impl EvidencePack {
    /// Create a new evidence pack with a generated ID
    pub fn new(
        query_embedding_id: EmbeddingId,
        neighbors: Vec<NeighborEvidence>,
        cluster_context: ClusterContext,
        sequence_context: Option<SequenceContext>,
        interpretation: Interpretation,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            query_embedding_id,
            neighbors,
            cluster_context,
            sequence_context,
            interpretation,
            created_at: Utc::now(),
        }
    }

    /// Get the total confidence score for this evidence pack
    pub fn overall_confidence(&self) -> f32 {
        let neighbor_confidence = if self.neighbors.is_empty() {
            0.0
        } else {
            // Higher confidence if neighbors are close (low distance)
            let avg_distance: f32 = self.neighbors.iter().map(|n| n.distance).sum::<f32>()
                / self.neighbors.len() as f32;
            (1.0 - avg_distance.min(1.0)).max(0.0)
        };

        let cluster_confidence = self.cluster_context.confidence;
        let interpretation_confidence = self.interpretation.confidence;

        // Weighted average
        (neighbor_confidence * 0.3 + cluster_confidence * 0.3 + interpretation_confidence * 0.4)
    }

    /// Get the number of distinct evidence sources
    pub fn evidence_source_count(&self) -> usize {
        let mut count = 0;
        if !self.neighbors.is_empty() {
            count += 1;
        }
        if self.cluster_context.assigned_cluster.is_some() {
            count += 1;
        }
        if self.sequence_context.is_some() {
            count += 1;
        }
        count
    }
}

/// Evidence from a single neighbor in vector space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborEvidence {
    /// The embedding ID of this neighbor
    pub embedding_id: EmbeddingId,

    /// Distance from the query embedding (lower = more similar)
    pub distance: f32,

    /// Cluster assignment if available
    pub cluster_id: Option<ClusterId>,

    /// Metadata about the source recording
    pub recording_metadata: RecordingMetadata,

    /// Optional URL to a spectrogram visualization
    pub spectrogram_url: Option<String>,
}

impl NeighborEvidence {
    /// Create new neighbor evidence
    pub fn new(
        embedding_id: EmbeddingId,
        distance: f32,
        recording_metadata: RecordingMetadata,
    ) -> Self {
        Self {
            embedding_id,
            distance,
            cluster_id: None,
            recording_metadata,
            spectrogram_url: None,
        }
    }

    /// Add cluster information
    pub fn with_cluster(mut self, cluster_id: ClusterId) -> Self {
        self.cluster_id = Some(cluster_id);
        self
    }

    /// Add spectrogram URL
    pub fn with_spectrogram(mut self, url: String) -> Self {
        self.spectrogram_url = Some(url);
        self
    }

    /// Convert distance to similarity score (0.0 to 1.0)
    pub fn similarity(&self) -> f32 {
        (1.0 - self.distance).max(0.0).min(1.0)
    }
}

/// Metadata about a source recording
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    /// Recording identifier
    pub recording_id: String,

    /// Species or taxon if known
    pub taxon: Option<String>,

    /// Geographic location
    pub location: Option<GeoLocation>,

    /// Recording timestamp
    pub recorded_at: Option<Timestamp>,

    /// Duration in seconds
    pub duration_seconds: Option<f32>,

    /// Sample rate in Hz
    pub sample_rate: Option<u32>,

    /// Additional tags or labels
    pub tags: Vec<String>,
}

impl RecordingMetadata {
    /// Create minimal recording metadata
    pub fn new(recording_id: impl Into<String>) -> Self {
        Self {
            recording_id: recording_id.into(),
            taxon: None,
            location: None,
            recorded_at: None,
            duration_seconds: None,
            sample_rate: None,
            tags: Vec::new(),
        }
    }

    /// Add taxon information
    pub fn with_taxon(mut self, taxon: impl Into<String>) -> Self {
        self.taxon = Some(taxon.into());
        self
    }

    /// Add location information
    pub fn with_location(mut self, location: GeoLocation) -> Self {
        self.location = Some(location);
        self
    }
}

/// Geographic location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub elevation_meters: Option<f32>,
    pub locality: Option<String>,
}

impl GeoLocation {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
            elevation_meters: None,
            locality: None,
        }
    }
}

/// Context from cluster analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterContext {
    /// The cluster this embedding was assigned to
    pub assigned_cluster: Option<ClusterId>,

    /// Human-readable label for the cluster
    pub cluster_label: Option<String>,

    /// Confidence in the cluster assignment (0.0 to 1.0)
    pub confidence: f32,

    /// Similarity to the cluster exemplar (0.0 to 1.0)
    pub exemplar_similarity: f32,
}

impl ClusterContext {
    /// Create a new cluster context
    pub fn new(
        assigned_cluster: Option<ClusterId>,
        confidence: f32,
        exemplar_similarity: f32,
    ) -> Self {
        Self {
            assigned_cluster,
            cluster_label: None,
            confidence,
            exemplar_similarity,
        }
    }

    /// Create an empty cluster context (no cluster assigned)
    pub fn empty() -> Self {
        Self {
            assigned_cluster: None,
            cluster_label: None,
            confidence: 0.0,
            exemplar_similarity: 0.0,
        }
    }

    /// Add a cluster label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.cluster_label = Some(label.into());
        self
    }

    /// Check if a cluster was assigned
    pub fn has_cluster(&self) -> bool {
        self.assigned_cluster.is_some()
    }
}

/// Temporal sequence context for understanding vocalization patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceContext {
    /// Segments that precede the query in time
    pub preceding_segments: Vec<SegmentId>,

    /// Segments that follow the query in time
    pub following_segments: Vec<SegmentId>,

    /// Detected acoustic motif pattern
    pub detected_motif: Option<String>,
}

impl SequenceContext {
    /// Create a new sequence context
    pub fn new(
        preceding_segments: Vec<SegmentId>,
        following_segments: Vec<SegmentId>,
    ) -> Self {
        Self {
            preceding_segments,
            following_segments,
            detected_motif: None,
        }
    }

    /// Create an empty sequence context
    pub fn empty() -> Self {
        Self {
            preceding_segments: Vec::new(),
            following_segments: Vec::new(),
            detected_motif: None,
        }
    }

    /// Add a detected motif
    pub fn with_motif(mut self, motif: impl Into<String>) -> Self {
        self.detected_motif = Some(motif.into());
        self
    }

    /// Check if sequence context has any temporal information
    pub fn has_temporal_context(&self) -> bool {
        !self.preceding_segments.is_empty() || !self.following_segments.is_empty()
    }

    /// Get total sequence length
    pub fn sequence_length(&self) -> usize {
        self.preceding_segments.len() + 1 + self.following_segments.len()
    }
}

/// Generated interpretation of the evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interpretation {
    /// Structural description of the acoustic signal
    pub structural_description: String,

    /// Claims made about the signal with evidence citations
    pub claims: Vec<Claim>,

    /// Overall confidence in the interpretation (0.0 to 1.0)
    pub confidence: f32,
}

impl Interpretation {
    /// Create a new interpretation
    pub fn new(structural_description: String, claims: Vec<Claim>, confidence: f32) -> Self {
        Self {
            structural_description,
            claims,
            confidence: confidence.clamp(0.0, 1.0),
        }
    }

    /// Create an empty interpretation with no claims
    pub fn empty() -> Self {
        Self {
            structural_description: String::new(),
            claims: Vec::new(),
            confidence: 0.0,
        }
    }

    /// Add a claim to the interpretation
    pub fn add_claim(&mut self, claim: Claim) {
        self.claims.push(claim);
        self.recalculate_confidence();
    }

    /// Recalculate overall confidence based on claims
    fn recalculate_confidence(&mut self) {
        if self.claims.is_empty() {
            return;
        }
        let total_confidence: f32 = self.claims.iter().map(|c| c.confidence).sum();
        self.confidence = total_confidence / self.claims.len() as f32;
    }

    /// Get claims above a confidence threshold
    pub fn high_confidence_claims(&self, threshold: f32) -> Vec<&Claim> {
        self.claims
            .iter()
            .filter(|c| c.confidence >= threshold)
            .collect()
    }

    /// Get the number of evidence-backed claims
    pub fn evidenced_claim_count(&self) -> usize {
        self.claims
            .iter()
            .filter(|c| !c.evidence_refs.is_empty())
            .count()
    }
}

/// A claim made about the acoustic signal with evidence citations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    /// The statement being made
    pub statement: String,

    /// References to evidence supporting this claim
    pub evidence_refs: Vec<EvidenceRef>,

    /// Confidence in this claim (0.0 to 1.0)
    pub confidence: f32,
}

impl Claim {
    /// Create a new claim
    pub fn new(statement: impl Into<String>, confidence: f32) -> Self {
        Self {
            statement: statement.into(),
            evidence_refs: Vec::new(),
            confidence: confidence.clamp(0.0, 1.0),
        }
    }

    /// Add an evidence reference
    pub fn add_evidence(&mut self, evidence_ref: EvidenceRef) {
        self.evidence_refs.push(evidence_ref);
    }

    /// Create a claim with evidence references
    pub fn with_evidence(mut self, evidence_refs: Vec<EvidenceRef>) -> Self {
        self.evidence_refs = evidence_refs;
        self
    }

    /// Check if this claim has supporting evidence
    pub fn has_evidence(&self) -> bool {
        !self.evidence_refs.is_empty()
    }

    /// Get evidence references of a specific type
    pub fn evidence_of_type(&self, ref_type: EvidenceRefType) -> Vec<&EvidenceRef> {
        self.evidence_refs
            .iter()
            .filter(|e| e.ref_type == ref_type)
            .collect()
    }
}

/// Reference to a piece of evidence supporting a claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// Type of evidence being referenced
    pub ref_type: EvidenceRefType,

    /// Identifier for the evidence (embedding ID, cluster ID, etc.)
    pub ref_id: String,

    /// Human-readable description of the evidence
    pub description: String,
}

impl EvidenceRef {
    /// Create a new evidence reference
    pub fn new(ref_type: EvidenceRefType, ref_id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            ref_type,
            ref_id: ref_id.into(),
            description: description.into(),
        }
    }

    /// Create a neighbor evidence reference
    pub fn neighbor(embedding_id: &EmbeddingId, description: impl Into<String>) -> Self {
        Self::new(EvidenceRefType::Neighbor, embedding_id.as_str(), description)
    }

    /// Create a cluster evidence reference
    pub fn cluster(cluster_id: &ClusterId, description: impl Into<String>) -> Self {
        Self::new(EvidenceRefType::Cluster, cluster_id.as_str(), description)
    }

    /// Create a sequence evidence reference
    pub fn sequence(segment_id: &SegmentId, description: impl Into<String>) -> Self {
        Self::new(EvidenceRefType::Sequence, segment_id.as_str(), description)
    }

    /// Create a taxon evidence reference
    pub fn taxon(taxon_name: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(EvidenceRefType::Taxon, taxon_name, description)
    }
}

/// Types of evidence that can be referenced in claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceRefType {
    /// Evidence from nearest neighbor search
    Neighbor,
    /// Evidence from cluster assignment
    Cluster,
    /// Evidence from temporal sequence analysis
    Sequence,
    /// Evidence from taxonomic classification
    Taxon,
}

impl std::fmt::Display for EvidenceRefType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvidenceRefType::Neighbor => write!(f, "neighbor"),
            EvidenceRefType::Cluster => write!(f, "cluster"),
            EvidenceRefType::Sequence => write!(f, "sequence"),
            EvidenceRefType::Taxon => write!(f, "taxon"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_id() {
        let id = EmbeddingId::new("test-123");
        assert_eq!(id.as_str(), "test-123");
        assert_eq!(id.to_string(), "test-123");

        let generated = EmbeddingId::generate();
        assert!(!generated.as_str().is_empty());
    }

    #[test]
    fn test_neighbor_evidence_similarity() {
        let metadata = RecordingMetadata::new("rec-1");
        let evidence = NeighborEvidence::new(
            EmbeddingId::new("emb-1"),
            0.2,
            metadata,
        );
        assert_eq!(evidence.similarity(), 0.8);

        let far_evidence = NeighborEvidence::new(
            EmbeddingId::new("emb-2"),
            1.5,
            RecordingMetadata::new("rec-2"),
        );
        assert_eq!(far_evidence.similarity(), 0.0);
    }

    #[test]
    fn test_cluster_context() {
        let context = ClusterContext::new(
            Some(ClusterId::new("cluster-1")),
            0.85,
            0.92,
        ).with_label("Song Type A");

        assert!(context.has_cluster());
        assert_eq!(context.cluster_label, Some("Song Type A".to_string()));
    }

    #[test]
    fn test_sequence_context() {
        let context = SequenceContext::new(
            vec![SegmentId::new("seg-1"), SegmentId::new("seg-2")],
            vec![SegmentId::new("seg-4")],
        ).with_motif("ABAB");

        assert!(context.has_temporal_context());
        assert_eq!(context.sequence_length(), 4);
        assert_eq!(context.detected_motif, Some("ABAB".to_string()));
    }

    #[test]
    fn test_claim_with_evidence() {
        let mut claim = Claim::new("This is a dawn chorus vocalization", 0.9);
        claim.add_evidence(EvidenceRef::neighbor(
            &EmbeddingId::new("emb-1"),
            "Similar to known dawn chorus recording",
        ));
        claim.add_evidence(EvidenceRef::cluster(
            &ClusterId::new("cluster-5"),
            "Assigned to dawn chorus cluster",
        ));

        assert!(claim.has_evidence());
        assert_eq!(claim.evidence_refs.len(), 2);
        assert_eq!(claim.evidence_of_type(EvidenceRefType::Neighbor).len(), 1);
    }

    #[test]
    fn test_interpretation_confidence() {
        let mut interp = Interpretation::new(
            "Complex harmonic structure with frequency modulation".to_string(),
            Vec::new(),
            0.0,
        );

        interp.add_claim(Claim::new("Claim 1", 0.8));
        interp.add_claim(Claim::new("Claim 2", 0.6));

        assert_eq!(interp.confidence, 0.7);
    }

    #[test]
    fn test_evidence_pack_overall_confidence() {
        let pack = EvidencePack::new(
            EmbeddingId::new("query-1"),
            vec![
                NeighborEvidence::new(
                    EmbeddingId::new("n-1"),
                    0.1,
                    RecordingMetadata::new("r-1"),
                ),
                NeighborEvidence::new(
                    EmbeddingId::new("n-2"),
                    0.2,
                    RecordingMetadata::new("r-2"),
                ),
            ],
            ClusterContext::new(Some(ClusterId::new("c-1")), 0.9, 0.85),
            None,
            Interpretation::new("Test".to_string(), vec![Claim::new("Test", 0.8)], 0.8),
        );

        let confidence = pack.overall_confidence();
        assert!(confidence > 0.0 && confidence <= 1.0);
        assert_eq!(pack.evidence_source_count(), 2);
    }
}
