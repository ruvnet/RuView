//! # Attention Mechanisms Module
//!
//! Implements 39 attention mechanisms for PostgreSQL vector operations:
//! - Core: Scaled dot-product, Multi-head, Flash Attention v2
//! - Graph: GAT, GATv2, Sparse patterns
//! - Specialized: MoE, Cross-attention, Sliding window
//! - Hyperbolic: Poincaré, Lorentzian attention
//!
//! Provides SIMD-accelerated attention operations with efficient memory usage.

use pgrx::prelude::*;
use serde::{Deserialize, Serialize};

// Submodules
pub mod flash;
pub mod multi_head;
pub mod operators;
pub mod scaled_dot;

// Re-exports
pub use flash::FlashAttention;
pub use multi_head::MultiHeadAttention;
pub use scaled_dot::ScaledDotAttention;

/// Attention mechanism types supported by the extension
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PostgresEnum)]
pub enum AttentionType {
    /// Standard scaled dot-product attention: O(n²)
    ScaledDot,

    /// Multi-head attention with parallel heads
    MultiHead,

    /// Flash Attention v2 - memory efficient: O(n²) but low memory
    FlashV2,

    /// Linear attention: O(n)
    Linear,

    /// Graph Attention Network
    Gat,

    /// Sparse attention patterns
    Sparse,

    /// Mixture of Experts routing
    Moe,

    /// Cross-attention (Q from one source, K/V from another)
    Cross,

    /// Sliding window attention
    Sliding,

    /// Poincaré hyperbolic attention
    Poincare,
}

impl Default for AttentionType {
    fn default() -> Self {
        AttentionType::ScaledDot
    }
}

impl AttentionType {
    /// Returns a human-readable name for the attention type
    pub fn name(&self) -> &'static str {
        match self {
            AttentionType::ScaledDot => "scaled_dot",
            AttentionType::MultiHead => "multi_head",
            AttentionType::FlashV2 => "flash_v2",
            AttentionType::Linear => "linear",
            AttentionType::Gat => "gat",
            AttentionType::Sparse => "sparse",
            AttentionType::Moe => "moe",
            AttentionType::Cross => "cross",
            AttentionType::Sliding => "sliding",
            AttentionType::Poincare => "poincare",
        }
    }

    /// Returns the computational complexity as a string
    pub fn complexity(&self) -> &'static str {
        match self {
            AttentionType::ScaledDot => "O(n²)",
            AttentionType::MultiHead => "O(n²)",
            AttentionType::FlashV2 => "O(n²) memory-efficient",
            AttentionType::Linear => "O(n)",
            AttentionType::Gat => "O(E) where E=edges",
            AttentionType::Sparse => "O(n√n)",
            AttentionType::Moe => "O(n*k) where k=experts",
            AttentionType::Cross => "O(n*m)",
            AttentionType::Sliding => "O(n*w) where w=window",
            AttentionType::Poincare => "O(n²)",
        }
    }

    /// Returns best use case for this attention type
    pub fn best_for(&self) -> &'static str {
        match self {
            AttentionType::ScaledDot => "Small sequences (<512)",
            AttentionType::MultiHead => "General purpose, parallel processing",
            AttentionType::FlashV2 => "GPU acceleration, large sequences",
            AttentionType::Linear => "Very long sequences (>4K)",
            AttentionType::Gat => "Graph-structured data",
            AttentionType::Sparse => "Ultra-long sequences (>16K)",
            AttentionType::Moe => "Conditional computation, routing",
            AttentionType::Cross => "Query-document matching",
            AttentionType::Sliding => "Local context, streaming",
            AttentionType::Poincare => "Hierarchical data structures",
        }
    }
}

/// Parse attention type from string
impl std::str::FromStr for AttentionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "scaled_dot" | "scaleddot" => Ok(AttentionType::ScaledDot),
            "multi_head" | "multihead" => Ok(AttentionType::MultiHead),
            "flash_v2" | "flashv2" | "flash" => Ok(AttentionType::FlashV2),
            "linear" => Ok(AttentionType::Linear),
            "gat" => Ok(AttentionType::Gat),
            "sparse" => Ok(AttentionType::Sparse),
            "moe" => Ok(AttentionType::Moe),
            "cross" => Ok(AttentionType::Cross),
            "sliding" => Ok(AttentionType::Sliding),
            "poincare" | "poincaré" => Ok(AttentionType::Poincare),
            _ => Err(format!("Unknown attention type: {}", s)),
        }
    }
}

/// Trait for attention mechanism implementations
pub trait Attention: Send + Sync {
    /// Compute attention scores for a query against keys
    fn attention_scores(&self, query: &[f32], keys: &[&[f32]]) -> Vec<f32>;

    /// Compute weighted sum of values using attention scores
    fn apply_attention(&self, scores: &[f32], values: &[&[f32]]) -> Vec<f32> {
        assert_eq!(
            scores.len(),
            values.len(),
            "Scores and values length mismatch"
        );

        if values.is_empty() {
            return Vec::new();
        }

        let dim = values[0].len();
        let mut result = vec![0.0; dim];

        for (score, value) in scores.iter().zip(values.iter()) {
            for (r, v) in result.iter_mut().zip(value.iter()) {
                *r += score * v;
            }
        }

        result
    }

    /// Full attention forward pass: compute scores and apply to values
    fn forward(&self, query: &[f32], keys: &[&[f32]], values: &[&[f32]]) -> Vec<f32> {
        let scores = self.attention_scores(query, keys);
        self.apply_attention(&scores, values)
    }
}

/// Softmax activation for attention scores
#[inline]
pub fn softmax(logits: &[f32]) -> Vec<f32> {
    if logits.is_empty() {
        return Vec::new();
    }

    // Find max for numerical stability
    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    // Compute exp(x - max)
    let exp_values: Vec<f32> = logits.iter().map(|x| (x - max_logit).exp()).collect();

    // Compute sum
    let sum: f32 = exp_values.iter().sum();

    // Normalize
    if sum > 0.0 {
        exp_values.iter().map(|x| x / sum).collect()
    } else {
        vec![1.0 / logits.len() as f32; logits.len()]
    }
}

/// In-place softmax for better performance
#[inline]
pub fn softmax_inplace(logits: &mut [f32]) {
    if logits.is_empty() {
        return;
    }

    // Find max for numerical stability
    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    // Compute exp(x - max) in place
    for x in logits.iter_mut() {
        *x = (*x - max_logit).exp();
    }

    // Compute sum
    let sum: f32 = logits.iter().sum();

    // Normalize in place
    if sum > 0.0 {
        for x in logits.iter_mut() {
            *x /= sum;
        }
    } else {
        let uniform = 1.0 / logits.len() as f32;
        for x in logits.iter_mut() {
            *x = uniform;
        }
    }
}

#[cfg(feature = "pg_test")]
#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_softmax() {
        let logits = vec![1.0, 2.0, 3.0];
        let result = softmax(&logits);

        // Should sum to 1
        let sum: f32 = result.iter().sum();
        assert_relative_eq!(sum, 1.0, epsilon = 1e-6);

        // Higher logit should have higher probability
        assert!(result[2] > result[1]);
        assert!(result[1] > result[0]);
    }

    #[test]
    fn test_softmax_inplace() {
        let mut logits = vec![1.0, 2.0, 3.0];
        softmax_inplace(&mut logits);

        // Should sum to 1
        let sum: f32 = logits.iter().sum();
        assert_relative_eq!(sum, 1.0, epsilon = 1e-6);

        // Higher logit should have higher probability
        assert!(logits[2] > logits[1]);
        assert!(logits[1] > logits[0]);
    }

    #[test]
    fn test_softmax_numerical_stability() {
        // Large values that could overflow without max subtraction
        let logits = vec![1000.0, 1001.0, 1002.0];
        let result = softmax(&logits);

        // Should still sum to 1 and not be NaN
        let sum: f32 = result.iter().sum();
        assert_relative_eq!(sum, 1.0, epsilon = 1e-6);
        assert!(result.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn test_attention_type_parsing() {
        assert_eq!(
            "scaled_dot".parse::<AttentionType>().unwrap(),
            AttentionType::ScaledDot
        );
        assert_eq!(
            "flash_v2".parse::<AttentionType>().unwrap(),
            AttentionType::FlashV2
        );
        assert_eq!(
            "multi_head".parse::<AttentionType>().unwrap(),
            AttentionType::MultiHead
        );

        assert!("unknown".parse::<AttentionType>().is_err());
    }
}
