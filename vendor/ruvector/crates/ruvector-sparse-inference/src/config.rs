//! Configuration structures for sparse inference.

use serde::{Deserialize, Serialize};

/// Configuration for sparsity settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparsityConfig {
    /// Activation threshold τ for neuron selection.
    pub threshold: Option<f32>,

    /// Top-K neuron selection (alternative to threshold).
    pub top_k: Option<usize>,

    /// Target sparsity ratio (0.0 to 1.0).
    /// Used for automatic threshold calibration.
    pub target_sparsity: Option<f32>,

    /// Enable adaptive threshold adjustment.
    pub adaptive_threshold: bool,
}

impl Default for SparsityConfig {
    fn default() -> Self {
        Self {
            threshold: Some(0.01),
            top_k: None,
            target_sparsity: None,
            adaptive_threshold: false,
        }
    }
}

impl SparsityConfig {
    /// Create config with threshold-based selection.
    pub fn with_threshold(threshold: f32) -> Self {
        Self {
            threshold: Some(threshold),
            top_k: None,
            target_sparsity: None,
            adaptive_threshold: false,
        }
    }

    /// Create config with top-K selection.
    pub fn with_top_k(k: usize) -> Self {
        Self {
            threshold: None,
            top_k: Some(k),
            target_sparsity: None,
            adaptive_threshold: false,
        }
    }

    /// Create config with target sparsity ratio.
    pub fn with_target_sparsity(sparsity: f32) -> Self {
        Self {
            threshold: None,
            top_k: None,
            target_sparsity: Some(sparsity),
            adaptive_threshold: true,
        }
    }

    /// Validate configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.threshold.is_none() && self.top_k.is_none() && self.target_sparsity.is_none() {
            return Err("Must specify threshold, top_k, or target_sparsity".to_string());
        }

        if let Some(threshold) = self.threshold {
            if threshold < 0.0 {
                return Err(format!("Threshold must be non-negative, got {}", threshold));
            }
        }

        if let Some(k) = self.top_k {
            if k == 0 {
                return Err("top_k must be greater than 0".to_string());
            }
        }

        if let Some(sparsity) = self.target_sparsity {
            if !(0.0..=1.0).contains(&sparsity) {
                return Err(format!(
                    "target_sparsity must be in [0, 1], got {}",
                    sparsity
                ));
            }
        }

        Ok(())
    }
}

/// Configuration for the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Input dimension.
    pub input_dim: usize,

    /// Hidden dimension (number of neurons).
    pub hidden_dim: usize,

    /// Output dimension.
    pub output_dim: usize,

    /// Activation function type.
    pub activation: ActivationType,

    /// Low-rank approximation rank.
    pub rank: usize,

    /// Sparsity configuration.
    pub sparsity: SparsityConfig,

    /// Enable quantization.
    pub quantization: Option<QuantizationType>,
}

impl ModelConfig {
    /// Create a new model configuration.
    pub fn new(input_dim: usize, hidden_dim: usize, output_dim: usize, rank: usize) -> Self {
        Self {
            input_dim,
            hidden_dim,
            output_dim,
            activation: ActivationType::Gelu,
            rank,
            sparsity: SparsityConfig::default(),
            quantization: None,
        }
    }

    /// Validate configuration.
    pub fn validate(&self) -> Result<(), String> {
        if self.input_dim == 0 {
            return Err("input_dim must be greater than 0".to_string());
        }
        if self.hidden_dim == 0 {
            return Err("hidden_dim must be greater than 0".to_string());
        }
        if self.output_dim == 0 {
            return Err("output_dim must be greater than 0".to_string());
        }
        if self.rank == 0 || self.rank > self.input_dim.min(self.hidden_dim) {
            return Err(format!(
                "rank must be in (0, min(input_dim, hidden_dim)], got {}",
                self.rank
            ));
        }
        self.sparsity.validate()?;
        Ok(())
    }
}

/// Cache strategy for cold neurons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CacheStrategy {
    /// Least Recently Used eviction.
    #[default]
    Lru,
    /// Least Frequently Used eviction.
    Lfu,
    /// First In First Out eviction.
    Fifo,
    /// No caching (always load from disk).
    None,
}

/// Cache configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Fraction of neurons to keep hot (0.0 to 1.0).
    pub hot_neuron_fraction: f32,

    /// Maximum number of cold neurons to cache.
    pub max_cold_cache_size: usize,

    /// Cache eviction strategy.
    pub cache_strategy: CacheStrategy,

    /// Number of hot neurons (always in memory).
    pub hot_neuron_count: usize,

    /// LRU cache size for cold neurons.
    pub lru_cache_size: usize,

    /// Enable memory-mapped cold weights.
    pub use_mmap: bool,

    /// Activation frequency threshold for hot classification.
    pub hot_threshold: f32,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            hot_neuron_fraction: 0.2,
            max_cold_cache_size: 1000,
            cache_strategy: CacheStrategy::Lru,
            hot_neuron_count: 1024,
            lru_cache_size: 4096,
            use_mmap: false,
            hot_threshold: 0.5,
        }
    }
}

/// Activation function types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationType {
    /// Rectified Linear Unit: max(0, x)
    Relu,

    /// Gaussian Error Linear Unit: x * Φ(x)
    Gelu,

    /// Sigmoid Linear Unit: x * sigmoid(x)
    Silu,

    /// Swish activation (same as SiLU)
    Swish,

    /// Identity (no activation)
    Identity,
}

impl ActivationType {
    /// Apply activation function to a single value.
    pub fn apply(&self, x: f32) -> f32 {
        match self {
            Self::Relu => x.max(0.0),
            Self::Gelu => {
                // Approximation: 0.5 * x * (1 + tanh(sqrt(2/π) * (x + 0.044715 * x^3)))
                const SQRT_2_OVER_PI: f32 = 0.7978845608;
                let x3 = x * x * x;
                let inner = SQRT_2_OVER_PI * (x + 0.044715 * x3);
                0.5 * x * (1.0 + inner.tanh())
            }
            Self::Silu | Self::Swish => {
                // x * sigmoid(x) = x / (1 + exp(-x))
                x / (1.0 + (-x).exp())
            }
            Self::Identity => x,
        }
    }

    /// Apply activation function to a slice in-place.
    pub fn apply_slice(&self, data: &mut [f32]) {
        for x in data.iter_mut() {
            *x = self.apply(*x);
        }
    }
}

/// Quantization types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantizationType {
    /// 32-bit floating point (no quantization).
    F32,

    /// 16-bit floating point.
    F16,

    /// 8-bit integer quantization.
    Int8,

    /// 4-bit integer quantization (GGUF-style).
    Int4 {
        /// Group size for quantization.
        group_size: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sparsity_config_validation() {
        let config = SparsityConfig::with_threshold(0.01);
        assert!(config.validate().is_ok());

        let config = SparsityConfig::with_top_k(100);
        assert!(config.validate().is_ok());

        let mut config = SparsityConfig::default();
        config.threshold = None;
        config.top_k = None;
        config.target_sparsity = None;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_model_config_validation() {
        let config = ModelConfig::new(128, 512, 128, 64);
        assert!(config.validate().is_ok());

        let mut config = ModelConfig::new(128, 512, 128, 0);
        assert!(config.validate().is_err());

        config.rank = 200;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_activation_functions() {
        let relu = ActivationType::Relu;
        assert_eq!(relu.apply(-1.0), 0.0);
        assert_eq!(relu.apply(1.0), 1.0);

        let gelu = ActivationType::Gelu;
        assert!(gelu.apply(0.0).abs() < 0.01);
        assert!(gelu.apply(1.0) > 0.8);

        let silu = ActivationType::Silu;
        assert!(silu.apply(0.0).abs() < 0.01);
        assert!(silu.apply(1.0) > 0.7);
    }
}
