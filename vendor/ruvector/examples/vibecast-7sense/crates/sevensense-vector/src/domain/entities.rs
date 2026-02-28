//! Domain entities for the Vector Space bounded context.
//!
//! These are the core domain objects that represent the vector indexing domain.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// A unique identifier for an embedding vector.
///
/// This wraps a UUID and provides domain-specific semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmbeddingId(Uuid);

impl EmbeddingId {
    /// Create a new random embedding ID.
    #[inline]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create an embedding ID from a UUID.
    #[inline]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Parse an embedding ID from a string.
    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }

    /// Get the inner UUID.
    #[inline]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Convert to bytes for storage.
    #[inline]
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    /// Create from bytes.
    #[inline]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }
}

impl Default for EmbeddingId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EmbeddingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for EmbeddingId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<EmbeddingId> for Uuid {
    fn from(id: EmbeddingId) -> Self {
        id.0
    }
}

/// Unix timestamp in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Create a timestamp for the current moment.
    pub fn now() -> Self {
        Self(chrono::Utc::now().timestamp_millis())
    }

    /// Create a timestamp from milliseconds since Unix epoch.
    #[inline]
    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    /// Get milliseconds since Unix epoch.
    #[inline]
    pub const fn as_millis(&self) -> i64 {
        self.0
    }

    /// Convert to chrono DateTime.
    pub fn to_datetime(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp_millis(self.0)
            .unwrap_or_else(|| chrono::DateTime::UNIX_EPOCH)
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::now()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_datetime().format("%Y-%m-%d %H:%M:%S%.3f UTC"))
    }
}

/// Configuration for the HNSW index.
///
/// These parameters control the trade-off between search accuracy,
/// index build time, and memory usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    /// Number of bi-directional links per element.
    /// Higher values improve recall but increase memory.
    /// Recommended: 32 for 1536-dimensional vectors.
    pub m: usize,

    /// Size of dynamic candidate list during construction.
    /// Higher values improve index quality but slow construction.
    /// Recommended: 200 for high-quality indices.
    pub ef_construction: usize,

    /// Size of dynamic candidate list during search.
    /// Higher values improve recall but slow queries.
    /// Recommended: 128 for balanced accuracy/speed.
    pub ef_search: usize,

    /// Maximum number of elements the index can hold.
    /// Pre-allocating improves construction performance.
    pub max_elements: usize,

    /// Dimensionality of vectors in this index.
    pub dimensions: usize,

    /// Whether to normalize vectors before indexing.
    pub normalize: bool,

    /// Distance metric to use.
    pub distance_metric: DistanceMetric,
}

impl HnswConfig {
    /// Create a configuration optimized for a given dimension.
    pub fn for_dimension(dim: usize) -> Self {
        Self {
            m: if dim >= 1024 { 32 } else { 16 },
            ef_construction: 200,
            ef_search: 128,
            max_elements: 1_000_000,
            dimensions: dim,
            normalize: true,
            distance_metric: DistanceMetric::Cosine,
        }
    }

    /// Create a configuration for OpenAI-style 1536-D embeddings.
    pub fn for_openai_embeddings() -> Self {
        Self::for_dimension(1536)
    }

    /// Create a configuration for smaller sentence transformers (384-D).
    pub fn for_sentence_transformers() -> Self {
        Self::for_dimension(384)
    }

    /// Builder: set M parameter.
    pub fn with_m(mut self, m: usize) -> Self {
        self.m = m;
        self
    }

    /// Builder: set ef_construction parameter.
    pub fn with_ef_construction(mut self, ef: usize) -> Self {
        self.ef_construction = ef;
        self
    }

    /// Builder: set ef_search parameter.
    pub fn with_ef_search(mut self, ef: usize) -> Self {
        self.ef_search = ef;
        self
    }

    /// Builder: set maximum elements.
    pub fn with_max_elements(mut self, max: usize) -> Self {
        self.max_elements = max;
        self
    }

    /// Builder: set distance metric.
    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Builder: set normalization flag.
    pub fn with_normalize(mut self, normalize: bool) -> Self {
        self.normalize = normalize;
        self
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.m < 2 {
            return Err(ConfigValidationError::InvalidM(self.m));
        }
        if self.ef_construction < self.m {
            return Err(ConfigValidationError::EfTooSmall {
                ef: self.ef_construction,
                m: self.m,
            });
        }
        if self.dimensions == 0 {
            return Err(ConfigValidationError::ZeroDimensions);
        }
        if self.max_elements == 0 {
            return Err(ConfigValidationError::ZeroMaxElements);
        }
        Ok(())
    }
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self::for_openai_embeddings()
    }
}

/// Distance metric for vector similarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistanceMetric {
    /// Cosine distance (1 - cosine_similarity).
    /// Best for normalized embeddings.
    Cosine,

    /// Euclidean (L2) distance.
    /// Best for spatial data.
    Euclidean,

    /// Dot product (negative for similarity ranking).
    /// Best for when vectors are already normalized.
    DotProduct,

    /// Poincaré distance in hyperbolic space.
    /// Best for hierarchical relationships.
    Poincare,
}

impl Default for DistanceMetric {
    fn default() -> Self {
        Self::Cosine
    }
}

/// Configuration validation errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigValidationError {
    #[error("M parameter must be >= 2, got {0}")]
    InvalidM(usize),

    #[error("ef_construction ({ef}) must be >= M ({m})")]
    EfTooSmall { ef: usize, m: usize },

    #[error("dimensions cannot be zero")]
    ZeroDimensions,

    #[error("max_elements cannot be zero")]
    ZeroMaxElements,
}

/// Metadata about a vector index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndex {
    /// Unique identifier for this index.
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Number of dimensions per vector.
    pub dimensions: usize,

    /// Current number of vectors in the index.
    pub size: usize,

    /// Configuration used for this index.
    pub config: HnswConfig,

    /// When the index was created.
    pub created_at: Timestamp,

    /// When the index was last modified.
    pub updated_at: Timestamp,

    /// Optional description.
    pub description: Option<String>,
}

impl VectorIndex {
    /// Create a new vector index metadata object.
    pub fn new(id: impl Into<String>, name: impl Into<String>, config: HnswConfig) -> Self {
        let now = Timestamp::now();
        Self {
            id: id.into(),
            name: name.into(),
            dimensions: config.dimensions,
            size: 0,
            config,
            created_at: now,
            updated_at: now,
            description: None,
        }
    }

    /// Update the size and modification timestamp.
    pub fn update_size(&mut self, size: usize) {
        self.size = size;
        self.updated_at = Timestamp::now();
    }

    /// Set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Type of relationship between embeddings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// Embeddings are similar based on vector proximity.
    Similar,

    /// Embeddings are sequential (temporal ordering).
    Sequential,

    /// Embeddings belong to the same cluster.
    SameCluster,

    /// Embeddings are from the same source/recording.
    SameSource,

    /// Custom relationship type.
    Custom,
}

impl Default for EdgeType {
    fn default() -> Self {
        Self::Similar
    }
}

impl fmt::Display for EdgeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Similar => write!(f, "similar"),
            Self::Sequential => write!(f, "sequential"),
            Self::SameCluster => write!(f, "same_cluster"),
            Self::SameSource => write!(f, "same_source"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

/// An edge in the similarity graph between embeddings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarityEdge {
    /// Source embedding ID.
    pub from_id: EmbeddingId,

    /// Target embedding ID.
    pub to_id: EmbeddingId,

    /// Distance between the embeddings.
    pub distance: f32,

    /// Type of relationship.
    pub edge_type: EdgeType,

    /// When this edge was created.
    pub created_at: Timestamp,

    /// Optional weight for weighted graph operations.
    pub weight: Option<f32>,

    /// Optional metadata.
    pub metadata: Option<EdgeMetadata>,
}

impl SimilarityEdge {
    /// Create a new similarity edge.
    pub fn new(from_id: EmbeddingId, to_id: EmbeddingId, distance: f32) -> Self {
        Self {
            from_id,
            to_id,
            distance,
            edge_type: EdgeType::Similar,
            created_at: Timestamp::now(),
            weight: None,
            metadata: None,
        }
    }

    /// Create a sequential edge (for temporal ordering).
    pub fn sequential(from_id: EmbeddingId, to_id: EmbeddingId) -> Self {
        Self {
            from_id,
            to_id,
            distance: 0.0,
            edge_type: EdgeType::Sequential,
            created_at: Timestamp::now(),
            weight: None,
            metadata: None,
        }
    }

    /// Set the edge type.
    pub fn with_type(mut self, edge_type: EdgeType) -> Self {
        self.edge_type = edge_type;
        self
    }

    /// Set the weight.
    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = Some(weight);
        self
    }

    /// Set metadata.
    pub fn with_metadata(mut self, metadata: EdgeMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Get similarity (1 - distance) for cosine metric.
    #[inline]
    pub fn similarity(&self) -> f32 {
        1.0 - self.distance.clamp(0.0, 1.0)
    }

    /// Check if this is a strong connection (high similarity).
    #[inline]
    pub fn is_strong(&self, threshold: f32) -> bool {
        self.similarity() >= threshold
    }
}

/// Optional metadata for edges.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EdgeMetadata {
    /// Source of this relationship.
    pub source: Option<String>,

    /// Confidence score for this relationship.
    pub confidence: Option<f32>,

    /// Additional key-value pairs.
    pub attributes: hashbrown::HashMap<String, String>,
}

impl EdgeMetadata {
    /// Create new empty metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the source.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set the confidence.
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence);
        self
    }

    /// Add an attribute.
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

/// A stored vector with its ID and optional metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredVector {
    /// Unique identifier.
    pub id: EmbeddingId,

    /// The vector data.
    pub vector: Vec<f32>,

    /// When this vector was stored.
    pub created_at: Timestamp,

    /// Optional metadata.
    pub metadata: Option<VectorMetadata>,
}

impl StoredVector {
    /// Create a new stored vector.
    pub fn new(id: EmbeddingId, vector: Vec<f32>) -> Self {
        Self {
            id,
            vector,
            created_at: Timestamp::now(),
            metadata: None,
        }
    }

    /// Set metadata.
    pub fn with_metadata(mut self, metadata: VectorMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Get the dimensionality.
    #[inline]
    pub fn dimensions(&self) -> usize {
        self.vector.len()
    }
}

/// Optional metadata for stored vectors.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VectorMetadata {
    /// Source file or recording ID.
    pub source_id: Option<String>,

    /// Timestamp within the source (e.g., audio timestamp).
    pub source_timestamp: Option<f64>,

    /// Labels or tags.
    pub labels: Vec<String>,

    /// Additional key-value pairs.
    pub attributes: hashbrown::HashMap<String, serde_json::Value>,
}

impl VectorMetadata {
    /// Create new empty metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the source ID.
    pub fn with_source_id(mut self, id: impl Into<String>) -> Self {
        self.source_id = Some(id.into());
        self
    }

    /// Set the source timestamp.
    pub fn with_source_timestamp(mut self, ts: f64) -> Self {
        self.source_timestamp = Some(ts);
        self
    }

    /// Add a label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    /// Add an attribute.
    pub fn with_attribute(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.attributes.insert(key.into(), value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_id_creation() {
        let id1 = EmbeddingId::new();
        let id2 = EmbeddingId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_embedding_id_parse() {
        let id = EmbeddingId::new();
        let s = id.to_string();
        let parsed = EmbeddingId::parse(&s).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_hnsw_config_default() {
        let config = HnswConfig::default();
        assert_eq!(config.dimensions, 1536);
        assert_eq!(config.m, 32);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_hnsw_config_validation() {
        let config = HnswConfig::default().with_m(1);
        assert!(config.validate().is_err());

        let config = HnswConfig::default().with_ef_construction(10);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_similarity_edge() {
        let from = EmbeddingId::new();
        let to = EmbeddingId::new();
        let edge = SimilarityEdge::new(from, to, 0.2);

        assert_eq!(edge.similarity(), 0.8);
        assert!(edge.is_strong(0.7));
        assert!(!edge.is_strong(0.9));
    }

    #[test]
    fn test_timestamp() {
        let ts1 = Timestamp::now();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ts2 = Timestamp::now();
        assert!(ts2 > ts1);
    }
}
