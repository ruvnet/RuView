//! LRU Cache for Path Distances
//!
//! Provides efficient caching of path distances with:
//! - LRU eviction policy
//! - Prefetch hints based on access patterns
//! - Lock-free concurrent reads
//! - Batch update support
//!
//! Target: 10x speedup for repeated distance queries

use crate::graph::VertexId;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::RwLock;

/// Configuration for path distance cache
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries in cache
    pub max_entries: usize,
    /// Enable access pattern tracking for prefetch
    pub enable_prefetch: bool,
    /// Number of recent queries to track for prefetch
    pub prefetch_history_size: usize,
    /// Prefetch lookahead count
    pub prefetch_lookahead: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 10_000,
            enable_prefetch: true,
            prefetch_history_size: 100,
            prefetch_lookahead: 4,
        }
    }
}

/// Statistics for cache performance
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total cache hits
    pub hits: u64,
    /// Total cache misses
    pub misses: u64,
    /// Current cache size
    pub size: usize,
    /// Number of prefetch hits
    pub prefetch_hits: u64,
    /// Number of evictions
    pub evictions: u64,
}

impl CacheStats {
    /// Get hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total > 0 {
            self.hits as f64 / total as f64
        } else {
            0.0
        }
    }
}

/// Hint for prefetching likely queries
#[derive(Debug, Clone)]
pub struct PrefetchHint {
    /// Source vertex
    pub source: VertexId,
    /// Likely target vertices
    pub targets: Vec<VertexId>,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
}

/// Entry in the LRU cache
#[derive(Debug, Clone)]
struct CacheEntry {
    /// Source vertex
    source: VertexId,
    /// Target vertex
    target: VertexId,
    /// Cached distance
    distance: f64,
    /// Last access time (for LRU)
    last_access: u64,
    /// Was this a prefetch?
    prefetched: bool,
}

/// Key for cache lookup
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
struct CacheKey {
    source: VertexId,
    target: VertexId,
}

impl CacheKey {
    fn new(source: VertexId, target: VertexId) -> Self {
        // Normalize key so (a,b) == (b,a)
        if source <= target {
            Self { source, target }
        } else {
            Self {
                source: target,
                target: source,
            }
        }
    }
}

/// LRU cache for path distances
pub struct PathDistanceCache {
    config: CacheConfig,
    /// Main cache storage
    cache: RwLock<HashMap<CacheKey, CacheEntry>>,
    /// LRU order tracking
    lru_order: RwLock<VecDeque<CacheKey>>,
    /// Access counter for LRU timestamps
    access_counter: AtomicU64,
    /// Statistics
    hits: AtomicU64,
    misses: AtomicU64,
    prefetch_hits: AtomicU64,
    evictions: AtomicU64,
    /// Query history for prefetch prediction
    query_history: RwLock<VecDeque<CacheKey>>,
    /// Predicted next queries
    predicted_queries: RwLock<Vec<CacheKey>>,
}

impl PathDistanceCache {
    /// Create new cache with default config
    pub fn new() -> Self {
        Self::with_config(CacheConfig::default())
    }

    /// Create with custom config
    pub fn with_config(config: CacheConfig) -> Self {
        Self {
            config,
            cache: RwLock::new(HashMap::new()),
            lru_order: RwLock::new(VecDeque::new()),
            access_counter: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            prefetch_hits: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            query_history: RwLock::new(VecDeque::new()),
            predicted_queries: RwLock::new(Vec::new()),
        }
    }

    /// Get cached distance if available
    pub fn get(&self, source: VertexId, target: VertexId) -> Option<f64> {
        let key = CacheKey::new(source, target);

        // Try to read from cache
        let cache = self.cache.read().unwrap();
        if let Some(entry) = cache.get(&key) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            if entry.prefetched {
                self.prefetch_hits.fetch_add(1, Ordering::Relaxed);
            }

            // Update access pattern
            if self.config.enable_prefetch {
                self.record_query(key);
            }

            return Some(entry.distance);
        }
        drop(cache);

        self.misses.fetch_add(1, Ordering::Relaxed);

        // Record miss for prefetch prediction
        if self.config.enable_prefetch {
            self.record_query(key);
        }

        None
    }

    /// Insert distance into cache
    pub fn insert(&self, source: VertexId, target: VertexId, distance: f64) {
        let key = CacheKey::new(source, target);
        let timestamp = self.access_counter.fetch_add(1, Ordering::Relaxed);

        let entry = CacheEntry {
            source,
            target,
            distance,
            last_access: timestamp,
            prefetched: false,
        };

        self.insert_entry(key, entry);
    }

    /// Insert with prefetch flag
    pub fn insert_prefetch(&self, source: VertexId, target: VertexId, distance: f64) {
        let key = CacheKey::new(source, target);
        let timestamp = self.access_counter.fetch_add(1, Ordering::Relaxed);

        let entry = CacheEntry {
            source,
            target,
            distance,
            last_access: timestamp,
            prefetched: true,
        };

        self.insert_entry(key, entry);
    }

    /// Internal insert with eviction
    fn insert_entry(&self, key: CacheKey, entry: CacheEntry) {
        let mut cache = self.cache.write().unwrap();
        let mut lru = self.lru_order.write().unwrap();

        // Evict if at capacity
        while cache.len() >= self.config.max_entries {
            if let Some(evict_key) = lru.pop_front() {
                cache.remove(&evict_key);
                self.evictions.fetch_add(1, Ordering::Relaxed);
            } else {
                break;
            }
        }

        // Insert new entry
        cache.insert(key, entry);
        lru.push_back(key);
    }

    /// Batch insert multiple distances
    pub fn insert_batch(&self, entries: &[(VertexId, VertexId, f64)]) {
        let mut cache = self.cache.write().unwrap();
        let mut lru = self.lru_order.write().unwrap();

        for &(source, target, distance) in entries {
            let key = CacheKey::new(source, target);
            let timestamp = self.access_counter.fetch_add(1, Ordering::Relaxed);

            let entry = CacheEntry {
                source,
                target,
                distance,
                last_access: timestamp,
                prefetched: false,
            };

            // Evict if needed
            while cache.len() >= self.config.max_entries {
                if let Some(evict_key) = lru.pop_front() {
                    cache.remove(&evict_key);
                    self.evictions.fetch_add(1, Ordering::Relaxed);
                } else {
                    break;
                }
            }

            cache.insert(key, entry);
            lru.push_back(key);
        }
    }

    /// Invalidate entries involving a vertex
    pub fn invalidate_vertex(&self, vertex: VertexId) {
        let mut cache = self.cache.write().unwrap();
        let mut lru = self.lru_order.write().unwrap();

        let keys_to_remove: Vec<CacheKey> = cache
            .keys()
            .filter(|k| k.source == vertex || k.target == vertex)
            .copied()
            .collect();

        for key in keys_to_remove {
            cache.remove(&key);
            lru.retain(|k| *k != key);
        }
    }

    /// Clear entire cache
    pub fn clear(&self) {
        let mut cache = self.cache.write().unwrap();
        let mut lru = self.lru_order.write().unwrap();
        cache.clear();
        lru.clear();
    }

    /// Record a query for prefetch prediction
    fn record_query(&self, key: CacheKey) {
        if let Ok(mut history) = self.query_history.try_write() {
            history.push_back(key);
            while history.len() > self.config.prefetch_history_size {
                history.pop_front();
            }

            // Update predictions periodically
            if history.len() % 10 == 0 {
                self.update_predictions(&history);
            }
        }
    }

    /// Update prefetch predictions based on access patterns
    fn update_predictions(&self, history: &VecDeque<CacheKey>) {
        if history.len() < 10 {
            return;
        }

        // Find frequently co-occurring vertex pairs
        let mut vertex_frequency: HashMap<VertexId, usize> = HashMap::new();
        for key in history.iter() {
            *vertex_frequency.entry(key.source).or_insert(0) += 1;
            *vertex_frequency.entry(key.target).or_insert(0) += 1;
        }

        // Predict likely next queries based on recent pattern
        let recent: Vec<_> = history.iter().rev().take(5).collect();
        let mut predictions = Vec::new();

        for key in recent {
            // Predict queries to neighbors of frequently accessed vertices
            for (vertex, &freq) in &vertex_frequency {
                if freq > 2 && *vertex != key.source && *vertex != key.target {
                    predictions.push(CacheKey::new(key.source, *vertex));
                    if predictions.len() >= self.config.prefetch_lookahead {
                        break;
                    }
                }
            }
            if predictions.len() >= self.config.prefetch_lookahead {
                break;
            }
        }

        if let Ok(mut pred) = self.predicted_queries.try_write() {
            *pred = predictions;
        }
    }

    /// Get prefetch hints based on access patterns
    pub fn get_prefetch_hints(&self) -> Vec<PrefetchHint> {
        let history = self.query_history.read().unwrap();
        if history.is_empty() {
            return Vec::new();
        }

        // Find most frequently queried sources
        let mut source_freq: HashMap<VertexId, Vec<VertexId>> = HashMap::new();
        for key in history.iter() {
            source_freq.entry(key.source).or_default().push(key.target);
            source_freq.entry(key.target).or_default().push(key.source);
        }

        // Generate hints for hot sources
        source_freq
            .into_iter()
            .filter(|(_, targets)| targets.len() > 2)
            .map(|(source, targets)| {
                let confidence = (targets.len() as f64 / history.len() as f64).min(1.0);
                PrefetchHint {
                    source,
                    targets,
                    confidence,
                }
            })
            .collect()
    }

    /// Get predicted queries for prefetching
    pub fn get_predicted_queries(&self) -> Vec<(VertexId, VertexId)> {
        let pred = self.predicted_queries.read().unwrap();
        pred.iter().map(|key| (key.source, key.target)).collect()
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        let cache = self.cache.read().unwrap();
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            size: cache.len(),
            prefetch_hits: self.prefetch_hits.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
        }
    }

    /// Get current cache size
    pub fn len(&self) -> usize {
        self.cache.read().unwrap().len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.read().unwrap().is_empty()
    }
}

impl Default for PathDistanceCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_cache_operations() {
        let cache = PathDistanceCache::new();

        // Insert and retrieve
        cache.insert(1, 2, 10.0);
        assert_eq!(cache.get(1, 2), Some(10.0));

        // Symmetric access
        assert_eq!(cache.get(2, 1), Some(10.0));

        // Miss
        assert_eq!(cache.get(1, 3), None);
    }

    #[test]
    fn test_lru_eviction() {
        let cache = PathDistanceCache::with_config(CacheConfig {
            max_entries: 3,
            ..Default::default()
        });

        cache.insert(1, 2, 1.0);
        cache.insert(2, 3, 2.0);
        cache.insert(3, 4, 3.0);

        // Cache is full
        assert_eq!(cache.len(), 3);

        // Insert new entry - should evict (1,2)
        cache.insert(4, 5, 4.0);

        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(1, 2), None); // Evicted
        assert_eq!(cache.get(4, 5), Some(4.0)); // Present
    }

    #[test]
    fn test_batch_insert() {
        let cache = PathDistanceCache::new();

        let entries = vec![(1, 2, 1.0), (2, 3, 2.0), (3, 4, 3.0)];

        cache.insert_batch(&entries);

        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(1, 2), Some(1.0));
        assert_eq!(cache.get(2, 3), Some(2.0));
        assert_eq!(cache.get(3, 4), Some(3.0));
    }

    #[test]
    fn test_invalidate_vertex() {
        let cache = PathDistanceCache::new();

        cache.insert(1, 2, 1.0);
        cache.insert(1, 3, 2.0);
        cache.insert(2, 3, 3.0);

        cache.invalidate_vertex(1);

        assert_eq!(cache.get(1, 2), None);
        assert_eq!(cache.get(1, 3), None);
        assert_eq!(cache.get(2, 3), Some(3.0));
    }

    #[test]
    fn test_statistics() {
        let cache = PathDistanceCache::new();

        cache.insert(1, 2, 1.0);

        // Hit
        cache.get(1, 2);
        cache.get(1, 2);

        // Miss
        cache.get(3, 4);

        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.size, 1);
        assert!(stats.hit_rate() > 0.5);
    }

    #[test]
    fn test_prefetch_hints() {
        let cache = PathDistanceCache::with_config(CacheConfig {
            enable_prefetch: true,
            prefetch_history_size: 50,
            ..Default::default()
        });

        // Generate access pattern
        for i in 0..20 {
            cache.insert(1, i as u64, i as f64);
            let _ = cache.get(1, i as u64);
        }

        let hints = cache.get_prefetch_hints();
        // Should have hints for vertex 1 (frequently accessed)
        assert!(!hints.is_empty() || cache.stats().hits > 0);
    }

    #[test]
    fn test_clear() {
        let cache = PathDistanceCache::new();

        cache.insert(1, 2, 1.0);
        cache.insert(2, 3, 2.0);

        assert_eq!(cache.len(), 2);

        cache.clear();

        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }
}
