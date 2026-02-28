//! Configuration for RuvLLM

use crate::error::{Error, Result};
use crate::types::ModelSize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main configuration for RuvLLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// System configuration
    pub system: SystemConfig,
    /// Embedding configuration
    pub embedding: EmbeddingConfig,
    /// Memory configuration
    pub memory: MemoryConfig,
    /// Router configuration
    pub router: RouterConfig,
    /// Inference configuration
    pub inference: InferenceConfig,
    /// Learning configuration
    pub learning: LearningConfig,
}

impl Config {
    /// Create a new config builder
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::default()
    }

    /// Load config from file
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content).map_err(|e| Error::Config(e.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.embedding.dimension == 0 {
            return Err(Error::Config("embedding dimension must be > 0".into()));
        }
        if self.memory.hnsw_m == 0 {
            return Err(Error::Config("HNSW M must be > 0".into()));
        }
        if self.router.hidden_dim == 0 {
            return Err(Error::Config("router hidden_dim must be > 0".into()));
        }
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            system: SystemConfig::default(),
            embedding: EmbeddingConfig::default(),
            memory: MemoryConfig::default(),
            router: RouterConfig::default(),
            inference: InferenceConfig::default(),
            learning: LearningConfig::default(),
        }
    }
}

/// System-wide configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    /// Device class (edge, mobile, server, gpu)
    pub device_class: String,
    /// Maximum memory in MB
    pub max_memory_mb: usize,
    /// Maximum concurrent requests
    pub max_concurrent_requests: usize,
    /// Data directory
    pub data_dir: PathBuf,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            device_class: "server".into(),
            max_memory_mb: 8192,
            max_concurrent_requests: 10,
            data_dir: PathBuf::from("./data"),
        }
    }
}

/// Embedding service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Embedding dimension
    pub dimension: usize,
    /// Maximum tokens
    pub max_tokens: usize,
    /// Batch size
    pub batch_size: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            dimension: 768,
            max_tokens: 512,
            batch_size: 8,
        }
    }
}

/// Memory service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Database path
    pub db_path: PathBuf,
    /// HNSW M parameter
    pub hnsw_m: usize,
    /// HNSW ef_construction
    pub hnsw_ef_construction: usize,
    /// HNSW ef_search default
    pub hnsw_ef_search: usize,
    /// Maximum nodes
    pub max_nodes: usize,
    /// Writeback batch size
    pub writeback_batch_size: usize,
    /// Writeback interval in ms
    pub writeback_interval_ms: u64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from("./data/memory.db"),
            hnsw_m: 32,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 64,
            max_nodes: 10_000_000,
            writeback_batch_size: 100,
            writeback_interval_ms: 1000,
        }
    }
}

/// Router configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    /// Input dimension (features)
    pub input_dim: usize,
    /// Hidden dimension
    pub hidden_dim: usize,
    /// Sparsity for weight matrices
    pub sparsity: f32,
    /// Rank for low-rank matrices
    pub rank: usize,
    /// Confidence threshold for fallback
    pub confidence_threshold: f32,
    /// Weights path
    pub weights_path: Option<PathBuf>,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            input_dim: 128,
            hidden_dim: 64,
            sparsity: 0.9,
            rank: 8,
            confidence_threshold: 0.7,
            weights_path: None,
        }
    }
}

/// Inference configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Available models
    pub models: Vec<ModelSize>,
    /// Model paths
    pub model_paths: HashMap<String, PathBuf>,
    /// Quantization type
    pub quantization: String,
    /// Maximum context length
    pub max_context: usize,
    /// Maximum models loaded concurrently
    pub max_loaded_models: usize,
    /// KV cache size per model
    pub kv_cache_size: usize,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self {
            models: vec![ModelSize::M700, ModelSize::B1_2],
            model_paths: HashMap::new(),
            quantization: "q4_k".into(),
            max_context: 4096,
            max_loaded_models: 2,
            kv_cache_size: 1000,
        }
    }
}

/// Learning service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningConfig {
    /// Enable learning
    pub enabled: bool,
    /// Quality threshold for writeback
    pub quality_threshold: f32,
    /// Replay buffer capacity
    pub replay_capacity: usize,
    /// Training batch size
    pub batch_size: usize,
    /// Learning rate
    pub learning_rate: f32,
    /// EWC lambda
    pub ewc_lambda: f32,
    /// Training interval in ms
    pub training_interval_ms: u64,
    /// Minimum samples before training
    pub min_samples: usize,
    /// Compression interval in ms
    pub compression_interval_ms: u64,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            quality_threshold: 0.75,
            replay_capacity: 100_000,
            batch_size: 32,
            learning_rate: 0.001,
            ewc_lambda: 0.4,
            training_interval_ms: 60_000,
            min_samples: 100,
            compression_interval_ms: 3600_000,
        }
    }
}

/// Config builder for fluent API
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    /// Set database path
    pub fn db_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.memory.db_path = path.into();
        self
    }

    /// Set data directory
    pub fn data_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.system.data_dir = path.into();
        self
    }

    /// Set embedding dimension
    pub fn embedding_dim(mut self, dim: usize) -> Self {
        self.config.embedding.dimension = dim;
        self
    }

    /// Set device class
    pub fn device_class(mut self, class: impl Into<String>) -> Self {
        self.config.system.device_class = class.into();
        self
    }

    /// Set max memory
    pub fn max_memory_mb(mut self, mb: usize) -> Self {
        self.config.system.max_memory_mb = mb;
        self
    }

    /// Add model path
    pub fn model_path(mut self, size: ModelSize, path: impl Into<PathBuf>) -> Self {
        let key = format!("{:?}", size).to_lowercase();
        self.config.inference.model_paths.insert(key, path.into());
        if !self.config.inference.models.contains(&size) {
            self.config.inference.models.push(size);
        }
        self
    }

    /// Enable/disable learning
    pub fn learning_enabled(mut self, enabled: bool) -> Self {
        self.config.learning.enabled = enabled;
        self
    }

    /// Set HNSW parameters
    pub fn hnsw_params(mut self, m: usize, ef_construction: usize, ef_search: usize) -> Self {
        self.config.memory.hnsw_m = m;
        self.config.memory.hnsw_ef_construction = ef_construction;
        self.config.memory.hnsw_ef_search = ef_search;
        self
    }

    /// Set router hidden dimension
    pub fn router_hidden_dim(mut self, dim: usize) -> Self {
        self.config.router.hidden_dim = dim;
        self
    }

    /// Build the config
    pub fn build(self) -> Result<Config> {
        self.config.validate()?;
        Ok(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_builder() {
        let config = Config::builder()
            .db_path("/tmp/test.db")
            .embedding_dim(384)
            .device_class("edge")
            .build()
            .unwrap();

        assert_eq!(config.memory.db_path, PathBuf::from("/tmp/test.db"));
        assert_eq!(config.embedding.dimension, 384);
        assert_eq!(config.system.device_class, "edge");
    }

    #[test]
    fn test_invalid_config() {
        let mut config = Config::default();
        config.embedding.dimension = 0;
        assert!(config.validate().is_err());
    }
}
