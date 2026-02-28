//! Integration tests for Interpretation Context
//!
//! Tests for evidence pack building, claim generation with citations,
//! and validation that all claims have evidence references.

use vibecast_tests::fixtures::*;
use vibecast_tests::mocks::*;
use std::collections::HashSet;

// ============================================================================
// Evidence Pack Building Tests
// ============================================================================

mod evidence_pack_building {
    use super::*;

    #[test]
    fn test_create_evidence_pack() {
        let pack = create_test_evidence_pack();

        assert!(!pack.neighbors.is_empty());
        assert!(!pack.exemplars.is_empty());
        assert!(pack.signal_quality.snr > 0.0);
    }

    #[test]
    fn test_evidence_pack_with_neighbors() {
        let pack = create_test_evidence_pack_with_neighbors(10);

        assert_eq!(pack.neighbors.len(), 10);

        // Verify neighbor properties
        for neighbor in &pack.neighbors {
            assert!(neighbor.distance >= 0.0);
            assert!(neighbor.relevance > 0.0);
        }
    }

    #[test]
    fn test_evidence_pack_builder() {
        let builder = MockEvidencePackBuilder::new()
            .with_neighbor_count(15)
            .with_exemplar_count(3);

        let segment = create_test_segment();
        let search_results = create_search_results(20);
        let clusters = create_test_clusters(5);

        let pack = builder.build(&segment, &search_results, &clusters).unwrap();

        assert_eq!(pack.neighbors.len(), 15);
        assert!(pack.exemplars.len() <= 3);
    }

    #[test]
    fn test_evidence_pack_signal_quality() {
        let builder = MockEvidencePackBuilder::new();

        let segment = create_test_segment_with_snr(25.0);
        let search_results = create_search_results(10);
        let clusters = create_test_clusters(2);

        let pack = builder.build(&segment, &search_results, &clusters).unwrap();

        assert_eq!(pack.signal_quality.snr, 25.0);
        assert!(matches!(
            pack.signal_quality.quality_grade,
            Some(QualityGrade::Excellent)
        ));
    }

    #[test]
    fn test_evidence_pack_includes_cluster_ids() {
        let pack = create_test_evidence_pack_with_neighbors(10);

        // Some neighbors should have cluster IDs
        let has_cluster = pack.neighbors.iter().any(|n| n.cluster_id.is_some());
        assert!(
            has_cluster,
            "At least some neighbors should have cluster assignments"
        );
    }

    #[test]
    fn test_evidence_pack_relevance_scoring() {
        let pack = create_test_evidence_pack_with_neighbors(10);

        for (i, neighbor) in pack.neighbors.iter().enumerate() {
            // Relevance should be inverse of distance
            let expected_relevance = 1.0 / (1.0 + neighbor.distance);
            assert!(
                (neighbor.relevance - expected_relevance).abs() < 0.001,
                "Neighbor {} has wrong relevance: {} vs expected {}",
                i,
                neighbor.relevance,
                expected_relevance
            );
        }
    }

    #[test]
    fn test_evidence_pack_from_empty_search() {
        let builder = MockEvidencePackBuilder::new();

        let segment = create_test_segment();
        let empty_results: Vec<SearchResult> = vec![];
        let clusters = create_test_clusters(2);

        let pack = builder.build(&segment, &empty_results, &clusters).unwrap();

        assert_eq!(pack.neighbors.len(), 0);
    }

    #[test]
    fn test_evidence_pack_timestamp() {
        let pack = create_test_evidence_pack();

        let now = chrono::Utc::now();
        let age = now - pack.created_at;

        // Pack should have been created recently
        assert!(
            age.num_seconds() < 60,
            "Evidence pack should be recently created"
        );
    }
}

// ============================================================================
// Claim Generation Tests
// ============================================================================

mod claim_generation {
    use super::*;

    #[test]
    fn test_generate_interpretation_from_evidence() {
        let generator = MockInterpretationGenerator::new();
        let evidence_pack = create_test_evidence_pack();

        let interpretation = generator.generate(&evidence_pack).unwrap();

        assert!(!interpretation.statements.is_empty());
        assert!(interpretation.confidence > 0.0);
    }

    #[test]
    fn test_interpretation_includes_citations() {
        let generator = MockInterpretationGenerator::new();
        let evidence_pack = create_test_evidence_pack();

        let interpretation = generator.generate(&evidence_pack).unwrap();

        assert!(
            !interpretation.citations.is_empty(),
            "Interpretation should have citations"
        );
    }

    #[test]
    fn test_all_claims_have_citations() {
        let generator = MockInterpretationGenerator::new();
        let evidence_pack = create_test_evidence_pack();

        let interpretation = generator.generate(&evidence_pack).unwrap();

        // Verify all statements have at least one citation
        let valid = generator.validate_citations(&interpretation);
        assert!(
            valid,
            "All claims should have corresponding citations"
        );
    }

    #[test]
    fn test_citation_evidence_types() {
        let generator = MockInterpretationGenerator::new();
        let evidence_pack = create_test_evidence_pack();

        let interpretation = generator.generate(&evidence_pack).unwrap();

        let evidence_types: HashSet<_> =
            interpretation.citations.iter().map(|c| &c.evidence_type).collect();

        // Should have neighbor citations at minimum
        assert!(
            evidence_types.contains(&EvidenceType::Neighbor),
            "Should cite neighbors as evidence"
        );
    }

    #[test]
    fn test_citation_strength_values() {
        let citations = create_test_citations(10);

        for citation in &citations {
            assert!(
                citation.strength >= 0.0 && citation.strength <= 1.0,
                "Citation strength should be in [0, 1]"
            );
        }
    }

    #[test]
    fn test_interpretation_confidence_from_citations() {
        let generator = MockInterpretationGenerator::new();

        // High-quality evidence
        let good_pack = create_test_evidence_pack_with_neighbors(20);
        let good_interpretation = generator.generate(&good_pack).unwrap();

        // Low-quality evidence (fewer neighbors)
        let poor_pack = create_test_evidence_pack_with_neighbors(2);
        let poor_interpretation = generator.generate(&poor_pack).unwrap();

        // Both should have non-zero confidence
        assert!(good_interpretation.confidence > 0.0);
        assert!(poor_interpretation.confidence > 0.0);
    }

    #[test]
    fn test_factory_interpretation() {
        let evidence_pack_id = EvidencePackId::new();
        let interpretation = create_test_interpretation(evidence_pack_id);

        assert_eq!(interpretation.evidence_pack_id, evidence_pack_id);
        assert!(!interpretation.statements.is_empty());
        assert!(!interpretation.citations.is_empty());
    }
}

// ============================================================================
// Citation Validation Tests
// ============================================================================

mod citation_validation {
    use super::*;

    #[test]
    fn test_citation_links_to_valid_evidence() {
        let evidence_pack = create_test_evidence_pack();
        let generator = MockInterpretationGenerator::new();

        let interpretation = generator.generate(&evidence_pack).unwrap();

        // Each citation should reference valid evidence
        for citation in &interpretation.citations {
            match &citation.evidence_type {
                EvidenceType::Neighbor => {
                    // Citation should reference a segment
                    assert!(!citation.evidence_id.is_empty());
                }
                EvidenceType::Exemplar => {
                    assert!(!citation.evidence_id.is_empty());
                }
                EvidenceType::Cluster => {
                    assert!(!citation.evidence_id.is_empty());
                }
                EvidenceType::Motif => {
                    assert!(!citation.evidence_id.is_empty());
                }
            }
        }
    }

    #[test]
    fn test_no_orphan_citations() {
        let generator = MockInterpretationGenerator::new();
        let evidence_pack = create_test_evidence_pack();

        let interpretation = generator.generate(&evidence_pack).unwrap();

        // All citations should reference an existing claim
        for citation in &interpretation.citations {
            let claim_exists = interpretation.statements.contains(&citation.claim);
            assert!(
                claim_exists,
                "Citation references non-existent claim: {}",
                citation.claim
            );
        }
    }

    #[test]
    fn test_citation_uuid_format() {
        let citations = create_test_citations(5);

        for citation in &citations {
            // Evidence ID should be valid UUID string
            let parse_result = uuid::Uuid::parse_str(&citation.evidence_id);
            assert!(
                parse_result.is_ok(),
                "Evidence ID should be valid UUID: {}",
                citation.evidence_id
            );
        }
    }

    #[test]
    fn test_citation_claim_matching() {
        let generator = MockInterpretationGenerator::new();
        let evidence_pack = create_test_evidence_pack();

        let interpretation = generator.generate(&evidence_pack).unwrap();

        // Group citations by claim
        let mut claims_with_citations: HashSet<String> = HashSet::new();
        for citation in &interpretation.citations {
            claims_with_citations.insert(citation.claim.clone());
        }

        // Every statement should have at least one citation
        for statement in &interpretation.statements {
            assert!(
                claims_with_citations.contains(statement),
                "Statement has no citation: {}",
                statement
            );
        }
    }
}

// ============================================================================
// RAB (Retrieval-Augmented Bioacoustics) Pattern Tests
// ============================================================================

mod rab_pattern {
    use super::*;

    #[test]
    fn test_rab_retrieval_depth() {
        // RAB should retrieve sufficient evidence
        let builder = MockEvidencePackBuilder::new().with_neighbor_count(10);

        let segment = create_test_segment();
        let search_results = create_search_results(50);
        let clusters = create_test_clusters(5);

        let pack = builder.build(&segment, &search_results, &clusters).unwrap();

        assert!(
            pack.neighbors.len() >= 10,
            "RAB should retrieve requested depth"
        );
    }

    #[test]
    fn test_rab_evidence_diversity() {
        let builder = MockEvidencePackBuilder::new()
            .with_neighbor_count(10)
            .with_exemplar_count(5);

        let segment = create_test_segment();
        let search_results = create_search_results(20);
        let clusters = create_test_clusters(5);

        let pack = builder.build(&segment, &search_results, &clusters).unwrap();

        // Should include both neighbors and exemplars
        assert!(!pack.neighbors.is_empty(), "Should have neighbors");
        assert!(!pack.exemplars.is_empty(), "Should have exemplars");
    }

    #[test]
    fn test_rab_constrained_interpretation() {
        let generator = MockInterpretationGenerator::new();
        let evidence_pack = create_test_evidence_pack();

        let interpretation = generator.generate(&evidence_pack).unwrap();

        // Statements should be descriptive (constrained to evidence)
        for statement in &interpretation.statements {
            // Check for structural descriptors (objective language)
            let is_structural = statement.contains("similar")
                || statement.contains("distance")
                || statement.contains("cluster")
                || statement.contains("neighbor")
                || statement.contains("aligns");

            assert!(
                is_structural,
                "Statement should use structural descriptors: {}",
                statement
            );
        }
    }

    #[test]
    fn test_rab_transparency() {
        let generator = MockInterpretationGenerator::new();
        let evidence_pack = create_test_evidence_pack();

        let interpretation = generator.generate(&evidence_pack).unwrap();

        // Every interpretation should be traceable to evidence
        let citation_count = interpretation.citations.len();
        let statement_count = interpretation.statements.len();

        // Average citations per statement
        let avg_citations = citation_count as f32 / statement_count.max(1) as f32;

        assert!(
            avg_citations >= 1.0,
            "Each statement should have at least one citation on average"
        );
    }

    #[test]
    fn test_rab_confidence_reflects_evidence_quality() {
        let generator = MockInterpretationGenerator::new();

        // Rich evidence
        let rich_pack = EvidencePack {
            neighbors: create_test_neighbors(20),
            exemplars: (0..5).map(|_| EmbeddingId::new()).collect(),
            signal_quality: SignalQuality {
                snr: 25.0,
                quality_grade: Some(QualityGrade::Excellent),
                ..Default::default()
            },
            ..Default::default()
        };

        // Sparse evidence
        let sparse_pack = EvidencePack {
            neighbors: create_test_neighbors(2),
            exemplars: vec![],
            signal_quality: SignalQuality {
                snr: 5.0,
                quality_grade: Some(QualityGrade::Fair),
                ..Default::default()
            },
            ..Default::default()
        };

        let rich_interp = generator.generate(&rich_pack).unwrap();
        let sparse_interp = generator.generate(&sparse_pack).unwrap();

        // Rich evidence should yield higher confidence
        assert!(
            rich_interp.citations.len() >= sparse_interp.citations.len(),
            "Rich evidence should produce more citations"
        );
    }
}

// ============================================================================
// Structural Descriptor Tests
// ============================================================================

mod structural_descriptors {
    #[test]
    fn test_pitch_contour_description() {
        // Mock pitch contour stats
        struct PitchContour {
            min_freq: f32,
            max_freq: f32,
            mean_freq: f32,
            contour_type: String,
        }

        let ascending = PitchContour {
            min_freq: 2000.0,
            max_freq: 4000.0,
            mean_freq: 3000.0,
            contour_type: "ascending".to_string(),
        };

        assert!(ascending.max_freq > ascending.min_freq);
        assert!(ascending.mean_freq >= ascending.min_freq);
        assert!(ascending.mean_freq <= ascending.max_freq);
    }

    #[test]
    fn test_spectral_texture_metrics() {
        struct SpectralTexture {
            harmonicity: f32,
            spectral_centroid: f32,
            spectral_flatness: f32,
        }

        let texture = SpectralTexture {
            harmonicity: 0.8,
            spectral_centroid: 3500.0,
            spectral_flatness: 0.2,
        };

        // Harmonicity and flatness should be in [0, 1]
        assert!(texture.harmonicity >= 0.0 && texture.harmonicity <= 1.0);
        assert!(texture.spectral_flatness >= 0.0 && texture.spectral_flatness <= 1.0);
        // Centroid should be in audible range
        assert!(texture.spectral_centroid >= 20.0 && texture.spectral_centroid <= 20000.0);
    }

    #[test]
    fn test_rhythm_profile() {
        struct RhythmProfile {
            duration_ms: u64,
            syllable_count: u32,
            inter_syllable_intervals: Vec<u64>,
            regularity: f32,
        }

        let profile = RhythmProfile {
            duration_ms: 2500,
            syllable_count: 4,
            inter_syllable_intervals: vec![200, 210, 205],
            regularity: 0.95,
        };

        assert_eq!(
            profile.inter_syllable_intervals.len(),
            profile.syllable_count as usize - 1
        );
        assert!(profile.regularity >= 0.0 && profile.regularity <= 1.0);
    }
}

// ============================================================================
// Hypothesis Generation Tests
// ============================================================================

mod hypothesis_generation {
    use super::*;

    #[test]
    fn test_hypothesis_testability() {
        #[derive(Debug)]
        enum Testability {
            High,
            Medium,
            Low,
        }

        struct Hypothesis {
            statement: String,
            testability: Testability,
            supporting_evidence: Vec<String>,
        }

        let hypothesis = Hypothesis {
            statement: "Similar calls may indicate territorial behavior".to_string(),
            testability: Testability::Medium,
            supporting_evidence: vec![
                "neighbor_1".to_string(),
                "cluster_1".to_string(),
            ],
        };

        assert!(!hypothesis.statement.is_empty());
        assert!(!hypothesis.supporting_evidence.is_empty());
    }

    #[test]
    fn test_hypothesis_grounded_in_evidence() {
        let evidence_pack = create_test_evidence_pack();

        // A valid hypothesis should reference observable patterns
        let hypothesis = format!(
            "Based on {} similar neighbors with average distance {:.3}, this call type may be common in this habitat.",
            evidence_pack.neighbors.len(),
            evidence_pack.neighbors.iter().map(|n| n.distance).sum::<f32>() / evidence_pack.neighbors.len() as f32
        );

        assert!(hypothesis.contains("neighbor"));
    }
}

// ============================================================================
// Monitoring Summary Tests
// ============================================================================

mod monitoring_summary {
    struct DiversityMetrics {
        species_richness: u32,
        shannon_index: f32,
        simpson_index: f32,
        evenness: f32,
    }

    fn compute_shannon_index(counts: &[u32]) -> f32 {
        let total: u32 = counts.iter().sum();
        if total == 0 {
            return 0.0;
        }

        let total_f = total as f32;
        counts
            .iter()
            .filter(|&&c| c > 0)
            .map(|&c| {
                let p = c as f32 / total_f;
                -p * p.ln()
            })
            .sum::<f32>()
    }

    #[test]
    fn test_shannon_index_uniform() {
        // Uniform distribution should maximize entropy
        let counts = vec![10, 10, 10, 10];
        let h = compute_shannon_index(&counts);

        let max_h = (counts.len() as f32).ln();
        assert!(
            (h - max_h).abs() < 0.001,
            "Uniform distribution should have max entropy"
        );
    }

    #[test]
    fn test_shannon_index_single_species() {
        // Single species should have zero entropy
        let counts = vec![100, 0, 0, 0];
        let h = compute_shannon_index(&counts);

        assert!(h < 0.001, "Single species should have near-zero entropy");
    }

    #[test]
    fn test_diversity_metrics_valid_ranges() {
        let metrics = DiversityMetrics {
            species_richness: 15,
            shannon_index: 2.5,
            simpson_index: 0.85,
            evenness: 0.9,
        };

        assert!(metrics.shannon_index >= 0.0);
        assert!(metrics.simpson_index >= 0.0 && metrics.simpson_index <= 1.0);
        assert!(metrics.evenness >= 0.0 && metrics.evenness <= 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpretation_integration_smoke_test() {
        // Build evidence pack
        let segment = create_test_segment();
        let search_results = create_search_results(20);
        let clusters = create_test_clusters(5);

        let builder = MockEvidencePackBuilder::new()
            .with_neighbor_count(10)
            .with_exemplar_count(5);

        let evidence_pack = builder.build(&segment, &search_results, &clusters).unwrap();

        // Generate interpretation
        let generator = MockInterpretationGenerator::new();
        let interpretation = generator.generate(&evidence_pack).unwrap();

        // Verify structure
        assert!(!interpretation.statements.is_empty());
        assert!(!interpretation.citations.is_empty());
        assert!(interpretation.confidence > 0.0);

        // Verify all claims have citations
        assert!(generator.validate_citations(&interpretation));
    }
}
