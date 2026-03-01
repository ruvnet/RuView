//! Dataset Export - HuggingFace-compatible dataset formats
//!
//! Exports SONA's learned patterns and preference pairs as JSONL datasets
//! compatible with HuggingFace's datasets library.

use super::{ExportConfig, ExportError, ExportResult, ExportType};
use crate::engine::SonaEngine;
use std::io::{BufWriter, Write};
use std::path::Path;

#[cfg(feature = "serde-support")]
use serde::{Deserialize, Serialize};

/// Dataset exporter for patterns and preferences
pub struct DatasetExporter<'a> {
    config: &'a ExportConfig,
}

impl<'a> DatasetExporter<'a> {
    /// Create new dataset exporter
    pub fn new(config: &'a ExportConfig) -> Self {
        Self { config }
    }

    /// Export learned patterns as JSONL dataset
    pub fn export_patterns<P: AsRef<Path>>(
        &self,
        engine: &SonaEngine,
        output_path: P,
    ) -> Result<ExportResult, ExportError> {
        let output_path = output_path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(ExportError::Io)?;
        }

        let file = std::fs::File::create(output_path).map_err(ExportError::Io)?;
        let mut writer = BufWriter::new(file);

        let patterns = engine.get_all_patterns();
        let mut items_exported = 0;

        for pattern in patterns {
            // Filter by quality threshold
            if pattern.avg_quality < self.config.min_quality_threshold {
                continue;
            }

            let record = PatternRecord {
                id: pattern.id.to_string(),
                embedding: pattern.centroid.clone(),
                cluster_size: pattern.cluster_size,
                avg_quality: pattern.avg_quality,
                pattern_type: pattern.pattern_type.to_string(),
                access_count: pattern.access_count as u64,
                metadata: PatternMetadata {
                    source: "sona".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    target_model: self.config.target_architecture.clone(),
                },
            };

            let json = serde_json::to_string(&record).map_err(ExportError::Serialization)?;
            writeln!(writer, "{}", json).map_err(ExportError::Io)?;
            items_exported += 1;
        }

        writer.flush().map_err(ExportError::Io)?;

        let size_bytes = std::fs::metadata(output_path).map(|m| m.len()).unwrap_or(0);

        Ok(ExportResult {
            export_type: ExportType::PatternsDataset,
            items_exported,
            output_path: output_path.to_string_lossy().to_string(),
            size_bytes,
        })
    }

    /// Export preference pairs for DPO/RLHF training
    pub fn export_preferences<P: AsRef<Path>>(
        &self,
        engine: &SonaEngine,
        output_path: P,
    ) -> Result<ExportResult, ExportError> {
        let output_path = output_path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(ExportError::Io)?;
        }

        let file = std::fs::File::create(output_path).map_err(ExportError::Io)?;
        let mut writer = BufWriter::new(file);

        let trajectories = engine.get_quality_trajectories();
        let mut items_exported = 0;

        // Generate preference pairs from trajectories
        // Sort by quality and pair high-quality with low-quality
        let mut sorted_trajectories = trajectories.clone();
        sorted_trajectories.sort_by(|a, b| {
            b.quality
                .partial_cmp(&a.quality)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mid = sorted_trajectories.len() / 2;
        let (high_quality, low_quality) = sorted_trajectories.split_at(mid);

        for (chosen, rejected) in high_quality.iter().zip(low_quality.iter().rev()) {
            // Skip if quality difference is too small
            if (chosen.quality - rejected.quality).abs() < 0.1 {
                continue;
            }

            let pair = PreferencePair {
                prompt: PreferencePrompt {
                    embedding: chosen.query_embedding.clone(),
                    context: chosen.context_ids.clone(),
                },
                chosen: PreferenceResponse {
                    route: chosen.route.clone(),
                    quality: chosen.quality,
                    embedding: chosen.response_embedding.clone(),
                },
                rejected: PreferenceResponse {
                    route: rejected.route.clone(),
                    quality: rejected.quality,
                    embedding: rejected.response_embedding.clone(),
                },
                metadata: PreferenceMetadata {
                    quality_delta: chosen.quality - rejected.quality,
                    source: "sona".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            };

            let json = serde_json::to_string(&pair).map_err(ExportError::Serialization)?;
            writeln!(writer, "{}", json).map_err(ExportError::Io)?;
            items_exported += 1;
        }

        writer.flush().map_err(ExportError::Io)?;

        let size_bytes = std::fs::metadata(output_path).map(|m| m.len()).unwrap_or(0);

        Ok(ExportResult {
            export_type: ExportType::PreferencePairs,
            items_exported,
            output_path: output_path.to_string_lossy().to_string(),
            size_bytes,
        })
    }

    /// Export distillation targets for knowledge distillation
    pub fn export_distillation_targets<P: AsRef<Path>>(
        &self,
        engine: &SonaEngine,
        output_path: P,
    ) -> Result<ExportResult, ExportError> {
        let output_path = output_path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(ExportError::Io)?;
        }

        let file = std::fs::File::create(output_path).map_err(ExportError::Io)?;
        let mut writer = BufWriter::new(file);

        let routing_decisions = engine.get_routing_decisions();
        let mut items_exported = 0;

        for decision in routing_decisions {
            // Filter by quality
            if decision.quality < self.config.min_quality_threshold {
                continue;
            }

            let target = DistillationTarget {
                input_embedding: decision.query_embedding.clone(),
                teacher_logits: decision.routing_logits.clone(),
                selected_route: decision.selected_route.clone(),
                confidence: decision.confidence,
                quality: decision.quality,
                metadata: DistillationMetadata {
                    source: "sona".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    temperature: 1.0,
                },
            };

            let json = serde_json::to_string(&target).map_err(ExportError::Serialization)?;
            writeln!(writer, "{}", json).map_err(ExportError::Io)?;
            items_exported += 1;
        }

        writer.flush().map_err(ExportError::Io)?;

        let size_bytes = std::fs::metadata(output_path).map(|m| m.len()).unwrap_or(0);

        Ok(ExportResult {
            export_type: ExportType::DistillationTargets,
            items_exported,
            output_path: output_path.to_string_lossy().to_string(),
            size_bytes,
        })
    }
}

/// Pattern record for JSONL export
#[cfg_attr(feature = "serde-support", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct PatternRecord {
    /// Pattern ID
    pub id: String,
    /// Embedding vector
    pub embedding: Vec<f32>,
    /// Number of trajectories in cluster
    pub cluster_size: usize,
    /// Average quality score
    pub avg_quality: f32,
    /// Pattern type (routing, reasoning, etc.)
    pub pattern_type: String,
    /// Access count
    pub access_count: u64,
    /// Export metadata
    pub metadata: PatternMetadata,
}

/// Pattern export metadata
#[cfg_attr(feature = "serde-support", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct PatternMetadata {
    /// Source system
    pub source: String,
    /// Version
    pub version: String,
    /// Target model architecture
    pub target_model: String,
}

/// Preference pair for DPO/RLHF
#[cfg_attr(feature = "serde-support", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct PreferencePair {
    /// Input prompt
    pub prompt: PreferencePrompt,
    /// Chosen (preferred) response
    pub chosen: PreferenceResponse,
    /// Rejected response
    pub rejected: PreferenceResponse,
    /// Metadata
    pub metadata: PreferenceMetadata,
}

/// Preference prompt
#[cfg_attr(feature = "serde-support", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct PreferencePrompt {
    /// Query embedding
    pub embedding: Vec<f32>,
    /// Context IDs
    pub context: Vec<String>,
}

/// Preference response
#[cfg_attr(feature = "serde-support", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct PreferenceResponse {
    /// Model route
    pub route: String,
    /// Quality score
    pub quality: f32,
    /// Response embedding
    pub embedding: Vec<f32>,
}

/// Preference pair metadata
#[cfg_attr(feature = "serde-support", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct PreferenceMetadata {
    /// Quality difference between chosen and rejected
    pub quality_delta: f32,
    /// Source system
    pub source: String,
    /// Version
    pub version: String,
}

/// Distillation target for knowledge distillation
#[cfg_attr(feature = "serde-support", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct DistillationTarget {
    /// Input embedding
    pub input_embedding: Vec<f32>,
    /// Teacher model logits
    pub teacher_logits: Vec<f32>,
    /// Selected route
    pub selected_route: String,
    /// Confidence score
    pub confidence: f32,
    /// Quality score
    pub quality: f32,
    /// Metadata
    pub metadata: DistillationMetadata,
}

/// Distillation metadata
#[cfg_attr(feature = "serde-support", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct DistillationMetadata {
    /// Source system
    pub source: String,
    /// Version
    pub version: String,
    /// Temperature for softmax
    pub temperature: f32,
}

/// Quality trajectory for preference learning
#[derive(Clone, Debug)]
pub struct QualityTrajectory {
    /// Query embedding
    pub query_embedding: Vec<f32>,
    /// Response embedding
    pub response_embedding: Vec<f32>,
    /// Model route
    pub route: String,
    /// Quality score
    pub quality: f32,
    /// Context IDs
    pub context_ids: Vec<String>,
}

/// Routing decision for distillation
#[derive(Clone, Debug)]
pub struct RoutingDecision {
    /// Query embedding
    pub query_embedding: Vec<f32>,
    /// Routing logits
    pub routing_logits: Vec<f32>,
    /// Selected route
    pub selected_route: String,
    /// Confidence
    pub confidence: f32,
    /// Quality
    pub quality: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_record() {
        let record = PatternRecord {
            id: "test-pattern".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            cluster_size: 10,
            avg_quality: 0.85,
            pattern_type: "routing".to_string(),
            access_count: 100,
            metadata: PatternMetadata {
                source: "sona".to_string(),
                version: "0.1.0".to_string(),
                target_model: "phi-4".to_string(),
            },
        };

        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("test-pattern"));
        assert!(json.contains("0.85"));
    }

    #[test]
    fn test_preference_pair() {
        let pair = PreferencePair {
            prompt: PreferencePrompt {
                embedding: vec![0.1, 0.2],
                context: vec!["ctx1".to_string()],
            },
            chosen: PreferenceResponse {
                route: "gpt-4".to_string(),
                quality: 0.9,
                embedding: vec![0.3, 0.4],
            },
            rejected: PreferenceResponse {
                route: "gpt-3.5".to_string(),
                quality: 0.6,
                embedding: vec![0.5, 0.6],
            },
            metadata: PreferenceMetadata {
                quality_delta: 0.3,
                source: "sona".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let json = serde_json::to_string(&pair).unwrap();
        assert!(json.contains("gpt-4"));
        assert!(json.contains("0.9"));
    }
}
