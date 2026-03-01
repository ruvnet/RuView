//! Core types for REFRAG pipeline
//!
//! These types extend ruvector's VectorEntry with tensor storage capabilities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for REFRAG entries
pub type PointId = String;

/// REFRAG-enhanced entry with representation tensor support
///
/// This struct extends the standard VectorEntry with:
/// - `representation_tensor`: Pre-computed chunk embedding for LLM injection
/// - `alignment_model_id`: Which LLM space the tensor is aligned to
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefragEntry {
    /// Unique identifier
    pub id: PointId,

    /// Standard search vector for HNSW indexing (e.g., 384-dim sentence embedding)
    pub search_vector: Vec<f32>,

    /// Pre-computed representation tensor (compressed chunk embedding)
    /// Stored as binary for zero-copy access
    /// Typical shapes: [768] for RoBERTa, [4096] for LLaMA
    pub representation_tensor: Option<Vec<u8>>,

    /// Identifies which LLM space this tensor is aligned to
    /// e.g., "llama3-8b", "gpt-4", "claude-3"
    pub alignment_model_id: Option<String>,

    /// Original text content (fallback for EXPAND action)
    pub text_content: String,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl RefragEntry {
    /// Create a new RefragEntry with minimal fields
    pub fn new(id: impl Into<String>, search_vector: Vec<f32>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            search_vector,
            representation_tensor: None,
            alignment_model_id: None,
            text_content: text.into(),
            metadata: HashMap::new(),
        }
    }

    /// Add representation tensor
    pub fn with_tensor(mut self, tensor: Vec<u8>, model_id: impl Into<String>) -> Self {
        self.representation_tensor = Some(tensor);
        self.alignment_model_id = Some(model_id.into());
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Check if this entry has a representation tensor
    pub fn has_tensor(&self) -> bool {
        self.representation_tensor.is_some()
    }

    /// Get tensor dimensions (assumes f32 encoding)
    pub fn tensor_dimensions(&self) -> Option<usize> {
        self.representation_tensor.as_ref().map(|t| t.len() / 4)
    }
}

/// Response type for REFRAG search results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefragResponseType {
    /// Return expanded text content
    Expand,
    /// Return compressed tensor representation
    Compress,
}

impl Default for RefragResponseType {
    fn default() -> Self {
        Self::Expand
    }
}

/// REFRAG-enhanced search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefragSearchResult {
    /// Entry ID
    pub id: PointId,

    /// Similarity score
    pub score: f32,

    /// Response type determined by policy
    pub response_type: RefragResponseType,

    /// Text content (present when response_type == Expand)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Base64-encoded tensor (present when response_type == Compress)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor_b64: Option<String>,

    /// Tensor dimensions (for client-side decoding)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor_dims: Option<usize>,

    /// Alignment model ID (for projection lookup)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alignment_model_id: Option<String>,

    /// Policy confidence score
    pub policy_confidence: f32,

    /// Additional metadata
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl RefragSearchResult {
    /// Create an EXPAND result (text content)
    pub fn expand(id: PointId, score: f32, content: String, confidence: f32) -> Self {
        Self {
            id,
            score,
            response_type: RefragResponseType::Expand,
            content: Some(content),
            tensor_b64: None,
            tensor_dims: None,
            alignment_model_id: None,
            policy_confidence: confidence,
            metadata: HashMap::new(),
        }
    }

    /// Create a COMPRESS result (tensor representation)
    pub fn compress(
        id: PointId,
        score: f32,
        tensor_b64: String,
        tensor_dims: usize,
        alignment_model_id: Option<String>,
        confidence: f32,
    ) -> Self {
        Self {
            id,
            score,
            response_type: RefragResponseType::Compress,
            content: None,
            tensor_b64: Some(tensor_b64),
            tensor_dims: Some(tensor_dims),
            alignment_model_id,
            policy_confidence: confidence,
            metadata: HashMap::new(),
        }
    }
}

/// Configuration for REFRAG pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefragConfig {
    /// Search vector dimensions (for HNSW index)
    pub search_dimensions: usize,

    /// Representation tensor dimensions
    pub tensor_dimensions: usize,

    /// Target LLM dimensions (for projection)
    pub target_dimensions: usize,

    /// Policy threshold for COMPRESS decision (0.0 - 1.0)
    /// Higher = more likely to return tensor
    pub compress_threshold: f32,

    /// Enable automatic projection when dimensions mismatch
    pub auto_project: bool,

    /// Maximum entries to evaluate with policy per search
    pub policy_batch_size: usize,
}

impl Default for RefragConfig {
    fn default() -> Self {
        Self {
            search_dimensions: 384,
            tensor_dimensions: 768,
            target_dimensions: 4096,
            compress_threshold: 0.85,
            auto_project: true,
            policy_batch_size: 100,
        }
    }
}

/// Statistics for REFRAG operations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RefragStats {
    /// Total searches performed
    pub total_searches: u64,

    /// Results returned as EXPAND (text)
    pub expand_count: u64,

    /// Results returned as COMPRESS (tensor)
    pub compress_count: u64,

    /// Average policy decision time (microseconds)
    pub avg_policy_time_us: f64,

    /// Average projection time (microseconds)
    pub avg_projection_time_us: f64,

    /// Total bytes saved by COMPRESS responses
    pub bytes_saved: u64,
}

impl RefragStats {
    /// Calculate compression ratio
    pub fn compression_ratio(&self) -> f64 {
        let total = self.expand_count + self.compress_count;
        if total == 0 {
            0.0
        } else {
            self.compress_count as f64 / total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refrag_entry_builder() {
        let entry = RefragEntry::new("doc_1", vec![0.1, 0.2, 0.3], "Hello world")
            .with_tensor(vec![0u8; 768 * 4], "llama3-8b")
            .with_metadata("source", serde_json::json!("wikipedia"));

        assert_eq!(entry.id, "doc_1");
        assert!(entry.has_tensor());
        assert_eq!(entry.tensor_dimensions(), Some(768));
        assert_eq!(entry.alignment_model_id, Some("llama3-8b".to_string()));
    }

    #[test]
    fn test_response_types() {
        let expand = RefragSearchResult::expand("doc_1".into(), 0.95, "Text content".into(), 0.9);
        assert_eq!(expand.response_type, RefragResponseType::Expand);
        assert!(expand.content.is_some());
        assert!(expand.tensor_b64.is_none());

        let compress = RefragSearchResult::compress(
            "doc_2".into(),
            0.88,
            "base64data".into(),
            768,
            Some("llama3-8b".into()),
            0.95,
        );
        assert_eq!(compress.response_type, RefragResponseType::Compress);
        assert!(compress.content.is_none());
        assert!(compress.tensor_b64.is_some());
    }
}
