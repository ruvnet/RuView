//! Attention Cache: LRU cache for computed attention scores
//!
//! Caches attention scores to avoid redundant computation for identical DAGs.
//! Uses LRU eviction policy to manage memory usage.

use super::trait_def::AttentionScores;
use crate::dag::QueryDag;
use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries
    pub capacity: usize,
    /// Time-to-live for entries
    pub ttl: Option<Duration>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            capacity: 1000,
            ttl: Some(Duration::from_secs(300)), // 5 minutes
        }
    }
}

#[derive(Debug)]
struct CacheEntry {
    scores: AttentionScores,
    timestamp: Instant,
    access_count: usize,
}

pub struct AttentionCache {
    config: CacheConfig,
    cache: HashMap<u64, CacheEntry>,
    access_order: Vec<u64>,
    hits: usize,
    misses: usize,
}

impl AttentionCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: HashMap::with_capacity(config.capacity),
            access_order: Vec::with_capacity(config.capacity),
            config,
            hits: 0,
            misses: 0,
        }
    }

    /// Hash a DAG for cache key
    fn hash_dag(dag: &QueryDag, mechanism: &str) -> u64 {
        let mut hasher = DefaultHasher::new();

        // Hash mechanism name
        mechanism.hash(&mut hasher);

        // Hash number of nodes
        dag.node_count().hash(&mut hasher);

        // Hash edges structure
        let mut edge_list: Vec<(usize, usize)> = Vec::new();
        for node_id in dag.node_ids() {
            for &child in dag.children(node_id) {
                edge_list.push((node_id, child));
            }
        }
        edge_list.sort_unstable();

        for (from, to) in edge_list {
            from.hash(&mut hasher);
            to.hash(&mut hasher);
        }

        hasher.finish()
    }

    /// Check if entry is expired
    fn is_expired(&self, entry: &CacheEntry) -> bool {
        if let Some(ttl) = self.config.ttl {
            entry.timestamp.elapsed() > ttl
        } else {
            false
        }
    }

    /// Get cached scores for a DAG and mechanism
    pub fn get(&mut self, dag: &QueryDag, mechanism: &str) -> Option<AttentionScores> {
        let key = Self::hash_dag(dag, mechanism);

        // Check if key exists and is not expired
        let is_expired = self
            .cache
            .get(&key)
            .map(|entry| self.is_expired(entry))
            .unwrap_or(true);

        if is_expired {
            self.cache.remove(&key);
            self.access_order.retain(|&k| k != key);
            self.misses += 1;
            return None;
        }

        // Update access and return clone
        if let Some(entry) = self.cache.get_mut(&key) {
            // Update access order (move to end = most recently used)
            self.access_order.retain(|&k| k != key);
            self.access_order.push(key);
            entry.access_count += 1;
            self.hits += 1;

            Some(entry.scores.clone())
        } else {
            self.misses += 1;
            None
        }
    }

    /// Insert scores into cache
    pub fn insert(&mut self, dag: &QueryDag, mechanism: &str, scores: AttentionScores) {
        let key = Self::hash_dag(dag, mechanism);

        // Evict if at capacity
        while self.cache.len() >= self.config.capacity && !self.access_order.is_empty() {
            if let Some(oldest) = self.access_order.first().copied() {
                self.cache.remove(&oldest);
                self.access_order.remove(0);
            }
        }

        let entry = CacheEntry {
            scores,
            timestamp: Instant::now(),
            access_count: 0,
        };

        self.cache.insert(key, entry);
        self.access_order.push(key);
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.cache.clear();
        self.access_order.clear();
        self.hits = 0;
        self.misses = 0;
    }

    /// Remove expired entries
    pub fn evict_expired(&mut self) {
        let expired_keys: Vec<u64> = self
            .cache
            .iter()
            .filter(|(_, entry)| self.is_expired(entry))
            .map(|(k, _)| *k)
            .collect();

        for key in expired_keys {
            self.cache.remove(&key);
            self.access_order.retain(|&k| k != key);
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            size: self.cache.len(),
            capacity: self.config.capacity,
            hits: self.hits,
            misses: self.misses,
            hit_rate: if self.hits + self.misses > 0 {
                self.hits as f64 / (self.hits + self.misses) as f64
            } else {
                0.0
            },
        }
    }

    /// Get entry with most accesses
    pub fn most_accessed(&self) -> Option<(&u64, usize)> {
        self.cache
            .iter()
            .max_by_key(|(_, entry)| entry.access_count)
            .map(|(k, entry)| (k, entry.access_count))
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: usize,
    pub capacity: usize,
    pub hits: usize,
    pub misses: usize,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{OperatorNode, OperatorType};

    fn create_test_dag(n: usize) -> QueryDag {
        let mut dag = QueryDag::new();
        for i in 0..n {
            let mut node = OperatorNode::new(i, OperatorType::Scan);
            node.estimated_cost = (i + 1) as f64;
            dag.add_node(node);
        }
        if n > 1 {
            let _ = dag.add_edge(0, 1);
        }
        dag
    }

    #[test]
    fn test_cache_insert_and_get() {
        let mut cache = AttentionCache::new(CacheConfig::default());
        let dag = create_test_dag(3);

        let scores = AttentionScores::new(vec![0.5, 0.3, 0.2]);
        let expected_scores = scores.scores.clone();
        cache.insert(&dag, "test_mechanism", scores);

        let retrieved = cache.get(&dag, "test_mechanism").unwrap();
        assert_eq!(retrieved.scores, expected_scores);
    }

    #[test]
    fn test_cache_miss() {
        let mut cache = AttentionCache::new(CacheConfig::default());
        let dag = create_test_dag(3);

        let result = cache.get(&dag, "nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = AttentionCache::new(CacheConfig {
            capacity: 2,
            ttl: None,
        });

        let dag1 = create_test_dag(1);
        let dag2 = create_test_dag(2);
        let dag3 = create_test_dag(3);

        cache.insert(&dag1, "mech", AttentionScores::new(vec![0.5]));
        cache.insert(&dag2, "mech", AttentionScores::new(vec![0.3, 0.7]));
        cache.insert(&dag3, "mech", AttentionScores::new(vec![0.2, 0.3, 0.5]));

        // dag1 should be evicted (LRU), dag2 and dag3 should still be present
        let result1 = cache.get(&dag1, "mech");
        let result2 = cache.get(&dag2, "mech");
        let result3 = cache.get(&dag3, "mech");

        assert!(result1.is_none());
        assert!(result2.is_some());
        assert!(result3.is_some());
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = AttentionCache::new(CacheConfig::default());
        let dag = create_test_dag(2);

        cache.insert(&dag, "mech", AttentionScores::new(vec![0.5, 0.5]));

        cache.get(&dag, "mech"); // hit
        cache.get(&dag, "nonexistent"); // miss

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_ttl_expiration() {
        let mut cache = AttentionCache::new(CacheConfig {
            capacity: 100,
            ttl: Some(Duration::from_millis(50)),
        });

        let dag = create_test_dag(2);
        cache.insert(&dag, "mech", AttentionScores::new(vec![0.5, 0.5]));

        // Should be present immediately
        assert!(cache.get(&dag, "mech").is_some());

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(60));

        // Should be expired
        assert!(cache.get(&dag, "mech").is_none());
    }

    #[test]
    fn test_hash_consistency() {
        let dag = create_test_dag(3);

        let hash1 = AttentionCache::hash_dag(&dag, "mechanism");
        let hash2 = AttentionCache::hash_dag(&dag, "mechanism");

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_different_mechanisms() {
        let dag = create_test_dag(3);

        let hash1 = AttentionCache::hash_dag(&dag, "mechanism1");
        let hash2 = AttentionCache::hash_dag(&dag, "mechanism2");

        assert_ne!(hash1, hash2);
    }
}
