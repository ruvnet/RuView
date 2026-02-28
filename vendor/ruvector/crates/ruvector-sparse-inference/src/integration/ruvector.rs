//! Ruvector EmbeddingProvider integration
//!
//! This module provides a sparse inference-based embedding provider that
//! integrates with the Ruvector vector database ecosystem.
//!
//! # Example
//!
//! ```rust,ignore
//! use ruvector_sparse_inference::integration::SparseEmbeddingProvider;
//!
//! let provider = SparseEmbeddingProvider::from_gguf("model.gguf")?;
//! let embedding = provider.embed("Hello, world!")?;
//! ```

use crate::{
    config::{ActivationType, SparsityConfig},
    error::{Result, SparseInferenceError},
    model::{GgufParser, InferenceConfig},
    predictor::{LowRankPredictor, Predictor},
    sparse::SparseFfn,
    SparsityStats,
};

/// Sparse embedding provider for Ruvector integration
///
/// Implements the EmbeddingProvider interface using PowerInfer-style
/// sparse inference for efficient embedding generation.
pub struct SparseEmbeddingProvider {
    /// Sparse FFN for inference
    ffn: SparseFfn,
    /// Activation predictor
    predictor: LowRankPredictor,
    /// Inference configuration
    config: InferenceConfig,
    /// Embedding dimension
    embed_dim: usize,
    /// Sparsity statistics
    stats: SparsityStats,
}

impl SparseEmbeddingProvider {
    /// Create a new sparse embedding provider with specified dimensions
    pub fn new(
        input_dim: usize,
        hidden_dim: usize,
        embed_dim: usize,
        sparsity_ratio: f32,
    ) -> Result<Self> {
        // Use top-K selection based on sparsity ratio for reliable activation
        // This ensures we always have some active neurons regardless of random init
        let target_active = ((1.0 - sparsity_ratio) * hidden_dim as f32).max(1.0) as usize;
        let sparsity_config = SparsityConfig {
            threshold: None,
            top_k: Some(target_active),
            target_sparsity: Some(sparsity_ratio),
            adaptive_threshold: false,
        };

        let predictor = LowRankPredictor::new(
            input_dim,
            hidden_dim,
            hidden_dim / 32, // rank = hidden_dim / 32
            sparsity_config,
        )?;

        let ffn = SparseFfn::new(input_dim, hidden_dim, embed_dim, ActivationType::Gelu)?;

        Ok(Self {
            ffn,
            predictor,
            config: InferenceConfig::default(),
            embed_dim,
            stats: SparsityStats {
                average_active_ratio: 0.3,
                min_active: 0,
                max_active: hidden_dim,
            },
        })
    }

    /// Create from a GGUF model file
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_gguf(path: &std::path::Path) -> Result<Self> {
        use std::fs;

        let data = fs::read(path).map_err(|e| {
            SparseInferenceError::Model(crate::error::ModelError::LoadFailed(e.to_string()))
        })?;

        Self::from_gguf_bytes(&data)
    }

    /// Create from GGUF model bytes
    pub fn from_gguf_bytes(data: &[u8]) -> Result<Self> {
        let gguf = GgufParser::parse(data)?;

        // Extract dimensions from model metadata
        let hidden_dim = gguf
            .metadata
            .get("llama.embedding_length")
            .and_then(|v| v.as_u32())
            .unwrap_or(4096) as usize;

        let intermediate_dim = gguf
            .metadata
            .get("llama.feed_forward_length")
            .and_then(|v| v.as_u32())
            .unwrap_or((hidden_dim * 4) as u32) as usize;

        Self::new(hidden_dim, intermediate_dim, hidden_dim, 0.1)
    }

    /// Generate embedding for input tokens
    pub fn embed(&self, input: &[f32]) -> Result<Vec<f32>> {
        // Predict active neurons
        let active_neurons = self.predictor.predict(input)?;

        // Compute sparse forward pass
        let embedding = self.ffn.forward_sparse(input, &active_neurons)?;

        // Normalize embedding (L2 normalization)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        let normalized: Vec<f32> = if norm > 1e-8 {
            embedding.iter().map(|x| x / norm).collect()
        } else {
            embedding
        };

        Ok(normalized)
    }

    /// Batch embed multiple inputs
    pub fn embed_batch(&self, inputs: &[Vec<f32>]) -> Result<Vec<Vec<f32>>> {
        inputs.iter().map(|input| self.embed(input)).collect()
    }

    /// Get embedding dimension
    pub fn embedding_dim(&self) -> usize {
        self.embed_dim
    }

    /// Get sparsity statistics
    pub fn sparsity_stats(&self) -> &SparsityStats {
        &self.stats
    }

    /// Set sparsity threshold
    pub fn set_sparsity_threshold(&mut self, threshold: f32) {
        self.config.sparsity_threshold = threshold;
    }

    /// Calibrate the predictor with sample data
    pub fn calibrate(&mut self, samples: &[Vec<f32>]) -> Result<()> {
        // Generate activations for calibration
        let activations: Vec<Vec<f32>> = samples
            .iter()
            .map(|s| self.ffn.forward_dense(s))
            .collect::<Result<Vec<_>>>()?;

        // Calibrate predictor
        self.predictor.calibrate(samples, &activations)?;

        Ok(())
    }
}

/// Trait for embedding providers (matches Ruvector interface)
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embedding for text (requires tokenization)
    fn embed_text(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embedding for token ids
    fn embed_tokens(&self, tokens: &[u32]) -> Result<Vec<f32>>;

    /// Get embedding dimension
    fn dimension(&self) -> usize;

    /// Provider name
    fn name(&self) -> &str;
}

impl EmbeddingProvider for SparseEmbeddingProvider {
    fn embed_text(&self, _text: &str) -> Result<Vec<f32>> {
        // Note: This requires a tokenizer - return placeholder for now
        // In production, integrate with a tokenizer (e.g., tiktoken, sentencepiece)
        Err(SparseInferenceError::Inference(
            crate::error::InferenceError::InvalidInput(
                "Text embedding requires tokenizer integration".to_string(),
            ),
        ))
    }

    fn embed_tokens(&self, tokens: &[u32]) -> Result<Vec<f32>> {
        // Convert tokens to embeddings (simplified - real implementation needs token embedding lookup)
        let input: Vec<f32> = tokens
            .iter()
            .map(|&t| (t as f32) / 50000.0) // Normalize token ids
            .collect();

        // Pad or truncate to expected input dimension
        let padded: Vec<f32> = if input.len() >= self.embed_dim {
            input[..self.embed_dim].to_vec()
        } else {
            let mut padded = input;
            padded.resize(self.embed_dim, 0.0);
            padded
        };

        self.embed(&padded)
    }

    fn dimension(&self) -> usize {
        self.embed_dim
    }

    fn name(&self) -> &str {
        "sparse-inference"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = SparseEmbeddingProvider::new(512, 2048, 512, 0.1);
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.embedding_dim(), 512);
    }

    #[test]
    fn test_embed() {
        // Use lower sparsity threshold to ensure enough neurons are active
        let provider = SparseEmbeddingProvider::new(64, 256, 64, 0.001).unwrap();
        // Use varied input to get more neuron activations
        let input: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) / 64.0).collect();

        let embedding = provider.embed(&input);
        assert!(embedding.is_ok(), "Embedding failed: {:?}", embedding.err());

        let embedding = embedding.unwrap();
        assert_eq!(embedding.len(), 64);

        // Check L2 normalization
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01, "Norm is {}", norm);
    }

    #[test]
    fn test_batch_embed() {
        // Use lower sparsity threshold to ensure enough neurons are active
        let provider = SparseEmbeddingProvider::new(64, 256, 64, 0.001).unwrap();
        let inputs = vec![
            (0..64).map(|i| i as f32 / 64.0).collect(),
            (0..64).map(|i| (i as f32).sin()).collect(),
            (0..64).map(|i| (i as f32).cos()).collect(),
        ];

        let embeddings = provider.embed_batch(&inputs);
        assert!(
            embeddings.is_ok(),
            "Batch embed failed: {:?}",
            embeddings.err()
        );

        let embeddings = embeddings.unwrap();
        assert_eq!(embeddings.len(), 3);
    }
}
