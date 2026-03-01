//! Query result caching for performance optimization
//!
//! Implements LRU cache with TTL support

use crate::executor::pipeline::RowBatch;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of cached entries
    pub max_entries: usize,
    /// Maximum memory usage in bytes
    pub max_memory_bytes: usize,
    /// Time-to-live for cache entries in seconds
    pub ttl_seconds: u64,
}

impl CacheConfig {
    /// Create new cache config
    pub fn new(max_entries: usize, max_memory_bytes: usize, ttl_seconds: u64) -> Self {
        Self {
            max_entries,
            max_memory_bytes,
            ttl_seconds,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            max_memory_bytes: 100 * 1024 * 1024, // 100MB
            ttl_seconds: 300,                    // 5 minutes
        }
    }
}

/// Cache entry with metadata
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Cached query results
    pub results: Vec<RowBatch>,
    /// Entry creation time
    pub created_at: Instant,
    /// Last access time
    pub last_accessed: Instant,
    /// Estimated memory size in bytes
    pub size_bytes: usize,
    /// Access count
    pub access_count: u64,
}

impl CacheEntry {
    /// Create new cache entry
    pub fn new(results: Vec<RowBatch>) -> Self {
        let size_bytes = Self::estimate_size(&results);
        let now = Instant::now();

        Self {
            results,
            created_at: now,
            last_accessed: now,
            size_bytes,
            access_count: 0,
        }
    }

    /// Estimate memory size of results
    fn estimate_size(results: &[RowBatch]) -> usize {
        results
            .iter()
            .map(|batch| {
                // Rough estimate: 8 bytes per value + overhead
                batch.len() * batch.schema.columns.len() * 8 + 1024
            })
            .sum()
    }

    /// Check if entry is expired
    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }

    /// Update access metadata
    pub fn mark_accessed(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
}

/// LRU cache for query results
pub struct QueryCache {
    /// Cache storage
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// LRU tracking
    lru_order: Arc<RwLock<Vec<String>>>,
    /// Configuration
    config: CacheConfig,
    /// Current memory usage
    memory_used: Arc<RwLock<usize>>,
    /// Cache statistics
    stats: Arc<RwLock<CacheStats>>,
}

impl QueryCache {
    /// Create a new query cache
    pub fn new(config: CacheConfig) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            lru_order: Arc::new(RwLock::new(Vec::new())),
            config,
            memory_used: Arc::new(RwLock::new(0)),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Get cached results
    pub fn get(&self, key: &str) -> Option<CacheEntry> {
        let mut entries = self.entries.write().ok()?;
        let mut lru = self.lru_order.write().ok()?;
        let mut stats = self.stats.write().ok()?;

        if let Some(entry) = entries.get_mut(key) {
            // Check if expired
            if entry.is_expired(Duration::from_secs(self.config.ttl_seconds)) {
                stats.misses += 1;
                return None;
            }

            // Update LRU order
            if let Some(pos) = lru.iter().position(|k| k == key) {
                lru.remove(pos);
            }
            lru.push(key.to_string());

            // Update access metadata
            entry.mark_accessed();
            stats.hits += 1;

            Some(entry.clone())
        } else {
            stats.misses += 1;
            None
        }
    }

    /// Insert results into cache
    pub fn insert(&self, key: String, results: Vec<RowBatch>) {
        let entry = CacheEntry::new(results);
        let entry_size = entry.size_bytes;

        let mut entries = self.entries.write().unwrap();
        let mut lru = self.lru_order.write().unwrap();
        let mut memory = self.memory_used.write().unwrap();
        let mut stats = self.stats.write().unwrap();

        // Evict if necessary
        while (entries.len() >= self.config.max_entries
            || *memory + entry_size > self.config.max_memory_bytes)
            && !lru.is_empty()
        {
            if let Some(old_key) = lru.first().cloned() {
                if let Some(old_entry) = entries.remove(&old_key) {
                    *memory = memory.saturating_sub(old_entry.size_bytes);
                    stats.evictions += 1;
                }
                lru.remove(0);
            }
        }

        // Insert new entry
        entries.insert(key.clone(), entry);
        lru.push(key);
        *memory += entry_size;
        stats.inserts += 1;
    }

    /// Remove entry from cache
    pub fn remove(&self, key: &str) -> bool {
        let mut entries = self.entries.write().unwrap();
        let mut lru = self.lru_order.write().unwrap();
        let mut memory = self.memory_used.write().unwrap();

        if let Some(entry) = entries.remove(key) {
            *memory = memory.saturating_sub(entry.size_bytes);
            if let Some(pos) = lru.iter().position(|k| k == key) {
                lru.remove(pos);
            }
            true
        } else {
            false
        }
    }

    /// Clear all cache entries
    pub fn clear(&self) {
        let mut entries = self.entries.write().unwrap();
        let mut lru = self.lru_order.write().unwrap();
        let mut memory = self.memory_used.write().unwrap();

        entries.clear();
        lru.clear();
        *memory = 0;
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.stats.read().unwrap().clone()
    }

    /// Get current memory usage
    pub fn memory_used(&self) -> usize {
        *self.memory_used.read().unwrap()
    }

    /// Get number of cached entries
    pub fn len(&self) -> usize {
        self.entries.read().unwrap().len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.read().unwrap().is_empty()
    }

    /// Clean expired entries
    pub fn clean_expired(&self) {
        let ttl = Duration::from_secs(self.config.ttl_seconds);
        let mut entries = self.entries.write().unwrap();
        let mut lru = self.lru_order.write().unwrap();
        let mut memory = self.memory_used.write().unwrap();
        let mut stats = self.stats.write().unwrap();

        let expired_keys: Vec<_> = entries
            .iter()
            .filter(|(_, entry)| entry.is_expired(ttl))
            .map(|(key, _)| key.clone())
            .collect();

        for key in expired_keys {
            if let Some(entry) = entries.remove(&key) {
                *memory = memory.saturating_sub(entry.size_bytes);
                if let Some(pos) = lru.iter().position(|k| k == &key) {
                    lru.remove(pos);
                }
                stats.evictions += 1;
            }
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of insertions
    pub inserts: u64,
    /// Number of evictions
    pub evictions: u64,
}

impl CacheStats {
    /// Calculate hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        self.hits = 0;
        self.misses = 0;
        self.inserts = 0;
        self.evictions = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::plan::{ColumnDef, DataType, QuerySchema};

    fn create_test_batch() -> RowBatch {
        let schema = QuerySchema::new(vec![ColumnDef {
            name: "id".to_string(),
            data_type: DataType::Int64,
            nullable: false,
        }]);
        RowBatch::new(schema)
    }

    #[test]
    fn test_cache_insert_and_get() {
        let cache = QueryCache::new(CacheConfig::default());
        let batch = create_test_batch();

        cache.insert("test_key".to_string(), vec![batch.clone()]);
        assert_eq!(cache.len(), 1);

        let cached = cache.get("test_key");
        assert!(cached.is_some());
    }

    #[test]
    fn test_cache_miss() {
        let cache = QueryCache::new(CacheConfig::default());
        let result = cache.get("nonexistent");
        assert!(result.is_none());

        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn test_cache_eviction() {
        let config = CacheConfig {
            max_entries: 2,
            max_memory_bytes: 1024 * 1024,
            ttl_seconds: 300,
        };
        let cache = QueryCache::new(config);
        let batch = create_test_batch();

        cache.insert("key1".to_string(), vec![batch.clone()]);
        cache.insert("key2".to_string(), vec![batch.clone()]);
        cache.insert("key3".to_string(), vec![batch.clone()]);

        // Should have evicted oldest entry
        assert_eq!(cache.len(), 2);
        assert!(cache.get("key1").is_none());
    }

    #[test]
    fn test_cache_clear() {
        let cache = QueryCache::new(CacheConfig::default());
        let batch = create_test_batch();

        cache.insert("key1".to_string(), vec![batch.clone()]);
        cache.insert("key2".to_string(), vec![batch.clone()]);

        cache.clear();
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.memory_used(), 0);
    }

    #[test]
    fn test_hit_rate() {
        let mut stats = CacheStats::default();
        stats.hits = 7;
        stats.misses = 3;

        assert!((stats.hit_rate() - 0.7).abs() < 0.001);
    }
}
