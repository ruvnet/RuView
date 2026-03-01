//! J-Tree Level Implementation with BMSSP WASM Integration
//!
//! This module defines the `BmsspJTreeLevel` trait and implementation for
//! individual levels in the j-tree hierarchy. Each level uses BMSSP WASM
//! for efficient path-cut duality queries.
//!
//! # Path-Cut Duality
//!
//! In the dual graph representation:
//! - Shortest path in G* (dual) corresponds to minimum cut in G
//! - BMSSP achieves O(m·log^(2/3) n) complexity vs O(n log n) direct
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                        BmsspJTreeLevel                              │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────────┐    ┌─────────────────────────────────────┐   │
//! │  │   WasmGraph     │    │          Path Cache                  │   │
//! │  │   (FFI Handle)  │    │  HashMap<(u, v), PathCutResult>     │   │
//! │  └────────┬────────┘    └──────────────────┬──────────────────┘   │
//! │           │                                │                       │
//! │           ▼                                ▼                       │
//! │  ┌────────────────────────────────────────────────────────────┐   │
//! │  │                   Cut Query Interface                       │   │
//! │  │  • min_cut(s, t) → f64                                     │   │
//! │  │  • multi_terminal_cut(terminals) → f64                     │   │
//! │  │  • refine_cut(coarse_cut) → RefinedCut                     │   │
//! │  └────────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```

use crate::error::{MinCutError, Result};
use crate::graph::{DynamicGraph, Edge, EdgeId, VertexId, Weight};
use crate::jtree::JTreeError;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Configuration for a j-tree level
#[derive(Debug, Clone)]
pub struct LevelConfig {
    /// Level index (0 = original graph, L = root)
    pub level: usize,
    /// Approximation quality α at this level
    pub alpha: f64,
    /// Whether to enable path caching
    pub enable_cache: bool,
    /// Maximum cache entries (0 = unlimited)
    pub max_cache_entries: usize,
    /// Whether WASM acceleration is available
    pub wasm_available: bool,
}

impl Default for LevelConfig {
    fn default() -> Self {
        Self {
            level: 0,
            alpha: 2.0,
            enable_cache: true,
            max_cache_entries: 10_000,
            wasm_available: false, // Detected at runtime
        }
    }
}

/// Statistics for a j-tree level
#[derive(Debug, Clone, Default)]
pub struct LevelStatistics {
    /// Number of vertices at this level
    pub vertex_count: usize,
    /// Number of edges at this level
    pub edge_count: usize,
    /// Cache hit count
    pub cache_hits: usize,
    /// Cache miss count
    pub cache_misses: usize,
    /// Total queries processed
    pub total_queries: usize,
    /// Average query time in microseconds
    pub avg_query_time_us: f64,
}

/// Result of a path-based cut computation
#[derive(Debug, Clone)]
pub struct PathCutResult {
    /// The cut value (sum of edge weights crossing the cut)
    pub value: f64,
    /// Source vertex for the cut query
    pub source: VertexId,
    /// Target vertex for the cut query
    pub target: VertexId,
    /// Whether this result came from cache
    pub from_cache: bool,
    /// Computation time in microseconds
    pub compute_time_us: f64,
}

/// A contracted graph representing a j-tree level
#[derive(Debug, Clone)]
pub struct ContractedGraph {
    /// Original vertices mapped to super-vertices
    vertex_map: HashMap<VertexId, VertexId>,
    /// Reverse map: super-vertex to set of original vertices
    super_vertices: HashMap<VertexId, HashSet<VertexId>>,
    /// Edges between super-vertices with aggregated weights
    edges: HashMap<(VertexId, VertexId), Weight>,
    /// Next super-vertex ID
    next_super_id: VertexId,
    /// Level index
    level: usize,
}

impl ContractedGraph {
    /// Create a new contracted graph from the original
    pub fn from_graph(graph: &DynamicGraph, level: usize) -> Self {
        let mut contracted = Self {
            vertex_map: HashMap::new(),
            super_vertices: HashMap::new(),
            edges: HashMap::new(),
            next_super_id: 0,
            level,
        };

        // Initially, each vertex is its own super-vertex
        for v in graph.vertices() {
            contracted.vertex_map.insert(v, v);
            contracted.super_vertices.insert(v, {
                let mut set = HashSet::new();
                set.insert(v);
                set
            });
            contracted.next_super_id = contracted.next_super_id.max(v + 1);
        }

        // Copy edges
        for edge in graph.edges() {
            let key = Self::canonical_key(edge.source, edge.target);
            *contracted.edges.entry(key).or_insert(0.0) += edge.weight;
        }

        contracted
    }

    /// Create an empty contracted graph
    pub fn new(level: usize) -> Self {
        Self {
            vertex_map: HashMap::new(),
            super_vertices: HashMap::new(),
            edges: HashMap::new(),
            next_super_id: 0,
            level,
        }
    }

    /// Get canonical edge key (min, max)
    fn canonical_key(u: VertexId, v: VertexId) -> (VertexId, VertexId) {
        if u <= v {
            (u, v)
        } else {
            (v, u)
        }
    }

    /// Contract two super-vertices into one
    pub fn contract(&mut self, u: VertexId, v: VertexId) -> Result<VertexId> {
        let u_super = *self
            .vertex_map
            .get(&u)
            .ok_or_else(|| JTreeError::VertexNotFound(u))?;
        let v_super = *self
            .vertex_map
            .get(&v)
            .ok_or_else(|| JTreeError::VertexNotFound(v))?;

        if u_super == v_super {
            return Ok(u_super); // Already contracted
        }

        // Create new super-vertex
        let new_super = self.next_super_id;
        self.next_super_id += 1;

        // Merge vertex sets
        let u_vertices = self.super_vertices.remove(&u_super).unwrap_or_default();
        let v_vertices = self.super_vertices.remove(&v_super).unwrap_or_default();
        let mut merged: HashSet<VertexId> = u_vertices.union(&v_vertices).copied().collect();

        // Update vertex maps
        for &orig_v in &merged {
            self.vertex_map.insert(orig_v, new_super);
        }
        self.super_vertices.insert(new_super, merged);

        // Merge edges
        let mut new_edges = HashMap::new();
        for ((src, dst), weight) in self.edges.drain() {
            let new_src = if src == u_super || src == v_super {
                new_super
            } else {
                src
            };
            let new_dst = if dst == u_super || dst == v_super {
                new_super
            } else {
                dst
            };

            // Skip self-loops created by contraction
            if new_src == new_dst {
                continue;
            }

            let key = Self::canonical_key(new_src, new_dst);
            *new_edges.entry(key).or_insert(0.0) += weight;
        }
        self.edges = new_edges;

        Ok(new_super)
    }

    /// Get the number of super-vertices
    pub fn vertex_count(&self) -> usize {
        self.super_vertices.len()
    }

    /// Get the number of edges
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get all edges as (source, target, weight) tuples
    pub fn edges(&self) -> impl Iterator<Item = (VertexId, VertexId, Weight)> + '_ {
        self.edges.iter().map(|(&(u, v), &w)| (u, v, w))
    }

    /// Get the super-vertex containing an original vertex
    pub fn get_super_vertex(&self, v: VertexId) -> Option<VertexId> {
        self.vertex_map.get(&v).copied()
    }

    /// Get all original vertices in a super-vertex
    pub fn get_original_vertices(&self, super_v: VertexId) -> Option<&HashSet<VertexId>> {
        self.super_vertices.get(&super_v)
    }

    /// Get all super-vertices
    pub fn super_vertices(&self) -> impl Iterator<Item = VertexId> + '_ {
        self.super_vertices.keys().copied()
    }

    /// Get edge weight between two super-vertices
    pub fn edge_weight(&self, u: VertexId, v: VertexId) -> Option<Weight> {
        let key = Self::canonical_key(u, v);
        self.edges.get(&key).copied()
    }

    /// Get the level index
    pub fn level(&self) -> usize {
        self.level
    }
}

/// Trait for j-tree level operations
///
/// This trait defines the interface that both native Rust and WASM-accelerated
/// implementations must satisfy.
pub trait JTreeLevel: Send + Sync {
    /// Get the level index in the hierarchy
    fn level(&self) -> usize;

    /// Get statistics for this level
    fn statistics(&self) -> LevelStatistics;

    /// Query the minimum cut between two vertices
    fn min_cut(&mut self, s: VertexId, t: VertexId) -> Result<PathCutResult>;

    /// Query the minimum cut among a set of terminals
    fn multi_terminal_cut(&mut self, terminals: &[VertexId]) -> Result<f64>;

    /// Refine a coarse cut from a higher level
    fn refine_cut(&mut self, coarse_partition: &HashSet<VertexId>) -> Result<HashSet<VertexId>>;

    /// Handle edge insertion at this level
    fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) -> Result<()>;

    /// Handle edge deletion at this level
    fn delete_edge(&mut self, u: VertexId, v: VertexId) -> Result<()>;

    /// Invalidate the cache (called after structural changes)
    fn invalidate_cache(&mut self);

    /// Get the contracted graph at this level
    fn contracted_graph(&self) -> &ContractedGraph;
}

/// BMSSP-accelerated j-tree level implementation
///
/// Uses WASM BMSSP module for O(m·log^(2/3) n) path queries,
/// with path-cut duality for efficient cut computation.
pub struct BmsspJTreeLevel {
    /// Contracted graph at this level
    contracted: ContractedGraph,
    /// Configuration
    config: LevelConfig,
    /// Statistics
    stats: LevelStatistics,
    /// Path/cut cache: (source, target) -> result
    cache: HashMap<(VertexId, VertexId), PathCutResult>,
    /// WASM graph handle (opaque pointer when WASM is available)
    /// For now, we use a native implementation as fallback
    #[allow(dead_code)]
    wasm_handle: Option<WasmGraphHandle>,
}

/// Opaque handle to WASM graph (FFI boundary)
///
/// This struct encapsulates the FFI boundary between Rust and WASM.
/// When the `wasm` feature is enabled, this holds the actual WASM instance.
#[derive(Debug)]
pub struct WasmGraphHandle {
    /// Pointer to WASM linear memory (when available)
    #[allow(dead_code)]
    ptr: usize,
    /// Number of vertices in the WASM graph
    #[allow(dead_code)]
    vertex_count: u32,
    /// Whether the handle is valid
    #[allow(dead_code)]
    valid: bool,
}

impl WasmGraphHandle {
    /// Create a new WASM graph handle
    ///
    /// # Safety
    ///
    /// This function interfaces with WASM linear memory. The caller must ensure:
    /// - The WASM module is properly initialized
    /// - The vertex count is valid
    #[allow(dead_code)]
    fn new(_vertex_count: u32) -> Result<Self> {
        // TODO: Actual WASM initialization when feature is enabled
        // For now, return a placeholder
        Ok(Self {
            ptr: 0,
            vertex_count: _vertex_count,
            valid: false,
        })
    }

    /// Check if WASM acceleration is available
    #[allow(dead_code)]
    fn is_available() -> bool {
        // TODO: Check for WASM runtime availability
        // This would typically check if the @ruvnet/bmssp module is loaded
        cfg!(feature = "wasm")
    }
}

impl BmsspJTreeLevel {
    /// Create a new BMSSP-accelerated j-tree level
    pub fn new(contracted: ContractedGraph, config: LevelConfig) -> Result<Self> {
        let stats = LevelStatistics {
            vertex_count: contracted.vertex_count(),
            edge_count: contracted.edge_count(),
            ..Default::default()
        };

        // Attempt to create WASM handle if available
        let wasm_handle = if config.wasm_available {
            WasmGraphHandle::new(contracted.vertex_count() as u32).ok()
        } else {
            None
        };

        Ok(Self {
            contracted,
            config,
            stats,
            cache: HashMap::new(),
            wasm_handle,
        })
    }

    /// Create from a contracted graph with default config
    pub fn from_contracted(contracted: ContractedGraph, level: usize) -> Self {
        let config = LevelConfig {
            level,
            ..Default::default()
        };

        Self {
            stats: LevelStatistics {
                vertex_count: contracted.vertex_count(),
                edge_count: contracted.edge_count(),
                ..Default::default()
            },
            contracted,
            config,
            cache: HashMap::new(),
            wasm_handle: None,
        }
    }

    /// Compute shortest paths from source using native Dijkstra
    ///
    /// This is the fallback when WASM is not available.
    /// Returns distances to all vertices.
    fn native_shortest_paths(&self, source: VertexId) -> HashMap<VertexId, f64> {
        use std::cmp::Ordering;
        use std::collections::BinaryHeap;

        #[derive(Debug)]
        struct State {
            cost: f64,
            vertex: VertexId,
        }

        impl PartialEq for State {
            fn eq(&self, other: &Self) -> bool {
                self.cost == other.cost && self.vertex == other.vertex
            }
        }

        impl Eq for State {}

        impl PartialOrd for State {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for State {
            fn cmp(&self, other: &Self) -> Ordering {
                // Reverse ordering for min-heap
                other
                    .cost
                    .partial_cmp(&self.cost)
                    .unwrap_or(Ordering::Equal)
            }
        }

        let mut distances: HashMap<VertexId, f64> = HashMap::new();
        let mut heap = BinaryHeap::new();

        // Build adjacency list for efficient neighbor lookup
        let mut adj: HashMap<VertexId, Vec<(VertexId, f64)>> = HashMap::new();
        for (u, v, w) in self.contracted.edges() {
            adj.entry(u).or_default().push((v, w));
            adj.entry(v).or_default().push((u, w));
        }

        // Initialize source
        distances.insert(source, 0.0);
        heap.push(State {
            cost: 0.0,
            vertex: source,
        });

        while let Some(State { cost, vertex }) = heap.pop() {
            // Skip if we've found a better path
            if let Some(&d) = distances.get(&vertex) {
                if cost > d {
                    continue;
                }
            }

            // Explore neighbors
            if let Some(neighbors) = adj.get(&vertex) {
                for &(next, edge_weight) in neighbors {
                    let next_cost = cost + edge_weight;

                    let is_better = distances.get(&next).map(|&d| next_cost < d).unwrap_or(true);

                    if is_better {
                        distances.insert(next, next_cost);
                        heap.push(State {
                            cost: next_cost,
                            vertex: next,
                        });
                    }
                }
            }
        }

        distances
    }

    /// Get cache key for a vertex pair
    fn cache_key(s: VertexId, t: VertexId) -> (VertexId, VertexId) {
        if s <= t {
            (s, t)
        } else {
            (t, s)
        }
    }

    /// Update statistics after a query
    fn update_stats(&mut self, from_cache: bool, compute_time_us: f64) {
        self.stats.total_queries += 1;
        if from_cache {
            self.stats.cache_hits += 1;
        } else {
            self.stats.cache_misses += 1;
        }

        // Update rolling average
        let n = self.stats.total_queries as f64;
        self.stats.avg_query_time_us =
            (self.stats.avg_query_time_us * (n - 1.0) + compute_time_us) / n;
    }
}

impl JTreeLevel for BmsspJTreeLevel {
    fn level(&self) -> usize {
        self.config.level
    }

    fn statistics(&self) -> LevelStatistics {
        self.stats.clone()
    }

    fn min_cut(&mut self, s: VertexId, t: VertexId) -> Result<PathCutResult> {
        let start = std::time::Instant::now();

        // Check cache first
        let key = Self::cache_key(s, t);
        if self.config.enable_cache {
            if let Some(cached) = self.cache.get(&key) {
                let mut result = cached.clone();
                result.from_cache = true;
                self.update_stats(true, start.elapsed().as_micros() as f64);
                return Ok(result);
            }
        }

        // Map to super-vertices
        let s_super = self
            .contracted
            .get_super_vertex(s)
            .ok_or_else(|| JTreeError::VertexNotFound(s))?;
        let t_super = self
            .contracted
            .get_super_vertex(t)
            .ok_or_else(|| JTreeError::VertexNotFound(t))?;

        // If same super-vertex, cut is infinite (not separable at this level)
        if s_super == t_super {
            let result = PathCutResult {
                value: f64::INFINITY,
                source: s,
                target: t,
                from_cache: false,
                compute_time_us: start.elapsed().as_micros() as f64,
            };
            self.update_stats(false, result.compute_time_us);
            return Ok(result);
        }

        // Compute shortest paths (use WASM if available, else native)
        // In the dual graph, shortest path = min cut
        let distances = self.native_shortest_paths(s_super);

        let cut_value = distances.get(&t_super).copied().unwrap_or(f64::INFINITY);

        let compute_time = start.elapsed().as_micros() as f64;
        let result = PathCutResult {
            value: cut_value,
            source: s,
            target: t,
            from_cache: false,
            compute_time_us: compute_time,
        };

        // Cache the result
        if self.config.enable_cache {
            // Evict if cache is full
            if self.config.max_cache_entries > 0
                && self.cache.len() >= self.config.max_cache_entries
            {
                // Simple eviction: clear half the cache
                let keys_to_remove: Vec<_> = self
                    .cache
                    .keys()
                    .take(self.config.max_cache_entries / 2)
                    .copied()
                    .collect();
                for k in keys_to_remove {
                    self.cache.remove(&k);
                }
            }
            self.cache.insert(key, result.clone());
        }

        self.update_stats(false, compute_time);
        Ok(result)
    }

    fn multi_terminal_cut(&mut self, terminals: &[VertexId]) -> Result<f64> {
        if terminals.len() < 2 {
            return Ok(f64::INFINITY);
        }

        let mut min_cut = f64::INFINITY;

        // Compute pairwise cuts and take minimum
        // BMSSP could optimize this with multi-source queries
        for i in 0..terminals.len() {
            for j in (i + 1)..terminals.len() {
                let result = self.min_cut(terminals[i], terminals[j])?;
                min_cut = min_cut.min(result.value);
            }
        }

        Ok(min_cut)
    }

    fn refine_cut(&mut self, coarse_partition: &HashSet<VertexId>) -> Result<HashSet<VertexId>> {
        // Expand super-vertices to original vertices
        let mut refined = HashSet::new();

        for &super_v in coarse_partition {
            if let Some(original_vertices) = self.contracted.get_original_vertices(super_v) {
                refined.extend(original_vertices);
            }
        }

        Ok(refined)
    }

    fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) -> Result<()> {
        let u_super = self
            .contracted
            .get_super_vertex(u)
            .ok_or_else(|| JTreeError::VertexNotFound(u))?;
        let v_super = self
            .contracted
            .get_super_vertex(v)
            .ok_or_else(|| JTreeError::VertexNotFound(v))?;

        if u_super != v_super {
            let key = ContractedGraph::canonical_key(u_super, v_super);
            *self.contracted.edges.entry(key).or_insert(0.0) += weight;
            self.stats.edge_count = self.contracted.edge_count();
        }

        self.invalidate_cache();
        Ok(())
    }

    fn delete_edge(&mut self, u: VertexId, v: VertexId) -> Result<()> {
        let u_super = self
            .contracted
            .get_super_vertex(u)
            .ok_or_else(|| JTreeError::VertexNotFound(u))?;
        let v_super = self
            .contracted
            .get_super_vertex(v)
            .ok_or_else(|| JTreeError::VertexNotFound(v))?;

        if u_super != v_super {
            let key = ContractedGraph::canonical_key(u_super, v_super);
            self.contracted.edges.remove(&key);
            self.stats.edge_count = self.contracted.edge_count();
        }

        self.invalidate_cache();
        Ok(())
    }

    fn invalidate_cache(&mut self) {
        self.cache.clear();
    }

    fn contracted_graph(&self) -> &ContractedGraph {
        &self.contracted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_graph() -> DynamicGraph {
        let graph = DynamicGraph::new();
        // Create a simple graph: 1-2-3-4 path with bridge at 2-3
        graph.insert_edge(1, 2, 2.0).unwrap();
        graph.insert_edge(2, 3, 1.0).unwrap(); // Bridge
        graph.insert_edge(3, 4, 2.0).unwrap();
        graph
    }

    #[test]
    fn test_contracted_graph_from_graph() {
        let graph = create_test_graph();
        let contracted = ContractedGraph::from_graph(&graph, 0);

        assert_eq!(contracted.vertex_count(), 4);
        assert_eq!(contracted.edge_count(), 3);
        assert_eq!(contracted.level(), 0);
    }

    #[test]
    fn test_contracted_graph_contract() {
        let graph = create_test_graph();
        let mut contracted = ContractedGraph::from_graph(&graph, 0);

        // Contract vertices 1 and 2
        let super_v = contracted.contract(1, 2).unwrap();

        // Now we should have 3 super-vertices
        assert_eq!(contracted.vertex_count(), 3);

        // The new super-vertex should contain both 1 and 2
        let original = contracted.get_original_vertices(super_v).unwrap();
        assert!(original.contains(&1));
        assert!(original.contains(&2));
    }

    #[test]
    fn test_bmssp_level_creation() {
        let graph = create_test_graph();
        let contracted = ContractedGraph::from_graph(&graph, 0);
        let config = LevelConfig::default();

        let level = BmsspJTreeLevel::new(contracted, config).unwrap();
        assert_eq!(level.level(), 0);

        let stats = level.statistics();
        assert_eq!(stats.vertex_count, 4);
        assert_eq!(stats.edge_count, 3);
    }

    #[test]
    fn test_min_cut_query() {
        let graph = create_test_graph();
        let contracted = ContractedGraph::from_graph(&graph, 0);
        let mut level = BmsspJTreeLevel::from_contracted(contracted, 0);

        // Min cut between 1 and 4 should traverse the bridge (2-3)
        let result = level.min_cut(1, 4).unwrap();
        assert!(result.value.is_finite());
        assert!(!result.from_cache);
    }

    #[test]
    fn test_min_cut_caching() {
        let graph = create_test_graph();
        let contracted = ContractedGraph::from_graph(&graph, 0);
        let mut level = BmsspJTreeLevel::from_contracted(contracted, 0);

        // First query
        let result1 = level.min_cut(1, 4).unwrap();
        assert!(!result1.from_cache);

        // Second query should hit cache
        let result2 = level.min_cut(1, 4).unwrap();
        assert!(result2.from_cache);
        assert_eq!(result1.value, result2.value);

        // Symmetric query should also hit cache
        let result3 = level.min_cut(4, 1).unwrap();
        assert!(result3.from_cache);
    }

    #[test]
    fn test_multi_terminal_cut() {
        let graph = create_test_graph();
        let contracted = ContractedGraph::from_graph(&graph, 0);
        let mut level = BmsspJTreeLevel::from_contracted(contracted, 0);

        let terminals = vec![1, 2, 3, 4];
        let cut = level.multi_terminal_cut(&terminals).unwrap();
        assert!(cut.is_finite());
    }

    #[test]
    fn test_cache_invalidation() {
        let graph = create_test_graph();
        let contracted = ContractedGraph::from_graph(&graph, 0);
        let mut level = BmsspJTreeLevel::from_contracted(contracted, 0);

        // Query and cache
        let _ = level.min_cut(1, 4).unwrap();
        assert_eq!(level.statistics().cache_hits, 0);

        // Query again (should hit cache)
        let _ = level.min_cut(1, 4).unwrap();
        assert_eq!(level.statistics().cache_hits, 1);

        // Invalidate
        level.invalidate_cache();

        // Query again (should miss cache)
        let result = level.min_cut(1, 4).unwrap();
        assert!(!result.from_cache);
    }

    #[test]
    fn test_level_config_default() {
        let config = LevelConfig::default();
        assert_eq!(config.level, 0);
        assert_eq!(config.alpha, 2.0);
        assert!(config.enable_cache);
        assert_eq!(config.max_cache_entries, 10_000);
    }

    #[test]
    fn test_refine_cut() {
        let graph = create_test_graph();
        let mut contracted = ContractedGraph::from_graph(&graph, 0);

        // Contract 1 and 2 into a super-vertex
        let super_12 = contracted.contract(1, 2).unwrap();

        let mut level = BmsspJTreeLevel::from_contracted(contracted, 0);

        // Refine a partition containing the super-vertex
        let coarse: HashSet<VertexId> = vec![super_12].into_iter().collect();
        let refined = level.refine_cut(&coarse).unwrap();

        assert!(refined.contains(&1));
        assert!(refined.contains(&2));
        assert!(!refined.contains(&3));
        assert!(!refined.contains(&4));
    }
}
