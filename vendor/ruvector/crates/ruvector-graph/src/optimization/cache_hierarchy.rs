//! Cache-optimized data layouts with hot/cold data separation
//!
//! This module implements cache-friendly storage patterns to minimize
//! cache misses and maximize memory bandwidth utilization.

use parking_lot::RwLock;
use std::alloc::{alloc, dealloc, Layout};
use std::sync::Arc;

/// Cache line size (64 bytes on x86-64)
const CACHE_LINE_SIZE: usize = 64;

/// L1 cache size estimate (32KB typical)
const L1_CACHE_SIZE: usize = 32 * 1024;

/// L2 cache size estimate (256KB typical)
const L2_CACHE_SIZE: usize = 256 * 1024;

/// L3 cache size estimate (8MB typical)
const L3_CACHE_SIZE: usize = 8 * 1024 * 1024;

/// Cache hierarchy manager for graph data
pub struct CacheHierarchy {
    /// Hot data stored in L1-friendly layout
    hot_storage: Arc<RwLock<HotStorage>>,
    /// Cold data stored in compressed format
    cold_storage: Arc<RwLock<ColdStorage>>,
    /// Access frequency tracker
    access_tracker: Arc<RwLock<AccessTracker>>,
}

impl CacheHierarchy {
    /// Create a new cache hierarchy
    pub fn new(hot_capacity: usize, cold_capacity: usize) -> Self {
        Self {
            hot_storage: Arc::new(RwLock::new(HotStorage::new(hot_capacity))),
            cold_storage: Arc::new(RwLock::new(ColdStorage::new(cold_capacity))),
            access_tracker: Arc::new(RwLock::new(AccessTracker::new())),
        }
    }

    /// Access node data with automatic hot/cold promotion
    pub fn get_node(&self, node_id: u64) -> Option<NodeData> {
        // Record access
        self.access_tracker.write().record_access(node_id);

        // Try hot storage first
        if let Some(data) = self.hot_storage.read().get(node_id) {
            return Some(data);
        }

        // Fall back to cold storage
        if let Some(data) = self.cold_storage.read().get(node_id) {
            // Promote to hot if frequently accessed
            if self.access_tracker.read().should_promote(node_id) {
                self.promote_to_hot(node_id, data.clone());
            }
            return Some(data);
        }

        None
    }

    /// Insert node data with automatic placement
    pub fn insert_node(&self, node_id: u64, data: NodeData) {
        // Record initial access for the new node
        self.access_tracker.write().record_access(node_id);

        // Check if we need to evict before inserting (to avoid double eviction with HotStorage)
        if self.hot_storage.read().is_at_capacity() {
            self.evict_one_to_cold(node_id); // Don't evict the one we're about to insert
        }

        // New data goes to hot storage
        self.hot_storage.write().insert(node_id, data.clone());
    }

    /// Promote node from cold to hot storage
    fn promote_to_hot(&self, node_id: u64, data: NodeData) {
        // First evict if needed to make room
        if self.hot_storage.read().is_full() {
            self.evict_one_to_cold(node_id); // Pass node_id to avoid evicting the one we're promoting
        }

        self.hot_storage.write().insert(node_id, data);
        self.cold_storage.write().remove(node_id);
    }

    /// Evict least recently used hot data to cold storage
    fn evict_cold(&self) {
        let tracker = self.access_tracker.read();
        let lru_nodes = tracker.get_lru_nodes_by_frequency(10);
        drop(tracker);

        let mut hot = self.hot_storage.write();
        let mut cold = self.cold_storage.write();

        for node_id in lru_nodes {
            if let Some(data) = hot.remove(node_id) {
                cold.insert(node_id, data);
            }
        }
    }

    /// Evict one node to cold storage, avoiding the protected node_id
    fn evict_one_to_cold(&self, protected_id: u64) {
        let tracker = self.access_tracker.read();
        // Get nodes sorted by frequency (least frequently accessed first)
        let candidates = tracker.get_lru_nodes_by_frequency(5);
        drop(tracker);

        let mut hot = self.hot_storage.write();
        let mut cold = self.cold_storage.write();

        for node_id in candidates {
            if node_id != protected_id {
                if let Some(data) = hot.remove(node_id) {
                    cold.insert(node_id, data);
                    return;
                }
            }
        }
    }

    /// Prefetch nodes that are likely to be accessed soon
    pub fn prefetch_neighbors(&self, node_ids: &[u64]) {
        // Use software prefetching hints
        for &node_id in node_ids {
            #[cfg(target_arch = "x86_64")]
            unsafe {
                // Prefetch to L1 cache
                std::arch::x86_64::_mm_prefetch(
                    &node_id as *const u64 as *const i8,
                    std::arch::x86_64::_MM_HINT_T0,
                );
            }
        }
    }
}

/// Hot storage with cache-line aligned entries
#[repr(align(64))]
struct HotStorage {
    /// Cache-line aligned storage
    entries: Vec<CacheLineEntry>,
    /// Capacity in number of entries
    capacity: usize,
    /// Current size
    size: usize,
}

impl HotStorage {
    fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
            size: 0,
        }
    }

    fn get(&self, node_id: u64) -> Option<NodeData> {
        self.entries
            .iter()
            .find(|e| e.node_id == node_id)
            .map(|e| e.data.clone())
    }

    fn insert(&mut self, node_id: u64, data: NodeData) {
        // Remove old entry if exists
        self.entries.retain(|e| e.node_id != node_id);

        if self.entries.len() >= self.capacity {
            self.entries.remove(0); // Simple FIFO eviction
        }

        self.entries.push(CacheLineEntry { node_id, data });
        self.size = self.entries.len();
    }

    fn remove(&mut self, node_id: u64) -> Option<NodeData> {
        if let Some(pos) = self.entries.iter().position(|e| e.node_id == node_id) {
            let entry = self.entries.remove(pos);
            self.size = self.entries.len();
            Some(entry.data)
        } else {
            None
        }
    }

    fn is_full(&self) -> bool {
        self.size >= self.capacity
    }

    fn is_at_capacity(&self) -> bool {
        self.size >= self.capacity
    }
}

/// Cache-line aligned entry (64 bytes)
#[repr(align(64))]
#[derive(Clone)]
struct CacheLineEntry {
    node_id: u64,
    data: NodeData,
}

/// Cold storage with compression
struct ColdStorage {
    /// Compressed data storage
    entries: dashmap::DashMap<u64, Vec<u8>>,
    capacity: usize,
}

impl ColdStorage {
    fn new(capacity: usize) -> Self {
        Self {
            entries: dashmap::DashMap::new(),
            capacity,
        }
    }

    fn get(&self, node_id: u64) -> Option<NodeData> {
        self.entries.get(&node_id).and_then(|compressed| {
            // Decompress data using bincode 2.0 API
            bincode::decode_from_slice(&compressed, bincode::config::standard())
                .ok()
                .map(|(data, _)| data)
        })
    }

    fn insert(&mut self, node_id: u64, data: NodeData) {
        // Compress data using bincode 2.0 API
        if let Ok(compressed) = bincode::encode_to_vec(&data, bincode::config::standard()) {
            self.entries.insert(node_id, compressed);
        }
    }

    fn remove(&mut self, node_id: u64) -> Option<NodeData> {
        self.entries.remove(&node_id).and_then(|(_, compressed)| {
            bincode::decode_from_slice(&compressed, bincode::config::standard())
                .ok()
                .map(|(data, _)| data)
        })
    }
}

/// Access frequency tracker for hot/cold promotion
struct AccessTracker {
    /// Access counts per node
    access_counts: dashmap::DashMap<u64, u32>,
    /// Last access timestamp
    last_access: dashmap::DashMap<u64, u64>,
    /// Global timestamp
    timestamp: u64,
}

impl AccessTracker {
    fn new() -> Self {
        Self {
            access_counts: dashmap::DashMap::new(),
            last_access: dashmap::DashMap::new(),
            timestamp: 0,
        }
    }

    fn record_access(&mut self, node_id: u64) {
        self.timestamp += 1;

        self.access_counts
            .entry(node_id)
            .and_modify(|count| *count += 1)
            .or_insert(1);

        self.last_access.insert(node_id, self.timestamp);
    }

    fn should_promote(&self, node_id: u64) -> bool {
        // Promote if accessed more than 5 times
        self.access_counts
            .get(&node_id)
            .map(|count| *count > 5)
            .unwrap_or(false)
    }

    fn get_lru_nodes(&self, count: usize) -> Vec<u64> {
        let mut nodes: Vec<_> = self
            .last_access
            .iter()
            .map(|entry| (*entry.key(), *entry.value()))
            .collect();

        nodes.sort_by_key(|(_, timestamp)| *timestamp);
        nodes
            .into_iter()
            .take(count)
            .map(|(node_id, _)| node_id)
            .collect()
    }

    /// Get least frequently accessed nodes (for smart eviction)
    fn get_lru_nodes_by_frequency(&self, count: usize) -> Vec<u64> {
        let mut nodes: Vec<_> = self
            .access_counts
            .iter()
            .map(|entry| (*entry.key(), *entry.value()))
            .collect();

        // Sort by access count (ascending - least frequently accessed first)
        nodes.sort_by_key(|(_, access_count)| *access_count);
        nodes
            .into_iter()
            .take(count)
            .map(|(node_id, _)| node_id)
            .collect()
    }
}

/// Node data structure
#[derive(Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
pub struct NodeData {
    pub id: u64,
    pub labels: Vec<String>,
    pub properties: Vec<(String, CachePropertyValue)>,
}

/// Property value types for cache storage
#[derive(Clone, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
pub enum CachePropertyValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
}

/// Hot/cold storage facade
pub struct HotColdStorage {
    cache_hierarchy: CacheHierarchy,
}

impl HotColdStorage {
    pub fn new() -> Self {
        Self {
            cache_hierarchy: CacheHierarchy::new(1000, 10000),
        }
    }

    pub fn get(&self, node_id: u64) -> Option<NodeData> {
        self.cache_hierarchy.get_node(node_id)
    }

    pub fn insert(&self, node_id: u64, data: NodeData) {
        self.cache_hierarchy.insert_node(node_id, data);
    }

    pub fn prefetch(&self, node_ids: &[u64]) {
        self.cache_hierarchy.prefetch_neighbors(node_ids);
    }
}

impl Default for HotColdStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_cache_hierarchy() {
        let cache = CacheHierarchy::new(10, 100);

        let data = NodeData {
            id: 1,
            labels: vec!["Person".to_string()],
            properties: vec![(
                "name".to_string(),
                CachePropertyValue::String("Alice".to_string()),
            )],
        };

        cache.insert_node(1, data.clone());

        let retrieved = cache.get_node(1);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_hot_cold_promotion() {
        let cache = CacheHierarchy::new(2, 10);

        // Insert 3 nodes (exceeds hot capacity)
        for i in 1..=3 {
            cache.insert_node(
                i,
                NodeData {
                    id: i,
                    labels: vec![],
                    properties: vec![],
                },
            );
        }

        // Access node 1 multiple times to trigger promotion
        for _ in 0..10 {
            cache.get_node(1);
        }

        // Node 1 should still be accessible
        assert!(cache.get_node(1).is_some());
    }
}
