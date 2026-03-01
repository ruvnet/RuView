//! Pooling strategies for combining token embeddings into sentence embeddings

use crate::config::PoolingStrategy;
use rayon::prelude::*;
use tracing::{debug, instrument};

/// Pooler for combining token embeddings
#[derive(Debug, Clone)]
pub struct Pooler {
    strategy: PoolingStrategy,
    normalize: bool,
}

impl Pooler {
    /// Create a new pooler with the given strategy
    pub fn new(strategy: PoolingStrategy, normalize: bool) -> Self {
        Self { strategy, normalize }
    }

    /// Pool token embeddings into sentence embeddings
    ///
    /// # Arguments
    /// * `token_embeddings` - Token embeddings for each sequence [batch][seq_len * hidden]
    /// * `attention_mask` - Attention mask for each sequence [batch][seq_len]
    /// * `seq_length` - Sequence length
    /// * `hidden_size` - Hidden dimension size
    #[instrument(skip_all, fields(batch_size = token_embeddings.len(), strategy = ?self.strategy))]
    pub fn pool(
        &self,
        token_embeddings: &[Vec<f32>],
        attention_mask: &[Vec<i64>],
        seq_length: usize,
        hidden_size: usize,
    ) -> Vec<Vec<f32>> {
        debug!(
            "Pooling {} sequences with strategy {:?}",
            token_embeddings.len(),
            self.strategy
        );

        let embeddings: Vec<Vec<f32>> = token_embeddings
            .par_iter()
            .zip(attention_mask.par_iter())
            .map(|(tokens, mask)| {
                self.pool_single(tokens, mask, seq_length, hidden_size)
            })
            .collect();

        if self.normalize {
            embeddings
                .into_par_iter()
                .map(|emb| Self::normalize_vector(&emb))
                .collect()
        } else {
            embeddings
        }
    }

    /// Pool a single sequence
    fn pool_single(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        seq_length: usize,
        hidden_size: usize,
    ) -> Vec<f32> {
        match self.strategy {
            PoolingStrategy::Mean => {
                self.mean_pool(token_embeddings, attention_mask, seq_length, hidden_size)
            }
            PoolingStrategy::Cls => {
                self.cls_pool(token_embeddings, hidden_size)
            }
            PoolingStrategy::Max => {
                self.max_pool(token_embeddings, attention_mask, seq_length, hidden_size)
            }
            PoolingStrategy::MeanSqrtLen => {
                self.mean_sqrt_len_pool(token_embeddings, attention_mask, seq_length, hidden_size)
            }
            PoolingStrategy::LastToken => {
                self.last_token_pool(token_embeddings, attention_mask, seq_length, hidden_size)
            }
            PoolingStrategy::WeightedMean => {
                self.weighted_mean_pool(token_embeddings, attention_mask, seq_length, hidden_size)
            }
        }
    }

    /// Mean pooling over all tokens (weighted by attention mask)
    fn mean_pool(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        seq_length: usize,
        hidden_size: usize,
    ) -> Vec<f32> {
        let mut result = vec![0.0f32; hidden_size];
        let mut count = 0.0f32;

        for (i, &mask) in attention_mask.iter().enumerate().take(seq_length) {
            if mask == 1 {
                let start = i * hidden_size;
                let end = start + hidden_size;
                for (j, val) in token_embeddings[start..end].iter().enumerate() {
                    result[j] += val;
                }
                count += 1.0;
            }
        }

        if count > 0.0 {
            for val in &mut result {
                *val /= count;
            }
        }

        result
    }

    /// CLS token pooling (first token)
    fn cls_pool(&self, token_embeddings: &[f32], hidden_size: usize) -> Vec<f32> {
        token_embeddings[..hidden_size].to_vec()
    }

    /// Max pooling over all tokens
    fn max_pool(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        seq_length: usize,
        hidden_size: usize,
    ) -> Vec<f32> {
        let mut result = vec![f32::NEG_INFINITY; hidden_size];

        for (i, &mask) in attention_mask.iter().enumerate().take(seq_length) {
            if mask == 1 {
                let start = i * hidden_size;
                let end = start + hidden_size;
                for (j, val) in token_embeddings[start..end].iter().enumerate() {
                    if *val > result[j] {
                        result[j] = *val;
                    }
                }
            }
        }

        // Replace -inf with 0 for empty sequences
        for val in &mut result {
            if val.is_infinite() {
                *val = 0.0;
            }
        }

        result
    }

    /// Mean pooling with sqrt(length) scaling
    fn mean_sqrt_len_pool(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        seq_length: usize,
        hidden_size: usize,
    ) -> Vec<f32> {
        let mut result = self.mean_pool(token_embeddings, attention_mask, seq_length, hidden_size);
        let length: f32 = attention_mask.iter().filter(|&&m| m == 1).count() as f32;

        if length > 0.0 {
            let scale = length.sqrt();
            for val in &mut result {
                *val *= scale;
            }
        }

        result
    }

    /// Last token pooling (for decoder models)
    fn last_token_pool(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        _seq_length: usize,
        hidden_size: usize,
    ) -> Vec<f32> {
        // Find last non-padding token
        let last_idx = attention_mask
            .iter()
            .rposition(|&m| m == 1)
            .unwrap_or(0);

        let start = last_idx * hidden_size;
        let end = start + hidden_size;

        if end <= token_embeddings.len() {
            token_embeddings[start..end].to_vec()
        } else {
            self.cls_pool(token_embeddings, hidden_size)
        }
    }

    /// Weighted mean pooling based on position
    fn weighted_mean_pool(
        &self,
        token_embeddings: &[f32],
        attention_mask: &[i64],
        seq_length: usize,
        hidden_size: usize,
    ) -> Vec<f32> {
        let mut result = vec![0.0f32; hidden_size];
        let mut total_weight = 0.0f32;

        for (i, &mask) in attention_mask.iter().enumerate().take(seq_length) {
            if mask == 1 {
                // Weight decreases with position (more weight to early tokens)
                let weight = 1.0 / (i + 1) as f32;
                let start = i * hidden_size;
                let end = start + hidden_size;

                for (j, val) in token_embeddings[start..end].iter().enumerate() {
                    result[j] += val * weight;
                }
                total_weight += weight;
            }
        }

        if total_weight > 0.0 {
            for val in &mut result {
                *val /= total_weight;
            }
        }

        result
    }

    /// L2 normalize a vector
    pub fn normalize_vector(vec: &[f32]) -> Vec<f32> {
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm > 1e-12 {
            vec.iter().map(|x| x / norm).collect()
        } else {
            vec.to_vec()
        }
    }

    /// Compute cosine similarity between two vectors (SIMD-optimized)
    #[cfg(feature = "simsimd")]
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        use simsimd::SpatialSimilarity;
        f32::cosine(a, b).unwrap_or(0.0) as f32
    }

    #[cfg(not(feature = "simsimd"))]
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a > 1e-12 && norm_b > 1e-12 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }

    /// Compute dot product between two vectors (SIMD-optimized)
    #[cfg(feature = "simsimd")]
    pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        use simsimd::SpatialSimilarity;
        f32::dot(a, b).unwrap_or(0.0) as f32
    }

    #[cfg(not(feature = "simsimd"))]
    pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    /// Compute Euclidean distance between two vectors (SIMD-optimized)
    #[cfg(feature = "simsimd")]
    pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        use simsimd::SpatialSimilarity;
        (f32::sqeuclidean(a, b).unwrap_or(0.0) as f32).sqrt()
    }

    #[cfg(not(feature = "simsimd"))]
    pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}

impl Default for Pooler {
    fn default() -> Self {
        Self::new(PoolingStrategy::Mean, true)
    }
}

/// Batch distance computation using ndarray
pub fn batch_cosine_similarity(
    query: &[f32],
    candidates: &[Vec<f32>],
) -> Vec<f32> {
    candidates
        .par_iter()
        .map(|c| Pooler::cosine_similarity(query, c))
        .collect()
}

/// Find top-k most similar vectors
pub fn top_k_similar(
    query: &[f32],
    candidates: &[Vec<f32>],
    k: usize,
) -> Vec<(usize, f32)> {
    let mut scores: Vec<(usize, f32)> = candidates
        .par_iter()
        .enumerate()
        .map(|(i, c)| (i, Pooler::cosine_similarity(query, c)))
        .collect();

    // Sort by score descending
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scores.truncate(k);
    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_vector() {
        let vec = vec![3.0, 4.0];
        let normalized = Pooler::normalize_vector(&vec);

        let norm: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];

        assert!((Pooler::cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
        assert!((Pooler::cosine_similarity(&a, &c)).abs() < 1e-6);
    }

    #[test]
    fn test_mean_pooling() {
        let pooler = Pooler::new(PoolingStrategy::Mean, false);

        // 2 tokens, 3 dimensions
        let embeddings = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mask = vec![1i64, 1];

        let result = pooler.pool_single(&embeddings, &mask, 2, 3);

        assert_eq!(result.len(), 3);
        assert!((result[0] - 2.5).abs() < 1e-6);
        assert!((result[1] - 3.5).abs() < 1e-6);
        assert!((result[2] - 4.5).abs() < 1e-6);
    }

    #[test]
    fn test_cls_pooling() {
        let pooler = Pooler::new(PoolingStrategy::Cls, false);

        let embeddings = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let mask = vec![1i64, 1];

        let result = pooler.pool_single(&embeddings, &mask, 2, 3);

        assert_eq!(result, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_top_k_similar() {
        let query = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.707, 0.707, 0.0],
        ];

        let results = top_k_similar(&query, &candidates, 2);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0); // Most similar
    }
}
