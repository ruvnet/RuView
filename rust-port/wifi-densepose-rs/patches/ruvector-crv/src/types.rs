//! Core types for the CRV (Coordinate Remote Viewing) protocol.
//!
//! Defines the data structures for the 6-stage CRV signal line methodology,
//! session management, and analytical overlay (AOL) detection.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a CRV session.
pub type SessionId = String;

/// Unique identifier for a target coordinate.
pub type TargetCoordinate = String;

/// Unique identifier for a stage data entry.
pub type EntryId = String;

/// Classification of gestalt primitives in Stage I.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GestaltType {
    /// Human-made structures, artifacts
    Manmade,
    /// Organic, natural formations
    Natural,
    /// Dynamic, kinetic signals
    Movement,
    /// Thermal, electromagnetic, force
    Energy,
    /// Aqueous, fluid, wet
    Water,
    /// Solid, terrain, geological
    Land,
}

impl GestaltType {
    /// Returns all gestalt types for iteration.
    pub fn all() -> &'static [GestaltType] {
        &[
            GestaltType::Manmade,
            GestaltType::Natural,
            GestaltType::Movement,
            GestaltType::Energy,
            GestaltType::Water,
            GestaltType::Land,
        ]
    }

    /// Returns the index of this gestalt type in the canonical ordering.
    pub fn index(&self) -> usize {
        match self {
            GestaltType::Manmade => 0,
            GestaltType::Natural => 1,
            GestaltType::Movement => 2,
            GestaltType::Energy => 3,
            GestaltType::Water => 4,
            GestaltType::Land => 5,
        }
    }
}

/// Stage I data: Ideogram traces and gestalt classifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageIData {
    /// Raw ideogram stroke trace as a sequence of (x, y) coordinates.
    pub stroke: Vec<(f32, f32)>,
    /// First spontaneous descriptor word.
    pub spontaneous_descriptor: String,
    /// Classified gestalt type.
    pub classification: GestaltType,
    /// Confidence in the classification (0.0 - 1.0).
    pub confidence: f32,
}

/// Sensory modality for Stage II data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SensoryModality {
    /// Surface textures (smooth, rough, grainy, etc.)
    Texture,
    /// Visual colors and patterns
    Color,
    /// Thermal impressions (hot, cold, warm)
    Temperature,
    /// Auditory impressions
    Sound,
    /// Olfactory impressions
    Smell,
    /// Taste impressions
    Taste,
    /// Size/scale impressions (large, small, vast)
    Dimension,
    /// Luminosity (bright, dark, glowing)
    Luminosity,
}

/// Stage II data: Sensory impressions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageIIData {
    /// Sensory impressions as modality-descriptor pairs.
    pub impressions: Vec<(SensoryModality, String)>,
    /// Raw sensory feature vector (encoded from descriptors).
    pub feature_vector: Option<Vec<f32>>,
}

/// Stage III data: Dimensional and spatial relationships.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageIIIData {
    /// Spatial sketch as a set of named geometric primitives.
    pub sketch_elements: Vec<SketchElement>,
    /// Spatial relationships between elements.
    pub relationships: Vec<SpatialRelationship>,
}

/// A geometric element in a Stage III sketch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SketchElement {
    /// Unique label for this element.
    pub label: String,
    /// Type of geometric primitive.
    pub kind: GeometricKind,
    /// Position in sketch space (x, y).
    pub position: (f32, f32),
    /// Optional size/scale.
    pub scale: Option<f32>,
}

/// Types of geometric primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeometricKind {
    Point,
    Line,
    Curve,
    Rectangle,
    Circle,
    Triangle,
    Polygon,
    Freeform,
}

/// Spatial relationship between two sketch elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialRelationship {
    /// Source element label.
    pub from: String,
    /// Target element label.
    pub to: String,
    /// Relationship type.
    pub relation: SpatialRelationType,
    /// Strength of the relationship (0.0 - 1.0).
    pub strength: f32,
}

/// Types of spatial relationships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpatialRelationType {
    Adjacent,
    Contains,
    Above,
    Below,
    Inside,
    Surrounding,
    Connected,
    Separated,
}

/// Stage IV data: Emotional, aesthetic, and intangible impressions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageIVData {
    /// Emotional impact descriptors with intensity.
    pub emotional_impact: Vec<(String, f32)>,
    /// Tangible object impressions.
    pub tangibles: Vec<String>,
    /// Intangible concept impressions (purpose, function, significance).
    pub intangibles: Vec<String>,
    /// Analytical overlay detections with timestamps.
    pub aol_detections: Vec<AOLDetection>,
}

/// An analytical overlay (AOL) detection event.
///
/// AOL occurs when the viewer's analytical mind attempts to assign
/// a known label/concept to incoming signal line data, potentially
/// contaminating the raw perception.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AOLDetection {
    /// The AOL content (what the viewer's mind jumped to).
    pub content: String,
    /// Timestamp within the session (milliseconds from start).
    pub timestamp_ms: u64,
    /// Whether it was flagged and set aside ("AOL break").
    pub flagged: bool,
    /// Anomaly score from spike rate analysis (0.0 - 1.0).
    /// Higher scores indicate stronger AOL contamination.
    pub anomaly_score: f32,
}

/// Stage V data: Interrogation and cross-referencing results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageVData {
    /// Probe queries and their results.
    pub probes: Vec<SignalLineProbe>,
    /// Cross-references to data from earlier stages.
    pub cross_references: Vec<CrossReference>,
}

/// A signal line probe query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalLineProbe {
    /// The question or aspect being probed.
    pub query: String,
    /// Stage being interrogated.
    pub target_stage: u8,
    /// Resulting soft attention weights over candidates.
    pub attention_weights: Vec<f32>,
    /// Top-k candidate indices from differentiable search.
    pub top_candidates: Vec<usize>,
}

/// A cross-reference between stage data entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossReference {
    /// Source stage number.
    pub from_stage: u8,
    /// Source entry index.
    pub from_entry: usize,
    /// Target stage number.
    pub to_stage: u8,
    /// Target entry index.
    pub to_entry: usize,
    /// Similarity/relevance score.
    pub score: f32,
}

/// Stage VI data: Composite 3D model from accumulated session data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageVIData {
    /// Cluster partitions discovered by MinCut.
    pub partitions: Vec<TargetPartition>,
    /// Overall composite descriptor.
    pub composite_description: String,
    /// Confidence scores per partition.
    pub partition_confidence: Vec<f32>,
}

/// A partition of the target, representing a distinct aspect or component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetPartition {
    /// Human-readable label for this partition.
    pub label: String,
    /// Stage data entry indices that belong to this partition.
    pub member_entries: Vec<(u8, usize)>,
    /// Centroid embedding of this partition.
    pub centroid: Vec<f32>,
    /// MinCut value separating this partition from others.
    pub separation_strength: f32,
}

/// A complete CRV session entry stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrvSessionEntry {
    /// Session identifier.
    pub session_id: SessionId,
    /// Target coordinate.
    pub coordinate: TargetCoordinate,
    /// CRV stage (1-6).
    pub stage: u8,
    /// Embedding vector for this entry.
    pub embedding: Vec<f32>,
    /// Arbitrary metadata.
    pub metadata: HashMap<String, serde_json::Value>,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Configuration for CRV session processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrvConfig {
    /// Embedding dimensionality.
    pub dimensions: usize,
    /// Curvature for Poincare ball (Stage I). Positive value.
    pub curvature: f32,
    /// AOL anomaly detection threshold (Stage IV).
    pub aol_threshold: f32,
    /// SNN refractory period in ms (Stage IV).
    pub refractory_period_ms: f64,
    /// SNN time step in ms (Stage IV).
    pub snn_dt: f64,
    /// Differentiable search temperature (Stage V).
    pub search_temperature: f32,
    /// Convergence threshold for cross-session matching.
    pub convergence_threshold: f32,
}

impl Default for CrvConfig {
    fn default() -> Self {
        Self {
            dimensions: 384,
            curvature: 1.0,
            aol_threshold: 0.7,
            refractory_period_ms: 50.0,
            snn_dt: 1.0,
            search_temperature: 1.0,
            convergence_threshold: 0.75,
        }
    }
}

/// Result of a convergence analysis across multiple sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergenceResult {
    /// Session pairs that converged.
    pub session_pairs: Vec<(SessionId, SessionId)>,
    /// Convergence scores per pair.
    pub scores: Vec<f32>,
    /// Stages where convergence was strongest.
    pub convergent_stages: Vec<u8>,
    /// Merged embedding representing the consensus signal.
    pub consensus_embedding: Option<Vec<f32>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gestalt_type_all() {
        let all = GestaltType::all();
        assert_eq!(all.len(), 6);
    }

    #[test]
    fn test_gestalt_type_index() {
        assert_eq!(GestaltType::Manmade.index(), 0);
        assert_eq!(GestaltType::Land.index(), 5);
    }

    #[test]
    fn test_default_config() {
        let config = CrvConfig::default();
        assert_eq!(config.dimensions, 384);
        assert_eq!(config.curvature, 1.0);
        assert_eq!(config.aol_threshold, 0.7);
    }

    #[test]
    fn test_session_entry_serialization() {
        let entry = CrvSessionEntry {
            session_id: "sess-001".to_string(),
            coordinate: "1234-5678".to_string(),
            stage: 1,
            embedding: vec![0.1, 0.2, 0.3],
            metadata: HashMap::new(),
            timestamp_ms: 1000,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: CrvSessionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.session_id, "sess-001");
        assert_eq!(deserialized.stage, 1);
    }
}
