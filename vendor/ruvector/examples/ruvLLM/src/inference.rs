//! LFM2 inference pool for model management
//!
//! Supports both mock inference (for testing/benchmarking orchestration) and
//! real SIMD-optimized CPU inference.

use crate::config::InferenceConfig;
use crate::error::{Error, InferenceError, Result};
use crate::simd_inference::{SimdGenerationConfig, SimdInferenceEngine};
use crate::types::ModelSize;

use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;

/// Generation configuration
#[derive(Debug, Clone)]
pub struct GenerationConfig {
    /// Maximum tokens to generate
    pub max_tokens: usize,
    /// Temperature
    pub temperature: f32,
    /// Top-p (nucleus sampling)
    pub top_p: f32,
    /// Top-k sampling
    pub top_k: usize,
    /// Repeat penalty
    pub repeat_penalty: f32,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            max_tokens: 256,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            repeat_penalty: 1.1,
        }
    }
}

impl From<&GenerationConfig> for SimdGenerationConfig {
    fn from(config: &GenerationConfig) -> Self {
        SimdGenerationConfig {
            max_tokens: config.max_tokens,
            temperature: config.temperature,
            top_p: config.top_p,
            top_k: config.top_k,
            repeat_penalty: config.repeat_penalty,
        }
    }
}

/// Result of generation
#[derive(Debug, Clone)]
pub struct GenerationResult {
    /// Generated text
    pub text: String,
    /// Tokens generated
    pub tokens_generated: usize,
    /// Model used
    pub model_used: ModelSize,
    /// Whether KV cache was hit
    pub cache_hit: bool,
    /// Inference time in milliseconds
    pub inference_time_ms: f64,
    /// Tokens per second
    pub tokens_per_second: f64,
}

/// Inference mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceMode {
    /// Mock inference (fast, for orchestration benchmarks)
    Mock,
    /// Real SIMD-optimized CPU inference
    RealSimd,
}

/// Pool of LFM2 models with lazy loading
pub struct InferencePool {
    /// Loaded mock models (for orchestration benchmarks)
    models: DashMap<ModelSize, Arc<MockModel>>,
    /// LRU tracking
    lru: RwLock<Vec<(ModelSize, Instant)>>,
    /// Configuration
    config: InferenceConfig,
    /// Real SIMD inference engine
    simd_engine: Option<Arc<SimdInferenceEngine>>,
    /// Current inference mode
    mode: InferenceMode,
}

/// Mock model for testing (measures orchestration overhead only)
struct MockModel {
    size: ModelSize,
}

impl InferencePool {
    /// Create a new inference pool with mock inference (fast orchestration benchmarks)
    pub async fn new(config: &InferenceConfig) -> Result<Self> {
        Ok(Self {
            models: DashMap::new(),
            lru: RwLock::new(Vec::new()),
            config: config.clone(),
            simd_engine: None,
            mode: InferenceMode::Mock,
        })
    }

    /// Create a new inference pool with real SIMD-optimized inference
    pub async fn new_with_real_inference(config: &InferenceConfig) -> Result<Self> {
        let engine = SimdInferenceEngine::new_demo();
        Ok(Self {
            models: DashMap::new(),
            lru: RwLock::new(Vec::new()),
            config: config.clone(),
            simd_engine: Some(Arc::new(engine)),
            mode: InferenceMode::RealSimd,
        })
    }

    /// Set inference mode
    pub fn set_mode(&mut self, mode: InferenceMode) {
        if mode == InferenceMode::RealSimd && self.simd_engine.is_none() {
            self.simd_engine = Some(Arc::new(SimdInferenceEngine::new_demo()));
        }
        self.mode = mode;
    }

    /// Get current inference mode
    pub fn mode(&self) -> InferenceMode {
        self.mode
    }

    /// Generate response from a model
    pub async fn generate(
        &self,
        model_size: ModelSize,
        prompt: &str,
        config: GenerationConfig,
        session_key: Option<&str>,
    ) -> Result<GenerationResult> {
        let start = Instant::now();

        match self.mode {
            InferenceMode::Mock => {
                // Get or load mock model
                let _model = self.get_or_load(model_size).await?;

                // Mock generation (measures orchestration overhead only)
                let response = self.mock_generate(prompt, &config, model_size);
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;

                Ok(GenerationResult {
                    text: response,
                    tokens_generated: config.max_tokens / 2,
                    model_used: model_size,
                    cache_hit: false,
                    inference_time_ms: elapsed,
                    tokens_per_second: (config.max_tokens as f64 / 2.0) / (elapsed / 1000.0),
                })
            }
            InferenceMode::RealSimd => {
                // Use real SIMD-optimized inference
                let engine = self.simd_engine.as_ref().ok_or_else(|| {
                    Error::Inference(InferenceError::InitFailed(
                        "SIMD engine not initialized".to_string(),
                    ))
                })?;

                let simd_config: SimdGenerationConfig = (&config).into();
                let (text, tokens_generated, inference_time_ms) =
                    engine.generate(prompt, &simd_config, session_key);

                let tokens_per_second = if inference_time_ms > 0.0 {
                    (tokens_generated as f64 / inference_time_ms) * 1000.0
                } else {
                    0.0
                };

                Ok(GenerationResult {
                    text,
                    tokens_generated,
                    model_used: model_size,
                    cache_hit: session_key.is_some(),
                    inference_time_ms,
                    tokens_per_second,
                })
            }
        }
    }

    /// Health check
    pub async fn health_check(&self) -> Result<HealthInfo> {
        let (simd_vocab, simd_layers) = if let Some(engine) = &self.simd_engine {
            engine.model_info()
        } else {
            (0, 0)
        };

        Ok(HealthInfo {
            latency: 0.0,
            loaded_models: self.models.len(),
            available_memory: 0,
            inference_mode: format!("{:?}", self.mode),
            simd_vocab_size: simd_vocab,
            simd_num_layers: simd_layers,
        })
    }

    async fn get_or_load(&self, size: ModelSize) -> Result<Arc<MockModel>> {
        // Check if already loaded
        if let Some(model) = self.models.get(&size) {
            self.update_lru(size);
            return Ok(model.clone());
        }

        // Evict if needed
        while self.models.len() >= self.config.max_loaded_models {
            if let Some((evict_size, _)) = self.get_lru_oldest() {
                self.models.remove(&evict_size);
            }
        }

        // Load model
        let model = Arc::new(MockModel { size });
        self.models.insert(size, model.clone());
        self.update_lru(size);

        Ok(model)
    }

    fn update_lru(&self, size: ModelSize) {
        let mut lru = self.lru.write();
        lru.retain(|(s, _)| *s != size);
        lru.push((size, Instant::now()));
    }

    fn get_lru_oldest(&self) -> Option<(ModelSize, Instant)> {
        let lru = self.lru.read();
        lru.first().cloned()
    }

    fn mock_generate(
        &self,
        prompt: &str,
        config: &GenerationConfig,
        model_size: ModelSize,
    ) -> String {
        // Simple mock response based on prompt
        let model_name = match model_size {
            ModelSize::M350 => "350M",
            ModelSize::M700 => "700M",
            ModelSize::B1_2 => "1.2B",
            ModelSize::B2_6 => "2.6B",
        };

        // Extract question from prompt
        let question = if let Some(q_start) = prompt.find("Question:") {
            let q = &prompt[q_start + 9..];
            if let Some(end) = q.find('\n') {
                q[..end].trim()
            } else {
                q.trim()
            }
        } else {
            "your question"
        };

        format!(
            "Based on the provided context, I can answer {}. \
            [This is a mock response from {} model with temperature {:.1}]",
            question, model_name, config.temperature
        )
    }
}

/// Health information
#[derive(Debug, Clone)]
pub struct HealthInfo {
    /// Check latency in ms
    pub latency: f32,
    /// Number of loaded models
    pub loaded_models: usize,
    /// Available memory in bytes
    pub available_memory: usize,
    /// Current inference mode
    pub inference_mode: String,
    /// SIMD engine vocabulary size
    pub simd_vocab_size: usize,
    /// SIMD engine number of layers
    pub simd_num_layers: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inference_pool_creation() {
        let config = InferenceConfig::default();
        let pool = InferencePool::new(&config).await.unwrap();
        assert_eq!(pool.models.len(), 0);
    }

    #[tokio::test]
    async fn test_generate() {
        let config = InferenceConfig::default();
        let pool = InferencePool::new(&config).await.unwrap();

        let result = pool
            .generate(
                ModelSize::M700,
                "Question: What is Rust?\n\nAnswer:",
                GenerationConfig::default(),
                None,
            )
            .await
            .unwrap();

        assert!(!result.text.is_empty());
        assert_eq!(result.model_used, ModelSize::M700);
    }

    #[tokio::test]
    async fn test_model_eviction() {
        let mut config = InferenceConfig::default();
        config.max_loaded_models = 2;
        let pool = InferencePool::new(&config).await.unwrap();

        // Load 3 models
        pool.generate(ModelSize::M350, "test", GenerationConfig::default(), None)
            .await
            .unwrap();
        pool.generate(ModelSize::M700, "test", GenerationConfig::default(), None)
            .await
            .unwrap();
        pool.generate(ModelSize::B1_2, "test", GenerationConfig::default(), None)
            .await
            .unwrap();

        // Should only have 2 models loaded
        assert!(pool.models.len() <= 2);
    }
}
