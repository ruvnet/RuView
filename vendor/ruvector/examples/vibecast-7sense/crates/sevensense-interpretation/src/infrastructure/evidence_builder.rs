//! Evidence builder for constructing RAB evidence packs.
//!
//! This module provides utilities for collecting and organizing evidence
//! from various sources (neighbors, clusters, sequences) into structured
//! evidence packs.

use tracing::{debug, instrument};

use crate::application::services::InterpretationConfig;
use crate::domain::entities::{
    ClusterContext, ClusterId, EmbeddingId, NeighborEvidence, RecordingMetadata,
    SegmentId, SequenceContext,
};
use crate::Result;

/// Builder for constructing evidence from various sources.
///
/// The `EvidenceBuilder` provides a structured way to collect and organize
/// evidence for RAB interpretations.
#[derive(Debug, Clone)]
pub struct EvidenceBuilder {
    /// Maximum number of neighbors to include
    max_neighbors: usize,

    /// Whether to include spectrogram URLs
    include_spectrograms: bool,

    /// Whether to include sequence context
    include_sequences: bool,

    /// Sequence context window size
    sequence_window: usize,

    /// Minimum distance threshold for neighbor inclusion
    min_distance_threshold: f32,

    /// Maximum distance threshold for neighbor inclusion
    max_distance_threshold: f32,
}

impl EvidenceBuilder {
    /// Create a new evidence builder from configuration.
    pub fn new(config: &InterpretationConfig) -> Self {
        Self {
            max_neighbors: config.max_neighbors,
            include_spectrograms: config.include_spectrograms,
            include_sequences: config.include_sequence_context,
            sequence_window: config.sequence_context_window,
            min_distance_threshold: 0.0,
            max_distance_threshold: 1.0,
        }
    }

    /// Create a builder with default settings.
    pub fn default_builder() -> Self {
        Self {
            max_neighbors: 10,
            include_spectrograms: true,
            include_sequences: true,
            sequence_window: 3,
            min_distance_threshold: 0.0,
            max_distance_threshold: 1.0,
        }
    }

    /// Set the maximum number of neighbors.
    pub fn with_max_neighbors(mut self, n: usize) -> Self {
        self.max_neighbors = n;
        self
    }

    /// Set whether to include spectrogram URLs.
    pub fn with_spectrograms(mut self, include: bool) -> Self {
        self.include_spectrograms = include;
        self
    }

    /// Set the distance threshold range.
    pub fn with_distance_threshold(mut self, min: f32, max: f32) -> Self {
        self.min_distance_threshold = min;
        self.max_distance_threshold = max;
        self
    }

    /// Get the maximum neighbors setting.
    pub fn max_neighbors(&self) -> usize {
        self.max_neighbors
    }

    /// Check if spectrograms are enabled.
    pub fn spectrograms_enabled(&self) -> bool {
        self.include_spectrograms
    }

    /// Collect neighbor evidence from raw neighbor data.
    ///
    /// This method processes raw neighbor data and builds structured
    /// `NeighborEvidence` objects with metadata.
    #[instrument(skip(self, neighbors))]
    pub async fn collect_neighbor_evidence(
        &self,
        neighbors: &[RawNeighbor],
    ) -> Result<Vec<NeighborEvidence>> {
        let filtered: Vec<&RawNeighbor> = neighbors
            .iter()
            .filter(|n| {
                n.distance >= self.min_distance_threshold
                    && n.distance <= self.max_distance_threshold
            })
            .take(self.max_neighbors)
            .collect();

        debug!(
            "Collecting evidence from {} neighbors (filtered from {})",
            filtered.len(),
            neighbors.len()
        );

        let evidence: Vec<NeighborEvidence> = filtered
            .into_iter()
            .map(|n| self.build_neighbor_evidence(n))
            .collect();

        Ok(evidence)
    }

    /// Build neighbor evidence from raw neighbor data.
    fn build_neighbor_evidence(&self, raw: &RawNeighbor) -> NeighborEvidence {
        let metadata = raw
            .metadata
            .clone()
            .unwrap_or_else(|| RecordingMetadata::new(&raw.embedding_id.0));

        let mut evidence = NeighborEvidence::new(
            raw.embedding_id.clone(),
            raw.distance,
            metadata,
        );

        if let Some(cluster_id) = &raw.cluster_id {
            evidence = evidence.with_cluster(cluster_id.clone());
        }

        if self.include_spectrograms {
            if let Some(url) = &raw.spectrogram_url {
                evidence = evidence.with_spectrogram(url.clone());
            }
        }

        evidence
    }

    /// Build cluster context from cluster assignment data.
    #[instrument(skip(self))]
    pub async fn build_cluster_context(
        &self,
        cluster_id: Option<ClusterId>,
        label: Option<String>,
        confidence: f32,
        exemplar_similarity: f32,
    ) -> Result<ClusterContext> {
        let context = ClusterContext {
            assigned_cluster: cluster_id,
            cluster_label: label,
            confidence,
            exemplar_similarity,
        };

        debug!(
            "Built cluster context: assigned={}, confidence={}",
            context.has_cluster(),
            context.confidence
        );

        Ok(context)
    }

    /// Build sequence context from temporal data.
    #[instrument(skip(self))]
    pub async fn build_sequence_context(
        &self,
        preceding: Vec<SegmentId>,
        following: Vec<SegmentId>,
        motif: Option<String>,
    ) -> Result<Option<SequenceContext>> {
        if !self.include_sequences {
            return Ok(None);
        }

        if preceding.is_empty() && following.is_empty() {
            return Ok(None);
        }

        let preceding = preceding
            .into_iter()
            .take(self.sequence_window)
            .collect();

        let following = following
            .into_iter()
            .take(self.sequence_window)
            .collect();

        let context = SequenceContext {
            preceding_segments: preceding,
            following_segments: following,
            detected_motif: motif,
        };

        debug!(
            "Built sequence context: {} preceding, {} following, motif={}",
            context.preceding_segments.len(),
            context.following_segments.len(),
            context.detected_motif.as_deref().unwrap_or("none")
        );

        Ok(Some(context))
    }

    /// Aggregate evidence from multiple sources.
    pub fn aggregate_evidence_scores(&self, neighbors: &[NeighborEvidence]) -> EvidenceScores {
        if neighbors.is_empty() {
            return EvidenceScores::default();
        }

        let distances: Vec<f32> = neighbors.iter().map(|n| n.distance).collect();
        let avg_distance = distances.iter().sum::<f32>() / distances.len() as f32;
        let min_distance = distances.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_distance = distances.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        let similarity = (1.0 - avg_distance).max(0.0).min(1.0);

        // Calculate cluster coherence (how many neighbors share the same cluster)
        let clustered_count = neighbors
            .iter()
            .filter(|n| n.cluster_id.is_some())
            .count();

        let cluster_coherence = if clustered_count > 0 {
            // Check if neighbors share clusters
            let mut cluster_counts = std::collections::HashMap::new();
            for neighbor in neighbors {
                if let Some(cid) = &neighbor.cluster_id {
                    *cluster_counts.entry(cid.0.clone()).or_insert(0) += 1;
                }
            }
            let max_cluster_count = cluster_counts.values().cloned().max().unwrap_or(0);
            max_cluster_count as f32 / neighbors.len() as f32
        } else {
            0.0
        };

        // Calculate taxon coherence
        let taxa: Vec<&str> = neighbors
            .iter()
            .filter_map(|n| n.recording_metadata.taxon.as_deref())
            .collect();

        let taxon_coherence = if !taxa.is_empty() {
            let mut taxon_counts = std::collections::HashMap::new();
            for taxon in &taxa {
                *taxon_counts.entry(*taxon).or_insert(0) += 1;
            }
            let max_taxon_count = taxon_counts.values().cloned().max().unwrap_or(0);
            max_taxon_count as f32 / taxa.len() as f32
        } else {
            0.0
        };

        EvidenceScores {
            neighbor_count: neighbors.len(),
            avg_distance,
            min_distance,
            max_distance,
            avg_similarity: similarity,
            cluster_coherence,
            taxon_coherence,
        }
    }
}

/// Raw neighbor data before processing.
#[derive(Debug, Clone)]
pub struct RawNeighbor {
    /// Embedding ID of the neighbor
    pub embedding_id: EmbeddingId,
    /// Distance from query
    pub distance: f32,
    /// Optional cluster assignment
    pub cluster_id: Option<ClusterId>,
    /// Optional recording metadata
    pub metadata: Option<RecordingMetadata>,
    /// Optional spectrogram URL
    pub spectrogram_url: Option<String>,
}

impl RawNeighbor {
    /// Create a new raw neighbor.
    pub fn new(embedding_id: EmbeddingId, distance: f32) -> Self {
        Self {
            embedding_id,
            distance,
            cluster_id: None,
            metadata: None,
            spectrogram_url: None,
        }
    }

    /// Add cluster ID.
    pub fn with_cluster(mut self, cluster_id: ClusterId) -> Self {
        self.cluster_id = Some(cluster_id);
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, metadata: RecordingMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Add spectrogram URL.
    pub fn with_spectrogram(mut self, url: String) -> Self {
        self.spectrogram_url = Some(url);
        self
    }
}

/// Aggregated scores from evidence analysis.
#[derive(Debug, Clone, Default)]
pub struct EvidenceScores {
    /// Number of neighbors
    pub neighbor_count: usize,
    /// Average distance to neighbors
    pub avg_distance: f32,
    /// Minimum distance (closest neighbor)
    pub min_distance: f32,
    /// Maximum distance (farthest neighbor)
    pub max_distance: f32,
    /// Average similarity (1 - avg_distance)
    pub avg_similarity: f32,
    /// Cluster coherence (0-1, how many neighbors share clusters)
    pub cluster_coherence: f32,
    /// Taxon coherence (0-1, how many neighbors share taxa)
    pub taxon_coherence: f32,
}

impl EvidenceScores {
    /// Calculate overall evidence strength.
    pub fn overall_strength(&self) -> f32 {
        if self.neighbor_count == 0 {
            return 0.0;
        }

        // Weighted combination of scores
        let similarity_weight = 0.4;
        let cluster_weight = 0.3;
        let taxon_weight = 0.3;

        self.avg_similarity * similarity_weight
            + self.cluster_coherence * cluster_weight
            + self.taxon_coherence * taxon_weight
    }

    /// Determine if evidence is strong enough for high-confidence claims.
    pub fn is_strong(&self) -> bool {
        self.neighbor_count >= 3 && self.overall_strength() >= 0.6
    }

    /// Determine if evidence is weak (should generate cautious claims).
    pub fn is_weak(&self) -> bool {
        self.neighbor_count < 2 || self.overall_strength() < 0.3
    }
}

/// Evidence aggregation context for building interpretations.
#[derive(Debug)]
pub struct EvidenceContext {
    /// Aggregated scores
    pub scores: EvidenceScores,
    /// Unique taxa found in neighbors
    pub unique_taxa: Vec<String>,
    /// Unique cluster labels found
    pub unique_clusters: Vec<String>,
    /// Whether temporal sequence is available
    pub has_sequence: bool,
    /// Detected motif if any
    pub motif: Option<String>,
}

impl EvidenceContext {
    /// Build evidence context from collected evidence.
    pub fn from_evidence(
        neighbors: &[NeighborEvidence],
        cluster_context: &ClusterContext,
        sequence_context: &Option<SequenceContext>,
    ) -> Self {
        let builder = EvidenceBuilder::default_builder();
        let scores = builder.aggregate_evidence_scores(neighbors);

        // Collect unique taxa
        let unique_taxa: Vec<String> = neighbors
            .iter()
            .filter_map(|n| n.recording_metadata.taxon.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Collect unique cluster labels
        let mut unique_clusters = Vec::new();
        if let Some(label) = &cluster_context.cluster_label {
            unique_clusters.push(label.clone());
        }

        let has_sequence = sequence_context
            .as_ref()
            .map(|s| s.has_temporal_context())
            .unwrap_or(false);

        let motif = sequence_context
            .as_ref()
            .and_then(|s| s.detected_motif.clone());

        Self {
            scores,
            unique_taxa,
            unique_clusters,
            has_sequence,
            motif,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_evidence_builder_collect_neighbors() {
        let builder = EvidenceBuilder::default_builder()
            .with_max_neighbors(5)
            .with_spectrograms(true);

        let raw_neighbors = vec![
            RawNeighbor::new(EmbeddingId::new("n1"), 0.1)
                .with_metadata(RecordingMetadata::new("r1").with_taxon("Species A")),
            RawNeighbor::new(EmbeddingId::new("n2"), 0.2)
                .with_metadata(RecordingMetadata::new("r2").with_taxon("Species A")),
            RawNeighbor::new(EmbeddingId::new("n3"), 0.3)
                .with_cluster(ClusterId::new("c1")),
        ];

        let evidence = builder.collect_neighbor_evidence(&raw_neighbors).await.unwrap();

        assert_eq!(evidence.len(), 3);
        assert_eq!(evidence[0].embedding_id.as_str(), "n1");
        assert_eq!(evidence[0].recording_metadata.taxon, Some("Species A".to_string()));
        assert!(evidence[2].cluster_id.is_some());
    }

    #[tokio::test]
    async fn test_evidence_builder_distance_filtering() {
        let builder = EvidenceBuilder::default_builder()
            .with_distance_threshold(0.0, 0.5);

        let raw_neighbors = vec![
            RawNeighbor::new(EmbeddingId::new("close"), 0.2),
            RawNeighbor::new(EmbeddingId::new("far"), 0.8),
        ];

        let evidence = builder.collect_neighbor_evidence(&raw_neighbors).await.unwrap();

        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].embedding_id.as_str(), "close");
    }

    #[test]
    fn test_evidence_scores_calculation() {
        let builder = EvidenceBuilder::default_builder();

        let neighbors = vec![
            NeighborEvidence::new(
                EmbeddingId::new("n1"),
                0.1,
                RecordingMetadata::new("r1").with_taxon("Species A"),
            ).with_cluster(ClusterId::new("c1")),
            NeighborEvidence::new(
                EmbeddingId::new("n2"),
                0.2,
                RecordingMetadata::new("r2").with_taxon("Species A"),
            ).with_cluster(ClusterId::new("c1")),
            NeighborEvidence::new(
                EmbeddingId::new("n3"),
                0.3,
                RecordingMetadata::new("r3").with_taxon("Species B"),
            ).with_cluster(ClusterId::new("c2")),
        ];

        let scores = builder.aggregate_evidence_scores(&neighbors);

        assert_eq!(scores.neighbor_count, 3);
        assert!((scores.avg_distance - 0.2).abs() < 0.001);
        assert!((scores.min_distance - 0.1).abs() < 0.001);
        assert!((scores.max_distance - 0.3).abs() < 0.001);
        assert!(scores.cluster_coherence > 0.0);
        assert!(scores.taxon_coherence > 0.0);
    }

    #[test]
    fn test_evidence_context_from_evidence() {
        let neighbors = vec![
            NeighborEvidence::new(
                EmbeddingId::new("n1"),
                0.1,
                RecordingMetadata::new("r1").with_taxon("Species A"),
            ),
            NeighborEvidence::new(
                EmbeddingId::new("n2"),
                0.2,
                RecordingMetadata::new("r2").with_taxon("Species B"),
            ),
        ];

        let cluster_context = ClusterContext::new(
            Some(ClusterId::new("c1")),
            0.9,
            0.85,
        ).with_label("Song Type A");

        let sequence_context = Some(SequenceContext::new(
            vec![SegmentId::new("s1")],
            vec![SegmentId::new("s3")],
        ).with_motif("ABAB"));

        let context = EvidenceContext::from_evidence(
            &neighbors,
            &cluster_context,
            &sequence_context,
        );

        assert_eq!(context.unique_taxa.len(), 2);
        assert_eq!(context.unique_clusters.len(), 1);
        assert!(context.has_sequence);
        assert_eq!(context.motif, Some("ABAB".to_string()));
    }

    #[tokio::test]
    async fn test_build_sequence_context() {
        let builder = EvidenceBuilder::default_builder();

        let context = builder
            .build_sequence_context(
                vec![SegmentId::new("s1"), SegmentId::new("s2")],
                vec![SegmentId::new("s4")],
                Some("AABB".to_string()),
            )
            .await
            .unwrap();

        assert!(context.is_some());
        let ctx = context.unwrap();
        assert_eq!(ctx.preceding_segments.len(), 2);
        assert_eq!(ctx.following_segments.len(), 1);
        assert_eq!(ctx.detected_motif, Some("AABB".to_string()));
    }
}
