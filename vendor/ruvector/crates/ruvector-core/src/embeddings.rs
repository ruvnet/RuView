//! Text Embedding Providers
//!
//! This module provides a pluggable embedding system for AgenticDB.
//!
//! ## Available Providers
//!
//! - **HashEmbedding**: Fast hash-based placeholder (default, not semantic)
//! - **CandleEmbedding**: Real embeddings using candle-transformers (feature: `real-embeddings`)
//! - **ApiEmbedding**: External API calls (OpenAI, Anthropic, Cohere, etc.)
//!
//! ## Usage
//!
//! ```rust,no_run
//! use ruvector_core::embeddings::{EmbeddingProvider, HashEmbedding, ApiEmbedding};
//! use ruvector_core::AgenticDB;
//!
//! // Default: Hash-based (fast, but not semantic)
//! let hash_provider = HashEmbedding::new(384);
//! let embedding = hash_provider.embed("hello world")?;
//!
//! // API-based (requires API key)
//! let api_provider = ApiEmbedding::openai("sk-...", "text-embedding-3-small");
//! let embedding = api_provider.embed("hello world")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::error::Result;
#[cfg(any(feature = "real-embeddings", feature = "api-embeddings"))]
use crate::error::RuvectorError;
use std::sync::Arc;

/// Trait for text embedding providers
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embedding vector for the given text
    fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Get the dimensionality of embeddings produced by this provider
    fn dimensions(&self) -> usize;

    /// Get a description of this provider (for logging/debugging)
    fn name(&self) -> &str;
}

/// Hash-based embedding provider (placeholder, not semantic)
///
/// ⚠️ **WARNING**: This does NOT produce semantic embeddings!
/// - "dog" and "cat" will NOT be similar
/// - "dog" and "god" WILL be similar (same characters)
///
/// Use this only for:
/// - Testing
/// - Prototyping
/// - When semantic similarity is not required
#[derive(Debug, Clone)]
pub struct HashEmbedding {
    dimensions: usize,
}

impl HashEmbedding {
    /// Create a new hash-based embedding provider
    pub fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }
}

impl EmbeddingProvider for HashEmbedding {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut embedding = vec![0.0; self.dimensions];
        let bytes = text.as_bytes();

        for (i, byte) in bytes.iter().enumerate() {
            embedding[i % self.dimensions] += (*byte as f32) / 255.0;
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embedding {
                *val /= norm;
            }
        }

        Ok(embedding)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn name(&self) -> &str {
        "HashEmbedding (placeholder)"
    }
}

/// Real embeddings using candle-transformers
///
/// Requires feature flag: `real-embeddings`
///
/// ⚠️ **Note**: Full candle integration is complex and model-specific.
/// For production use, we recommend:
/// 1. Using the API-based providers (simpler, always up-to-date)
/// 2. Using ONNX Runtime with pre-exported models
/// 3. Implementing your own candle wrapper for your specific model
///
/// This is a stub implementation showing the structure.
/// Users should implement `EmbeddingProvider` trait for their specific models.
#[cfg(feature = "real-embeddings")]
pub mod candle {
    use super::*;

    /// Candle-based embedding provider stub
    ///
    /// This is a placeholder. For real implementation:
    /// 1. Add candle dependencies for your specific model type
    /// 2. Implement model loading and inference
    /// 3. Handle tokenization appropriately
    ///
    /// Example structure:
    /// ```rust,ignore
    /// pub struct CandleEmbedding {
    ///     model: YourModelType,
    ///     tokenizer: Tokenizer,
    ///     device: Device,
    ///     dimensions: usize,
    /// }
    /// ```
    pub struct CandleEmbedding {
        dimensions: usize,
        model_id: String,
    }

    impl CandleEmbedding {
        /// Create a stub candle embedding provider
        ///
        /// **This is not a real implementation!**
        /// For production, implement with actual model loading.
        ///
        /// # Example
        /// ```rust,no_run
        /// # #[cfg(feature = "real-embeddings")]
        /// # {
        /// use ruvector_core::embeddings::candle::CandleEmbedding;
        ///
        /// // This returns an error - real implementation required
        /// let result = CandleEmbedding::from_pretrained(
        ///     "sentence-transformers/all-MiniLM-L6-v2",
        ///     false
        /// );
        /// assert!(result.is_err());
        /// # }
        /// ```
        pub fn from_pretrained(model_id: &str, _use_gpu: bool) -> Result<Self> {
            Err(RuvectorError::ModelLoadError(format!(
                "Candle embedding support is a stub. Please:\n\
                     1. Use ApiEmbedding for production (recommended)\n\
                     2. Or implement CandleEmbedding for model: {}\n\
                     3. See docs for ONNX Runtime integration examples",
                model_id
            )))
        }
    }

    impl EmbeddingProvider for CandleEmbedding {
        fn embed(&self, _text: &str) -> Result<Vec<f32>> {
            Err(RuvectorError::ModelInferenceError(
                "Candle embedding not implemented - use ApiEmbedding instead".to_string(),
            ))
        }

        fn dimensions(&self) -> usize {
            self.dimensions
        }

        fn name(&self) -> &str {
            "CandleEmbedding (stub - not implemented)"
        }
    }
}

#[cfg(feature = "real-embeddings")]
pub use candle::CandleEmbedding;

/// API-based embedding provider (OpenAI, Anthropic, Cohere, etc.)
///
/// Supports any API that accepts JSON and returns embeddings in a standard format.
///
/// # Example (OpenAI)
/// ```rust,no_run
/// use ruvector_core::embeddings::{EmbeddingProvider, ApiEmbedding};
///
/// let provider = ApiEmbedding::openai("sk-...", "text-embedding-3-small");
/// let embedding = provider.embed("hello world")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg(feature = "api-embeddings")]
#[derive(Clone)]
pub struct ApiEmbedding {
    api_key: String,
    endpoint: String,
    model: String,
    dimensions: usize,
    client: reqwest::blocking::Client,
}

#[cfg(feature = "api-embeddings")]
impl ApiEmbedding {
    /// Create a new API embedding provider
    ///
    /// # Arguments
    /// * `api_key` - API key for authentication
    /// * `endpoint` - API endpoint URL
    /// * `model` - Model identifier
    /// * `dimensions` - Expected embedding dimensions
    pub fn new(api_key: String, endpoint: String, model: String, dimensions: usize) -> Self {
        Self {
            api_key,
            endpoint,
            model,
            dimensions,
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Create OpenAI embedding provider
    ///
    /// # Models
    /// - `text-embedding-3-small` - 1536 dimensions, $0.02/1M tokens
    /// - `text-embedding-3-large` - 3072 dimensions, $0.13/1M tokens
    /// - `text-embedding-ada-002` - 1536 dimensions (legacy)
    pub fn openai(api_key: &str, model: &str) -> Self {
        let dimensions = match model {
            "text-embedding-3-large" => 3072,
            _ => 1536, // text-embedding-3-small and ada-002
        };

        Self::new(
            api_key.to_string(),
            "https://api.openai.com/v1/embeddings".to_string(),
            model.to_string(),
            dimensions,
        )
    }

    /// Create Cohere embedding provider
    ///
    /// # Models
    /// - `embed-english-v3.0` - 1024 dimensions
    /// - `embed-multilingual-v3.0` - 1024 dimensions
    pub fn cohere(api_key: &str, model: &str) -> Self {
        Self::new(
            api_key.to_string(),
            "https://api.cohere.ai/v1/embed".to_string(),
            model.to_string(),
            1024,
        )
    }

    /// Create Voyage AI embedding provider
    ///
    /// # Models
    /// - `voyage-2` - 1024 dimensions
    /// - `voyage-large-2` - 1536 dimensions
    pub fn voyage(api_key: &str, model: &str) -> Self {
        let dimensions = if model.contains("large") { 1536 } else { 1024 };

        Self::new(
            api_key.to_string(),
            "https://api.voyageai.com/v1/embeddings".to_string(),
            model.to_string(),
            dimensions,
        )
    }
}

#[cfg(feature = "api-embeddings")]
impl EmbeddingProvider for ApiEmbedding {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let request_body = serde_json::json!({
            "input": text,
            "model": self.model,
        });

        let response = self
            .client
            .post(&self.endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .map_err(|e| {
                RuvectorError::ModelInferenceError(format!("API request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(RuvectorError::ModelInferenceError(format!(
                "API returned error {}: {}",
                status, error_text
            )));
        }

        let response_json: serde_json::Value = response.json().map_err(|e| {
            RuvectorError::ModelInferenceError(format!("Failed to parse response: {}", e))
        })?;

        // Handle different API response formats
        let embedding = if let Some(data) = response_json.get("data") {
            // OpenAI format: {"data": [{"embedding": [...]}]}
            data.as_array()
                .and_then(|arr| arr.first())
                .and_then(|obj| obj.get("embedding"))
                .and_then(|emb| emb.as_array())
                .ok_or_else(|| {
                    RuvectorError::ModelInferenceError("Invalid OpenAI response format".to_string())
                })?
        } else if let Some(embeddings) = response_json.get("embeddings") {
            // Cohere format: {"embeddings": [[...]]}
            embeddings
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|emb| emb.as_array())
                .ok_or_else(|| {
                    RuvectorError::ModelInferenceError("Invalid Cohere response format".to_string())
                })?
        } else {
            return Err(RuvectorError::ModelInferenceError(
                "Unknown API response format".to_string(),
            ));
        };

        let embedding_vec: Result<Vec<f32>> = embedding
            .iter()
            .map(|v| {
                v.as_f64().map(|f| f as f32).ok_or_else(|| {
                    RuvectorError::ModelInferenceError("Invalid embedding value".to_string())
                })
            })
            .collect();

        embedding_vec
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn name(&self) -> &str {
        "ApiEmbedding"
    }
}

/// Type-erased embedding provider for dynamic dispatch
pub type BoxedEmbeddingProvider = Arc<dyn EmbeddingProvider>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_embedding() {
        let provider = HashEmbedding::new(128);

        let emb1 = provider.embed("hello world").unwrap();
        let emb2 = provider.embed("hello world").unwrap();

        assert_eq!(emb1.len(), 128);
        assert_eq!(emb1, emb2, "Same text should produce same embedding");

        // Check normalization
        let norm: f32 = emb1.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "Embedding should be normalized");
    }

    #[test]
    fn test_hash_embedding_different_text() {
        let provider = HashEmbedding::new(128);

        let emb1 = provider.embed("hello").unwrap();
        let emb2 = provider.embed("world").unwrap();

        assert_ne!(
            emb1, emb2,
            "Different text should produce different embeddings"
        );
    }

    #[cfg(feature = "real-embeddings")]
    #[test]
    #[ignore] // Requires model download
    fn test_candle_embedding() {
        let provider =
            CandleEmbedding::from_pretrained("sentence-transformers/all-MiniLM-L6-v2", false)
                .unwrap();

        let embedding = provider.embed("hello world").unwrap();
        assert_eq!(embedding.len(), 384);

        // Check normalization
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "Embedding should be normalized");
    }

    #[test]
    #[ignore] // Requires API key
    fn test_api_embedding_openai() {
        let api_key = std::env::var("OPENAI_API_KEY").unwrap();
        let provider = ApiEmbedding::openai(&api_key, "text-embedding-3-small");

        let embedding = provider.embed("hello world").unwrap();
        assert_eq!(embedding.len(), 1536);
    }
}
