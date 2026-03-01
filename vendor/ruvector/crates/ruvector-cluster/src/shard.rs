//! Sharding logic for distributed vector storage
//!
//! Implements consistent hashing for shard distribution and routing.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tracing::debug;

const VIRTUAL_NODE_COUNT: usize = 150;

/// Consistent hash ring for node assignment
#[derive(Debug)]
pub struct ConsistentHashRing {
    /// Virtual nodes on the ring (hash -> node_id)
    ring: BTreeMap<u64, String>,
    /// Real nodes in the cluster
    nodes: HashMap<String, usize>,
    /// Replication factor
    replication_factor: usize,
}

impl ConsistentHashRing {
    /// Create a new consistent hash ring
    pub fn new(replication_factor: usize) -> Self {
        Self {
            ring: BTreeMap::new(),
            nodes: HashMap::new(),
            replication_factor,
        }
    }

    /// Add a node to the ring
    pub fn add_node(&mut self, node_id: String) {
        if self.nodes.contains_key(&node_id) {
            return;
        }

        // Add virtual nodes for better distribution
        for i in 0..VIRTUAL_NODE_COUNT {
            let virtual_key = format!("{}:{}", node_id, i);
            let hash = Self::hash_key(&virtual_key);
            self.ring.insert(hash, node_id.clone());
        }

        self.nodes.insert(node_id, VIRTUAL_NODE_COUNT);
        debug!(
            "Added node to hash ring with {} virtual nodes",
            VIRTUAL_NODE_COUNT
        );
    }

    /// Remove a node from the ring
    pub fn remove_node(&mut self, node_id: &str) {
        if !self.nodes.contains_key(node_id) {
            return;
        }

        // Remove all virtual nodes
        self.ring.retain(|_, v| v != node_id);
        self.nodes.remove(node_id);
        debug!("Removed node from hash ring");
    }

    /// Get nodes responsible for a key
    pub fn get_nodes(&self, key: &str, count: usize) -> Vec<String> {
        if self.ring.is_empty() {
            return Vec::new();
        }

        let hash = Self::hash_key(key);
        let mut nodes = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Find the first node on or after the hash
        for (_, node_id) in self.ring.range(hash..) {
            if seen.insert(node_id.clone()) {
                nodes.push(node_id.clone());
                if nodes.len() >= count {
                    return nodes;
                }
            }
        }

        // Wrap around to the beginning if needed
        for (_, node_id) in self.ring.iter() {
            if seen.insert(node_id.clone()) {
                nodes.push(node_id.clone());
                if nodes.len() >= count {
                    return nodes;
                }
            }
        }

        nodes
    }

    /// Get the primary node for a key
    pub fn get_primary_node(&self, key: &str) -> Option<String> {
        self.get_nodes(key, 1).first().cloned()
    }

    /// Hash a key to a u64
    fn hash_key(key: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// Get the number of real nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// List all real nodes
    pub fn list_nodes(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }
}

/// Routes queries to the correct shard
pub struct ShardRouter {
    /// Total number of shards
    shard_count: u32,
    /// Shard assignment cache
    cache: Arc<RwLock<HashMap<String, u32>>>,
}

impl ShardRouter {
    /// Create a new shard router
    pub fn new(shard_count: u32) -> Self {
        Self {
            shard_count,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the shard ID for a key using jump consistent hashing
    pub fn get_shard(&self, key: &str) -> u32 {
        // Check cache first
        {
            let cache = self.cache.read();
            if let Some(&shard_id) = cache.get(key) {
                return shard_id;
            }
        }

        // Calculate using jump consistent hash
        let shard_id = self.jump_consistent_hash(key, self.shard_count);

        // Update cache
        {
            let mut cache = self.cache.write();
            cache.insert(key.to_string(), shard_id);
        }

        shard_id
    }

    /// Jump consistent hash algorithm
    /// Provides minimal key migration on shard count changes
    fn jump_consistent_hash(&self, key: &str, num_buckets: u32) -> u32 {
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let mut hash = hasher.finish();

        let mut b: i64 = -1;
        let mut j: i64 = 0;

        while j < num_buckets as i64 {
            b = j;
            hash = hash.wrapping_mul(2862933555777941757).wrapping_add(1);
            j = ((b.wrapping_add(1) as f64)
                * ((1i64 << 31) as f64 / ((hash >> 33).wrapping_add(1) as f64)))
                as i64;
        }

        b as u32
    }

    /// Get shard ID for a vector ID
    pub fn get_shard_for_vector(&self, vector_id: &str) -> u32 {
        self.get_shard(vector_id)
    }

    /// Get shard IDs for a range query (may span multiple shards)
    pub fn get_shards_for_range(&self, _start: &str, _end: &str) -> Vec<u32> {
        // For range queries, we might need to check multiple shards
        // For simplicity, return all shards (can be optimized based on key distribution)
        (0..self.shard_count).collect()
    }

    /// Clear the routing cache
    pub fn clear_cache(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        let cache = self.cache.read();
        CacheStats {
            entries: cache.len(),
            shard_count: self.shard_count as usize,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub entries: usize,
    pub shard_count: usize,
}

/// Shard migration manager
pub struct ShardMigration {
    /// Source shard ID
    pub source_shard: u32,
    /// Target shard ID
    pub target_shard: u32,
    /// Migration progress (0.0 to 1.0)
    pub progress: f64,
    /// Keys migrated
    pub keys_migrated: usize,
    /// Total keys to migrate
    pub total_keys: usize,
}

impl ShardMigration {
    /// Create a new shard migration
    pub fn new(source_shard: u32, target_shard: u32, total_keys: usize) -> Self {
        Self {
            source_shard,
            target_shard,
            progress: 0.0,
            keys_migrated: 0,
            total_keys,
        }
    }

    /// Update migration progress
    pub fn update_progress(&mut self, keys_migrated: usize) {
        self.keys_migrated = keys_migrated;
        self.progress = if self.total_keys > 0 {
            keys_migrated as f64 / self.total_keys as f64
        } else {
            1.0
        };
    }

    /// Check if migration is complete
    pub fn is_complete(&self) -> bool {
        self.progress >= 1.0 || self.keys_migrated >= self.total_keys
    }
}

/// Load balancer for shard distribution
pub struct LoadBalancer {
    /// Shard load statistics (shard_id -> load)
    loads: Arc<RwLock<HashMap<u32, f64>>>,
}

impl LoadBalancer {
    /// Create a new load balancer
    pub fn new() -> Self {
        Self {
            loads: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update load for a shard
    pub fn update_load(&self, shard_id: u32, load: f64) {
        let mut loads = self.loads.write();
        loads.insert(shard_id, load);
    }

    /// Get load for a shard
    pub fn get_load(&self, shard_id: u32) -> f64 {
        let loads = self.loads.read();
        loads.get(&shard_id).copied().unwrap_or(0.0)
    }

    /// Get the least loaded shard
    pub fn get_least_loaded_shard(&self, shard_ids: &[u32]) -> Option<u32> {
        let loads = self.loads.read();

        shard_ids
            .iter()
            .min_by(|&&a, &&b| {
                let load_a = loads.get(&a).copied().unwrap_or(0.0);
                let load_b = loads.get(&b).copied().unwrap_or(0.0);
                load_a
                    .partial_cmp(&load_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
    }

    /// Get load statistics
    pub fn get_stats(&self) -> LoadStats {
        let loads = self.loads.read();

        let total: f64 = loads.values().sum();
        let count = loads.len();
        let avg = if count > 0 { total / count as f64 } else { 0.0 };

        let max = loads.values().copied().fold(f64::NEG_INFINITY, f64::max);
        let min = loads.values().copied().fold(f64::INFINITY, f64::min);

        LoadStats {
            total_load: total,
            avg_load: avg,
            max_load: if max.is_finite() { max } else { 0.0 },
            min_load: if min.is_finite() { min } else { 0.0 },
            shard_count: count,
        }
    }
}

impl Default for LoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

/// Load statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadStats {
    pub total_load: f64,
    pub avg_load: f64,
    pub max_load: f64,
    pub min_load: f64,
    pub shard_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consistent_hash_ring() {
        let mut ring = ConsistentHashRing::new(3);

        ring.add_node("node1".to_string());
        ring.add_node("node2".to_string());
        ring.add_node("node3".to_string());

        assert_eq!(ring.node_count(), 3);

        let nodes = ring.get_nodes("test-key", 3);
        assert_eq!(nodes.len(), 3);

        // Test primary node selection
        let primary = ring.get_primary_node("test-key");
        assert!(primary.is_some());
    }

    #[test]
    fn test_consistent_hashing_distribution() {
        let mut ring = ConsistentHashRing::new(3);

        ring.add_node("node1".to_string());
        ring.add_node("node2".to_string());
        ring.add_node("node3".to_string());

        let mut distribution: HashMap<String, usize> = HashMap::new();

        // Test distribution across many keys
        for i in 0..1000 {
            let key = format!("key{}", i);
            if let Some(node) = ring.get_primary_node(&key) {
                *distribution.entry(node).or_insert(0) += 1;
            }
        }

        // Each node should get roughly 1/3 of the keys (within 20% tolerance)
        for count in distribution.values() {
            let ratio = *count as f64 / 1000.0;
            assert!(ratio > 0.2 && ratio < 0.5, "Distribution ratio: {}", ratio);
        }
    }

    #[test]
    fn test_shard_router() {
        let router = ShardRouter::new(16);

        let shard1 = router.get_shard("test-key-1");
        let shard2 = router.get_shard("test-key-1"); // Should be cached

        assert_eq!(shard1, shard2);
        assert!(shard1 < 16);

        let stats = router.cache_stats();
        assert_eq!(stats.entries, 1);
    }

    #[test]
    fn test_jump_consistent_hash() {
        let router = ShardRouter::new(10);

        // Same key should always map to same shard
        let shard1 = router.get_shard("consistent-key");
        let shard2 = router.get_shard("consistent-key");

        assert_eq!(shard1, shard2);
    }

    #[test]
    fn test_shard_migration() {
        let mut migration = ShardMigration::new(0, 1, 100);

        assert!(!migration.is_complete());
        assert_eq!(migration.progress, 0.0);

        migration.update_progress(50);
        assert_eq!(migration.progress, 0.5);

        migration.update_progress(100);
        assert!(migration.is_complete());
    }

    #[test]
    fn test_load_balancer() {
        let balancer = LoadBalancer::new();

        balancer.update_load(0, 0.5);
        balancer.update_load(1, 0.8);
        balancer.update_load(2, 0.3);

        let least_loaded = balancer.get_least_loaded_shard(&[0, 1, 2]);
        assert_eq!(least_loaded, Some(2));

        let stats = balancer.get_stats();
        assert_eq!(stats.shard_count, 3);
        assert!(stats.avg_load > 0.0);
    }
}
