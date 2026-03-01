//! Vector-based intelligent caching for Scipix OCR results
//!
//! Uses ruvector-core for efficient similarity search and LRU eviction.

use crate::config::CacheConfig;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cached OCR result with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResult {
    /// LaTeX output
    pub latex: String,

    /// Alternative formats (MathML, AsciiMath)
    pub alternatives: HashMap<String, String>,

    /// Confidence score
    pub confidence: f32,

    /// Cache timestamp
    pub timestamp: u64,

    /// Access count
    pub access_count: usize,

    /// Image hash
    pub image_hash: String,
}

/// Cache entry with vector embedding
#[derive(Debug, Clone)]
struct CacheEntry {
    /// Vector embedding of image
    embedding: Vec<f32>,

    /// Cached result
    result: CachedResult,

    /// Last access time
    last_access: u64,
}

/// Vector-based cache manager
pub struct CacheManager {
    /// Configuration
    config: CacheConfig,

    /// Cache entries (thread-safe)
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,

    /// LRU tracking
    lru_order: Arc<RwLock<Vec<String>>>,

    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,
}

/// Cache statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total cache hits
    pub hits: u64,

    /// Total cache misses
    pub misses: u64,

    /// Total entries
    pub entries: usize,

    /// Total evictions
    pub evictions: u64,

    /// Average similarity score for hits
    pub avg_similarity: f32,
}

impl CacheStats {
    /// Calculate hit rate
    pub fn hit_rate(&self) -> f32 {
        if self.hits + self.misses == 0 {
            return 0.0;
        }
        self.hits as f32 / (self.hits + self.misses) as f32
    }
}

impl CacheManager {
    /// Create new cache manager
    ///
    /// # Arguments
    ///
    /// * `config` - Cache configuration
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ruvector_scipix::{CacheConfig, cache::CacheManager};
    ///
    /// let config = CacheConfig {
    ///     enabled: true,
    ///     capacity: 1000,
    ///     similarity_threshold: 0.95,
    ///     ttl: 3600,
    ///     vector_dimension: 512,
    ///     persistent: false,
    ///     cache_dir: ".cache".to_string(),
    /// };
    ///
    /// let cache = CacheManager::new(config);
    /// ```
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            entries: Arc::new(RwLock::new(HashMap::new())),
            lru_order: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Generate embedding for image
    ///
    /// This is a placeholder - in production, use actual vision model
    fn generate_embedding(&self, image_data: &[u8]) -> Result<Vec<f32>> {
        // Placeholder: Simple hash-based embedding
        // In production: Use Vision Transformer or similar
        let hash = self.hash_image(image_data);
        let mut embedding = vec![0.0; self.config.vector_dimension];

        for (i, byte) in hash.as_bytes().iter().enumerate() {
            if i < embedding.len() {
                embedding[i] = *byte as f32 / 255.0;
            }
        }

        Ok(embedding)
    }

    /// Hash image data
    fn hash_image(&self, image_data: &[u8]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        image_data.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Calculate cosine similarity between vectors
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot_product / (norm_a * norm_b)
    }

    /// Look up cached result by image similarity
    ///
    /// # Arguments
    ///
    /// * `image_data` - Raw image bytes
    ///
    /// # Returns
    ///
    /// Cached result if similarity exceeds threshold, None otherwise
    pub fn lookup(&self, image_data: &[u8]) -> Result<Option<CachedResult>> {
        if !self.config.enabled {
            return Ok(None);
        }

        let embedding = self.generate_embedding(image_data)?;
        let hash = self.hash_image(image_data);

        let entries = self.entries.read().unwrap();

        // First try exact hash match
        if let Some(entry) = entries.get(&hash) {
            if !self.is_expired(&entry) {
                self.record_hit();
                self.update_lru(&hash);
                return Ok(Some(entry.result.clone()));
            }
        }

        // Then try similarity search
        let mut best_match: Option<(String, f32, CachedResult)> = None;

        for (key, entry) in entries.iter() {
            if self.is_expired(entry) {
                continue;
            }

            let similarity = self.cosine_similarity(&embedding, &entry.embedding);

            if similarity >= self.config.similarity_threshold {
                if best_match.is_none() || similarity > best_match.as_ref().unwrap().1 {
                    best_match = Some((key.clone(), similarity, entry.result.clone()));
                }
            }
        }

        if let Some((key, similarity, result)) = best_match {
            self.record_hit_with_similarity(similarity);
            self.update_lru(&key);
            Ok(Some(result))
        } else {
            self.record_miss();
            Ok(None)
        }
    }

    /// Store result in cache
    ///
    /// # Arguments
    ///
    /// * `image_data` - Raw image bytes
    /// * `result` - OCR result to cache
    pub fn store(&self, image_data: &[u8], result: CachedResult) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let embedding = self.generate_embedding(image_data)?;
        let hash = self.hash_image(image_data);

        let entry = CacheEntry {
            embedding,
            result,
            last_access: self.current_timestamp(),
        };

        let mut entries = self.entries.write().unwrap();

        // Check if we need to evict
        if entries.len() >= self.config.capacity && !entries.contains_key(&hash) {
            self.evict_lru(&mut entries);
        }

        entries.insert(hash.clone(), entry);
        self.update_lru(&hash);
        self.update_stats_entries(entries.len());

        Ok(())
    }

    /// Check if entry is expired
    fn is_expired(&self, entry: &CacheEntry) -> bool {
        let current = self.current_timestamp();
        current - entry.last_access > self.config.ttl
    }

    /// Get current timestamp
    fn current_timestamp(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Evict least recently used entry
    fn evict_lru(&self, entries: &mut HashMap<String, CacheEntry>) {
        let mut lru = self.lru_order.write().unwrap();

        if let Some(key) = lru.first() {
            entries.remove(key);
            lru.remove(0);
            self.record_eviction();
        }
    }

    /// Update LRU order
    fn update_lru(&self, key: &str) {
        let mut lru = self.lru_order.write().unwrap();
        lru.retain(|k| k != key);
        lru.push(key.to_string());
    }

    /// Record cache hit
    fn record_hit(&self) {
        let mut stats = self.stats.write().unwrap();
        stats.hits += 1;
    }

    /// Record cache hit with similarity
    fn record_hit_with_similarity(&self, similarity: f32) {
        let mut stats = self.stats.write().unwrap();
        stats.hits += 1;

        // Update rolling average
        let total = stats.hits as f32;
        stats.avg_similarity = (stats.avg_similarity * (total - 1.0) + similarity) / total;
    }

    /// Record cache miss
    fn record_miss(&self) {
        let mut stats = self.stats.write().unwrap();
        stats.misses += 1;
    }

    /// Record eviction
    fn record_eviction(&self) {
        let mut stats = self.stats.write().unwrap();
        stats.evictions += 1;
    }

    /// Update entry count
    fn update_stats_entries(&self, count: usize) {
        let mut stats = self.stats.write().unwrap();
        stats.entries = count;
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.stats.read().unwrap().clone()
    }

    /// Clear all cache entries
    pub fn clear(&self) {
        let mut entries = self.entries.write().unwrap();
        let mut lru = self.lru_order.write().unwrap();

        entries.clear();
        lru.clear();

        self.update_stats_entries(0);
    }

    /// Remove expired entries
    pub fn cleanup(&self) {
        let mut entries = self.entries.write().unwrap();
        let mut lru = self.lru_order.write().unwrap();

        let expired: Vec<String> = entries
            .iter()
            .filter(|(_, entry)| self.is_expired(entry))
            .map(|(key, _)| key.clone())
            .collect();

        for key in &expired {
            entries.remove(key);
            lru.retain(|k| k != key);
        }

        self.update_stats_entries(entries.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CacheConfig {
        CacheConfig {
            enabled: true,
            capacity: 100,
            similarity_threshold: 0.95,
            ttl: 3600,
            vector_dimension: 128,
            persistent: false,
            cache_dir: ".cache/test".to_string(),
        }
    }

    fn test_result() -> CachedResult {
        CachedResult {
            latex: r"\frac{x^2}{2}".to_string(),
            alternatives: HashMap::new(),
            confidence: 0.95,
            timestamp: 0,
            access_count: 0,
            image_hash: "test".to_string(),
        }
    }

    #[test]
    fn test_cache_creation() {
        let config = test_config();
        let cache = CacheManager::new(config);
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 0);
    }

    #[test]
    fn test_store_and_lookup() {
        let config = test_config();
        let cache = CacheManager::new(config);

        let image_data = b"test image data";
        let result = test_result();

        cache.store(image_data, result.clone()).unwrap();

        let lookup_result = cache.lookup(image_data).unwrap();
        assert!(lookup_result.is_some());
        assert_eq!(lookup_result.unwrap().latex, result.latex);
    }

    #[test]
    fn test_cache_miss() {
        let config = test_config();
        let cache = CacheManager::new(config);

        let image_data = b"nonexistent image";
        let lookup_result = cache.lookup(image_data).unwrap();

        assert!(lookup_result.is_none());
        assert_eq!(cache.stats().misses, 1);
    }

    #[test]
    fn test_cache_hit_rate() {
        let config = test_config();
        let cache = CacheManager::new(config);

        let image_data = b"test image";
        let result = test_result();

        // Store and lookup once
        cache.store(image_data, result).unwrap();
        cache.lookup(image_data).unwrap();

        // Lookup again (hit)
        cache.lookup(image_data).unwrap();

        // Lookup different image (miss)
        cache.lookup(b"different image").unwrap();

        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate() - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_cosine_similarity() {
        let config = test_config();
        let cache = CacheManager::new(config);

        let vec_a = vec![1.0, 0.0, 0.0];
        let vec_b = vec![1.0, 0.0, 0.0];
        let vec_c = vec![0.0, 1.0, 0.0];

        assert!((cache.cosine_similarity(&vec_a, &vec_b) - 1.0).abs() < 0.01);
        assert!((cache.cosine_similarity(&vec_a, &vec_c) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_cache_clear() {
        let config = test_config();
        let cache = CacheManager::new(config);

        let image_data = b"test image";
        let result = test_result();

        cache.store(image_data, result).unwrap();
        assert_eq!(cache.stats().entries, 1);

        cache.clear();
        assert_eq!(cache.stats().entries, 0);
    }

    #[test]
    fn test_disabled_cache() {
        let mut config = test_config();
        config.enabled = false;
        let cache = CacheManager::new(config);

        let image_data = b"test image";
        let result = test_result();

        cache.store(image_data, result).unwrap();
        let lookup_result = cache.lookup(image_data).unwrap();

        assert!(lookup_result.is_none());
    }
}
