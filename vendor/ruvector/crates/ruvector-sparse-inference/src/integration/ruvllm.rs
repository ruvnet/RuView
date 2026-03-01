//! RuvLLM InferenceBackend integration
//!
//! This module provides a sparse inference backend that integrates with
//! the RuvLLM language model framework for efficient text generation.
//!
//! # Example
//!
//! ```rust,ignore
//! use ruvector_sparse_inference::integration::SparseInferenceBackend;
//!
//! let backend = SparseInferenceBackend::from_gguf("llama-7b.gguf")?;
//! let output = backend.generate(&[1, 2, 3], 100)?;
//! ```

use crate::{
    config::{ActivationType, CacheConfig, SparsityConfig},
    error::{Result, SparseInferenceError},
    memory::NeuronCache,
    model::{GgufModel, GgufParser, InferenceConfig, ModelMetadata, ModelRunner},
    predictor::{LowRankPredictor, Predictor},
    sparse::SparseFfn,
};

/// KV Cache for autoregressive generation
#[derive(Debug)]
pub struct KVCache {
    /// Key cache per layer
    keys: Vec<Vec<Vec<f32>>>,
    /// Value cache per layer
    values: Vec<Vec<Vec<f32>>>,
    /// Maximum sequence length
    max_length: usize,
    /// Current sequence length
    current_length: usize,
}

impl KVCache {
    /// Create a new KV cache
    pub fn new(num_layers: usize, max_length: usize, head_dim: usize) -> Self {
        Self {
            keys: vec![Vec::new(); num_layers],
            values: vec![Vec::new(); num_layers],
            max_length,
            current_length: 0,
        }
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        for layer_keys in &mut self.keys {
            layer_keys.clear();
        }
        for layer_values in &mut self.values {
            layer_values.clear();
        }
        self.current_length = 0;
    }

    /// Get current sequence length
    pub fn len(&self) -> usize {
        self.current_length
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.current_length == 0
    }

    /// Append key-value pair for a layer
    pub fn append(&mut self, layer: usize, key: Vec<f32>, value: Vec<f32>) {
        if layer < self.keys.len() {
            self.keys[layer].push(key);
            self.values[layer].push(value);
            if layer == 0 {
                self.current_length += 1;
            }
        }
    }
}

/// Generation configuration
#[derive(Debug, Clone)]
pub struct GenerationConfig {
    /// Maximum new tokens to generate
    pub max_new_tokens: usize,
    /// Temperature for sampling
    pub temperature: f32,
    /// Top-K sampling
    pub top_k: usize,
    /// Top-P (nucleus) sampling
    pub top_p: f32,
    /// Repetition penalty
    pub repetition_penalty: f32,
    /// Stop tokens
    pub stop_tokens: Vec<u32>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            max_new_tokens: 100,
            temperature: 0.7,
            top_k: 50,
            top_p: 0.9,
            repetition_penalty: 1.1,
            stop_tokens: vec![2], // Default EOS token
        }
    }
}

/// Generation statistics
#[derive(Debug, Clone, Default)]
pub struct GenerationStats {
    /// Total tokens generated
    pub tokens_generated: usize,
    /// Average inference time per token (ms)
    pub avg_token_time_ms: f64,
    /// Average sparsity ratio
    pub avg_sparsity: f64,
    /// Total inference time (ms)
    pub total_time_ms: f64,
}

/// Sparse inference backend for RuvLLM integration
pub struct SparseInferenceBackend {
    /// Model metadata
    metadata: ModelMetadata,
    /// Layer predictors (one per layer)
    predictors: Vec<LowRankPredictor>,
    /// Layer FFNs (one per layer)
    ffns: Vec<SparseFfn>,
    /// Neuron cache for hot neurons
    neuron_cache: NeuronCache,
    /// Inference configuration
    config: InferenceConfig,
    /// Generation statistics
    stats: GenerationStats,
    /// Vocabulary size
    vocab_size: usize,
}

impl SparseInferenceBackend {
    /// Create a new sparse inference backend
    pub fn new(
        num_layers: usize,
        hidden_dim: usize,
        intermediate_dim: usize,
        vocab_size: usize,
        sparsity_ratio: f32,
    ) -> Result<Self> {
        // Use top-K selection based on sparsity ratio for reliable activation
        let target_active = ((1.0 - sparsity_ratio) * intermediate_dim as f32).max(1.0) as usize;
        let sparsity_config = SparsityConfig {
            threshold: None,
            top_k: Some(target_active),
            target_sparsity: Some(sparsity_ratio),
            adaptive_threshold: false,
        };

        let cache_config = CacheConfig {
            hot_neuron_fraction: 0.2, // 20% hot neurons
            max_cold_cache_size: 1000,
            cache_strategy: crate::config::CacheStrategy::Lru,
            hot_neuron_count: (intermediate_dim as f32 * 0.2) as usize,
            lru_cache_size: 4096,
            use_mmap: false,
            hot_threshold: 0.5,
        };

        // Create predictors and FFNs for each layer
        let mut predictors = Vec::with_capacity(num_layers);
        let mut ffns = Vec::with_capacity(num_layers);

        for _ in 0..num_layers {
            let predictor = LowRankPredictor::new(
                hidden_dim,
                intermediate_dim,
                intermediate_dim / 32,
                sparsity_config.clone(),
            )?;
            predictors.push(predictor);

            let ffn = SparseFfn::new(
                hidden_dim,
                intermediate_dim,
                hidden_dim,
                ActivationType::Silu, // Llama uses SiLU
            )?;
            ffns.push(ffn);
        }

        let neuron_cache = NeuronCache::new(intermediate_dim, cache_config);

        let metadata = ModelMetadata {
            hidden_size: hidden_dim,
            intermediate_size: intermediate_dim,
            num_layers,
            num_heads: hidden_dim / 64, // Assuming head_dim = 64
            num_key_value_heads: None,
            vocab_size,
            max_position_embeddings: 4096,
            architecture: crate::model::ModelArchitecture::Llama,
            quantization: None,
            rope_theta: Some(10000.0),
            rope_scaling: None,
        };

        Ok(Self {
            metadata,
            predictors,
            ffns,
            neuron_cache,
            config: InferenceConfig::default(),
            stats: GenerationStats::default(),
            vocab_size,
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

        // Extract model configuration from GGUF metadata
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

        let num_layers = gguf
            .metadata
            .get("llama.block_count")
            .and_then(|v| v.as_u32())
            .unwrap_or(32) as usize;

        let vocab_size = gguf
            .metadata
            .get("llama.vocab_size")
            .and_then(|v| v.as_u32())
            .unwrap_or(32000) as usize;

        Self::new(num_layers, hidden_dim, intermediate_dim, vocab_size, 0.1)
    }

    /// Generate next token
    pub fn next_token(&mut self, input_ids: &[u32], kv_cache: &mut KVCache) -> Result<u32> {
        // Simplified next token prediction
        // In production, this would:
        // 1. Look up token embeddings
        // 2. Apply rotary position embeddings
        // 3. Run through transformer layers with sparse FFN
        // 4. Compute logits and sample

        let hidden_dim = self.metadata.hidden_size;

        // Create mock hidden state from input
        let mut hidden: Vec<f32> = input_ids
            .iter()
            .map(|&t| (t as f32) / (self.vocab_size as f32))
            .collect();
        hidden.resize(hidden_dim, 0.0);

        // Process through sparse FFN layers
        for (layer_idx, (predictor, ffn)) in
            self.predictors.iter().zip(self.ffns.iter()).enumerate()
        {
            // Predict active neurons
            let active = predictor.predict(&hidden)?;

            // Sparse FFN forward
            hidden = ffn.forward_sparse(&hidden, &active)?;

            // Update cache stats
            self.neuron_cache.record_activations(&active);
        }

        // Compute logits (simplified - use output projection)
        let logit_sum: f32 = hidden.iter().sum();
        let next_token = ((logit_sum.abs() * 1000.0) as u32) % (self.vocab_size as u32);

        self.stats.tokens_generated += 1;

        Ok(next_token)
    }

    /// Generate multiple tokens
    pub fn generate(&mut self, input_ids: &[u32], config: &GenerationConfig) -> Result<Vec<u32>> {
        let mut output_ids = input_ids.to_vec();
        let mut kv_cache = KVCache::new(
            self.metadata.num_layers,
            config.max_new_tokens + input_ids.len(),
            self.metadata.hidden_size / self.metadata.num_heads,
        );

        let start_time = std::time::Instant::now();

        for _ in 0..config.max_new_tokens {
            let next_token = self.next_token(&output_ids, &mut kv_cache)?;

            // Check for stop token
            if config.stop_tokens.contains(&next_token) {
                break;
            }

            output_ids.push(next_token);
        }

        let elapsed = start_time.elapsed();
        self.stats.total_time_ms = elapsed.as_secs_f64() * 1000.0;
        self.stats.avg_token_time_ms =
            self.stats.total_time_ms / self.stats.tokens_generated as f64;

        Ok(output_ids)
    }

    /// Get model metadata
    pub fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    /// Get generation statistics
    pub fn generation_stats(&self) -> &GenerationStats {
        &self.stats
    }

    /// Set sparsity threshold
    pub fn set_sparsity(&mut self, threshold: f32) {
        self.config.sparsity_threshold = threshold;
    }

    /// Calibrate predictors with sample data
    pub fn calibrate(&mut self, samples: &[Vec<f32>]) -> Result<()> {
        for (predictor, ffn) in self.predictors.iter_mut().zip(self.ffns.iter()) {
            // Generate activations for each sample
            let activations: Vec<Vec<f32>> = samples
                .iter()
                .map(|s| ffn.forward_dense(s))
                .collect::<Result<Vec<_>>>()?;

            predictor.calibrate(samples, &activations)?;
        }
        Ok(())
    }

    /// Reset KV cache (for new conversation)
    pub fn reset(&mut self) {
        self.stats = GenerationStats::default();
        self.neuron_cache.clear();
    }
}

/// Trait for inference backends (matches RuvLLM interface)
pub trait InferenceBackend: Send + Sync {
    /// Generate next token probabilities
    fn forward(&mut self, input_ids: &[u32]) -> Result<Vec<f32>>;

    /// Generate tokens
    fn generate(&mut self, input_ids: &[u32], max_new_tokens: usize) -> Result<Vec<u32>>;

    /// Get vocabulary size
    fn vocab_size(&self) -> usize;

    /// Backend name
    fn name(&self) -> &str;
}

impl InferenceBackend for SparseInferenceBackend {
    fn forward(&mut self, input_ids: &[u32]) -> Result<Vec<f32>> {
        // Return logits (simplified)
        let hidden_dim = self.metadata.hidden_size;
        let mut hidden: Vec<f32> = input_ids
            .iter()
            .map(|&t| (t as f32) / (self.vocab_size as f32))
            .collect();
        hidden.resize(hidden_dim, 0.0);

        for (predictor, ffn) in self.predictors.iter().zip(self.ffns.iter()) {
            let active = predictor.predict(&hidden)?;
            hidden = ffn.forward_sparse(&hidden, &active)?;
        }

        Ok(hidden)
    }

    fn generate(&mut self, input_ids: &[u32], max_new_tokens: usize) -> Result<Vec<u32>> {
        let config = GenerationConfig {
            max_new_tokens,
            ..Default::default()
        };
        self.generate(input_ids, &config)
    }

    fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    fn name(&self) -> &str {
        "sparse-inference"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_creation() {
        let backend = SparseInferenceBackend::new(4, 256, 1024, 32000, 0.1);
        assert!(backend.is_ok());

        let backend = backend.unwrap();
        assert_eq!(backend.metadata.num_layers, 4);
        assert_eq!(backend.vocab_size(), 32000);
    }

    #[test]
    fn test_next_token() {
        // Use lower sparsity threshold to ensure enough neurons are active
        let mut backend = SparseInferenceBackend::new(2, 64, 256, 1000, 0.001).unwrap();
        let mut kv_cache = KVCache::new(2, 100, 64);

        let result = backend.next_token(&[1, 2, 3], &mut kv_cache);
        assert!(result.is_ok(), "next_token failed: {:?}", result.err());

        let token = result.unwrap();
        assert!(token < 1000);
    }

    #[test]
    fn test_generate() {
        // Use lower sparsity threshold to ensure enough neurons are active
        let mut backend = SparseInferenceBackend::new(2, 64, 256, 1000, 0.001).unwrap();
        let config = GenerationConfig {
            max_new_tokens: 10,
            ..Default::default()
        };

        let result = backend.generate(&[1, 2, 3], &config);
        assert!(result.is_ok(), "generate failed: {:?}", result.err());

        let output = result.unwrap();
        assert!(output.len() >= 3); // At least input tokens
        assert!(output.len() <= 13); // At most input + max_new_tokens
    }

    #[test]
    fn test_kv_cache() {
        let mut cache = KVCache::new(4, 100, 64);
        assert!(cache.is_empty());

        cache.append(0, vec![1.0; 64], vec![2.0; 64]);
        assert_eq!(cache.len(), 1);

        cache.clear();
        assert!(cache.is_empty());
    }
}
