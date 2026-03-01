//! Attention Mechanism Module for iOS WASM
//!
//! Lightweight self-attention for content ranking and sequence modeling.
//! Optimized for minimal memory footprint on mobile devices.

/// Maximum sequence length for attention
const MAX_SEQ_LEN: usize = 64;

/// Single attention head
pub struct AttentionHead {
    /// Dimension of key/query/value
    dim: usize,
    /// Query projection weights
    w_query: Vec<f32>,
    /// Key projection weights
    w_key: Vec<f32>,
    /// Value projection weights
    w_value: Vec<f32>,
    /// Scaling factor (1/sqrt(dim))
    scale: f32,
}

impl AttentionHead {
    /// Create a new attention head with random initialization
    pub fn new(input_dim: usize, head_dim: usize, seed: u32) -> Self {
        let dim = head_dim;
        let weight_size = input_dim * dim;

        // Xavier initialization with deterministic pseudo-random
        let std_dev = (2.0 / (input_dim + dim) as f32).sqrt();

        let w_query = Self::init_weights(weight_size, seed, std_dev);
        let w_key = Self::init_weights(weight_size, seed.wrapping_add(1), std_dev);
        let w_value = Self::init_weights(weight_size, seed.wrapping_add(2), std_dev);

        Self {
            dim,
            w_query,
            w_key,
            w_value,
            scale: 1.0 / (dim as f32).sqrt(),
        }
    }

    /// Initialize weights with pseudo-random values
    fn init_weights(size: usize, seed: u32, std_dev: f32) -> Vec<f32> {
        let mut weights = Vec::with_capacity(size);
        let mut s = seed;

        for _ in 0..size {
            s = s.wrapping_mul(1103515245).wrapping_add(12345);
            let uniform = ((s >> 16) as f32 / 32768.0) - 1.0;
            weights.push(uniform * std_dev);
        }

        weights
    }

    /// Project input to query/key/value space
    #[inline]
    fn project(&self, input: &[f32], weights: &[f32]) -> Vec<f32> {
        let input_dim = self.w_query.len() / self.dim;
        let mut output = vec![0.0; self.dim];

        for (i, o) in output.iter_mut().enumerate() {
            for (j, &inp) in input.iter().take(input_dim).enumerate() {
                let idx = j * self.dim + i;
                if idx < weights.len() {
                    *o += inp * weights[idx];
                }
            }
        }

        output
    }

    /// Compute attention scores between query and key
    #[inline]
    fn attention_score(&self, query: &[f32], key: &[f32]) -> f32 {
        let dot: f32 = query.iter().zip(key.iter()).map(|(q, k)| q * k).sum();
        dot * self.scale
    }

    /// Apply softmax to attention scores
    fn softmax(scores: &mut [f32]) {
        if scores.is_empty() {
            return;
        }

        // Numerical stability: subtract max
        let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        let mut sum = 0.0;
        for s in scores.iter_mut() {
            *s = (*s - max_score).exp();
            sum += *s;
        }

        if sum > 1e-8 {
            for s in scores.iter_mut() {
                *s /= sum;
            }
        }
    }

    /// Compute self-attention over a sequence
    pub fn forward(&self, sequence: &[Vec<f32>]) -> Vec<Vec<f32>> {
        let seq_len = sequence.len().min(MAX_SEQ_LEN);
        if seq_len == 0 {
            return vec![];
        }

        // Project to Q, K, V
        let queries: Vec<Vec<f32>> = sequence.iter().take(seq_len)
            .map(|x| self.project(x, &self.w_query))
            .collect();
        let keys: Vec<Vec<f32>> = sequence.iter().take(seq_len)
            .map(|x| self.project(x, &self.w_key))
            .collect();
        let values: Vec<Vec<f32>> = sequence.iter().take(seq_len)
            .map(|x| self.project(x, &self.w_value))
            .collect();

        // Compute attention for each position
        let mut outputs = Vec::with_capacity(seq_len);

        for q in &queries {
            // Compute attention scores
            let mut scores: Vec<f32> = keys.iter()
                .map(|k| self.attention_score(q, k))
                .collect();

            Self::softmax(&mut scores);

            // Weighted sum of values
            let mut output = vec![0.0; self.dim];
            for (score, value) in scores.iter().zip(values.iter()) {
                for (o, v) in output.iter_mut().zip(value.iter()) {
                    *o += score * v;
                }
            }

            outputs.push(output);
        }

        outputs
    }

    /// Get output dimension
    pub fn dim(&self) -> usize {
        self.dim
    }
}

/// Multi-head attention layer
pub struct MultiHeadAttention {
    heads: Vec<AttentionHead>,
    /// Output projection weights
    w_out: Vec<f32>,
    output_dim: usize,
}

impl MultiHeadAttention {
    /// Create new multi-head attention
    pub fn new(input_dim: usize, num_heads: usize, head_dim: usize, seed: u32) -> Self {
        let heads: Vec<AttentionHead> = (0..num_heads)
            .map(|i| AttentionHead::new(input_dim, head_dim, seed.wrapping_add(i as u32 * 10)))
            .collect();

        let concat_dim = num_heads * head_dim;
        let output_dim = input_dim;
        let w_out = AttentionHead::init_weights(
            concat_dim * output_dim,
            seed.wrapping_add(1000),
            (2.0 / (concat_dim + output_dim) as f32).sqrt(),
        );

        Self {
            heads,
            w_out,
            output_dim,
        }
    }

    /// Forward pass through multi-head attention
    pub fn forward(&self, sequence: &[Vec<f32>]) -> Vec<Vec<f32>> {
        if sequence.is_empty() {
            return vec![];
        }

        // Get outputs from all heads
        let head_outputs: Vec<Vec<Vec<f32>>> = self.heads.iter()
            .map(|head| head.forward(sequence))
            .collect();

        // Concatenate and project
        let seq_len = head_outputs[0].len();
        let head_dim = if self.heads.is_empty() { 0 } else { self.heads[0].dim() };
        let concat_dim = self.heads.len() * head_dim;

        let mut outputs = Vec::with_capacity(seq_len);

        for pos in 0..seq_len {
            // Concatenate heads
            let mut concat = Vec::with_capacity(concat_dim);
            for head_out in &head_outputs {
                concat.extend_from_slice(&head_out[pos]);
            }

            // Output projection
            let mut output = vec![0.0; self.output_dim];
            for (i, o) in output.iter_mut().enumerate() {
                for (j, &c) in concat.iter().enumerate() {
                    let idx = j * self.output_dim + i;
                    if idx < self.w_out.len() {
                        *o += c * self.w_out[idx];
                    }
                }
            }

            outputs.push(output);
        }

        outputs
    }

    /// Apply attention pooling to get single output
    pub fn pool(&self, sequence: &[Vec<f32>]) -> Vec<f32> {
        let attended = self.forward(sequence);

        if attended.is_empty() {
            return vec![0.0; self.output_dim];
        }

        // Mean pooling over sequence
        let mut pooled = vec![0.0; self.output_dim];
        for item in &attended {
            for (p, v) in pooled.iter_mut().zip(item.iter()) {
                *p += v;
            }
        }

        let n = attended.len() as f32;
        for p in &mut pooled {
            *p /= n;
        }

        pooled
    }
}

/// Context-aware content ranker using attention
pub struct AttentionRanker {
    attention: MultiHeadAttention,
    /// Query transformation weights
    w_query_transform: Vec<f32>,
    dim: usize,
}

impl AttentionRanker {
    /// Create new attention-based ranker
    pub fn new(dim: usize, num_heads: usize) -> Self {
        let head_dim = dim / num_heads.max(1);
        let attention = MultiHeadAttention::new(dim, num_heads, head_dim, 54321);

        let w_query_transform = AttentionHead::init_weights(
            dim * dim,
            99999,
            (2.0 / (dim * 2) as f32).sqrt(),
        );

        Self {
            attention,
            w_query_transform,
            dim,
        }
    }

    /// Rank content items based on user context
    ///
    /// Returns indices sorted by relevance score
    pub fn rank(&self, query: &[f32], items: &[Vec<f32>]) -> Vec<(usize, f32)> {
        if items.is_empty() || query.len() != self.dim {
            return vec![];
        }

        // Transform query
        let mut transformed_query = vec![0.0; self.dim];
        for (i, tq) in transformed_query.iter_mut().enumerate() {
            for (j, &q) in query.iter().enumerate() {
                let idx = j * self.dim + i;
                if idx < self.w_query_transform.len() {
                    *tq += q * self.w_query_transform[idx];
                }
            }
        }

        // Create sequence with query prepended
        let mut sequence = vec![transformed_query.clone()];
        sequence.extend(items.iter().cloned());

        // Apply attention
        let attended = self.attention.forward(&sequence);

        // Score each item by similarity to attended query
        let query_attended = &attended[0];
        let mut scores: Vec<(usize, f32)> = attended[1..].iter()
            .enumerate()
            .map(|(i, item)| {
                let sim: f32 = query_attended.iter()
                    .zip(item.iter())
                    .map(|(q, v)| q * v)
                    .sum();
                (i, sim)
            })
            .collect();

        // Sort by score descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));

        scores
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attention_head() {
        let head = AttentionHead::new(64, 16, 12345);
        let sequence = vec![vec![0.5; 64]; 5];

        let output = head.forward(&sequence);
        assert_eq!(output.len(), 5);
        assert_eq!(output[0].len(), 16);
    }

    #[test]
    fn test_multi_head_attention() {
        let mha = MultiHeadAttention::new(64, 4, 16, 12345);
        let sequence = vec![vec![0.5; 64]; 5];

        let output = mha.forward(&sequence);
        assert_eq!(output.len(), 5);
        assert_eq!(output[0].len(), 64);
    }

    #[test]
    fn test_attention_ranker() {
        let ranker = AttentionRanker::new(64, 4);
        let query = vec![0.5; 64];
        let items = vec![vec![0.3; 64], vec![0.7; 64], vec![0.1; 64]];

        let ranked = ranker.rank(&query, &items);
        assert_eq!(ranked.len(), 3);
    }
}
