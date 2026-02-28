//! Configuration types for attention mechanisms.
//!
//! This module provides configuration structs and builders for various
//! attention mechanisms including standard, graph, and sparse attention.

use serde::{Deserialize, Serialize};

use crate::error::{AttentionError, AttentionResult};

/// Configuration for standard attention mechanisms.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttentionConfig {
    /// Model dimension (d_model)
    pub dim: usize,
    /// Number of attention heads
    pub num_heads: usize,
    /// Dropout probability (0.0 to 1.0)
    pub dropout: f32,
    /// Scaling factor (default: 1/sqrt(d_k))
    pub scale: Option<f32>,
    /// Whether to use causal masking
    pub causal: bool,
}

impl AttentionConfig {
    /// Creates a new builder for AttentionConfig.
    pub fn builder() -> AttentionConfigBuilder {
        AttentionConfigBuilder::default()
    }

    /// Validates the configuration.
    pub fn validate(&self) -> AttentionResult<()> {
        if self.dim == 0 {
            return Err(AttentionError::InvalidConfig(
                "dimension must be greater than 0".to_string(),
            ));
        }

        if self.num_heads == 0 {
            return Err(AttentionError::InvalidConfig(
                "num_heads must be greater than 0".to_string(),
            ));
        }

        if self.dim % self.num_heads != 0 {
            return Err(AttentionError::InvalidHeadCount {
                dim: self.dim,
                num_heads: self.num_heads,
            });
        }

        if self.dropout < 0.0 || self.dropout > 1.0 {
            return Err(AttentionError::InvalidConfig(
                "dropout must be in range [0.0, 1.0]".to_string(),
            ));
        }

        if let Some(scale) = self.scale {
            if !scale.is_finite() || scale <= 0.0 {
                return Err(AttentionError::InvalidConfig(
                    "scale must be positive and finite".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Returns the dimension per head (d_k).
    #[inline]
    pub fn head_dim(&self) -> usize {
        self.dim / self.num_heads
    }

    /// Returns the effective scale factor.
    #[inline]
    pub fn effective_scale(&self) -> f32 {
        self.scale
            .unwrap_or_else(|| 1.0 / (self.head_dim() as f32).sqrt())
    }
}

/// Builder for AttentionConfig.
#[derive(Default)]
pub struct AttentionConfigBuilder {
    dim: Option<usize>,
    num_heads: Option<usize>,
    dropout: f32,
    scale: Option<f32>,
    causal: bool,
}

impl AttentionConfigBuilder {
    /// Sets the model dimension.
    pub fn dim(mut self, dim: usize) -> Self {
        self.dim = Some(dim);
        self
    }

    /// Sets the number of attention heads.
    pub fn num_heads(mut self, num_heads: usize) -> Self {
        self.num_heads = Some(num_heads);
        self
    }

    /// Sets the dropout probability.
    pub fn dropout(mut self, dropout: f32) -> Self {
        self.dropout = dropout;
        self
    }

    /// Sets a custom scale factor.
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = Some(scale);
        self
    }

    /// Enables causal masking.
    pub fn causal(mut self, causal: bool) -> Self {
        self.causal = causal;
        self
    }

    /// Builds the AttentionConfig.
    pub fn build(self) -> AttentionResult<AttentionConfig> {
        let config = AttentionConfig {
            dim: self.dim.ok_or_else(|| {
                AttentionError::InvalidConfig("dimension must be specified".to_string())
            })?,
            num_heads: self.num_heads.ok_or_else(|| {
                AttentionError::InvalidConfig("num_heads must be specified".to_string())
            })?,
            dropout: self.dropout,
            scale: self.scale,
            causal: self.causal,
        };

        config.validate()?;
        Ok(config)
    }
}

/// Configuration for graph attention networks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphAttentionConfig {
    /// Base attention configuration
    pub base: AttentionConfig,
    /// Edge feature dimension (if using edge features)
    pub edge_dim: Option<usize>,
    /// Negative slope for LeakyReLU
    pub negative_slope: f32,
    /// Whether to concatenate multi-head outputs (vs averaging)
    pub concat_heads: bool,
}

impl GraphAttentionConfig {
    /// Creates a new builder for GraphAttentionConfig.
    pub fn builder() -> GraphAttentionConfigBuilder {
        GraphAttentionConfigBuilder::default()
    }

    /// Validates the configuration.
    pub fn validate(&self) -> AttentionResult<()> {
        self.base.validate()?;

        if self.negative_slope <= 0.0 || !self.negative_slope.is_finite() {
            return Err(AttentionError::InvalidConfig(
                "negative_slope must be positive and finite".to_string(),
            ));
        }

        if let Some(edge_dim) = self.edge_dim {
            if edge_dim == 0 {
                return Err(AttentionError::InvalidConfig(
                    "edge_dim must be greater than 0".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Builder for GraphAttentionConfig.
#[derive(Default)]
pub struct GraphAttentionConfigBuilder {
    base_builder: AttentionConfigBuilder,
    edge_dim: Option<usize>,
    negative_slope: f32,
    concat_heads: bool,
}

impl GraphAttentionConfigBuilder {
    /// Sets the model dimension.
    pub fn dim(mut self, dim: usize) -> Self {
        self.base_builder = self.base_builder.dim(dim);
        self
    }

    /// Sets the number of attention heads.
    pub fn num_heads(mut self, num_heads: usize) -> Self {
        self.base_builder = self.base_builder.num_heads(num_heads);
        self
    }

    /// Sets the edge feature dimension.
    pub fn edge_dim(mut self, edge_dim: usize) -> Self {
        self.edge_dim = Some(edge_dim);
        self
    }

    /// Sets the negative slope for LeakyReLU.
    pub fn negative_slope(mut self, slope: f32) -> Self {
        self.negative_slope = slope;
        self
    }

    /// Sets whether to concatenate multi-head outputs.
    pub fn concat_heads(mut self, concat: bool) -> Self {
        self.concat_heads = concat;
        self
    }

    /// Builds the GraphAttentionConfig.
    pub fn build(self) -> AttentionResult<GraphAttentionConfig> {
        let config = GraphAttentionConfig {
            base: self.base_builder.build()?,
            edge_dim: self.edge_dim,
            negative_slope: if self.negative_slope == 0.0 {
                0.2
            } else {
                self.negative_slope
            },
            concat_heads: self.concat_heads,
        };

        config.validate()?;
        Ok(config)
    }
}

/// Configuration for sparse attention mechanisms.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SparseAttentionConfig {
    /// Base attention configuration
    pub base: AttentionConfig,
    /// Block size for block-sparse attention
    pub block_size: usize,
    /// Number of random blocks per query
    pub num_random_blocks: usize,
    /// Number of global tokens
    pub num_global_tokens: usize,
}

impl SparseAttentionConfig {
    /// Creates a new builder for SparseAttentionConfig.
    pub fn builder() -> SparseAttentionConfigBuilder {
        SparseAttentionConfigBuilder::default()
    }

    /// Validates the configuration.
    pub fn validate(&self) -> AttentionResult<()> {
        self.base.validate()?;

        if self.block_size == 0 {
            return Err(AttentionError::InvalidConfig(
                "block_size must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

/// Builder for SparseAttentionConfig.
#[derive(Default)]
pub struct SparseAttentionConfigBuilder {
    base_builder: AttentionConfigBuilder,
    block_size: usize,
    num_random_blocks: usize,
    num_global_tokens: usize,
}

impl SparseAttentionConfigBuilder {
    /// Sets the model dimension.
    pub fn dim(mut self, dim: usize) -> Self {
        self.base_builder = self.base_builder.dim(dim);
        self
    }

    /// Sets the number of attention heads.
    pub fn num_heads(mut self, num_heads: usize) -> Self {
        self.base_builder = self.base_builder.num_heads(num_heads);
        self
    }

    /// Sets the block size.
    pub fn block_size(mut self, block_size: usize) -> Self {
        self.block_size = block_size;
        self
    }

    /// Sets the number of random blocks.
    pub fn num_random_blocks(mut self, num_random_blocks: usize) -> Self {
        self.num_random_blocks = num_random_blocks;
        self
    }

    /// Sets the number of global tokens.
    pub fn num_global_tokens(mut self, num_global_tokens: usize) -> Self {
        self.num_global_tokens = num_global_tokens;
        self
    }

    /// Builds the SparseAttentionConfig.
    pub fn build(self) -> AttentionResult<SparseAttentionConfig> {
        let config = SparseAttentionConfig {
            base: self.base_builder.build()?,
            block_size: if self.block_size == 0 {
                64
            } else {
                self.block_size
            },
            num_random_blocks: self.num_random_blocks,
            num_global_tokens: self.num_global_tokens,
        };

        config.validate()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_config_builder() {
        let config = AttentionConfig::builder()
            .dim(512)
            .num_heads(8)
            .dropout(0.1)
            .causal(true)
            .build()
            .unwrap();

        assert_eq!(config.dim, 512);
        assert_eq!(config.num_heads, 8);
        assert_eq!(config.dropout, 0.1);
        assert!(config.causal);
        assert_eq!(config.head_dim(), 64);
    }

    #[test]
    fn test_config_validation() {
        let result = AttentionConfig::builder()
            .dim(512)
            .num_heads(7) // Not divisible
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_graph_attention_config() {
        let config = GraphAttentionConfig::builder()
            .dim(256)
            .num_heads(4)
            .edge_dim(16)
            .negative_slope(0.2)
            .concat_heads(true)
            .build()
            .unwrap();

        assert_eq!(config.base.dim, 256);
        assert_eq!(config.edge_dim, Some(16));
        assert!(config.concat_heads);
    }

    #[test]
    fn test_sparse_attention_config() {
        let config = SparseAttentionConfig::builder()
            .dim(512)
            .num_heads(8)
            .block_size(64)
            .num_random_blocks(3)
            .num_global_tokens(64)
            .build()
            .unwrap();

        assert_eq!(config.base.dim, 512);
        assert_eq!(config.block_size, 64);
        assert_eq!(config.num_random_blocks, 3);
    }
}
