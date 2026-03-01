//! GraphQL type definitions for 7sense API.
//!
//! This module contains all GraphQL object types, input types, and enums
//! used by the schema.

use async_graphql::*;
use chrono::{DateTime, Utc};

// ============================================================================
// Object Types
// ============================================================================

/// A similar segment found through vector search.
#[derive(Debug, Clone, SimpleObject)]
pub struct Neighbor {
    /// Segment identifier
    pub segment_id: ID,
    /// Parent recording ID
    pub recording_id: ID,
    /// Similarity score (0.0 to 1.0)
    pub similarity: f32,
    /// Distance in embedding space
    pub distance: f32,
    /// Segment start time
    pub start_time: f64,
    /// Segment end time
    pub end_time: f64,
    /// Detected species
    pub species: Option<Species>,
}

/// Species information.
#[derive(Debug, Clone, SimpleObject)]
pub struct Species {
    /// Common name
    pub common_name: String,
    /// Scientific name (binomial)
    pub scientific_name: Option<String>,
    /// Detection confidence
    pub confidence: f32,
}

/// A cluster of similar calls.
#[derive(Debug, Clone, SimpleObject)]
pub struct Cluster {
    /// Cluster identifier
    pub id: ID,
    /// Human-assigned label
    pub label: Option<String>,
    /// Number of segments in cluster
    pub size: i32,
    /// Cluster density/compactness
    pub density: f32,
    /// Representative segment IDs
    pub exemplar_ids: Vec<ID>,
    /// Species distribution
    pub species_distribution: Vec<SpeciesCount>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Species count within a cluster.
#[derive(Debug, Clone, SimpleObject)]
pub struct SpeciesCount {
    /// Species common name
    pub name: String,
    /// Scientific name
    pub scientific_name: Option<String>,
    /// Count of segments
    pub count: i32,
    /// Percentage of cluster
    pub percentage: f64,
}

/// Processing status update.
#[derive(Debug, Clone, SimpleObject)]
pub struct ProcessingUpdate {
    /// Recording ID
    pub recording_id: ID,
    /// Current status
    pub status: ProcessingStatusGql,
    /// Progress (0.0 to 1.0)
    pub progress: f32,
    /// Status message
    pub message: Option<String>,
}

/// Health status response.
#[derive(Debug, Clone, SimpleObject)]
pub struct HealthStatus {
    /// Service status
    pub status: String,
    /// Version
    pub version: String,
}

// ============================================================================
// Enums
// ============================================================================

/// Processing status stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum ProcessingStatusGql {
    /// Queued for processing
    Queued,
    /// Loading audio
    Loading,
    /// Segmenting
    Segmenting,
    /// Generating embeddings
    Embedding,
    /// Indexing vectors
    Indexing,
    /// Analyzing clusters
    Analyzing,
    /// Complete
    Complete,
    /// Failed
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processing_status_enum() {
        let status = ProcessingStatusGql::Embedding;
        assert_eq!(status, ProcessingStatusGql::Embedding);
    }
}
