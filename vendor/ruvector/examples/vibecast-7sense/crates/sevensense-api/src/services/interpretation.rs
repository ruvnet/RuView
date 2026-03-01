//! Interpretation service.
//!
//! This module provides the `InterpretationEngine` service for generating
//! evidence packs that explain similarity relationships.

use std::collections::HashMap;
use std::sync::RwLock;

use chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

use super::{
    EvidencePackData, EvidenceSegment, FeatureContributionData, NeighborEvidenceData,
    SearchResult, SharedFeature, VisualizationUrls,
};

/// Interpretation error.
#[derive(Debug, Error)]
pub enum InterpretationError {
    /// Evidence pack generation failed
    #[error("Evidence generation failed: {0}")]
    GenerationError(String),

    /// Not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Interpretation engine configuration.
#[derive(Debug, Clone)]
pub struct InterpretationEngineConfig {
    /// Number of top features to include
    pub top_features: usize,
    /// Generate spectrograms
    pub generate_spectrograms: bool,
    /// Generate UMAP visualizations
    pub generate_umap: bool,
}

impl Default for InterpretationEngineConfig {
    fn default() -> Self {
        Self {
            top_features: 5,
            generate_spectrograms: true,
            generate_umap: true,
        }
    }
}

/// Interpretation engine for generating evidence packs.
///
/// Creates interpretable explanations for similarity relationships.
pub struct InterpretationEngine {
    config: InterpretationEngineConfig,
    // Cache for generated evidence packs
    cache: RwLock<HashMap<Uuid, EvidencePackData>>,
}

impl InterpretationEngine {
    /// Create a new interpretation engine with the given configuration.
    pub fn new(config: InterpretationEngineConfig) -> Result<Self, InterpretationError> {
        Ok(Self {
            config,
            cache: RwLock::new(HashMap::new()),
        })
    }

    /// Get a cached evidence pack by query ID.
    pub fn get_evidence_pack(
        &self,
        query_id: &Uuid,
    ) -> Result<Option<EvidencePackData>, InterpretationError> {
        let cache = self
            .cache
            .read()
            .map_err(|e| InterpretationError::Internal(e.to_string()))?;

        Ok(cache.get(query_id).cloned())
    }

    /// Generate an evidence pack for a query segment and its neighbors.
    pub async fn generate_evidence_pack(
        &self,
        segment_id: &Uuid,
        neighbors: &[SearchResult],
    ) -> Result<EvidencePackData, InterpretationError> {
        let query_id = Uuid::new_v4();

        // Create query segment info
        let query_segment = EvidenceSegment {
            id: *segment_id,
            recording_id: Uuid::new_v4(), // Would be looked up
            start_time: 0.0,
            end_time: 1.0,
            species: None,
        };

        // Generate evidence for each neighbor
        let neighbor_evidence: Vec<NeighborEvidenceData> = neighbors
            .iter()
            .map(|n| {
                // In a real implementation, this would:
                // 1. Analyze embedding dimensions
                // 2. Identify contributing features
                // 3. Generate spectrogram comparisons

                let contributing_features = vec![
                    FeatureContributionData {
                        name: "fundamental_frequency".to_string(),
                        weight: 0.25,
                        query_value: 2500.0,
                        neighbor_value: 2480.0,
                    },
                    FeatureContributionData {
                        name: "duration".to_string(),
                        weight: 0.15,
                        query_value: 0.5,
                        neighbor_value: 0.48,
                    },
                    FeatureContributionData {
                        name: "bandwidth".to_string(),
                        weight: 0.12,
                        query_value: 1500.0,
                        neighbor_value: 1520.0,
                    },
                ];

                NeighborEvidenceData {
                    segment: EvidenceSegment {
                        id: n.id,
                        recording_id: n.recording_id,
                        start_time: n.start_time,
                        end_time: n.end_time,
                        species: n.species.clone(),
                    },
                    similarity: 1.0 - n.distance,
                    contributing_features,
                    spectrogram_comparison_url: if self.config.generate_spectrograms {
                        Some(format!("/api/v1/evidence/{}/spectrograms/{}", query_id, n.id))
                    } else {
                        None
                    },
                }
            })
            .collect();

        // Identify shared features across neighbors
        let shared_features = vec![
            SharedFeature {
                name: "frequency_modulation".to_string(),
                description: "Rapid upward frequency sweep in 100-200ms range".to_string(),
                confidence: 0.92,
            },
            SharedFeature {
                name: "harmonic_structure".to_string(),
                description: "Clear harmonic overtones at 2x and 3x fundamental".to_string(),
                confidence: 0.87,
            },
        ];

        // Generate visualization URLs
        let visualizations = VisualizationUrls {
            umap_url: if self.config.generate_umap {
                Some(format!("/api/v1/evidence/{}/umap", query_id))
            } else {
                None
            },
            spectrogram_grid_url: if self.config.generate_spectrograms {
                Some(format!("/api/v1/evidence/{}/grid", query_id))
            } else {
                None
            },
            feature_importance_url: Some(format!("/api/v1/evidence/{}/features", query_id)),
        };

        let evidence_pack = EvidencePackData {
            query_id,
            query_segment,
            neighbors: neighbor_evidence,
            shared_features,
            visualizations,
            generated_at: Utc::now(),
        };

        // Cache the evidence pack
        {
            let mut cache = self
                .cache
                .write()
                .map_err(|e| InterpretationError::Internal(e.to_string()))?;

            cache.insert(query_id, evidence_pack.clone());
        }

        Ok(evidence_pack)
    }

    /// Clear the evidence pack cache.
    pub fn clear_cache(&self) -> Result<(), InterpretationError> {
        let mut cache = self
            .cache
            .write()
            .map_err(|e| InterpretationError::Internal(e.to_string()))?;

        cache.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpretation_engine_creation() {
        let engine = InterpretationEngine::new(Default::default());
        assert!(engine.is_ok());
    }

    #[tokio::test]
    async fn test_generate_evidence_pack() {
        let engine = InterpretationEngine::new(Default::default()).unwrap();

        let segment_id = Uuid::new_v4();
        let neighbors = vec![SearchResult {
            id: Uuid::new_v4(),
            recording_id: Uuid::new_v4(),
            distance: 0.1,
            start_time: 0.0,
            end_time: 1.0,
            species: None,
        }];

        let result = engine.generate_evidence_pack(&segment_id, &neighbors).await;
        assert!(result.is_ok());

        let pack = result.unwrap();
        assert!(!pack.neighbors.is_empty());
        assert!(!pack.shared_features.is_empty());
    }
}
