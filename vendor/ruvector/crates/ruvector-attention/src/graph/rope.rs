//! Rotary Position Embeddings (RoPE) for Graph Attention
//!
//! Adapts RoPE for graph structures where positions are defined by graph topology
//! (e.g., hop distance, shortest path length, or learned positional encodings).

use crate::error::{AttentionError, AttentionResult};
use crate::traits::Attention;
use crate::utils::stable_softmax;

/// Configuration for Graph RoPE
#[derive(Clone, Debug)]
pub struct RoPEConfig {
    pub dim: usize,
    pub base: f32,
    pub max_position: usize,
    pub scaling_factor: f32,
}

impl Default for RoPEConfig {
    fn default() -> Self {
        Self {
            dim: 256,
            base: 10000.0,
            max_position: 512,
            scaling_factor: 1.0,
        }
    }
}

impl RoPEConfig {
    pub fn builder() -> RoPEConfigBuilder {
        RoPEConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct RoPEConfigBuilder {
    config: RoPEConfig,
}

impl RoPEConfigBuilder {
    pub fn dim(mut self, d: usize) -> Self {
        self.config.dim = d;
        self
    }

    pub fn base(mut self, b: f32) -> Self {
        self.config.base = b;
        self
    }

    pub fn max_position(mut self, m: usize) -> Self {
        self.config.max_position = m;
        self
    }

    pub fn scaling_factor(mut self, s: f32) -> Self {
        self.config.scaling_factor = s;
        self
    }

    pub fn build(self) -> RoPEConfig {
        self.config
    }
}

/// Graph attention with Rotary Position Embeddings
pub struct GraphRoPE {
    config: RoPEConfig,
    /// Precomputed cos/sin tables: [max_position, dim]
    cos_cache: Vec<f32>,
    sin_cache: Vec<f32>,
    scale: f32,
}

impl GraphRoPE {
    pub fn new(config: RoPEConfig) -> Self {
        let dim = config.dim;
        let max_pos = config.max_position;
        let base = config.base;
        let scaling = config.scaling_factor;

        // Compute frequency bands
        let half_dim = dim / 2;
        let inv_freq: Vec<f32> = (0..half_dim)
            .map(|i| 1.0 / (base.powf(2.0 * i as f32 / dim as f32)))
            .collect();

        // Precompute cos/sin for all positions
        let mut cos_cache = Vec::with_capacity(max_pos * dim);
        let mut sin_cache = Vec::with_capacity(max_pos * dim);

        for pos in 0..max_pos {
            let scaled_pos = pos as f32 / scaling;
            for i in 0..half_dim {
                let theta = scaled_pos * inv_freq[i];
                cos_cache.push(theta.cos());
                sin_cache.push(theta.sin());
            }
            // Duplicate for both halves (interleaved format)
            for i in 0..half_dim {
                let theta = scaled_pos * inv_freq[i];
                cos_cache.push(theta.cos());
                sin_cache.push(theta.sin());
            }
        }

        Self {
            scale: 1.0 / (dim as f32).sqrt(),
            config,
            cos_cache,
            sin_cache,
        }
    }

    /// Apply rotary embedding to a vector at given position
    pub fn apply_rotary(&self, x: &[f32], position: usize) -> Vec<f32> {
        let dim = self.config.dim;
        let half = dim / 2;
        let pos = position.min(self.config.max_position - 1);
        let offset = pos * dim;

        let mut result = vec![0.0f32; dim];

        // Apply rotation to first half
        for i in 0..half {
            let cos = self.cos_cache[offset + i];
            let sin = self.sin_cache[offset + i];
            result[i] = x[i] * cos - x[half + i] * sin;
            result[half + i] = x[i] * sin + x[half + i] * cos;
        }

        result
    }

    /// Compute attention with positional encoding based on graph distances
    pub fn compute_with_positions(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        query_pos: usize,
        key_positions: &[usize],
    ) -> AttentionResult<Vec<f32>> {
        if keys.is_empty() {
            return Err(AttentionError::InvalidConfig("Empty keys".to_string()));
        }
        if keys.len() != key_positions.len() {
            return Err(AttentionError::InvalidConfig(
                "Keys and positions must have same length".to_string(),
            ));
        }
        if query.len() != self.config.dim {
            return Err(AttentionError::DimensionMismatch {
                expected: self.config.dim,
                actual: query.len(),
            });
        }

        // Apply rotary to query
        let q_rot = self.apply_rotary(query, query_pos);

        // Compute attention scores with rotary keys
        let scores: Vec<f32> = keys
            .iter()
            .zip(key_positions.iter())
            .map(|(key, &pos)| {
                let k_rot = self.apply_rotary(key, pos);
                q_rot
                    .iter()
                    .zip(k_rot.iter())
                    .map(|(q, k)| q * k)
                    .sum::<f32>()
                    * self.scale
            })
            .collect();

        // Softmax
        let weights = stable_softmax(&scores);

        // Weighted sum
        let value_dim = values[0].len();
        let mut output = vec![0.0f32; value_dim];
        for (w, v) in weights.iter().zip(values.iter()) {
            for (o, &vi) in output.iter_mut().zip(v.iter()) {
                *o += w * vi;
            }
        }

        Ok(output)
    }

    /// Get relative position for graph distance
    /// Converts graph hop distance to position index
    pub fn distance_to_position(distance: usize, max_distance: usize) -> usize {
        // Bucketize distances logarithmically for larger graphs
        if distance <= 8 {
            distance
        } else {
            let log_dist = (distance as f32).log2().ceil() as usize;
            8 + log_dist.min(max_distance - 8)
        }
    }
}

impl Attention for GraphRoPE {
    fn compute(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
    ) -> AttentionResult<Vec<f32>> {
        // Default: use sequential positions (0, 1, 2, ...)
        let query_pos = 0;
        let key_positions: Vec<usize> = (0..keys.len()).collect();
        self.compute_with_positions(query, keys, values, query_pos, &key_positions)
    }

    fn compute_with_mask(
        &self,
        query: &[f32],
        keys: &[&[f32]],
        values: &[&[f32]],
        mask: Option<&[bool]>,
    ) -> AttentionResult<Vec<f32>> {
        if let Some(m) = mask {
            let filtered: Vec<(usize, bool)> = m
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, keep)| *keep)
                .collect();
            let filtered_keys: Vec<&[f32]> = filtered.iter().map(|(i, _)| keys[*i]).collect();
            let filtered_values: Vec<&[f32]> = filtered.iter().map(|(i, _)| values[*i]).collect();
            self.compute(query, &filtered_keys, &filtered_values)
        } else {
            self.compute(query, keys, values)
        }
    }

    fn dim(&self) -> usize {
        self.config.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rope_basic() {
        let config = RoPEConfig::builder().dim(64).max_position(100).build();

        let rope = GraphRoPE::new(config);

        let query = vec![0.5; 64];
        let keys: Vec<Vec<f32>> = (0..10).map(|_| vec![0.3; 64]).collect();
        let values: Vec<Vec<f32>> = (0..10).map(|_| vec![1.0; 64]).collect();

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        let result = rope.compute(&query, &keys_refs, &values_refs).unwrap();
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn test_rope_with_positions() {
        let config = RoPEConfig::builder().dim(32).max_position(50).build();

        let rope = GraphRoPE::new(config);

        let query = vec![0.5; 32];
        let keys: Vec<Vec<f32>> = vec![vec![0.3; 32]; 5];
        let values: Vec<Vec<f32>> = vec![vec![1.0; 32]; 5];

        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
        let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

        // Graph distances as positions
        let key_positions = vec![1, 2, 3, 2, 4];

        let result = rope
            .compute_with_positions(&query, &keys_refs, &values_refs, 0, &key_positions)
            .unwrap();
        assert_eq!(result.len(), 32);
    }

    #[test]
    fn test_rotary_embedding() {
        let config = RoPEConfig::builder().dim(16).max_position(10).build();

        let rope = GraphRoPE::new(config);

        let x = vec![1.0; 16];

        // Rotary should preserve norm approximately
        let rotated = rope.apply_rotary(&x, 5);
        let norm_orig: f32 = x.iter().map(|v| v * v).sum::<f32>().sqrt();
        let norm_rot: f32 = rotated.iter().map(|v| v * v).sum::<f32>().sqrt();

        assert!((norm_orig - norm_rot).abs() < 1e-5);
    }

    #[test]
    fn test_distance_to_position() {
        // Direct mapping for small distances
        assert_eq!(GraphRoPE::distance_to_position(0, 20), 0);
        assert_eq!(GraphRoPE::distance_to_position(5, 20), 5);
        assert_eq!(GraphRoPE::distance_to_position(8, 20), 8);

        // Logarithmic for larger distances
        let pos_16 = GraphRoPE::distance_to_position(16, 20);
        let pos_32 = GraphRoPE::distance_to_position(32, 20);
        assert!(pos_16 > 8);
        assert!(pos_32 > pos_16);
    }
}
