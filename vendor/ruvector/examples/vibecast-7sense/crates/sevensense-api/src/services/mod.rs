//! Service layer abstractions for 7sense API.
//!
//! This module defines the interfaces and implementations for core services:
//! - `AudioPipeline` - Audio loading and segmentation
//! - `EmbeddingModel` - Segment embedding generation
//! - `VectorIndex` - Similarity search
//! - `ClusterEngine` - Cluster analysis
//! - `InterpretationEngine` - Evidence pack generation
//!
//! These services wrap the underlying crate implementations and provide
//! API-specific functionality.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use utoipa::ToSchema;

// Re-export service types
pub use audio::*;
pub use cluster::*;
pub use embedding::*;
pub use interpretation::*;
pub use vector::*;

mod audio;
mod cluster;
mod embedding;
mod interpretation;
mod vector;

/// Species information attached to a segment.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SpeciesInfo {
    /// Common name
    pub common_name: String,
    /// Scientific name (binomial nomenclature)
    pub scientific_name: Option<String>,
    /// Detection confidence (0.0 to 1.0)
    pub confidence: f32,
}

/// A detected audio segment.
#[derive(Debug, Clone)]
pub struct Segment {
    /// Unique identifier
    pub id: Uuid,
    /// Parent recording ID
    pub recording_id: Uuid,
    /// Start time in seconds
    pub start_time: f64,
    /// End time in seconds
    pub end_time: f64,
    /// Audio samples (mono, normalized)
    pub samples: Vec<f32>,
    /// Sample rate
    pub sample_rate: u32,
    /// Detected species
    pub species: Option<SpeciesInfo>,
    /// Quality score
    pub quality_score: f32,
}

/// Audio metadata.
#[derive(Debug, Clone)]
pub struct AudioMetadata {
    /// Duration in seconds
    pub duration_secs: f64,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u16,
}

/// Loaded audio data.
#[derive(Debug, Clone)]
pub struct Audio {
    /// Mono samples (normalized to -1.0 to 1.0)
    pub samples: Vec<f32>,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Original duration in seconds
    pub duration_secs: f64,
}

/// Segment embedding for vector storage.
#[derive(Debug, Clone)]
pub struct SegmentEmbedding {
    /// Segment ID
    pub id: Uuid,
    /// Recording ID
    pub recording_id: Uuid,
    /// Embedding vector
    pub embedding: Vec<f32>,
    /// Start time
    pub start_time: f64,
    /// End time
    pub end_time: f64,
    /// Detected species
    pub species: Option<SpeciesInfo>,
}

/// Search result from vector index.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Segment ID
    pub id: Uuid,
    /// Recording ID
    pub recording_id: Uuid,
    /// Distance to query
    pub distance: f32,
    /// Start time
    pub start_time: f64,
    /// End time
    pub end_time: f64,
    /// Detected species
    pub species: Option<SpeciesInfo>,
}

/// Cluster data.
#[derive(Debug, Clone)]
pub struct ClusterData {
    /// Cluster ID
    pub id: Uuid,
    /// Human-assigned label
    pub label: Option<String>,
    /// Number of segments
    pub size: usize,
    /// Centroid embedding
    pub centroid: Vec<f32>,
    /// Cluster density
    pub density: f32,
    /// Representative segment IDs
    pub exemplar_ids: Vec<Uuid>,
    /// Species distribution: (name, count, percentage)
    pub species_distribution: Vec<(String, usize, f64)>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Evidence pack data.
#[derive(Debug, Clone)]
pub struct EvidencePackData {
    /// Query ID
    pub query_id: Uuid,
    /// Query segment
    pub query_segment: EvidenceSegment,
    /// Neighbor evidence
    pub neighbors: Vec<NeighborEvidenceData>,
    /// Shared features
    pub shared_features: Vec<SharedFeature>,
    /// Visualizations
    pub visualizations: VisualizationUrls,
    /// Generation timestamp
    pub generated_at: DateTime<Utc>,
}

/// Segment for evidence pack.
#[derive(Debug, Clone)]
pub struct EvidenceSegment {
    /// Segment ID
    pub id: Uuid,
    /// Recording ID
    pub recording_id: Uuid,
    /// Start time
    pub start_time: f64,
    /// End time
    pub end_time: f64,
    /// Species info
    pub species: Option<SpeciesInfo>,
}

/// Neighbor evidence data.
#[derive(Debug, Clone)]
pub struct NeighborEvidenceData {
    /// Neighbor segment
    pub segment: EvidenceSegment,
    /// Similarity score
    pub similarity: f32,
    /// Contributing features
    pub contributing_features: Vec<FeatureContributionData>,
    /// Spectrogram comparison URL
    pub spectrogram_comparison_url: Option<String>,
}

/// Feature contribution data.
#[derive(Debug, Clone)]
pub struct FeatureContributionData {
    /// Feature name
    pub name: String,
    /// Contribution weight
    pub weight: f32,
    /// Query value
    pub query_value: f64,
    /// Neighbor value
    pub neighbor_value: f64,
}

/// Shared acoustic feature.
#[derive(Debug, Clone)]
pub struct SharedFeature {
    /// Feature name
    pub name: String,
    /// Description
    pub description: String,
    /// Confidence score
    pub confidence: f32,
}

/// Visualization URLs.
#[derive(Debug, Clone)]
pub struct VisualizationUrls {
    /// UMAP projection URL
    pub umap_url: Option<String>,
    /// Spectrogram grid URL
    pub spectrogram_grid_url: Option<String>,
    /// Feature importance URL
    pub feature_importance_url: Option<String>,
}
