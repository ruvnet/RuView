//! Application services for RAB interpretation.
//!
//! The `InterpretationService` orchestrates the building of evidence packs
//! and generation of interpretations with cited claims.

use std::sync::Arc;

use tracing::{debug, info, instrument, warn};

use crate::domain::entities::{
    Claim, ClusterContext, EmbeddingId, EvidencePack, EvidenceRef, EvidenceRefType,
    Interpretation, NeighborEvidence, RecordingMetadata, SequenceContext, SegmentId,
};
use crate::domain::repository::{ClusterRepository, EvidencePackRepository};
use crate::infrastructure::claim_generator::ClaimGenerator;
use crate::infrastructure::evidence_builder::EvidenceBuilder;
use crate::templates::InterpretationTemplates;
use crate::{Error, Result};

/// Configuration for the interpretation service.
#[derive(Debug, Clone)]
pub struct InterpretationConfig {
    /// Maximum number of neighbors to include in evidence packs
    pub max_neighbors: usize,

    /// Whether to include spectrogram URLs in evidence
    pub include_spectrograms: bool,

    /// Minimum confidence threshold for claims
    pub min_claim_confidence: f32,

    /// Maximum number of claims per interpretation
    pub max_claims: usize,

    /// Whether to include sequence context
    pub include_sequence_context: bool,

    /// Number of preceding/following segments to include
    pub sequence_context_window: usize,

    /// Minimum overall confidence to accept an interpretation
    pub min_interpretation_confidence: f32,
}

impl Default for InterpretationConfig {
    fn default() -> Self {
        Self {
            max_neighbors: 10,
            include_spectrograms: true,
            min_claim_confidence: 0.5,
            max_claims: 10,
            include_sequence_context: true,
            sequence_context_window: 3,
            min_interpretation_confidence: 0.3,
        }
    }
}

impl InterpretationConfig {
    /// Create a new configuration builder
    pub fn builder() -> InterpretationConfigBuilder {
        InterpretationConfigBuilder::default()
    }
}

/// Builder for InterpretationConfig
#[derive(Debug, Default)]
pub struct InterpretationConfigBuilder {
    config: InterpretationConfig,
}

impl InterpretationConfigBuilder {
    pub fn max_neighbors(mut self, n: usize) -> Self {
        self.config.max_neighbors = n;
        self
    }

    pub fn include_spectrograms(mut self, include: bool) -> Self {
        self.config.include_spectrograms = include;
        self
    }

    pub fn min_claim_confidence(mut self, confidence: f32) -> Self {
        self.config.min_claim_confidence = confidence;
        self
    }

    pub fn max_claims(mut self, n: usize) -> Self {
        self.config.max_claims = n;
        self
    }

    pub fn include_sequence_context(mut self, include: bool) -> Self {
        self.config.include_sequence_context = include;
        self
    }

    pub fn sequence_context_window(mut self, window: usize) -> Self {
        self.config.sequence_context_window = window;
        self
    }

    pub fn min_interpretation_confidence(mut self, confidence: f32) -> Self {
        self.config.min_interpretation_confidence = confidence;
        self
    }

    pub fn build(self) -> InterpretationConfig {
        self.config
    }
}

/// Neighbor data from vector search (simplified interface).
///
/// This represents the data returned from the vector space service.
#[derive(Debug, Clone)]
pub struct Neighbor {
    /// The embedding ID of the neighbor
    pub embedding_id: EmbeddingId,
    /// Distance from the query (lower = more similar)
    pub distance: f32,
    /// Optional metadata about the neighbor
    pub metadata: Option<serde_json::Value>,
}

impl Neighbor {
    /// Create a new neighbor
    pub fn new(embedding_id: EmbeddingId, distance: f32) -> Self {
        Self {
            embedding_id,
            distance,
            metadata: None,
        }
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Trait for vector space operations needed by the interpretation service.
///
/// This abstracts the vector search operations from sevensense-vector.
#[async_trait::async_trait]
pub trait VectorSpaceService: Send + Sync {
    /// Find k nearest neighbors for an embedding
    async fn find_neighbors(&self, embedding_id: &EmbeddingId, k: usize) -> Result<Vec<Neighbor>>;

    /// Get the embedding vector for an ID
    async fn get_embedding(&self, embedding_id: &EmbeddingId) -> Result<Option<Vec<f32>>>;

    /// Calculate similarity between two embeddings
    async fn calculate_similarity(
        &self,
        embedding_id_a: &EmbeddingId,
        embedding_id_b: &EmbeddingId,
    ) -> Result<f32>;
}

/// Trait for sequence operations needed by the interpretation service.
///
/// This abstracts sequence analysis operations from sevensense-analysis.
#[async_trait::async_trait]
pub trait SequenceService: Send + Sync {
    /// Get segments preceding the given segment in time
    async fn get_preceding_segments(
        &self,
        segment_id: &SegmentId,
        count: usize,
    ) -> Result<Vec<SegmentId>>;

    /// Get segments following the given segment in time
    async fn get_following_segments(
        &self,
        segment_id: &SegmentId,
        count: usize,
    ) -> Result<Vec<SegmentId>>;

    /// Detect motif patterns in a sequence
    async fn detect_motif(&self, segment_ids: &[SegmentId]) -> Result<Option<String>>;
}

/// Trait for metadata lookup operations.
#[async_trait::async_trait]
pub trait MetadataService: Send + Sync {
    /// Get recording metadata for an embedding
    async fn get_recording_metadata(
        &self,
        embedding_id: &EmbeddingId,
    ) -> Result<RecordingMetadata>;

    /// Get spectrogram URL for an embedding
    async fn get_spectrogram_url(&self, embedding_id: &EmbeddingId) -> Result<Option<String>>;

    /// Get segment ID for an embedding (if it represents a segment)
    async fn get_segment_id(&self, embedding_id: &EmbeddingId) -> Result<Option<SegmentId>>;
}

/// Service for building evidence packs and generating interpretations.
///
/// This is the main entry point for the interpretation bounded context.
pub struct InterpretationService {
    vector_service: Arc<dyn VectorSpaceService>,
    cluster_repo: Arc<dyn ClusterRepository>,
    metadata_service: Arc<dyn MetadataService>,
    sequence_service: Option<Arc<dyn SequenceService>>,
    evidence_pack_repo: Option<Arc<dyn EvidencePackRepository>>,
    evidence_builder: EvidenceBuilder,
    claim_generator: ClaimGenerator,
    config: InterpretationConfig,
}

impl InterpretationService {
    /// Create a new interpretation service.
    pub fn new(
        vector_service: Arc<dyn VectorSpaceService>,
        cluster_repo: Arc<dyn ClusterRepository>,
        metadata_service: Arc<dyn MetadataService>,
        config: InterpretationConfig,
    ) -> Self {
        let evidence_builder = EvidenceBuilder::new(&config);
        let claim_generator = ClaimGenerator::new(&config);

        Self {
            vector_service,
            cluster_repo,
            metadata_service,
            sequence_service: None,
            evidence_pack_repo: None,
            evidence_builder,
            claim_generator,
            config,
        }
    }

    /// Add sequence service for temporal context
    pub fn with_sequence_service(mut self, service: Arc<dyn SequenceService>) -> Self {
        self.sequence_service = Some(service);
        self
    }

    /// Add evidence pack repository for persistence
    pub fn with_repository(mut self, repo: Arc<dyn EvidencePackRepository>) -> Self {
        self.evidence_pack_repo = Some(repo);
        self
    }

    /// Build an evidence pack for a query embedding.
    ///
    /// This gathers all relevant evidence (neighbors, cluster context, sequence context)
    /// and generates an interpretation with cited claims.
    #[instrument(skip(self), fields(query_id = %query_id))]
    pub async fn build_evidence_pack(&self, query_id: &EmbeddingId) -> Result<EvidencePack> {
        info!("Building evidence pack for query: {}", query_id);

        // Step 1: Find neighbors
        let neighbors = self.vector_service
            .find_neighbors(query_id, self.config.max_neighbors)
            .await
            .map_err(|e| Error::VectorServiceError(e.to_string()))?;

        debug!("Found {} neighbors", neighbors.len());

        // Step 2: Collect neighbor evidence
        let neighbor_evidence = self
            .collect_neighbor_evidence(&neighbors)
            .await?;

        // Step 3: Build cluster context
        let cluster_context = self.build_cluster_context(query_id).await?;

        debug!(
            "Cluster context: assigned={}, confidence={}",
            cluster_context.has_cluster(),
            cluster_context.confidence
        );

        // Step 4: Build sequence context (if enabled and available)
        let sequence_context = if self.config.include_sequence_context {
            self.build_sequence_context(query_id).await?
        } else {
            None
        };

        // Step 5: Generate interpretation
        let interpretation = self
            .generate_interpretation_internal(
                query_id,
                &neighbor_evidence,
                &cluster_context,
                &sequence_context,
            )
            .await?;

        // Step 6: Create evidence pack
        let evidence_pack = EvidencePack::new(
            query_id.clone(),
            neighbor_evidence,
            cluster_context,
            sequence_context,
            interpretation,
        );

        info!(
            "Built evidence pack {} with {} neighbors, confidence={}",
            evidence_pack.id,
            evidence_pack.neighbors.len(),
            evidence_pack.overall_confidence()
        );

        // Step 7: Persist if repository is available
        if let Some(repo) = &self.evidence_pack_repo {
            repo.save(&evidence_pack).await?;
            debug!("Persisted evidence pack {}", evidence_pack.id);
        }

        Ok(evidence_pack)
    }

    /// Generate an interpretation for an existing evidence pack.
    ///
    /// Useful for regenerating interpretations with different parameters.
    #[instrument(skip(self, evidence))]
    pub async fn generate_interpretation(
        &self,
        evidence: &EvidencePack,
    ) -> Result<Interpretation> {
        self.generate_interpretation_internal(
            &evidence.query_embedding_id,
            &evidence.neighbors,
            &evidence.cluster_context,
            &evidence.sequence_context,
        )
        .await
    }

    /// Validate claims against evidence.
    ///
    /// Returns each claim paired with a boolean indicating if it's well-supported.
    #[instrument(skip(self, claims))]
    pub async fn validate_claims(&self, claims: &[Claim]) -> Result<Vec<(Claim, bool)>> {
        let mut results = Vec::with_capacity(claims.len());

        for claim in claims {
            let is_valid = self.validate_single_claim(claim).await?;
            results.push((claim.clone(), is_valid));
        }

        let valid_count = results.iter().filter(|(_, v)| *v).count();
        info!(
            "Validated {} claims: {} valid, {} invalid",
            claims.len(),
            valid_count,
            claims.len() - valid_count
        );

        Ok(results)
    }

    /// Collect neighbor evidence with metadata.
    async fn collect_neighbor_evidence(
        &self,
        neighbors: &[Neighbor],
    ) -> Result<Vec<NeighborEvidence>> {
        let mut evidence = Vec::with_capacity(neighbors.len());

        for neighbor in neighbors {
            let metadata = self
                .metadata_service
                .get_recording_metadata(&neighbor.embedding_id)
                .await
                .unwrap_or_else(|_| RecordingMetadata::new("unknown"));

            let mut neighbor_ev = NeighborEvidence::new(
                neighbor.embedding_id.clone(),
                neighbor.distance,
                metadata,
            );

            // Add cluster info
            let cluster_ctx = self
                .cluster_repo
                .get_cluster_context(&neighbor.embedding_id)
                .await
                .unwrap_or_else(|_| ClusterContext::empty());

            if let Some(cluster_id) = cluster_ctx.assigned_cluster {
                neighbor_ev = neighbor_ev.with_cluster(cluster_id);
            }

            // Add spectrogram URL if enabled
            if self.config.include_spectrograms {
                if let Ok(Some(url)) = self
                    .metadata_service
                    .get_spectrogram_url(&neighbor.embedding_id)
                    .await
                {
                    neighbor_ev = neighbor_ev.with_spectrogram(url);
                }
            }

            evidence.push(neighbor_ev);
        }

        Ok(evidence)
    }

    /// Build cluster context for an embedding.
    async fn build_cluster_context(&self, embedding_id: &EmbeddingId) -> Result<ClusterContext> {
        self.cluster_repo
            .get_cluster_context(embedding_id)
            .await
            .map_err(|e| Error::ClusterServiceError(e.to_string()))
    }

    /// Build sequence context if sequence service is available.
    async fn build_sequence_context(
        &self,
        embedding_id: &EmbeddingId,
    ) -> Result<Option<SequenceContext>> {
        let sequence_service = match &self.sequence_service {
            Some(s) => s,
            None => return Ok(None),
        };

        // Get segment ID for this embedding
        let segment_id = match self.metadata_service.get_segment_id(embedding_id).await? {
            Some(id) => id,
            None => return Ok(None),
        };

        let window = self.config.sequence_context_window;

        // Get preceding segments
        let preceding = sequence_service
            .get_preceding_segments(&segment_id, window)
            .await
            .unwrap_or_default();

        // Get following segments
        let following = sequence_service
            .get_following_segments(&segment_id, window)
            .await
            .unwrap_or_default();

        if preceding.is_empty() && following.is_empty() {
            return Ok(None);
        }

        // Try to detect motif
        let mut all_segments = preceding.clone();
        all_segments.push(segment_id);
        all_segments.extend(following.clone());

        let motif = sequence_service.detect_motif(&all_segments).await.ok().flatten();

        let context = SequenceContext::new(preceding, following);
        let context = if let Some(m) = motif {
            context.with_motif(m)
        } else {
            context
        };

        Ok(Some(context))
    }

    /// Internal implementation of interpretation generation.
    async fn generate_interpretation_internal(
        &self,
        query_id: &EmbeddingId,
        neighbors: &[NeighborEvidence],
        cluster_context: &ClusterContext,
        sequence_context: &Option<SequenceContext>,
    ) -> Result<Interpretation> {
        // Generate structural description
        let structural_description = self
            .generate_structural_description(neighbors, cluster_context, sequence_context);

        // Generate claims with evidence citations
        let claims = self
            .claim_generator
            .generate_claims(query_id, neighbors, cluster_context, sequence_context)
            .await?;

        // Filter claims by confidence threshold
        let claims: Vec<Claim> = claims
            .into_iter()
            .filter(|c| c.confidence >= self.config.min_claim_confidence)
            .take(self.config.max_claims)
            .collect();

        // Calculate overall confidence
        let confidence = if claims.is_empty() {
            0.0
        } else {
            let sum: f32 = claims.iter().map(|c| c.confidence).sum();
            sum / claims.len() as f32
        };

        Ok(Interpretation::new(structural_description, claims, confidence))
    }

    /// Generate a structural description of the acoustic signal.
    fn generate_structural_description(
        &self,
        neighbors: &[NeighborEvidence],
        cluster_context: &ClusterContext,
        sequence_context: &Option<SequenceContext>,
    ) -> String {
        let templates = InterpretationTemplates::new();

        let mut parts = Vec::new();

        // Describe based on neighbors
        if !neighbors.is_empty() {
            let avg_distance: f32 = neighbors.iter().map(|n| n.distance).sum::<f32>()
                / neighbors.len() as f32;
            let similarity = 1.0 - avg_distance.min(1.0);

            parts.push(templates.neighbor_description(neighbors.len(), similarity));

            // Add taxon info if available
            let taxa: Vec<&str> = neighbors
                .iter()
                .filter_map(|n| n.recording_metadata.taxon.as_deref())
                .collect();
            if !taxa.is_empty() {
                parts.push(templates.taxon_description(&taxa));
            }
        }

        // Describe cluster context
        if cluster_context.has_cluster() {
            let label = cluster_context
                .cluster_label
                .as_deref()
                .unwrap_or("unlabeled");
            parts.push(templates.cluster_description(
                label,
                cluster_context.confidence,
                cluster_context.exemplar_similarity,
            ));
        }

        // Describe sequence context
        if let Some(seq) = sequence_context {
            if seq.has_temporal_context() {
                parts.push(templates.sequence_description(
                    seq.sequence_length(),
                    seq.detected_motif.as_deref(),
                ));
            }
        }

        if parts.is_empty() {
            "Insufficient evidence for structural description.".to_string()
        } else {
            parts.join(" ")
        }
    }

    /// Validate a single claim against its evidence.
    async fn validate_single_claim(&self, claim: &Claim) -> Result<bool> {
        // A claim is valid if it has evidence AND confidence is above threshold
        if claim.evidence_refs.is_empty() {
            warn!("Claim has no evidence references: {}", claim.statement);
            return Ok(false);
        }

        if claim.confidence < self.config.min_claim_confidence {
            debug!(
                "Claim confidence {} below threshold {}: {}",
                claim.confidence, self.config.min_claim_confidence, claim.statement
            );
            return Ok(false);
        }

        // Verify each evidence reference exists
        for evidence_ref in &claim.evidence_refs {
            let exists = match evidence_ref.ref_type {
                EvidenceRefType::Neighbor => {
                    let emb_id = EmbeddingId::new(&evidence_ref.ref_id);
                    self.vector_service
                        .get_embedding(&emb_id)
                        .await
                        .map(|e| e.is_some())
                        .unwrap_or(false)
                }
                EvidenceRefType::Cluster => {
                    let cluster_id = crate::domain::entities::ClusterId::new(&evidence_ref.ref_id);
                    self.cluster_repo
                        .get_cluster_label(&cluster_id)
                        .await
                        .is_ok()
                }
                EvidenceRefType::Sequence | EvidenceRefType::Taxon => {
                    // These are derived evidence, assume valid if present
                    true
                }
            };

            if !exists {
                warn!(
                    "Evidence reference not found: {} ({})",
                    evidence_ref.ref_id, evidence_ref.ref_type
                );
                return Ok(false);
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repository::InMemoryClusterRepository;
    use std::collections::HashMap;
    use std::sync::RwLock;

    // Mock implementations for testing

    struct MockVectorService {
        neighbors: RwLock<HashMap<String, Vec<Neighbor>>>,
        embeddings: RwLock<HashMap<String, Vec<f32>>>,
    }

    impl MockVectorService {
        fn new() -> Self {
            Self {
                neighbors: RwLock::new(HashMap::new()),
                embeddings: RwLock::new(HashMap::new()),
            }
        }

        fn add_neighbor(&self, query_id: &str, neighbor: Neighbor) {
            let mut neighbors = self.neighbors.write().unwrap();
            neighbors
                .entry(query_id.to_string())
                .or_default()
                .push(neighbor);
        }

        fn add_embedding(&self, id: &str, embedding: Vec<f32>) {
            let mut embeddings = self.embeddings.write().unwrap();
            embeddings.insert(id.to_string(), embedding);
        }
    }

    #[async_trait::async_trait]
    impl VectorSpaceService for MockVectorService {
        async fn find_neighbors(&self, embedding_id: &EmbeddingId, k: usize) -> Result<Vec<Neighbor>> {
            let neighbors = self.neighbors.read().unwrap();
            let result = neighbors
                .get(embedding_id.as_str())
                .map(|n| n.iter().take(k).cloned().collect())
                .unwrap_or_default();
            Ok(result)
        }

        async fn get_embedding(&self, embedding_id: &EmbeddingId) -> Result<Option<Vec<f32>>> {
            let embeddings = self.embeddings.read().unwrap();
            Ok(embeddings.get(embedding_id.as_str()).cloned())
        }

        async fn calculate_similarity(
            &self,
            _embedding_id_a: &EmbeddingId,
            _embedding_id_b: &EmbeddingId,
        ) -> Result<f32> {
            Ok(0.85)
        }
    }

    struct MockMetadataService;

    #[async_trait::async_trait]
    impl MetadataService for MockMetadataService {
        async fn get_recording_metadata(
            &self,
            embedding_id: &EmbeddingId,
        ) -> Result<RecordingMetadata> {
            Ok(RecordingMetadata::new(format!("recording-{}", embedding_id)))
        }

        async fn get_spectrogram_url(&self, embedding_id: &EmbeddingId) -> Result<Option<String>> {
            Ok(Some(format!(
                "https://spectrograms.example.com/{}",
                embedding_id
            )))
        }

        async fn get_segment_id(&self, _embedding_id: &EmbeddingId) -> Result<Option<SegmentId>> {
            Ok(None)
        }
    }

    #[tokio::test]
    async fn test_interpretation_service_build_evidence_pack() {
        let vector_service = Arc::new(MockVectorService::new());
        let cluster_repo = Arc::new(InMemoryClusterRepository::new());
        let metadata_service = Arc::new(MockMetadataService);

        // Add some test data
        vector_service.add_neighbor(
            "query-1",
            Neighbor::new(EmbeddingId::new("neighbor-1"), 0.1),
        );
        vector_service.add_neighbor(
            "query-1",
            Neighbor::new(EmbeddingId::new("neighbor-2"), 0.2),
        );
        vector_service.add_embedding("neighbor-1", vec![0.1, 0.2, 0.3]);
        vector_service.add_embedding("neighbor-2", vec![0.2, 0.3, 0.4]);

        let config = InterpretationConfig::default();
        let service = InterpretationService::new(
            vector_service,
            cluster_repo,
            metadata_service,
            config,
        );

        let query_id = EmbeddingId::new("query-1");
        let result = service.build_evidence_pack(&query_id).await;

        assert!(result.is_ok());
        let pack = result.unwrap();
        assert_eq!(pack.query_embedding_id, query_id);
        assert_eq!(pack.neighbors.len(), 2);
    }

    #[tokio::test]
    async fn test_validate_claims() {
        let vector_service = Arc::new(MockVectorService::new());
        let cluster_repo = Arc::new(InMemoryClusterRepository::new());
        let metadata_service = Arc::new(MockMetadataService);

        vector_service.add_embedding("evidence-1", vec![0.1, 0.2, 0.3]);

        let config = InterpretationConfig::default();
        let service = InterpretationService::new(
            vector_service,
            cluster_repo,
            metadata_service,
            config,
        );

        let valid_claim = Claim::new("Valid claim with evidence", 0.9)
            .with_evidence(vec![EvidenceRef::neighbor(
                &EmbeddingId::new("evidence-1"),
                "Supporting evidence",
            )]);

        let invalid_claim = Claim::new("Invalid claim without evidence", 0.9);

        let results = service
            .validate_claims(&[valid_claim, invalid_claim])
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].1); // Valid claim
        assert!(!results[1].1); // Invalid claim (no evidence)
    }
}
