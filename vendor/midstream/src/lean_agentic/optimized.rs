//! Optimized implementations for ultra-low latency processing
//!
//! These optimizations focus on:
//! - Reducing allocations
//! - Lock-free data structures where possible
//! - Pre-computed feature extractors
//! - Cached predictions
//! - Batch processing

use super::types::*;
use super::agent::Action;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Optimized feature cache for fast lookup
pub struct FeatureCache {
    cache: HashMap<u64, Vec<f64>>,
    max_size: usize,
}

impl FeatureCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::with_capacity(max_size),
            max_size,
        }
    }

    pub fn get(&self, key: u64) -> Option<&Vec<f64>> {
        self.cache.get(&key)
    }

    pub fn insert(&mut self, key: u64, features: Vec<f64>) {
        if self.cache.len() >= self.max_size {
            // Simple eviction: remove first entry (in practice, use LRU)
            if let Some(first_key) = self.cache.keys().next().copied() {
                self.cache.remove(&first_key);
            }
        }
        self.cache.insert(key, features);
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

/// Pre-allocated buffer pool for zero-allocation processing
pub struct BufferPool {
    buffers: Vec<Vec<u8>>,
    buffer_size: usize,
}

impl BufferPool {
    pub fn new(pool_size: usize, buffer_size: usize) -> Self {
        let mut buffers = Vec::with_capacity(pool_size);
        for _ in 0..pool_size {
            buffers.push(Vec::with_capacity(buffer_size));
        }

        Self {
            buffers,
            buffer_size,
        }
    }

    pub fn acquire(&mut self) -> Vec<u8> {
        self.buffers.pop().unwrap_or_else(|| Vec::with_capacity(self.buffer_size))
    }

    pub fn release(&mut self, mut buffer: Vec<u8>) {
        buffer.clear();
        if buffer.capacity() == self.buffer_size {
            self.buffers.push(buffer);
        }
    }
}

/// Fast hash function for action fingerprinting
#[inline(always)]
pub fn fast_hash(action: &Action) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    action.action_type.hash(&mut hasher);
    action.parameters.len().hash(&mut hasher);
    hasher.finish()
}

/// Optimized entity extraction with pre-allocated buffers
pub struct FastEntityExtractor {
    buffer: String,
    patterns: Vec<EntityPattern>,
}

#[derive(Clone)]
struct EntityPattern {
    prefix: &'static str,
    entity_type: EntityType,
}

impl FastEntityExtractor {
    pub fn new() -> Self {
        Self {
            buffer: String::with_capacity(1024),
            patterns: vec![
                EntityPattern {
                    prefix: "weather",
                    entity_type: EntityType::Concept,
                },
                EntityPattern {
                    prefix: "schedule",
                    entity_type: EntityType::Event,
                },
                EntityPattern {
                    prefix: "calendar",
                    entity_type: EntityType::Concept,
                },
            ],
        }
    }

    pub fn extract(&mut self, text: &str) -> Vec<(String, EntityType)> {
        let mut entities = Vec::new();
        let text_lower = text.to_lowercase();

        // Fast pattern matching
        for pattern in &self.patterns {
            if text_lower.contains(pattern.prefix) {
                entities.push((pattern.prefix.to_string(), pattern.entity_type.clone()));
            }
        }

        // Extract capitalized words (potential names)
        for word in text.split_whitespace() {
            if let Some(first_char) = word.chars().next() {
                if first_char.is_uppercase() && word.len() > 1 {
                    entities.push((word.to_string(), EntityType::Unknown));
                }
            }
        }

        entities
    }
}

/// Lock-free prediction cache for concurrent access
pub struct PredictionCache {
    predictions: Arc<dashmap::DashMap<u64, f64>>,
    max_size: usize,
}

impl PredictionCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            predictions: Arc::new(dashmap::DashMap::with_capacity(max_size)),
            max_size,
        }
    }

    pub fn get(&self, key: u64) -> Option<f64> {
        self.predictions.get(&key).map(|v| *v)
    }

    pub fn insert(&self, key: u64, value: f64) {
        if self.predictions.len() >= self.max_size {
            // Simple eviction
            if let Some(entry) = self.predictions.iter().next() {
                let key_to_remove = *entry.key();
                drop(entry);
                self.predictions.remove(&key_to_remove);
            }
        }
        self.predictions.insert(key, value);
    }

    pub fn len(&self) -> usize {
        self.predictions.len()
    }
}

/// Batch processor for amortizing costs
pub struct BatchProcessor<T> {
    batch: Vec<T>,
    batch_size: usize,
}

impl<T> BatchProcessor<T> {
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch: Vec::with_capacity(batch_size),
            batch_size,
        }
    }

    pub fn add(&mut self, item: T) -> Option<Vec<T>> {
        self.batch.push(item);

        if self.batch.len() >= self.batch_size {
            Some(std::mem::replace(&mut self.batch, Vec::with_capacity(self.batch_size)))
        } else {
            None
        }
    }

    pub fn flush(&mut self) -> Vec<T> {
        std::mem::replace(&mut self.batch, Vec::with_capacity(self.batch_size))
    }

    pub fn len(&self) -> usize {
        self.batch.len()
    }
}

/// SIMD-optimized vector operations (when available)
#[cfg(target_arch = "x86_64")]
pub mod simd {
    #[inline(always)]
    pub fn dot_product(a: &[f64], b: &[f64]) -> f64 {
        assert_eq!(a.len(), b.len());

        let mut sum = 0.0;
        let len = a.len();
        let chunks = len / 4;

        // Process 4 elements at a time
        for i in 0..chunks {
            let idx = i * 4;
            sum += a[idx] * b[idx]
                + a[idx + 1] * b[idx + 1]
                + a[idx + 2] * b[idx + 2]
                + a[idx + 3] * b[idx + 3];
        }

        // Handle remainder
        for i in (chunks * 4)..len {
            sum += a[i] * b[i];
        }

        sum
    }

    #[inline(always)]
    pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
        let dot = dot_product(a, b);
        let norm_a = dot_product(a, a).sqrt();
        let norm_b = dot_product(b, b).sqrt();

        if norm_a > 0.0 && norm_b > 0.0 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub mod simd {
    #[inline(always)]
    pub fn dot_product(a: &[f64], b: &[f64]) -> f64 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    #[inline(always)]
    pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
        let dot = dot_product(a, b);
        let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
        let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

        if norm_a > 0.0 && norm_b > 0.0 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }
}

/// Zero-copy message parser
pub struct MessageParser<'a> {
    data: &'a str,
    position: usize,
}

impl<'a> MessageParser<'a> {
    pub fn new(data: &'a str) -> Self {
        Self { data, position: 0 }
    }

    pub fn next_word(&mut self) -> Option<&'a str> {
        self.skip_whitespace();

        if self.position >= self.data.len() {
            return None;
        }

        let start = self.position;
        while self.position < self.data.len() && !self.data.as_bytes()[self.position].is_ascii_whitespace() {
            self.position += 1;
        }

        Some(&self.data[start..self.position])
    }

    fn skip_whitespace(&mut self) {
        while self.position < self.data.len() && self.data.as_bytes()[self.position].is_ascii_whitespace() {
            self.position += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_cache() {
        let mut cache = FeatureCache::new(2);
        cache.insert(1, vec![1.0, 2.0, 3.0]);
        cache.insert(2, vec![4.0, 5.0, 6.0]);

        assert!(cache.get(1).is_some());
        assert!(cache.get(2).is_some());

        // Should evict oldest
        cache.insert(3, vec![7.0, 8.0, 9.0]);
        assert!(cache.get(3).is_some());
    }

    #[test]
    fn test_buffer_pool() {
        let mut pool = BufferPool::new(2, 1024);

        let buf1 = pool.acquire();
        let buf2 = pool.acquire();

        assert_eq!(buf1.capacity(), 1024);
        assert_eq!(buf2.capacity(), 1024);

        pool.release(buf1);
        pool.release(buf2);

        let buf3 = pool.acquire();
        assert_eq!(buf3.capacity(), 1024);
    }

    #[test]
    fn test_simd_operations() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![2.0, 3.0, 4.0, 5.0];

        let dot = simd::dot_product(&a, &b);
        assert!((dot - 40.0).abs() < 1e-10);

        let sim = simd::cosine_similarity(&a, &b);
        assert!(sim > 0.9 && sim <= 1.0);
    }

    #[test]
    fn test_message_parser() {
        let mut parser = MessageParser::new("Hello world test");

        assert_eq!(parser.next_word(), Some("Hello"));
        assert_eq!(parser.next_word(), Some("world"));
        assert_eq!(parser.next_word(), Some("test"));
        assert_eq!(parser.next_word(), None);
    }
}
