//! Interpretation text templates.
//!
//! This module provides templates for generating human-readable
//! interpretations and claims.

use std::collections::HashMap;

/// Templates for generating interpretation text.
#[derive(Debug, Clone)]
pub struct InterpretationTemplates {
    /// Custom template overrides
    custom_templates: HashMap<String, String>,
}

impl Default for InterpretationTemplates {
    fn default() -> Self {
        Self::new()
    }
}

impl InterpretationTemplates {
    /// Create a new templates instance with default templates.
    pub fn new() -> Self {
        Self {
            custom_templates: HashMap::new(),
        }
    }

    /// Add a custom template override.
    pub fn with_template(mut self, key: &str, template: &str) -> Self {
        self.custom_templates.insert(key.to_string(), template.to_string());
        self
    }

    /// Get a template by key, falling back to default.
    fn get_template(&self, key: &str, default: &str) -> String {
        self.custom_templates
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    // === Structural Description Templates ===

    /// Generate neighbor-based description.
    pub fn neighbor_description(&self, count: usize, similarity: f32) -> String {
        let similarity_level = if similarity >= 0.8 {
            "high"
        } else if similarity >= 0.6 {
            "moderate"
        } else if similarity >= 0.4 {
            "low"
        } else {
            "minimal"
        };

        format!(
            "Acoustic signal shows {} similarity ({:.1}%) to {} reference recordings in the database.",
            similarity_level,
            similarity * 100.0,
            count
        )
    }

    /// Generate taxon-based description.
    pub fn taxon_description(&self, taxa: &[&str]) -> String {
        if taxa.is_empty() {
            return String::new();
        }

        let unique_taxa: Vec<&str> = {
            let mut seen = std::collections::HashSet::new();
            taxa.iter().filter(|t| seen.insert(**t)).copied().collect()
        };

        if unique_taxa.len() == 1 {
            format!(
                "Reference recordings are primarily associated with {}.",
                unique_taxa[0]
            )
        } else {
            let taxa_list = unique_taxa.join(", ");
            format!(
                "Reference recordings span multiple taxa: {}.",
                taxa_list
            )
        }
    }

    /// Generate cluster-based description.
    pub fn cluster_description(
        &self,
        label: &str,
        confidence: f32,
        exemplar_similarity: f32,
    ) -> String {
        let confidence_level = if confidence >= 0.9 {
            "very high"
        } else if confidence >= 0.7 {
            "high"
        } else if confidence >= 0.5 {
            "moderate"
        } else {
            "low"
        };

        format!(
            "Cluster analysis places this vocalization in '{}' with {} confidence ({:.1}%) and {:.1}% similarity to the cluster exemplar.",
            label,
            confidence_level,
            confidence * 100.0,
            exemplar_similarity * 100.0
        )
    }

    /// Generate sequence-based description.
    pub fn sequence_description(&self, sequence_length: usize, motif: Option<&str>) -> String {
        let base = format!(
            "Temporal analysis reveals this segment is part of a {} vocalization sequence.",
            sequence_length
        );

        if let Some(m) = motif {
            format!("{} A recurring motif pattern '{}' has been detected.", base, m)
        } else {
            base
        }
    }

    // === Claim Templates ===

    /// High similarity claim.
    pub fn high_similarity_claim(&self, count: usize, similarity: f32) -> String {
        format!(
            "Strong acoustic similarity ({:.1}%) to {} database recordings suggests a reliable identification.",
            similarity * 100.0,
            count
        )
    }

    /// Moderate similarity claim.
    pub fn moderate_similarity_claim(&self, count: usize) -> String {
        format!(
            "Moderate acoustic similarity to {} reference recordings found. Additional context recommended for confident identification.",
            count
        )
    }

    /// Low similarity claim.
    pub fn low_similarity_claim(&self) -> String {
        self.get_template(
            "low_similarity",
            "Limited similarity to existing reference recordings. This may represent an unusual vocalization variant or a novel recording."
        )
    }

    /// Cluster assignment claim.
    pub fn cluster_assignment_claim(
        &self,
        label: &str,
        confidence: f32,
        exemplar_similarity: f32,
    ) -> String {
        format!(
            "This vocalization is classified as '{}' based on acoustic clustering (confidence: {:.1}%, exemplar similarity: {:.1}%).",
            label,
            confidence * 100.0,
            exemplar_similarity * 100.0
        )
    }

    /// Dominant taxon claim.
    pub fn dominant_taxon_claim(&self, taxon: &str, proportion: f32) -> String {
        format!(
            "Acoustic features strongly suggest {} ({:.0}% of similar recordings in the database belong to this taxon).",
            taxon,
            proportion * 100.0
        )
    }

    /// Sequence context claim.
    pub fn sequence_context_claim(&self, preceding: usize, following: usize) -> String {
        format!(
            "This vocalization appears within a temporal sequence with {} preceding and {} following segments, providing additional context for interpretation.",
            preceding,
            following
        )
    }

    /// Motif claim.
    pub fn motif_claim(&self, motif: &str) -> String {
        format!(
            "A repeating acoustic motif '{}' has been detected in the vocalization sequence, suggesting a structured call pattern.",
            motif
        )
    }

    // === Evidence Description Templates ===

    /// Format neighbor evidence description.
    pub fn neighbor_evidence_description(
        &self,
        recording_id: &str,
        distance: f32,
        taxon: Option<&str>,
    ) -> String {
        let similarity = ((1.0 - distance) * 100.0).max(0.0);

        if let Some(t) = taxon {
            format!(
                "Recording {} ({}) with {:.1}% acoustic similarity",
                recording_id, t, similarity
            )
        } else {
            format!(
                "Recording {} with {:.1}% acoustic similarity",
                recording_id, similarity
            )
        }
    }

    /// Format cluster evidence description.
    pub fn cluster_evidence_description(
        &self,
        label: &str,
        confidence: f32,
    ) -> String {
        format!(
            "Assigned to cluster '{}' with {:.1}% confidence",
            label,
            confidence * 100.0
        )
    }

    /// Format sequence evidence description.
    pub fn sequence_evidence_description(
        &self,
        segment_id: &str,
        position: i32,
    ) -> String {
        let position_desc = if position < 0 {
            format!("position {} before target", -position)
        } else if position > 0 {
            format!("position {} after target", position)
        } else {
            "target position".to_string()
        };

        format!("Segment {} at {}", segment_id, position_desc)
    }

    // === Summary Templates ===

    /// Generate an overall summary.
    pub fn generate_summary(
        &self,
        neighbor_count: usize,
        avg_similarity: f32,
        cluster_label: Option<&str>,
        dominant_taxon: Option<&str>,
        confidence: f32,
    ) -> String {
        let mut parts = Vec::new();

        // Similarity summary
        let similarity_desc = if avg_similarity >= 0.8 {
            format!(
                "highly similar ({:.1}%) to {} reference recordings",
                avg_similarity * 100.0,
                neighbor_count
            )
        } else if avg_similarity >= 0.5 {
            format!(
                "moderately similar ({:.1}%) to {} reference recordings",
                avg_similarity * 100.0,
                neighbor_count
            )
        } else {
            format!(
                "shows limited similarity ({:.1}%) to {} reference recordings",
                avg_similarity * 100.0,
                neighbor_count
            )
        };
        parts.push(similarity_desc);

        // Cluster summary
        if let Some(label) = cluster_label {
            parts.push(format!("classified in cluster '{}'", label));
        }

        // Taxon summary
        if let Some(taxon) = dominant_taxon {
            parts.push(format!("likely associated with {}", taxon));
        }

        let main_summary = parts.join(", ");

        // Confidence qualifier
        let confidence_qualifier = if confidence >= 0.8 {
            "High confidence interpretation"
        } else if confidence >= 0.5 {
            "Moderate confidence interpretation"
        } else {
            "Low confidence interpretation"
        };

        format!(
            "{}. This vocalization is {}. Overall confidence: {:.1}%.",
            confidence_qualifier,
            main_summary,
            confidence * 100.0
        )
    }

    /// Generate a confidence explanation.
    pub fn confidence_explanation(&self, confidence: f32) -> String {
        if confidence >= 0.9 {
            "Very high confidence: Strong evidence from multiple sources supports this interpretation.".to_string()
        } else if confidence >= 0.7 {
            "High confidence: Good evidence supports this interpretation with minor uncertainty.".to_string()
        } else if confidence >= 0.5 {
            "Moderate confidence: Evidence partially supports this interpretation. Additional verification recommended.".to_string()
        } else if confidence >= 0.3 {
            "Low confidence: Limited evidence available. Interpretation should be considered tentative.".to_string()
        } else {
            "Very low confidence: Insufficient evidence for reliable interpretation. Expert review recommended.".to_string()
        }
    }
}

/// Formatter for evidence pack output.
#[derive(Debug)]
pub struct EvidencePackFormatter {
    templates: InterpretationTemplates,
    include_details: bool,
    max_evidence_items: usize,
}

impl Default for EvidencePackFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl EvidencePackFormatter {
    /// Create a new formatter.
    pub fn new() -> Self {
        Self {
            templates: InterpretationTemplates::new(),
            include_details: true,
            max_evidence_items: 5,
        }
    }

    /// Set whether to include detailed evidence.
    pub fn with_details(mut self, include: bool) -> Self {
        self.include_details = include;
        self
    }

    /// Set maximum evidence items to show.
    pub fn with_max_evidence(mut self, max: usize) -> Self {
        self.max_evidence_items = max;
        self
    }

    /// Format an evidence pack as a structured report.
    pub fn format_report(&self, pack: &crate::domain::entities::EvidencePack) -> String {
        let mut sections = Vec::new();

        // Header
        sections.push(format!(
            "# Evidence Pack Report\n\nID: {}\nQuery: {}\nCreated: {}",
            pack.id,
            pack.query_embedding_id,
            pack.created_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));

        // Summary
        sections.push(format!(
            "\n## Summary\n\n{}",
            self.templates.generate_summary(
                pack.neighbors.len(),
                pack.overall_confidence(),
                pack.cluster_context.cluster_label.as_deref(),
                pack.neighbors.first().and_then(|n| n.recording_metadata.taxon.as_deref()),
                pack.interpretation.confidence,
            )
        ));

        // Structural description
        sections.push(format!(
            "\n## Structural Analysis\n\n{}",
            pack.interpretation.structural_description
        ));

        // Claims
        if !pack.interpretation.claims.is_empty() {
            let claims_text: Vec<String> = pack
                .interpretation
                .claims
                .iter()
                .map(|c| {
                    let evidence_count = c.evidence_refs.len();
                    format!(
                        "- {} (confidence: {:.1}%, {} evidence reference{})",
                        c.statement,
                        c.confidence * 100.0,
                        evidence_count,
                        if evidence_count == 1 { "" } else { "s" }
                    )
                })
                .collect();

            sections.push(format!(
                "\n## Claims\n\n{}",
                claims_text.join("\n")
            ));
        }

        // Detailed evidence (if enabled)
        if self.include_details && !pack.neighbors.is_empty() {
            let evidence_text: Vec<String> = pack
                .neighbors
                .iter()
                .take(self.max_evidence_items)
                .map(|n| {
                    self.templates.neighbor_evidence_description(
                        &n.recording_metadata.recording_id,
                        n.distance,
                        n.recording_metadata.taxon.as_deref(),
                    )
                })
                .collect();

            let more_text = if pack.neighbors.len() > self.max_evidence_items {
                format!(
                    "\n... and {} more neighbors",
                    pack.neighbors.len() - self.max_evidence_items
                )
            } else {
                String::new()
            };

            sections.push(format!(
                "\n## Evidence Details\n\n### Neighbors\n{}\n{}",
                evidence_text.join("\n"),
                more_text
            ));
        }

        // Confidence explanation
        sections.push(format!(
            "\n## Confidence Assessment\n\n{}",
            self.templates.confidence_explanation(pack.interpretation.confidence)
        ));

        sections.join("\n")
    }

    /// Format a compact single-line summary.
    pub fn format_compact(&self, pack: &crate::domain::entities::EvidencePack) -> String {
        let taxon = pack
            .neighbors
            .first()
            .and_then(|n| n.recording_metadata.taxon.as_deref())
            .unwrap_or("unknown");

        let cluster = pack
            .cluster_context
            .cluster_label
            .as_deref()
            .unwrap_or("unassigned");

        format!(
            "[{}] {} neighbors, cluster='{}', taxon='{}', confidence={:.1}%",
            pack.id,
            pack.neighbors.len(),
            cluster,
            taxon,
            pack.overall_confidence() * 100.0
        )
    }

    /// Format as JSON-compatible structure.
    pub fn format_json(&self, pack: &crate::domain::entities::EvidencePack) -> serde_json::Value {
        serde_json::json!({
            "id": pack.id,
            "query_embedding_id": pack.query_embedding_id.0,
            "created_at": pack.created_at.to_rfc3339(),
            "summary": {
                "neighbor_count": pack.neighbors.len(),
                "overall_confidence": pack.overall_confidence(),
                "cluster_assigned": pack.cluster_context.has_cluster(),
                "has_sequence_context": pack.sequence_context.is_some(),
            },
            "interpretation": {
                "structural_description": pack.interpretation.structural_description,
                "claim_count": pack.interpretation.claims.len(),
                "confidence": pack.interpretation.confidence,
            },
            "claims": pack.interpretation.claims.iter().map(|c| {
                serde_json::json!({
                    "statement": c.statement,
                    "confidence": c.confidence,
                    "evidence_count": c.evidence_refs.len(),
                })
            }).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neighbor_description() {
        let templates = InterpretationTemplates::new();

        let desc = templates.neighbor_description(5, 0.85);
        assert!(desc.contains("high similarity"));
        assert!(desc.contains("85.0%"));
        assert!(desc.contains("5 reference"));

        let desc = templates.neighbor_description(3, 0.45);
        assert!(desc.contains("low similarity"));
    }

    #[test]
    fn test_taxon_description() {
        let templates = InterpretationTemplates::new();

        let desc = templates.taxon_description(&["Species A", "Species A", "Species A"]);
        assert!(desc.contains("Species A"));
        assert!(!desc.contains("multiple taxa"));

        let desc = templates.taxon_description(&["Species A", "Species B"]);
        assert!(desc.contains("multiple taxa"));
        assert!(desc.contains("Species A"));
        assert!(desc.contains("Species B"));
    }

    #[test]
    fn test_cluster_description() {
        let templates = InterpretationTemplates::new();

        let desc = templates.cluster_description("Song Type A", 0.9, 0.85);
        assert!(desc.contains("Song Type A"));
        assert!(desc.contains("very high confidence"));
        assert!(desc.contains("90.0%"));
        assert!(desc.contains("85.0%"));
    }

    #[test]
    fn test_sequence_description() {
        let templates = InterpretationTemplates::new();

        let desc = templates.sequence_description(5, None);
        assert!(desc.contains("5 vocalization sequence"));
        assert!(!desc.contains("motif"));

        let desc = templates.sequence_description(5, Some("ABAB"));
        assert!(desc.contains("motif pattern 'ABAB'"));
    }

    #[test]
    fn test_generate_summary() {
        let templates = InterpretationTemplates::new();

        let summary = templates.generate_summary(
            10,
            0.85,
            Some("Dawn Chorus"),
            Some("Turdus merula"),
            0.9,
        );

        assert!(summary.contains("High confidence"));
        assert!(summary.contains("highly similar"));
        assert!(summary.contains("Dawn Chorus"));
        assert!(summary.contains("Turdus merula"));
        assert!(summary.contains("90.0%"));
    }

    #[test]
    fn test_confidence_explanation() {
        let templates = InterpretationTemplates::new();

        let high = templates.confidence_explanation(0.95);
        assert!(high.contains("Very high confidence"));

        let moderate = templates.confidence_explanation(0.55);
        assert!(moderate.contains("Moderate confidence"));

        let low = templates.confidence_explanation(0.2);
        assert!(low.contains("Very low confidence"));
    }

    #[test]
    fn test_custom_template_override() {
        let templates = InterpretationTemplates::new()
            .with_template("low_similarity", "Custom low similarity message");

        let desc = templates.low_similarity_claim();
        assert_eq!(desc, "Custom low similarity message");
    }

    #[test]
    fn test_evidence_pack_formatter() {
        use crate::domain::entities::*;

        let pack = EvidencePack::new(
            EmbeddingId::new("query-1"),
            vec![
                NeighborEvidence::new(
                    EmbeddingId::new("n1"),
                    0.1,
                    RecordingMetadata::new("r1").with_taxon("Species A"),
                ),
            ],
            ClusterContext::new(
                Some(ClusterId::new("c1")),
                0.9,
                0.85,
            ).with_label("Song Type A"),
            None,
            Interpretation::new(
                "Test structural description".to_string(),
                vec![Claim::new("Test claim", 0.9)],
                0.85,
            ),
        );

        let formatter = EvidencePackFormatter::new();

        // Test full report
        let report = formatter.format_report(&pack);
        assert!(report.contains("Evidence Pack Report"));
        assert!(report.contains("query-1"));
        assert!(report.contains("Test structural description"));

        // Test compact format
        let compact = formatter.format_compact(&pack);
        assert!(compact.contains("1 neighbors"));
        assert!(compact.contains("Song Type A"));

        // Test JSON format
        let json = formatter.format_json(&pack);
        assert_eq!(json["query_embedding_id"], "query-1");
        assert_eq!(json["summary"]["neighbor_count"], 1);
    }
}
