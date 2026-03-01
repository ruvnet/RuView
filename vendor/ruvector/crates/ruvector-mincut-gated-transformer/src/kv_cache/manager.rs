//! Adaptive KV Cache Manager
//!
//! Orchestrates tier transitions between Hot, Warm, and Archive tiers.
//! Provides the primary user-facing API for the three-tier KV cache system.

#[cfg(feature = "no_std_gateway")]
use alloc::vec::Vec;

#[cfg(not(feature = "no_std_gateway"))]
use std::vec::Vec;

use super::hot_buffer::{HotBuffer, HotBufferConfig};
use super::kvquant::KVQuantQuantizer;
use super::metrics::{MemoryStats, QualityFeedback, QualityMetric, QualityTracker};
use super::policy::{EvictionDecision, RematerializationPolicy, TierPolicy};
use super::quantized_store::{QuantizedStore, QuantizedStoreConfig};
use super::squat::SQuatQuantizer;
use super::tier::{TierBoundary, TierCounts};

/// Archive tier quantizer selection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveQuantizer {
    /// Standard 2-bit KIVI
    Kivi2Bit,
    /// SQuat for extreme contexts (additional 2.2-2.8x compression)
    SQuat { num_subspaces: usize },
    /// KVQuant for quality-critical applications (pre-RoPE)
    KVQuant { bits: u8 },
    /// Adaptive: choose based on context length and quality metrics
    Adaptive,
}

impl Default for ArchiveQuantizer {
    fn default() -> Self {
        ArchiveQuantizer::Kivi2Bit
    }
}

/// Configuration for the adaptive KV cache
#[derive(Clone, Debug)]
pub struct AdaptiveKVCacheConfig {
    /// Number of transformer layers
    pub num_layers: usize,
    /// Number of attention heads per layer
    pub num_heads: usize,
    /// Dimension per head
    pub head_dim: usize,
    /// Maximum sequence length
    pub max_seq_len: usize,
    /// Number of tokens to keep in hot buffer (FP16)
    pub tail_length: usize,
    /// Number of tokens in warm zone (4-bit KIVI)
    pub warm_length: usize,
    /// Archive tier quantizer selection
    pub archive_quantizer: ArchiveQuantizer,
    /// Quality target (1.0 - expected PPL degradation)
    pub quality_target: f32,
    /// Enable rematerialization for extreme memory pressure
    pub enable_rematerialization: bool,
}

impl Default for AdaptiveKVCacheConfig {
    fn default() -> Self {
        Self {
            num_layers: 12,
            num_heads: 8,
            head_dim: 64,
            max_seq_len: 4096,
            tail_length: 64,
            warm_length: 448,
            archive_quantizer: ArchiveQuantizer::Kivi2Bit,
            quality_target: 0.97,
            enable_rematerialization: false,
        }
    }
}

impl AdaptiveKVCacheConfig {
    /// Configuration for small models
    pub fn small() -> Self {
        Self {
            num_layers: 6,
            num_heads: 4,
            head_dim: 64,
            max_seq_len: 2048,
            tail_length: 32,
            warm_length: 224,
            archive_quantizer: ArchiveQuantizer::Kivi2Bit,
            quality_target: 0.97,
            enable_rematerialization: false,
        }
    }

    /// Configuration for large models with long context
    pub fn large_context() -> Self {
        Self {
            num_layers: 32,
            num_heads: 32,
            head_dim: 128,
            max_seq_len: 32768,
            tail_length: 128,
            warm_length: 896,
            archive_quantizer: ArchiveQuantizer::SQuat { num_subspaces: 4 },
            quality_target: 0.95,
            enable_rematerialization: true,
        }
    }

    /// Configuration for extreme contexts (100K+ tokens)
    pub fn extreme_context() -> Self {
        Self {
            num_layers: 80,
            num_heads: 64,
            head_dim: 128,
            max_seq_len: 131072,
            tail_length: 256,
            warm_length: 1792,
            archive_quantizer: ArchiveQuantizer::KVQuant { bits: 3 },
            quality_target: 0.97,
            enable_rematerialization: true,
        }
    }

    /// Estimate memory usage in bytes
    pub fn estimate_memory(&self) -> usize {
        // Hot buffer: FP16
        let hot_bytes = self.num_layers * self.num_heads * self.head_dim * self.tail_length * 2 * 2; // 2 bytes * 2 (kv)

        // Warm: 4-bit
        let warm_bytes =
            self.num_layers * self.num_heads * self.head_dim * self.warm_length / 2 * 2; // 0.5 bytes * 2 (kv)

        // Archive: varies by quantizer
        let archive_len = self
            .max_seq_len
            .saturating_sub(self.tail_length + self.warm_length);
        let archive_bytes_per_element = match self.archive_quantizer {
            ArchiveQuantizer::Kivi2Bit => 0.25,
            ArchiveQuantizer::SQuat { .. } => 0.1,
            ArchiveQuantizer::KVQuant { bits } => bits as f64 / 8.0,
            ArchiveQuantizer::Adaptive => 0.25,
        };
        let archive_bytes = (self.num_layers * self.num_heads * self.head_dim * archive_len) as f64
            * archive_bytes_per_element
            * 2.0;

        hot_bytes + warm_bytes + archive_bytes as usize
    }
}

/// Adaptive KV Cache with three-tier management
pub struct AdaptiveKVCache {
    /// Configuration
    config: AdaptiveKVCacheConfig,

    /// Hot buffer (Tier 1: FP16)
    hot_buffer: HotBuffer,

    /// Quantized store (Tier 2 + 3)
    quantized_store: QuantizedStore,

    /// Tier policy for transitions
    tier_policy: TierPolicy,

    /// Rematerialization policy (optional)
    remat_policy: Option<RematerializationPolicy>,

    /// Quality tracker
    quality_tracker: QualityTracker,

    /// SQuat quantizer (lazily initialized, reserved for future archive tier optimization)
    #[allow(dead_code)]
    squat_quantizer: Option<SQuatQuantizer>,

    /// KVQuant quantizer (lazily initialized, reserved for future archive tier optimization)
    #[allow(dead_code)]
    kvquant_quantizer: Option<KVQuantQuantizer>,

    /// Current sequence length per layer
    seq_len: Vec<usize>,
}

impl AdaptiveKVCache {
    /// Create a new adaptive KV cache
    pub fn new(config: AdaptiveKVCacheConfig) -> Self {
        let hot_config = HotBufferConfig::new(
            config.num_layers,
            config.num_heads,
            config.head_dim,
            config.tail_length,
        );

        let store_config = QuantizedStoreConfig {
            num_layers: config.num_layers,
            num_heads: config.num_heads,
            head_dim: config.head_dim,
            warm_capacity: config.warm_length,
            archive_capacity: config
                .max_seq_len
                .saturating_sub(config.tail_length + config.warm_length),
            warm_bits: 4,
            archive_bits: 2,
        };

        let tier_boundary =
            TierBoundary::new(config.tail_length, config.tail_length + config.warm_length);
        let tier_policy = TierPolicy::new(tier_boundary, config.quality_target);

        let remat_policy = if config.enable_rematerialization {
            Some(RematerializationPolicy::new(0.9, 512))
        } else {
            None
        };

        Self {
            config: config.clone(),
            hot_buffer: HotBuffer::new(hot_config),
            quantized_store: QuantizedStore::new(store_config),
            tier_policy,
            remat_policy,
            quality_tracker: QualityTracker::new(config.quality_target),
            squat_quantizer: None,
            kvquant_quantizer: None,
            seq_len: vec![0; config.num_layers],
        }
    }

    /// Append a new KV pair to the cache
    ///
    /// Automatically handles tier transitions:
    /// 1. New tokens go to hot buffer
    /// 2. When hot buffer is full, oldest graduates to warm
    /// 3. When warm is full, oldest graduates to archive
    pub fn append(&mut self, layer: usize, key: &[f32], value: &[f32]) {
        assert!(layer < self.config.num_layers);
        assert_eq!(key.len(), self.config.head_dim * self.config.num_heads);
        assert_eq!(value.len(), self.config.head_dim * self.config.num_heads);

        // Step 1: Try to push to hot buffer
        let evicted = self.hot_buffer.push(layer, key, value);

        // Step 2: If hot buffer was full, graduate to warm
        if let Some((old_key, old_value)) = evicted {
            // Check if warm is full
            if self.quantized_store.warm_is_full(layer) {
                // Graduate oldest warm to archive
                self.quantized_store.graduate_to_archive(layer, 1);
            }

            // Push to warm tier
            for head in 0..self.config.num_heads {
                let head_offset = head * self.config.head_dim;
                let k = &old_key[head_offset..head_offset + self.config.head_dim];
                let v = &old_value[head_offset..head_offset + self.config.head_dim];
                self.quantized_store.push_warm(layer, head, k, v);
            }
        }

        self.seq_len[layer] += 1;
    }

    /// Compute attention with tiered cache
    ///
    /// Returns attention output: [num_heads * head_dim]
    pub fn attention(&self, layer: usize, query: &[f32], scale: f32) -> Vec<f32> {
        assert!(layer < self.config.num_layers);
        assert_eq!(query.len(), self.config.head_dim * self.config.num_heads);

        let mut output = vec![0.0f32; self.config.head_dim * self.config.num_heads];

        for head in 0..self.config.num_heads {
            let head_offset = head * self.config.head_dim;
            let q = &query[head_offset..head_offset + self.config.head_dim];

            // Gather keys and values from all tiers
            let mut all_keys: Vec<f32> = Vec::new();
            let mut all_values: Vec<f32> = Vec::new();

            // 1. Archive tier (oldest)
            let archive_keys = self.quantized_store.dequantize_archive_keys(layer, head);
            let archive_values = self.quantized_store.dequantize_archive_values(layer, head);
            all_keys.extend_from_slice(&archive_keys);
            all_values.extend_from_slice(&archive_values);

            // 2. Warm tier
            let warm_keys = self.quantized_store.dequantize_warm_keys(layer, head);
            let warm_values = self.quantized_store.dequantize_warm_values(layer, head);
            all_keys.extend_from_slice(&warm_keys);
            all_values.extend_from_slice(&warm_values);

            // 3. Hot tier (most recent)
            let hot_keys = self.hot_buffer.keys(layer, head);
            let hot_values = self.hot_buffer.values(layer, head);
            all_keys.extend_from_slice(&hot_keys);
            all_values.extend_from_slice(&hot_values);

            // Compute attention
            let num_tokens = all_keys.len() / self.config.head_dim;
            if num_tokens == 0 {
                continue;
            }

            // Compute attention scores
            let mut scores = vec![0.0f32; num_tokens];
            for t in 0..num_tokens {
                let k_offset = t * self.config.head_dim;
                let k = &all_keys[k_offset..k_offset + self.config.head_dim];

                // Dot product
                let mut dot = 0.0f32;
                for d in 0..self.config.head_dim {
                    dot += q[d] * k[d];
                }
                scores[t] = dot * scale;
            }

            // Softmax
            let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let mut sum_exp = 0.0f32;
            for score in scores.iter_mut() {
                *score = (*score - max_score).exp();
                sum_exp += *score;
            }
            for score in scores.iter_mut() {
                *score /= sum_exp;
            }

            // Weighted sum of values
            let out = &mut output[head_offset..head_offset + self.config.head_dim];
            for t in 0..num_tokens {
                let v_offset = t * self.config.head_dim;
                let v = &all_values[v_offset..v_offset + self.config.head_dim];
                for d in 0..self.config.head_dim {
                    out[d] += scores[t] * v[d];
                }
            }
        }

        output
    }

    /// Get current memory usage
    pub fn memory_usage(&self) -> MemoryStats {
        let hot_bytes = self.hot_buffer.memory_bytes();
        let quantized_bytes = self.quantized_store.memory_bytes();

        MemoryStats {
            hot_bytes,
            warm_bytes: quantized_bytes / 2, // Approximate split
            archive_bytes: quantized_bytes / 2,
            total_bytes: hot_bytes + quantized_bytes,
            compression_ratio: self.compression_ratio(),
        }
    }

    /// Get quality metrics
    pub fn quality_metrics(&self) -> QualityMetric {
        self.quality_tracker.current_metrics()
    }

    /// Adapt tier boundaries based on quality feedback
    pub fn adapt_thresholds(&mut self, feedback: QualityFeedback) {
        self.quality_tracker.record(feedback.clone());

        // If quality is degrading, expand hot buffer
        if feedback.score < self.config.quality_target {
            self.tier_policy.expand_hot_boundary(1.1);
        } else if feedback.score > self.config.quality_target * 1.05 {
            // Quality is good, can be more aggressive
            self.tier_policy.shrink_hot_boundary(0.95);
        }
    }

    /// Flush all pending data
    pub fn flush(&mut self) {
        // Force all warm to archive
        for layer in 0..self.config.num_layers {
            let warm_len = self.quantized_store.warm_len(layer);
            if warm_len > 0 {
                self.quantized_store.graduate_to_archive(layer, warm_len);
            }
        }
    }

    /// Reset cache for a specific layer
    pub fn reset_layer(&mut self, layer: usize) {
        self.hot_buffer.reset_layer(layer);
        self.quantized_store.reset_layer(layer);
        self.seq_len[layer] = 0;
    }

    /// Reset entire cache
    pub fn reset(&mut self) {
        self.hot_buffer.reset();
        self.quantized_store.reset();
        self.quality_tracker.reset();
        for len in self.seq_len.iter_mut() {
            *len = 0;
        }
    }

    /// Get tier counts for a layer
    pub fn tier_counts(&self, layer: usize) -> TierCounts {
        TierCounts {
            hot: self.hot_buffer.len(layer),
            warm: self.quantized_store.warm_len(layer),
            archive: self.quantized_store.archive_len(layer),
        }
    }

    /// Get current sequence length for a layer
    pub fn seq_len(&self, layer: usize) -> usize {
        self.seq_len[layer]
    }

    /// Get compression ratio compared to FP32
    pub fn compression_ratio(&self) -> f32 {
        let tier_counts = self.tier_counts(0); // Use layer 0 as representative
        let fp32_bytes = tier_counts.total() * self.config.head_dim * 4 * 2; // 4 bytes * 2 (kv)

        let actual_bytes = tier_counts.memory_bytes(
            self.config.head_dim,
            self.config.num_heads,
            self.config.num_layers,
        );

        if actual_bytes == 0 {
            1.0
        } else {
            fp32_bytes as f32 / actual_bytes as f32
        }
    }

    /// Get configuration
    pub fn config(&self) -> &AdaptiveKVCacheConfig {
        &self.config
    }

    /// Check if rematerialization should be triggered
    pub fn should_rematerialize(&self) -> Option<EvictionDecision> {
        if let Some(ref policy) = self.remat_policy {
            let memory_usage = self.memory_usage();
            policy.evaluate(memory_usage.total_bytes)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_cache_config() {
        let config = AdaptiveKVCacheConfig::default();
        assert_eq!(config.num_layers, 12);
        assert_eq!(config.tail_length, 64);
        assert_eq!(config.warm_length, 448);
    }

    #[test]
    fn test_adaptive_cache_new() {
        let config = AdaptiveKVCacheConfig {
            num_layers: 2,
            num_heads: 2,
            head_dim: 8,
            max_seq_len: 32,
            tail_length: 4,
            warm_length: 8,
            archive_quantizer: ArchiveQuantizer::Kivi2Bit,
            quality_target: 0.95,
            enable_rematerialization: false,
        };

        let cache = AdaptiveKVCache::new(config);
        assert_eq!(cache.seq_len(0), 0);
        assert_eq!(cache.seq_len(1), 0);
    }

    #[test]
    fn test_adaptive_cache_append() {
        let config = AdaptiveKVCacheConfig {
            num_layers: 1,
            num_heads: 1,
            head_dim: 8,
            max_seq_len: 16,
            tail_length: 4,
            warm_length: 4,
            archive_quantizer: ArchiveQuantizer::Kivi2Bit,
            quality_target: 0.95,
            enable_rematerialization: false,
        };

        let mut cache = AdaptiveKVCache::new(config);

        for i in 0..8 {
            let key: Vec<f32> = (0..8).map(|j| (i * 8 + j) as f32).collect();
            let value: Vec<f32> = (0..8).map(|j| (i * 8 + j + 100) as f32).collect();
            cache.append(0, &key, &value);
        }

        assert_eq!(cache.seq_len(0), 8);
        let counts = cache.tier_counts(0);
        assert_eq!(counts.hot, 4); // tail_length
        assert!(counts.warm > 0 || counts.archive > 0);
    }

    #[test]
    fn test_adaptive_cache_attention() {
        let config = AdaptiveKVCacheConfig {
            num_layers: 1,
            num_heads: 1,
            head_dim: 8,
            max_seq_len: 16,
            tail_length: 4,
            warm_length: 4,
            archive_quantizer: ArchiveQuantizer::Kivi2Bit,
            quality_target: 0.95,
            enable_rematerialization: false,
        };

        let mut cache = AdaptiveKVCache::new(config);

        // Add some entries
        for i in 0..4 {
            let key: Vec<f32> = (0..8).map(|j| (i * 8 + j) as f32 * 0.1).collect();
            let value: Vec<f32> = (0..8).map(|j| (i * 8 + j + 100) as f32 * 0.1).collect();
            cache.append(0, &key, &value);
        }

        // Query
        let query = vec![1.0f32; 8];
        let scale = 1.0 / (8.0f32).sqrt();
        let output = cache.attention(0, &query, scale);

        assert_eq!(output.len(), 8);
    }

    #[test]
    fn test_adaptive_cache_memory_usage() {
        let config = AdaptiveKVCacheConfig::default();
        let cache = AdaptiveKVCache::new(config);

        let stats = cache.memory_usage();
        assert!(stats.total_bytes > 0);
    }

    #[test]
    fn test_adaptive_cache_reset() {
        let config = AdaptiveKVCacheConfig {
            num_layers: 2,
            num_heads: 1,
            head_dim: 8,
            max_seq_len: 16,
            tail_length: 4,
            warm_length: 4,
            archive_quantizer: ArchiveQuantizer::Kivi2Bit,
            quality_target: 0.95,
            enable_rematerialization: false,
        };

        let mut cache = AdaptiveKVCache::new(config);

        // Add entries to both layers
        let key = vec![1.0f32; 8];
        let value = vec![2.0f32; 8];
        cache.append(0, &key, &value);
        cache.append(1, &key, &value);

        assert_eq!(cache.seq_len(0), 1);
        assert_eq!(cache.seq_len(1), 1);

        cache.reset_layer(0);
        assert_eq!(cache.seq_len(0), 0);
        assert_eq!(cache.seq_len(1), 1);

        cache.reset();
        assert_eq!(cache.seq_len(0), 0);
        assert_eq!(cache.seq_len(1), 0);
    }

    #[test]
    fn test_archive_quantizer_selection() {
        let kivi = ArchiveQuantizer::Kivi2Bit;
        let squat = ArchiveQuantizer::SQuat { num_subspaces: 4 };
        let kvquant = ArchiveQuantizer::KVQuant { bits: 3 };
        let adaptive = ArchiveQuantizer::Adaptive;

        assert_eq!(kivi, ArchiveQuantizer::Kivi2Bit);
        assert_ne!(squat, kivi);
        assert_ne!(kvquant, kivi);
        assert_ne!(adaptive, kivi);
    }
}
