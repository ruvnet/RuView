//! Claim generator for RAB interpretations.
//!
//! This module generates claims with evidence citations based on
//! neighbor evidence, cluster context, and sequence context.

use tracing::{debug, instrument};

use crate::application::services::InterpretationConfig;
use crate::domain::entities::{
    Claim, ClusterContext, ClusterId, EmbeddingId, EvidenceRef, EvidenceRefType,
    NeighborEvidence, SequenceContext,
};
use crate::infrastructure::evidence_builder::EvidenceContext;
use crate::templates::InterpretationTemplates;
use crate::Result;

/// Generator for evidence-backed claims.
///
/// The `ClaimGenerator` creates claims based on available evidence
/// and ensures each claim has proper citations.
#[derive(Debug, Clone)]
pub struct ClaimGenerator {
    /// Minimum confidence threshold for claims
    min_confidence: f32,
    /// Maximum claims to generate
    max_claims: usize,
    /// Templates for claim text
    templates: InterpretationTemplates,
}

impl ClaimGenerator {
    /// Create a new claim generator from configuration.
    pub fn new(config: &InterpretationConfig) -> Self {
        Self {
            min_confidence: config.min_claim_confidence,
            max_claims: config.max_claims,
            templates: InterpretationTemplates::new(),
        }
    }

    /// Create a claim generator with custom parameters.
    pub fn with_params(min_confidence: f32, max_claims: usize) -> Self {
        Self {
            min_confidence,
            max_claims,
            templates: InterpretationTemplates::new(),
        }
    }

    /// Generate claims from collected evidence.
    ///
    /// Claims are generated based on:
    /// - Neighbor similarity patterns
    /// - Cluster assignments
    /// - Taxonomic information
    /// - Temporal sequence patterns
    #[instrument(skip(self, neighbors, cluster_context, sequence_context))]
    pub async fn generate_claims(
        &self,
        query_id: &EmbeddingId,
        neighbors: &[NeighborEvidence],
        cluster_context: &ClusterContext,
        sequence_context: &Option<SequenceContext>,
    ) -> Result<Vec<Claim>> {
        let context = EvidenceContext::from_evidence(neighbors, cluster_context, sequence_context);
        let mut claims = Vec::new();

        // Generate similarity-based claims
        let similarity_claims = self.generate_similarity_claims(neighbors, &context);
        claims.extend(similarity_claims);

        // Generate cluster-based claims
        if cluster_context.has_cluster() {
            let cluster_claims = self.generate_cluster_claims(cluster_context, &context);
            claims.extend(cluster_claims);
        }

        // Generate taxonomy-based claims
        if !context.unique_taxa.is_empty() {
            let taxon_claims = self.generate_taxon_claims(neighbors, &context);
            claims.extend(taxon_claims);
        }

        // Generate sequence-based claims
        if let Some(seq) = sequence_context {
            if seq.has_temporal_context() {
                let sequence_claims = self.generate_sequence_claims(seq, &context);
                claims.extend(sequence_claims);
            }
        }

        // Filter by confidence and limit
        let mut claims: Vec<Claim> = claims
            .into_iter()
            .filter(|c| c.confidence >= self.min_confidence)
            .collect();

        // Sort by confidence (highest first)
        claims.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        // Limit number of claims
        claims.truncate(self.max_claims);

        debug!(
            "Generated {} claims for query {}",
            claims.len(),
            query_id
        );

        Ok(claims)
    }

    /// Generate claims based on neighbor similarity.
    fn generate_similarity_claims(
        &self,
        neighbors: &[NeighborEvidence],
        context: &EvidenceContext,
    ) -> Vec<Claim> {
        let mut claims = Vec::new();

        if neighbors.is_empty() {
            return claims;
        }

        // Claim about overall similarity
        let similarity = context.scores.avg_similarity;
        if similarity >= 0.7 {
            let statement = self.templates.high_similarity_claim(
                neighbors.len(),
                similarity,
            );
            let confidence = similarity * 0.9;

            let evidence: Vec<EvidenceRef> = neighbors
                .iter()
                .take(3)
                .map(|n| {
                    EvidenceRef::neighbor(
                        &n.embedding_id,
                        format!(
                            "Similarity: {:.1}% (distance: {:.3})",
                            n.similarity() * 100.0,
                            n.distance
                        ),
                    )
                })
                .collect();

            claims.push(Claim::new(statement, confidence).with_evidence(evidence));
        } else if similarity >= 0.5 {
            let statement = self.templates.moderate_similarity_claim(neighbors.len());
            let confidence = similarity * 0.8;

            let evidence: Vec<EvidenceRef> = neighbors
                .iter()
                .take(2)
                .map(|n| {
                    EvidenceRef::neighbor(
                        &n.embedding_id,
                        format!("Distance: {:.3}", n.distance),
                    )
                })
                .collect();

            claims.push(Claim::new(statement, confidence).with_evidence(evidence));
        } else if neighbors.len() >= 3 {
            let statement = self.templates.low_similarity_claim();
            let confidence = 0.4;

            let evidence = vec![EvidenceRef::new(
                EvidenceRefType::Neighbor,
                "aggregate",
                format!(
                    "Average distance: {:.3} across {} neighbors",
                    context.scores.avg_distance,
                    neighbors.len()
                ),
            )];

            claims.push(Claim::new(statement, confidence).with_evidence(evidence));
        }

        // Claim about closest match
        if let Some(closest) = neighbors.first() {
            if closest.distance < 0.2 {
                let taxon_info = closest
                    .recording_metadata
                    .taxon
                    .as_deref()
                    .map(|t| format!(" ({})", t))
                    .unwrap_or_default();

                let statement = format!(
                    "Strong acoustic match found with recording {}{}",
                    closest.recording_metadata.recording_id,
                    taxon_info
                );
                let confidence = (1.0 - closest.distance) * 0.95;

                let evidence = vec![
                    EvidenceRef::neighbor(
                        &closest.embedding_id,
                        format!(
                            "Closest neighbor with distance {:.3} ({:.1}% similarity)",
                            closest.distance,
                            closest.similarity() * 100.0
                        ),
                    ),
                ];

                claims.push(Claim::new(statement, confidence).with_evidence(evidence));
            }
        }

        claims
    }

    /// Generate claims based on cluster assignment.
    fn generate_cluster_claims(
        &self,
        cluster_context: &ClusterContext,
        context: &EvidenceContext,
    ) -> Vec<Claim> {
        let mut claims = Vec::new();

        let cluster_id = match &cluster_context.assigned_cluster {
            Some(id) => id,
            None => return claims,
        };

        let label = cluster_context
            .cluster_label
            .as_deref()
            .unwrap_or("unlabeled cluster");

        // Main cluster assignment claim
        let statement = self.templates.cluster_assignment_claim(
            label,
            cluster_context.confidence,
            cluster_context.exemplar_similarity,
        );
        let confidence = cluster_context.confidence * cluster_context.exemplar_similarity;

        let evidence = vec![
            EvidenceRef::cluster(
                cluster_id,
                format!(
                    "Assigned to cluster '{}' with {:.1}% confidence, {:.1}% exemplar similarity",
                    label,
                    cluster_context.confidence * 100.0,
                    cluster_context.exemplar_similarity * 100.0
                ),
            ),
        ];

        claims.push(Claim::new(statement, confidence).with_evidence(evidence));

        // Claim about cluster coherence with neighbors
        if context.scores.cluster_coherence > 0.5 {
            let statement = format!(
                "Acoustic features are consistent with {} - {:.0}% of similar recordings belong to the same cluster",
                label,
                context.scores.cluster_coherence * 100.0
            );
            let confidence = context.scores.cluster_coherence * 0.85;

            let evidence = vec![
                EvidenceRef::cluster(
                    cluster_id,
                    format!(
                        "{:.0}% cluster coherence among neighbors",
                        context.scores.cluster_coherence * 100.0
                    ),
                ),
            ];

            claims.push(Claim::new(statement, confidence).with_evidence(evidence));
        }

        claims
    }

    /// Generate claims based on taxonomic information.
    fn generate_taxon_claims(
        &self,
        neighbors: &[NeighborEvidence],
        context: &EvidenceContext,
    ) -> Vec<Claim> {
        let mut claims = Vec::new();

        if context.unique_taxa.is_empty() {
            return claims;
        }

        // Count taxa occurrences
        let mut taxon_counts: std::collections::HashMap<&str, (usize, Vec<&NeighborEvidence>)> =
            std::collections::HashMap::new();

        for neighbor in neighbors {
            if let Some(taxon) = &neighbor.recording_metadata.taxon {
                let entry = taxon_counts.entry(taxon.as_str()).or_insert((0, Vec::new()));
                entry.0 += 1;
                entry.1.push(neighbor);
            }
        }

        // Find dominant taxon
        if let Some((taxon, (count, examples))) = taxon_counts
            .iter()
            .max_by_key(|(_, (count, _))| count)
        {
            let proportion = *count as f32 / neighbors.len() as f32;

            if proportion >= 0.6 {
                let statement = self.templates.dominant_taxon_claim(taxon, proportion);
                let confidence = proportion * context.scores.avg_similarity;

                let evidence: Vec<EvidenceRef> = examples
                    .iter()
                    .take(3)
                    .map(|n| {
                        EvidenceRef::taxon(
                            *taxon,
                            format!(
                                "Recording {} identified as {} (similarity: {:.1}%)",
                                n.recording_metadata.recording_id,
                                taxon,
                                n.similarity() * 100.0
                            ),
                        )
                    })
                    .collect();

                claims.push(Claim::new(statement, confidence).with_evidence(evidence));
            } else if context.unique_taxa.len() > 1 {
                // Multiple taxa present
                let taxa_list = context.unique_taxa.join(", ");
                let statement = format!(
                    "Acoustic features show similarity to multiple taxa: {}. Further analysis recommended.",
                    taxa_list
                );
                let confidence = 0.5;

                let evidence: Vec<EvidenceRef> = context
                    .unique_taxa
                    .iter()
                    .take(3)
                    .map(|t| {
                        let count = taxon_counts.get(t.as_str()).map(|(c, _)| *c).unwrap_or(0);
                        EvidenceRef::taxon(
                            t,
                            format!("{} neighbors identified as {}", count, t),
                        )
                    })
                    .collect();

                claims.push(Claim::new(statement, confidence).with_evidence(evidence));
            }
        }

        claims
    }

    /// Generate claims based on sequence context.
    fn generate_sequence_claims(
        &self,
        sequence_context: &SequenceContext,
        context: &EvidenceContext,
    ) -> Vec<Claim> {
        let mut claims = Vec::new();

        // Claim about temporal context
        let preceding = sequence_context.preceding_segments.len();
        let following = sequence_context.following_segments.len();

        if preceding > 0 || following > 0 {
            let statement = self.templates.sequence_context_claim(preceding, following);
            let confidence = 0.7;

            let mut evidence = Vec::new();

            for (i, seg) in sequence_context.preceding_segments.iter().enumerate() {
                evidence.push(EvidenceRef::sequence(
                    seg,
                    format!("Preceding segment {} at position -{}", seg.0, preceding - i),
                ));
            }

            for (i, seg) in sequence_context.following_segments.iter().enumerate() {
                evidence.push(EvidenceRef::sequence(
                    seg,
                    format!("Following segment {} at position +{}", seg.0, i + 1),
                ));
            }

            claims.push(Claim::new(statement, confidence).with_evidence(evidence));
        }

        // Claim about detected motif
        if let Some(motif) = &sequence_context.detected_motif {
            let statement = self.templates.motif_claim(motif);
            let confidence = 0.75;

            let evidence = vec![EvidenceRef::new(
                EvidenceRefType::Sequence,
                "motif-detection",
                format!(
                    "Motif pattern '{}' detected across {} segments",
                    motif,
                    sequence_context.sequence_length()
                ),
            )];

            claims.push(Claim::new(statement, confidence).with_evidence(evidence));
        }

        claims
    }

    /// Generate a claim from manual input with evidence validation.
    pub fn create_manual_claim(
        &self,
        statement: &str,
        confidence: f32,
        evidence_refs: Vec<EvidenceRef>,
    ) -> Result<Claim> {
        if evidence_refs.is_empty() {
            return Err(crate::Error::ClaimValidationFailed(
                "Claims must have at least one evidence reference".to_string(),
            ));
        }

        let confidence = confidence.clamp(0.0, 1.0);

        if confidence < self.min_confidence {
            debug!(
                "Claim confidence {} below threshold {}: {}",
                confidence, self.min_confidence, statement
            );
        }

        Ok(Claim::new(statement, confidence).with_evidence(evidence_refs))
    }

    /// Merge multiple claims about the same topic.
    pub fn merge_claims(&self, claims: &[Claim]) -> Option<Claim> {
        if claims.is_empty() {
            return None;
        }

        if claims.len() == 1 {
            return Some(claims[0].clone());
        }

        // Combine evidence from all claims
        let mut all_evidence: Vec<EvidenceRef> = Vec::new();
        let mut total_confidence = 0.0;

        for claim in claims {
            all_evidence.extend(claim.evidence_refs.clone());
            total_confidence += claim.confidence;
        }

        // Deduplicate evidence by ref_id
        let mut seen_ids = std::collections::HashSet::new();
        all_evidence.retain(|e| seen_ids.insert(e.ref_id.clone()));

        let avg_confidence = total_confidence / claims.len() as f32;

        // Use the statement from the highest-confidence claim
        let best_claim = claims
            .iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
            .unwrap();

        Some(
            Claim::new(&best_claim.statement, avg_confidence)
                .with_evidence(all_evidence),
        )
    }
}

/// Builder for creating claims with proper evidence citations.
#[derive(Debug)]
pub struct ClaimBuilder {
    statement: String,
    confidence: f32,
    evidence: Vec<EvidenceRef>,
}

impl ClaimBuilder {
    /// Start building a new claim.
    pub fn new(statement: impl Into<String>) -> Self {
        Self {
            statement: statement.into(),
            confidence: 0.5,
            evidence: Vec::new(),
        }
    }

    /// Set the confidence level.
    pub fn confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add a neighbor evidence reference.
    pub fn cite_neighbor(
        mut self,
        embedding_id: &EmbeddingId,
        description: impl Into<String>,
    ) -> Self {
        self.evidence.push(EvidenceRef::neighbor(embedding_id, description));
        self
    }

    /// Add a cluster evidence reference.
    pub fn cite_cluster(
        mut self,
        cluster_id: &ClusterId,
        description: impl Into<String>,
    ) -> Self {
        self.evidence.push(EvidenceRef::cluster(cluster_id, description));
        self
    }

    /// Add a taxon evidence reference.
    pub fn cite_taxon(
        mut self,
        taxon: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        self.evidence.push(EvidenceRef::taxon(taxon, description));
        self
    }

    /// Add a sequence evidence reference.
    pub fn cite_sequence(
        mut self,
        segment_id: &crate::domain::entities::SegmentId,
        description: impl Into<String>,
    ) -> Self {
        self.evidence.push(EvidenceRef::sequence(segment_id, description));
        self
    }

    /// Build the claim.
    pub fn build(self) -> Claim {
        Claim::new(self.statement, self.confidence).with_evidence(self.evidence)
    }

    /// Build the claim, requiring at least one evidence reference.
    pub fn build_validated(self) -> Result<Claim> {
        if self.evidence.is_empty() {
            return Err(crate::Error::ClaimValidationFailed(
                "Claims must have at least one evidence reference".to_string(),
            ));
        }
        Ok(self.build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::RecordingMetadata;

    fn create_test_neighbors() -> Vec<NeighborEvidence> {
        vec![
            NeighborEvidence::new(
                EmbeddingId::new("n1"),
                0.1,
                RecordingMetadata::new("r1").with_taxon("Species A"),
            ).with_cluster(ClusterId::new("c1")),
            NeighborEvidence::new(
                EmbeddingId::new("n2"),
                0.15,
                RecordingMetadata::new("r2").with_taxon("Species A"),
            ).with_cluster(ClusterId::new("c1")),
            NeighborEvidence::new(
                EmbeddingId::new("n3"),
                0.2,
                RecordingMetadata::new("r3").with_taxon("Species B"),
            ).with_cluster(ClusterId::new("c2")),
        ]
    }

    #[tokio::test]
    async fn test_generate_claims_with_neighbors() {
        let generator = ClaimGenerator::with_params(0.3, 10);
        let neighbors = create_test_neighbors();
        let cluster_context = ClusterContext::empty();
        let query_id = EmbeddingId::new("query-1");

        let claims = generator
            .generate_claims(&query_id, &neighbors, &cluster_context, &None)
            .await
            .unwrap();

        assert!(!claims.is_empty());

        // All claims should have evidence
        for claim in &claims {
            assert!(claim.has_evidence(), "Claim should have evidence: {}", claim.statement);
        }
    }

    #[tokio::test]
    async fn test_generate_claims_with_cluster() {
        let generator = ClaimGenerator::with_params(0.3, 10);
        let neighbors = create_test_neighbors();
        let cluster_context = ClusterContext::new(
            Some(ClusterId::new("c1")),
            0.9,
            0.85,
        ).with_label("Song Type A");
        let query_id = EmbeddingId::new("query-1");

        let claims = generator
            .generate_claims(&query_id, &neighbors, &cluster_context, &None)
            .await
            .unwrap();

        // Should have cluster-related claims
        let cluster_claims: Vec<_> = claims
            .iter()
            .filter(|c| c.statement.contains("cluster") || c.statement.contains("Song Type A"))
            .collect();

        assert!(!cluster_claims.is_empty());

        // Cluster claims should cite the cluster
        for claim in cluster_claims {
            let cluster_refs = claim.evidence_of_type(EvidenceRefType::Cluster);
            assert!(!cluster_refs.is_empty());
        }
    }

    #[tokio::test]
    async fn test_generate_claims_with_sequence() {
        let generator = ClaimGenerator::with_params(0.3, 10);
        let neighbors = create_test_neighbors();
        let cluster_context = ClusterContext::empty();
        let sequence_context = Some(SequenceContext::new(
            vec![
                crate::domain::entities::SegmentId::new("seg-1"),
                crate::domain::entities::SegmentId::new("seg-2"),
            ],
            vec![
                crate::domain::entities::SegmentId::new("seg-4"),
            ],
        ).with_motif("ABAB"));

        let query_id = EmbeddingId::new("query-1");

        let claims = generator
            .generate_claims(&query_id, &neighbors, &cluster_context, &sequence_context)
            .await
            .unwrap();

        // Should have sequence-related claims
        let sequence_claims: Vec<_> = claims
            .iter()
            .filter(|c| {
                c.statement.contains("sequence")
                    || c.statement.contains("temporal")
                    || c.statement.contains("motif")
                    || c.statement.contains("ABAB")
            })
            .collect();

        assert!(!sequence_claims.is_empty());
    }

    #[test]
    fn test_claim_builder() {
        let claim = ClaimBuilder::new("Test claim statement")
            .confidence(0.85)
            .cite_neighbor(
                &EmbeddingId::new("n1"),
                "Supporting neighbor evidence",
            )
            .cite_cluster(
                &ClusterId::new("c1"),
                "Cluster assignment evidence",
            )
            .build();

        assert_eq!(claim.statement, "Test claim statement");
        assert_eq!(claim.confidence, 0.85);
        assert_eq!(claim.evidence_refs.len(), 2);
    }

    #[test]
    fn test_claim_builder_validated() {
        // Should fail without evidence
        let result = ClaimBuilder::new("Unsupported claim")
            .confidence(0.9)
            .build_validated();

        assert!(result.is_err());

        // Should succeed with evidence
        let result = ClaimBuilder::new("Supported claim")
            .confidence(0.9)
            .cite_taxon("Species A", "Taxon evidence")
            .build_validated();

        assert!(result.is_ok());
    }

    #[test]
    fn test_merge_claims() {
        let generator = ClaimGenerator::with_params(0.3, 10);

        let claim1 = ClaimBuilder::new("Similar acoustic features observed")
            .confidence(0.8)
            .cite_neighbor(&EmbeddingId::new("n1"), "Evidence 1")
            .build();

        let claim2 = ClaimBuilder::new("Similar acoustic features observed")
            .confidence(0.7)
            .cite_neighbor(&EmbeddingId::new("n2"), "Evidence 2")
            .build();

        let merged = generator.merge_claims(&[claim1, claim2]);

        assert!(merged.is_some());
        let merged = merged.unwrap();
        assert_eq!(merged.evidence_refs.len(), 2);
        assert_eq!(merged.confidence, 0.75);
    }

    #[test]
    fn test_create_manual_claim() {
        let generator = ClaimGenerator::with_params(0.5, 10);

        // Should fail without evidence
        let result = generator.create_manual_claim(
            "Test claim",
            0.8,
            Vec::new(),
        );
        assert!(result.is_err());

        // Should succeed with evidence
        let result = generator.create_manual_claim(
            "Test claim",
            0.8,
            vec![EvidenceRef::taxon("Species A", "Manual evidence")],
        );
        assert!(result.is_ok());
    }
}
