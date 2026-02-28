//! Pooling strategies for converting token embeddings to sentence embeddings

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Strategy for pooling token embeddings into a single sentence embedding
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub enum PoolingStrategy {
    /// Average all token embeddings (most common)
    #[default]
    Mean,
    /// Use only the [CLS] token embedding
    Cls,
    /// Take the maximum value across all tokens for each dimension
    Max,
    /// Mean pooling normalized by sqrt of sequence length
    MeanSqrtLen,
    /// Use the last token embedding (for decoder models)
    LastToken,
}

impl PoolingStrategy {
    /// Apply pooling to token embeddings
    ///
    /// # Arguments
    /// * `embeddings` - Token embeddings [seq_len, hidden_size]
    /// * `attention_mask` - Attention mask [seq_len]
    ///
    /// # Returns
    /// Pooled embedding [hidden_size]
    pub fn apply(&self, embeddings: &[f32], attention_mask: &[i64], hidden_size: usize) -> Vec<f32> {
        let seq_len = attention_mask.len();

        if embeddings.is_empty() || hidden_size == 0 {
            return vec![0.0; hidden_size];
        }

        match self {
            PoolingStrategy::Mean => {
                self.mean_pooling(embeddings, attention_mask, hidden_size, seq_len)
            }
            PoolingStrategy::Cls => {
                // First token (CLS)
                embeddings[..hidden_size].to_vec()
            }
            PoolingStrategy::Max => {
                self.max_pooling(embeddings, attention_mask, hidden_size, seq_len)
            }
            PoolingStrategy::MeanSqrtLen => {
                let mut pooled = self.mean_pooling(embeddings, attention_mask, hidden_size, seq_len);
                let valid_tokens: f32 = attention_mask.iter().map(|&m| m as f32).sum();
                let scale = 1.0 / valid_tokens.sqrt();
                for v in &mut pooled {
                    *v *= scale;
                }
                pooled
            }
            PoolingStrategy::LastToken => {
                // Find last valid token
                let last_idx = attention_mask
                    .iter()
                    .rposition(|&m| m == 1)
                    .unwrap_or(0);
                let start = last_idx * hidden_size;
                embeddings[start..start + hidden_size].to_vec()
            }
        }
    }

    fn mean_pooling(
        &self,
        embeddings: &[f32],
        attention_mask: &[i64],
        hidden_size: usize,
        seq_len: usize,
    ) -> Vec<f32> {
        let mut pooled = vec![0.0f32; hidden_size];
        let mut count = 0.0f32;

        for (i, &mask) in attention_mask.iter().enumerate() {
            if mask == 1 && i < seq_len {
                let start = i * hidden_size;
                if start + hidden_size <= embeddings.len() {
                    for (j, v) in pooled.iter_mut().enumerate() {
                        *v += embeddings[start + j];
                    }
                    count += 1.0;
                }
            }
        }

        if count > 0.0 {
            for v in &mut pooled {
                *v /= count;
            }
        }

        pooled
    }

    fn max_pooling(
        &self,
        embeddings: &[f32],
        attention_mask: &[i64],
        hidden_size: usize,
        seq_len: usize,
    ) -> Vec<f32> {
        let mut pooled = vec![f32::NEG_INFINITY; hidden_size];

        for (i, &mask) in attention_mask.iter().enumerate() {
            if mask == 1 && i < seq_len {
                let start = i * hidden_size;
                if start + hidden_size <= embeddings.len() {
                    for (j, v) in pooled.iter_mut().enumerate() {
                        *v = v.max(embeddings[start + j]);
                    }
                }
            }
        }

        // Replace -inf with 0 for dimensions with no valid tokens
        for v in &mut pooled {
            if v.is_infinite() {
                *v = 0.0;
            }
        }

        pooled
    }
}

/// L2 normalize a vector in place
pub fn normalize_l2(embedding: &mut [f32]) {
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in embedding {
            *v /= norm;
        }
    }
}

/// Compute cosine similarity between two embeddings
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_l2() {
        let mut v = vec![3.0, 4.0];
        normalize_l2(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }
}
