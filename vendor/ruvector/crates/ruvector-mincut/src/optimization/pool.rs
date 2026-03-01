//! Pool Allocators and Lazy Level Deallocation
//!
//! Memory-efficient allocation strategies:
//! - Pool allocators for frequent allocations
//! - Lazy deallocation of unused j-tree levels
//! - Compact representations (u16 for small graphs)
//! - Demand-paged level materialization
//!
//! Target: 50-75% memory reduction

use crate::graph::VertexId;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

/// Configuration for level pool
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum number of materialized levels
    pub max_materialized_levels: usize,
    /// Eviction threshold (levels unused for this many operations)
    pub eviction_threshold: u64,
    /// Preallocation size for level data
    pub prealloc_size: usize,
    /// Enable lazy deallocation
    pub lazy_dealloc: bool,
    /// Memory budget in bytes (0 = unlimited)
    pub memory_budget: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_materialized_levels: 16,
            eviction_threshold: 100,
            prealloc_size: 1024,
            lazy_dealloc: true,
            memory_budget: 0,
        }
    }
}

/// Statistics for pool allocation
#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    /// Total allocations
    pub allocations: u64,
    /// Total deallocations
    pub deallocations: u64,
    /// Current pool size (bytes)
    pub pool_size_bytes: usize,
    /// Number of materialized levels
    pub materialized_levels: usize,
    /// Number of evictions
    pub evictions: u64,
    /// Peak memory usage (bytes)
    pub peak_memory: usize,
}

/// State of a lazy level in the j-tree
#[derive(Debug, Clone)]
pub enum LazyLevel {
    /// Level not yet materialized
    Unmaterialized,
    /// Level is materialized and valid
    Materialized(LevelData),
    /// Level is materialized but dirty (needs recomputation)
    Dirty(LevelData),
    /// Level was evicted (can be recomputed)
    Evicted {
        /// Last known vertex count (for preallocation)
        last_vertex_count: usize,
    },
}

impl LazyLevel {
    /// Check if level is materialized
    pub fn is_materialized(&self) -> bool {
        matches!(self, LazyLevel::Materialized(_) | LazyLevel::Dirty(_))
    }

    /// Check if level needs recomputation
    pub fn is_dirty(&self) -> bool {
        matches!(self, LazyLevel::Dirty(_))
    }

    /// Get level data if materialized
    pub fn data(&self) -> Option<&LevelData> {
        match self {
            LazyLevel::Materialized(data) | LazyLevel::Dirty(data) => Some(data),
            _ => None,
        }
    }

    /// Get mutable level data if materialized
    pub fn data_mut(&mut self) -> Option<&mut LevelData> {
        match self {
            LazyLevel::Materialized(data) | LazyLevel::Dirty(data) => Some(data),
            _ => None,
        }
    }
}

/// Data stored for a j-tree level
#[derive(Debug, Clone)]
pub struct LevelData {
    /// Level index
    pub level: usize,
    /// Vertices in this level (compact representation)
    pub vertices: Vec<u16>,
    /// Adjacency list (compact)
    pub adjacency: CompactAdjacency,
    /// Cut value for this level
    pub cut_value: f64,
    /// Last access timestamp
    last_access: u64,
    /// Memory size in bytes
    memory_size: usize,
}

impl LevelData {
    /// Create new level data
    pub fn new(level: usize, capacity: usize) -> Self {
        Self {
            level,
            vertices: Vec::with_capacity(capacity),
            adjacency: CompactAdjacency::new(capacity),
            cut_value: f64::INFINITY,
            last_access: 0,
            memory_size: 0,
        }
    }

    /// Update memory size estimate
    pub fn update_memory_size(&mut self) {
        self.memory_size =
            self.vertices.len() * std::mem::size_of::<u16>() + self.adjacency.memory_size();
    }

    /// Get memory size
    pub fn memory_size(&self) -> usize {
        self.memory_size
    }
}

/// Compact adjacency list using u16 vertex IDs
#[derive(Debug, Clone)]
pub struct CompactAdjacency {
    /// Offset for each vertex into neighbors array
    offsets: Vec<u32>,
    /// Packed neighbors (vertex_id, weight as u16)
    neighbors: Vec<(u16, u16)>,
}

impl CompactAdjacency {
    /// Create new compact adjacency
    pub fn new(capacity: usize) -> Self {
        Self {
            offsets: Vec::with_capacity(capacity + 1),
            neighbors: Vec::new(),
        }
    }

    /// Build from edge list
    pub fn from_edges(edges: &[(u16, u16, u16)], num_vertices: usize) -> Self {
        let mut adj: Vec<Vec<(u16, u16)>> = vec![Vec::new(); num_vertices];

        for &(u, v, w) in edges {
            adj[u as usize].push((v, w));
            adj[v as usize].push((u, w));
        }

        let mut offsets = Vec::with_capacity(num_vertices + 1);
        let mut neighbors = Vec::new();

        offsets.push(0);
        for vertex_neighbors in &adj {
            neighbors.extend_from_slice(vertex_neighbors);
            offsets.push(neighbors.len() as u32);
        }

        Self { offsets, neighbors }
    }

    /// Get neighbors of vertex
    pub fn neighbors(&self, v: u16) -> &[(u16, u16)] {
        let idx = v as usize;
        if idx + 1 >= self.offsets.len() {
            return &[];
        }
        let start = self.offsets[idx] as usize;
        let end = self.offsets[idx + 1] as usize;
        &self.neighbors[start..end]
    }

    /// Get degree of vertex
    pub fn degree(&self, v: u16) -> usize {
        let idx = v as usize;
        if idx + 1 >= self.offsets.len() {
            return 0;
        }
        (self.offsets[idx + 1] - self.offsets[idx]) as usize
    }

    /// Memory size in bytes
    pub fn memory_size(&self) -> usize {
        self.offsets.len() * std::mem::size_of::<u32>()
            + self.neighbors.len() * std::mem::size_of::<(u16, u16)>()
    }

    /// Number of vertices
    pub fn num_vertices(&self) -> usize {
        if self.offsets.is_empty() {
            0
        } else {
            self.offsets.len() - 1
        }
    }
}

/// Pool allocator for j-tree levels
pub struct LevelPool {
    config: PoolConfig,
    /// Levels storage
    levels: RwLock<HashMap<usize, LazyLevel>>,
    /// LRU tracking
    lru_order: RwLock<VecDeque<usize>>,
    /// Operation counter
    operation_counter: AtomicU64,
    /// Current memory usage
    memory_usage: AtomicUsize,
    /// Statistics
    allocations: AtomicU64,
    deallocations: AtomicU64,
    evictions: AtomicU64,
    peak_memory: AtomicUsize,
    /// Free list for reusable allocations
    free_list: RwLock<Vec<LevelData>>,
}

impl LevelPool {
    /// Create new level pool with default config
    pub fn new() -> Self {
        Self::with_config(PoolConfig::default())
    }

    /// Create with custom config
    pub fn with_config(config: PoolConfig) -> Self {
        Self {
            config,
            levels: RwLock::new(HashMap::new()),
            lru_order: RwLock::new(VecDeque::new()),
            operation_counter: AtomicU64::new(0),
            memory_usage: AtomicUsize::new(0),
            allocations: AtomicU64::new(0),
            deallocations: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            peak_memory: AtomicUsize::new(0),
            free_list: RwLock::new(Vec::new()),
        }
    }

    /// Get or materialize a level
    pub fn get_level(&self, level_idx: usize) -> Option<LazyLevel> {
        self.touch(level_idx);

        let levels = self.levels.read().unwrap();
        levels.get(&level_idx).cloned()
    }

    /// Check if level is materialized
    pub fn is_materialized(&self, level_idx: usize) -> bool {
        let levels = self.levels.read().unwrap();
        levels
            .get(&level_idx)
            .map(|l| l.is_materialized())
            .unwrap_or(false)
    }

    /// Materialize a level with data
    pub fn materialize(&self, level_idx: usize, data: LevelData) {
        self.ensure_capacity();

        let memory_size = data.memory_size();
        self.memory_usage.fetch_add(memory_size, Ordering::Relaxed);

        // Update peak memory
        let current = self.memory_usage.load(Ordering::Relaxed);
        let peak = self.peak_memory.load(Ordering::Relaxed);
        if current > peak {
            self.peak_memory.store(current, Ordering::Relaxed);
        }

        let mut levels = self.levels.write().unwrap();
        levels.insert(level_idx, LazyLevel::Materialized(data));

        let mut lru = self.lru_order.write().unwrap();
        lru.retain(|&l| l != level_idx);
        lru.push_back(level_idx);

        self.allocations.fetch_add(1, Ordering::Relaxed);
    }

    /// Mark level as dirty
    pub fn mark_dirty(&self, level_idx: usize) {
        let mut levels = self.levels.write().unwrap();
        if let Some(level) = levels.get_mut(&level_idx) {
            if let LazyLevel::Materialized(data) = level.clone() {
                *level = LazyLevel::Dirty(data);
            }
        }
    }

    /// Mark level as clean (after recomputation)
    pub fn mark_clean(&self, level_idx: usize) {
        let mut levels = self.levels.write().unwrap();
        if let Some(level) = levels.get_mut(&level_idx) {
            if let LazyLevel::Dirty(data) = level.clone() {
                *level = LazyLevel::Materialized(data);
            }
        }
    }

    /// Evict a level (lazy deallocation)
    pub fn evict(&self, level_idx: usize) {
        let mut levels = self.levels.write().unwrap();

        if let Some(level) = levels.get(&level_idx) {
            let last_vertex_count = level.data().map(|d| d.vertices.len()).unwrap_or(0);

            let memory_freed = level.data().map(|d| d.memory_size()).unwrap_or(0);

            // Try to recycle the allocation
            if self.config.lazy_dealloc {
                if let Some(data) = level.data().cloned() {
                    let mut free_list = self.free_list.write().unwrap();
                    if free_list.len() < 10 {
                        free_list.push(data);
                    }
                }
            }

            levels.insert(level_idx, LazyLevel::Evicted { last_vertex_count });

            self.memory_usage.fetch_sub(memory_freed, Ordering::Relaxed);
            self.evictions.fetch_add(1, Ordering::Relaxed);
            self.deallocations.fetch_add(1, Ordering::Relaxed);
        }

        let mut lru = self.lru_order.write().unwrap();
        lru.retain(|&l| l != level_idx);
    }

    /// Ensure we have capacity (evict if needed)
    fn ensure_capacity(&self) {
        let levels = self.levels.read().unwrap();
        let materialized_count = levels.values().filter(|l| l.is_materialized()).count();
        drop(levels);

        if materialized_count >= self.config.max_materialized_levels {
            // Evict least recently used
            let lru = self.lru_order.read().unwrap();
            if let Some(&evict_idx) = lru.front() {
                drop(lru);
                self.evict(evict_idx);
            }
        }

        // Also check memory budget
        if self.config.memory_budget > 0 {
            while self.memory_usage.load(Ordering::Relaxed) > self.config.memory_budget {
                let lru = self.lru_order.read().unwrap();
                if let Some(&evict_idx) = lru.front() {
                    drop(lru);
                    self.evict(evict_idx);
                } else {
                    break;
                }
            }
        }
    }

    /// Update access timestamp for level
    fn touch(&self, level_idx: usize) {
        let timestamp = self.operation_counter.fetch_add(1, Ordering::Relaxed);

        let mut levels = self.levels.write().unwrap();
        if let Some(level) = levels.get_mut(&level_idx) {
            if let Some(data) = level.data_mut() {
                data.last_access = timestamp;
            }
        }
        drop(levels);

        // Update LRU order
        let mut lru = self.lru_order.write().unwrap();
        lru.retain(|&l| l != level_idx);
        lru.push_back(level_idx);
    }

    /// Get a recycled allocation or create new
    pub fn allocate_level(&self, level_idx: usize, capacity: usize) -> LevelData {
        // Try to get from free list
        let mut free_list = self.free_list.write().unwrap();
        if let Some(mut data) = free_list.pop() {
            data.level = level_idx;
            data.vertices.clear();
            data.cut_value = f64::INFINITY;
            return data;
        }
        drop(free_list);

        // Allocate new
        LevelData::new(level_idx, capacity)
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        let levels = self.levels.read().unwrap();
        let materialized_count = levels.values().filter(|l| l.is_materialized()).count();

        PoolStats {
            allocations: self.allocations.load(Ordering::Relaxed),
            deallocations: self.deallocations.load(Ordering::Relaxed),
            pool_size_bytes: self.memory_usage.load(Ordering::Relaxed),
            materialized_levels: materialized_count,
            evictions: self.evictions.load(Ordering::Relaxed),
            peak_memory: self.peak_memory.load(Ordering::Relaxed),
        }
    }

    /// Get current memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        self.memory_usage.load(Ordering::Relaxed)
    }

    /// Clear all levels
    pub fn clear(&self) {
        let mut levels = self.levels.write().unwrap();
        levels.clear();

        let mut lru = self.lru_order.write().unwrap();
        lru.clear();

        self.memory_usage.store(0, Ordering::Relaxed);
    }
}

impl Default for LevelPool {
    fn default() -> Self {
        Self::new()
    }
}

/// Vertex ID converter for compact representations
pub struct CompactVertexMapper {
    /// Original vertex ID to compact ID
    to_compact: HashMap<VertexId, u16>,
    /// Compact ID to original vertex ID
    to_original: Vec<VertexId>,
    /// Next compact ID
    next_id: u16,
}

impl CompactVertexMapper {
    /// Create new mapper
    pub fn new() -> Self {
        Self {
            to_compact: HashMap::new(),
            to_original: Vec::new(),
            next_id: 0,
        }
    }

    /// Create from vertex list
    pub fn from_vertices(vertices: &[VertexId]) -> Self {
        let mut mapper = Self::new();
        for &v in vertices {
            mapper.get_or_insert(v);
        }
        mapper
    }

    /// Get compact ID, creating if needed
    pub fn get_or_insert(&mut self, original: VertexId) -> u16 {
        if let Some(&compact) = self.to_compact.get(&original) {
            return compact;
        }

        let compact = self.next_id;
        self.next_id += 1;
        self.to_compact.insert(original, compact);
        self.to_original.push(original);
        compact
    }

    /// Get compact ID if exists
    pub fn get(&self, original: VertexId) -> Option<u16> {
        self.to_compact.get(&original).copied()
    }

    /// Get original vertex ID from compact
    pub fn to_original(&self, compact: u16) -> Option<VertexId> {
        self.to_original.get(compact as usize).copied()
    }

    /// Number of mapped vertices
    pub fn len(&self) -> usize {
        self.to_original.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.to_original.is_empty()
    }
}

impl Default for CompactVertexMapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lazy_level_states() {
        let level = LazyLevel::Unmaterialized;
        assert!(!level.is_materialized());

        let data = LevelData::new(0, 100);
        let level = LazyLevel::Materialized(data.clone());
        assert!(level.is_materialized());
        assert!(!level.is_dirty());

        let level = LazyLevel::Dirty(data);
        assert!(level.is_materialized());
        assert!(level.is_dirty());
    }

    #[test]
    fn test_compact_adjacency() {
        let edges = vec![(0u16, 1u16, 10u16), (1, 2, 20), (2, 0, 30)];

        let adj = CompactAdjacency::from_edges(&edges, 3);

        assert_eq!(adj.num_vertices(), 3);
        assert_eq!(adj.degree(0), 2);
        assert_eq!(adj.degree(1), 2);
        assert_eq!(adj.degree(2), 2);
    }

    #[test]
    fn test_level_pool_materialize() {
        let pool = LevelPool::new();

        let data = LevelData::new(0, 100);
        pool.materialize(0, data);

        assert!(pool.is_materialized(0));
        assert!(!pool.is_materialized(1));
    }

    #[test]
    fn test_level_pool_eviction() {
        let pool = LevelPool::with_config(PoolConfig {
            max_materialized_levels: 2,
            ..Default::default()
        });

        pool.materialize(0, LevelData::new(0, 100));
        pool.materialize(1, LevelData::new(1, 100));

        assert!(pool.is_materialized(0));
        assert!(pool.is_materialized(1));

        // This should evict level 0
        pool.materialize(2, LevelData::new(2, 100));

        assert!(!pool.is_materialized(0));
        assert!(pool.is_materialized(1));
        assert!(pool.is_materialized(2));
    }

    #[test]
    fn test_level_pool_dirty() {
        let pool = LevelPool::new();

        let data = LevelData::new(0, 100);
        pool.materialize(0, data);

        pool.mark_dirty(0);

        if let Some(LazyLevel::Dirty(_)) = pool.get_level(0) {
            // OK
        } else {
            panic!("Level should be dirty");
        }

        pool.mark_clean(0);

        if let Some(LazyLevel::Materialized(_)) = pool.get_level(0) {
            // OK
        } else {
            panic!("Level should be clean");
        }
    }

    #[test]
    fn test_compact_vertex_mapper() {
        let mut mapper = CompactVertexMapper::new();

        let c1 = mapper.get_or_insert(100);
        let c2 = mapper.get_or_insert(200);
        let c3 = mapper.get_or_insert(100); // Should return same as c1

        assert_eq!(c1, 0);
        assert_eq!(c2, 1);
        assert_eq!(c3, 0);

        assert_eq!(mapper.to_original(c1), Some(100));
        assert_eq!(mapper.to_original(c2), Some(200));
    }

    #[test]
    fn test_pool_stats() {
        let pool = LevelPool::new();

        let data = LevelData::new(0, 100);
        pool.materialize(0, data);

        let stats = pool.stats();
        assert_eq!(stats.allocations, 1);
        assert_eq!(stats.materialized_levels, 1);
    }

    #[test]
    fn test_level_data_memory_size() {
        let mut data = LevelData::new(0, 100);
        data.vertices = vec![0, 1, 2, 3, 4];
        data.update_memory_size();

        assert!(data.memory_size() > 0);
    }
}
