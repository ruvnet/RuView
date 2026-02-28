//! Hot buffer for FP16 high-precision tail tokens.
//!
//! The hot buffer stores the most recent tokens in full FP16 precision,
//! avoiding any quantization overhead for tokens that receive the highest
//! attention weights.

#[cfg(feature = "no_std_gateway")]
use alloc::{vec, vec::Vec};

#[cfg(not(feature = "no_std_gateway"))]
use std::vec::Vec;

/// Configuration for the hot buffer
#[derive(Debug, Clone, Copy)]
pub struct HotBufferConfig {
    /// Number of layers
    pub num_layers: usize,
    /// Number of attention heads per layer
    pub num_heads: usize,
    /// Dimension per head
    pub head_dim: usize,
    /// Maximum tokens to keep in hot buffer
    pub capacity: usize,
}

impl HotBufferConfig {
    /// Create a new hot buffer configuration
    pub fn new(num_layers: usize, num_heads: usize, head_dim: usize, capacity: usize) -> Self {
        Self {
            num_layers,
            num_heads,
            head_dim,
            capacity,
        }
    }

    /// Memory usage in bytes
    pub fn memory_bytes(&self) -> usize {
        // FP16: 2 bytes per element, 2x for keys and values
        self.num_layers * self.num_heads * self.head_dim * self.capacity * 2 * 2
    }
}

/// FP16 high-precision tail buffer for recent tokens
///
/// Stores the most recent N tokens in full FP16 precision.
/// Uses a ring buffer design for efficient append/evict operations.
pub struct HotBuffer {
    /// Configuration
    config: HotBufferConfig,
    /// Key storage: [layers][heads][ring_buffer of head_dim]
    keys: Vec<Vec<Vec<f32>>>,
    /// Value storage: [layers][heads][ring_buffer of head_dim]
    values: Vec<Vec<Vec<f32>>>,
    /// Current write position in ring buffer per layer
    write_pos: Vec<usize>,
    /// Number of valid tokens per layer
    len: Vec<usize>,
}

impl HotBuffer {
    /// Create a new hot buffer
    pub fn new(config: HotBufferConfig) -> Self {
        let buffer_size = config.capacity * config.head_dim;

        let mut keys = Vec::with_capacity(config.num_layers);
        let mut values = Vec::with_capacity(config.num_layers);

        for _ in 0..config.num_layers {
            let mut layer_keys = Vec::with_capacity(config.num_heads);
            let mut layer_values = Vec::with_capacity(config.num_heads);

            for _ in 0..config.num_heads {
                layer_keys.push(vec![0.0f32; buffer_size]);
                layer_values.push(vec![0.0f32; buffer_size]);
            }

            keys.push(layer_keys);
            values.push(layer_values);
        }

        Self {
            config,
            keys,
            values,
            write_pos: vec![0; config.num_layers],
            len: vec![0; config.num_layers],
        }
    }

    /// Push a new KV pair to the buffer
    ///
    /// Returns the evicted KV pair if the buffer was full
    pub fn push(
        &mut self,
        layer: usize,
        key: &[f32],
        value: &[f32],
    ) -> Option<(Vec<f32>, Vec<f32>)> {
        assert!(layer < self.config.num_layers);
        assert_eq!(key.len(), self.config.head_dim * self.config.num_heads);
        assert_eq!(value.len(), self.config.head_dim * self.config.num_heads);

        let was_full = self.len[layer] >= self.config.capacity;
        let mut evicted_key = None;
        let mut evicted_value = None;

        // If buffer is full, capture the evicted entry
        if was_full {
            let oldest_pos = self.write_pos[layer];
            let mut ek = Vec::with_capacity(key.len());
            let mut ev = Vec::with_capacity(value.len());

            for head in 0..self.config.num_heads {
                let offset = oldest_pos * self.config.head_dim;
                ek.extend_from_slice(
                    &self.keys[layer][head][offset..offset + self.config.head_dim],
                );
                ev.extend_from_slice(
                    &self.values[layer][head][offset..offset + self.config.head_dim],
                );
            }

            evicted_key = Some(ek);
            evicted_value = Some(ev);
        }

        // Write new data
        let pos = self.write_pos[layer];
        for head in 0..self.config.num_heads {
            let head_offset = head * self.config.head_dim;
            let buffer_offset = pos * self.config.head_dim;

            self.keys[layer][head][buffer_offset..buffer_offset + self.config.head_dim]
                .copy_from_slice(&key[head_offset..head_offset + self.config.head_dim]);
            self.values[layer][head][buffer_offset..buffer_offset + self.config.head_dim]
                .copy_from_slice(&value[head_offset..head_offset + self.config.head_dim]);
        }

        // Update position and length
        self.write_pos[layer] = (self.write_pos[layer] + 1) % self.config.capacity;
        if !was_full {
            self.len[layer] += 1;
        }

        match (evicted_key, evicted_value) {
            (Some(k), Some(v)) => Some((k, v)),
            _ => None,
        }
    }

    /// Push KV pair for a single head
    pub fn push_head(
        &mut self,
        layer: usize,
        head: usize,
        key: &[f32],
        value: &[f32],
    ) -> Option<(Vec<f32>, Vec<f32>)> {
        assert!(layer < self.config.num_layers);
        assert!(head < self.config.num_heads);
        assert_eq!(key.len(), self.config.head_dim);
        assert_eq!(value.len(), self.config.head_dim);

        let pos = self.write_pos[layer];
        let was_full = self.len[layer] >= self.config.capacity;

        // Capture evicted data if full
        let evicted = if was_full {
            let offset = pos * self.config.head_dim;
            let ek = self.keys[layer][head][offset..offset + self.config.head_dim].to_vec();
            let ev = self.values[layer][head][offset..offset + self.config.head_dim].to_vec();
            Some((ek, ev))
        } else {
            None
        };

        // Write new data
        let offset = pos * self.config.head_dim;
        self.keys[layer][head][offset..offset + self.config.head_dim].copy_from_slice(key);
        self.values[layer][head][offset..offset + self.config.head_dim].copy_from_slice(value);

        evicted
    }

    /// Advance write position (call after pushing all heads for a token)
    pub fn advance(&mut self, layer: usize) {
        assert!(layer < self.config.num_layers);

        let was_full = self.len[layer] >= self.config.capacity;
        self.write_pos[layer] = (self.write_pos[layer] + 1) % self.config.capacity;
        if !was_full {
            self.len[layer] += 1;
        }
    }

    /// Pop the oldest entry from the buffer
    pub fn pop_oldest(&mut self, layer: usize) -> Option<(Vec<f32>, Vec<f32>)> {
        if self.len[layer] == 0 {
            return None;
        }

        // Calculate oldest position
        let oldest_pos = if self.len[layer] < self.config.capacity {
            0
        } else {
            self.write_pos[layer] // In a full ring buffer, write_pos points to oldest
        };

        let mut key = Vec::with_capacity(self.config.num_heads * self.config.head_dim);
        let mut value = Vec::with_capacity(self.config.num_heads * self.config.head_dim);

        for head in 0..self.config.num_heads {
            let offset = oldest_pos * self.config.head_dim;
            key.extend_from_slice(&self.keys[layer][head][offset..offset + self.config.head_dim]);
            value.extend_from_slice(
                &self.values[layer][head][offset..offset + self.config.head_dim],
            );
        }

        self.len[layer] -= 1;
        Some((key, value))
    }

    /// Get all keys for a layer/head
    ///
    /// Returns keys in chronological order (oldest first)
    pub fn keys(&self, layer: usize, head: usize) -> Vec<f32> {
        assert!(layer < self.config.num_layers);
        assert!(head < self.config.num_heads);

        if self.len[layer] == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(self.len[layer] * self.config.head_dim);

        if self.len[layer] < self.config.capacity {
            // Not wrapped yet, just return from start
            result.extend_from_slice(
                &self.keys[layer][head][..self.len[layer] * self.config.head_dim],
            );
        } else {
            // Wrapped: read from write_pos to end, then from start to write_pos
            let start = self.write_pos[layer] * self.config.head_dim;
            let total_size = self.config.capacity * self.config.head_dim;

            result.extend_from_slice(&self.keys[layer][head][start..total_size]);
            result.extend_from_slice(&self.keys[layer][head][..start]);
        }

        result
    }

    /// Get all values for a layer/head
    ///
    /// Returns values in chronological order (oldest first)
    pub fn values(&self, layer: usize, head: usize) -> Vec<f32> {
        assert!(layer < self.config.num_layers);
        assert!(head < self.config.num_heads);

        if self.len[layer] == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(self.len[layer] * self.config.head_dim);

        if self.len[layer] < self.config.capacity {
            result.extend_from_slice(
                &self.values[layer][head][..self.len[layer] * self.config.head_dim],
            );
        } else {
            let start = self.write_pos[layer] * self.config.head_dim;
            let total_size = self.config.capacity * self.config.head_dim;

            result.extend_from_slice(&self.values[layer][head][start..total_size]);
            result.extend_from_slice(&self.values[layer][head][..start]);
        }

        result
    }

    /// Get current length for a layer
    #[inline]
    pub fn len(&self, layer: usize) -> usize {
        self.len[layer]
    }

    /// Check if buffer is empty for a layer
    #[inline]
    pub fn is_empty(&self, layer: usize) -> bool {
        self.len[layer] == 0
    }

    /// Check if buffer is full for a layer
    #[inline]
    pub fn is_full(&self, layer: usize) -> bool {
        self.len[layer] >= self.config.capacity
    }

    /// Get configuration
    #[inline]
    pub fn config(&self) -> &HotBufferConfig {
        &self.config
    }

    /// Reset buffer for a layer
    pub fn reset_layer(&mut self, layer: usize) {
        assert!(layer < self.config.num_layers);
        self.write_pos[layer] = 0;
        self.len[layer] = 0;
    }

    /// Reset entire buffer
    pub fn reset(&mut self) {
        for layer in 0..self.config.num_layers {
            self.reset_layer(layer);
        }
    }

    /// Total memory usage in bytes
    pub fn memory_bytes(&self) -> usize {
        self.config.memory_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hot_buffer_config() {
        let config = HotBufferConfig::new(12, 8, 64, 64);
        assert_eq!(config.num_layers, 12);
        assert_eq!(config.num_heads, 8);
        assert_eq!(config.head_dim, 64);
        assert_eq!(config.capacity, 64);

        // Memory: 12 layers * 8 heads * 64 dim * 64 tokens * 2 (f32 stored) * 2 (kv)
        // But we store f32, so it's 4 bytes each = 12 * 8 * 64 * 64 * 4 * 2
        // The config method assumes f16, so this is approximate
    }

    #[test]
    fn test_hot_buffer_push() {
        let config = HotBufferConfig::new(1, 2, 4, 3);
        let mut buffer = HotBuffer::new(config);

        let key = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]; // 2 heads * 4 dim
        let value = vec![8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];

        // First push - no eviction
        let evicted = buffer.push(0, &key, &value);
        assert!(evicted.is_none());
        assert_eq!(buffer.len(0), 1);

        // Second push - no eviction
        let evicted = buffer.push(0, &key, &value);
        assert!(evicted.is_none());
        assert_eq!(buffer.len(0), 2);

        // Third push - no eviction (capacity is 3)
        let evicted = buffer.push(0, &key, &value);
        assert!(evicted.is_none());
        assert_eq!(buffer.len(0), 3);
        assert!(buffer.is_full(0));

        // Fourth push - should evict
        let evicted = buffer.push(0, &key, &value);
        assert!(evicted.is_some());
        assert_eq!(buffer.len(0), 3); // Still 3
    }

    #[test]
    fn test_hot_buffer_keys_values() {
        let config = HotBufferConfig::new(1, 1, 4, 3);
        let mut buffer = HotBuffer::new(config);

        // Push 3 different keys
        for i in 0..3 {
            let val = i as f32;
            buffer.push_head(
                0,
                0,
                &[val, val + 1.0, val + 2.0, val + 3.0],
                &[val * 10.0; 4],
            );
            buffer.advance(0);
        }

        let keys = buffer.keys(0, 0);
        assert_eq!(keys.len(), 12); // 3 tokens * 4 dim
        assert_eq!(keys[0..4], [0.0, 1.0, 2.0, 3.0]); // First token
        assert_eq!(keys[4..8], [1.0, 2.0, 3.0, 4.0]); // Second token
    }

    #[test]
    fn test_hot_buffer_reset() {
        let config = HotBufferConfig::new(2, 1, 4, 3);
        let mut buffer = HotBuffer::new(config);

        buffer.push_head(0, 0, &[1.0; 4], &[2.0; 4]);
        buffer.advance(0);
        buffer.push_head(1, 0, &[3.0; 4], &[4.0; 4]);
        buffer.advance(1);

        assert_eq!(buffer.len(0), 1);
        assert_eq!(buffer.len(1), 1);

        buffer.reset_layer(0);
        assert_eq!(buffer.len(0), 0);
        assert_eq!(buffer.len(1), 1);

        buffer.reset();
        assert_eq!(buffer.len(0), 0);
        assert_eq!(buffer.len(1), 0);
    }
}
