//! Embedding Sharding - Distribute Vocabulary Across Chips
//!
//! For large vocabularies, shard embeddings across chips.
//! Each chip holds a portion of the embedding table.

use heapless::Vec as HVec;
use super::protocol::ChipId;

/// Sharding configuration
#[derive(Debug, Clone)]
pub struct ShardConfig {
    /// Total vocabulary size
    pub vocab_size: usize,
    /// Number of shards (chips)
    pub num_shards: usize,
    /// This chip's shard ID
    pub shard_id: usize,
    /// Embedding dimension
    pub embed_dim: usize,
    /// Vocab range for this shard
    pub vocab_start: usize,
    pub vocab_end: usize,
}

impl ShardConfig {
    /// Create config for a specific shard
    pub fn for_shard(
        shard_id: usize,
        num_shards: usize,
        vocab_size: usize,
        embed_dim: usize,
    ) -> Self {
        let vocab_per_shard = (vocab_size + num_shards - 1) / num_shards;
        let vocab_start = shard_id * vocab_per_shard;
        let vocab_end = (vocab_start + vocab_per_shard).min(vocab_size);

        Self {
            vocab_size,
            num_shards,
            shard_id,
            embed_dim,
            vocab_start,
            vocab_end,
        }
    }

    /// Check if this shard handles a token
    pub fn handles_token(&self, token_id: u16) -> bool {
        let t = token_id as usize;
        t >= self.vocab_start && t < self.vocab_end
    }

    /// Get shard that handles a token
    pub fn shard_for_token(token_id: u16, num_shards: usize, vocab_size: usize) -> usize {
        let vocab_per_shard = (vocab_size + num_shards - 1) / num_shards;
        (token_id as usize) / vocab_per_shard
    }

    /// Vocab size for this shard
    pub fn shard_vocab_size(&self) -> usize {
        self.vocab_end - self.vocab_start
    }
}

/// Sharded embedding table
pub struct ShardedEmbedding<const MAX_VOCAB: usize, const DIM: usize> {
    config: ShardConfig,
    /// Local embedding weights (only our shard)
    weights: HVec<i8, 8192>, // Max 8KB per shard
}

impl<const MAX_VOCAB: usize, const DIM: usize> ShardedEmbedding<MAX_VOCAB, DIM> {
    /// Create sharded embedding
    pub fn new(config: ShardConfig, seed: u32) -> crate::Result<Self> {
        let shard_size = config.shard_vocab_size() * config.embed_dim;

        let mut weights = HVec::new();
        let mut rng_state = seed.wrapping_add(config.shard_id as u32 * 12345);

        for _ in 0..shard_size {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let val = (((rng_state >> 16) & 0xFF) as i16 - 128) as i8;
            weights.push(val).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(Self { config, weights })
    }

    /// Lookup embedding (only works if we have the token)
    pub fn lookup(&self, token_id: u16, output: &mut [i8]) -> crate::Result<bool> {
        if !self.config.handles_token(token_id) {
            return Ok(false);
        }

        let local_idx = token_id as usize - self.config.vocab_start;
        let start = local_idx * self.config.embed_dim;
        let end = start + self.config.embed_dim;

        if end > self.weights.len() || output.len() < self.config.embed_dim {
            return Err(crate::Error::BufferOverflow);
        }

        output[..self.config.embed_dim].copy_from_slice(&self.weights[start..end]);
        Ok(true)
    }

    /// Memory per shard vs full embedding
    pub fn memory_saved(&self) -> f32 {
        self.config.num_shards as f32
    }

    /// Get responsible chip for a token
    pub fn responsible_chip(&self, token_id: u16) -> ChipId {
        let shard = ShardConfig::shard_for_token(
            token_id,
            self.config.num_shards,
            self.config.vocab_size,
        );
        ChipId(shard as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sharding() {
        // 1000 vocab, 5 shards
        let config = ShardConfig::for_shard(2, 5, 1000, 32);

        assert_eq!(config.vocab_start, 400);
        assert_eq!(config.vocab_end, 600);
        assert!(config.handles_token(450));
        assert!(!config.handles_token(300));
    }

    #[test]
    fn test_shard_lookup() {
        let shard = ShardConfig::shard_for_token(450, 5, 1000);
        assert_eq!(shard, 2);
    }
}
