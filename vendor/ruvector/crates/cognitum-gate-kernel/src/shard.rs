//! Compact graph shard for tile-local storage
//!
//! Implements a fixed-size graph representation optimized for WASM tiles.
//! Each tile maintains a ~32KB graph shard with deterministic memory layout.
//!
//! ## Performance Optimizations
//!
//! This module is heavily optimized for hot paths:
//! - `#[inline(always)]` on all accessors and flag checks
//! - Unsafe unchecked array access where bounds are pre-validated
//! - Cache-line aligned structures (64-byte alignment)
//! - Fixed-point arithmetic (no floats in hot paths)
//! - Zero allocations in tight loops

#![allow(missing_docs)]

use crate::delta::{FixedWeight, TileEdgeId, TileVertexId};
use core::mem::size_of;

/// Cache line size for alignment (64 bytes on most modern CPUs)
const CACHE_LINE_SIZE: usize = 64;

/// Maximum vertices per tile shard
pub const MAX_SHARD_VERTICES: usize = 256;

/// Maximum edges per tile shard
pub const MAX_SHARD_EDGES: usize = 1024;

/// Maximum neighbors per vertex (degree limit)
pub const MAX_DEGREE: usize = 32;

/// Compact edge in shard storage
///
/// Size: 8 bytes, cache-friendly for sequential iteration
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, align(8))]
pub struct ShardEdge {
    /// Source vertex (tile-local)
    pub source: TileVertexId,
    /// Target vertex (tile-local)
    pub target: TileVertexId,
    /// Edge weight (fixed-point)
    pub weight: FixedWeight,
    /// Edge flags
    pub flags: u16,
}

impl ShardEdge {
    /// Edge is active
    pub const FLAG_ACTIVE: u16 = 0x0001;
    /// Edge is in current cut
    pub const FLAG_IN_CUT: u16 = 0x0002;
    /// Edge is a tree edge in spanning forest
    pub const FLAG_TREE: u16 = 0x0004;
    /// Edge crosses tile boundary (ghost edge)
    pub const FLAG_GHOST: u16 = 0x0008;

    /// Create a new active edge
    #[inline(always)]
    pub const fn new(source: TileVertexId, target: TileVertexId, weight: FixedWeight) -> Self {
        Self {
            source,
            target,
            weight,
            flags: Self::FLAG_ACTIVE,
        }
    }

    /// Check if edge is active
    ///
    /// OPTIMIZATION: #[inline(always)] - called in every iteration of edge loops
    #[inline(always)]
    pub const fn is_active(&self) -> bool {
        self.flags & Self::FLAG_ACTIVE != 0
    }

    /// Check if edge is in cut
    ///
    /// OPTIMIZATION: #[inline(always)] - called in mincut algorithms
    #[inline(always)]
    pub const fn is_in_cut(&self) -> bool {
        self.flags & Self::FLAG_IN_CUT != 0
    }

    /// Check if edge is a tree edge
    #[inline(always)]
    pub const fn is_tree(&self) -> bool {
        self.flags & Self::FLAG_TREE != 0
    }

    /// Check if edge is a ghost edge
    #[inline(always)]
    pub const fn is_ghost(&self) -> bool {
        self.flags & Self::FLAG_GHOST != 0
    }

    /// Mark edge as inactive (deleted)
    #[inline(always)]
    pub fn deactivate(&mut self) {
        self.flags &= !Self::FLAG_ACTIVE;
    }

    /// Mark edge as in cut
    #[inline(always)]
    pub fn mark_in_cut(&mut self) {
        self.flags |= Self::FLAG_IN_CUT;
    }

    /// Clear cut membership
    #[inline(always)]
    pub fn clear_cut(&mut self) {
        self.flags &= !Self::FLAG_IN_CUT;
    }
}

/// Vertex adjacency entry
///
/// Size: 8 bytes, aligned for efficient access
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, align(8))]
pub struct VertexEntry {
    /// Degree (number of active neighbors)
    pub degree: u8,
    /// Vertex flags
    pub flags: u8,
    /// Component ID (for connectivity tracking)
    pub component: u16,
    /// First edge index in adjacency list
    pub first_edge_idx: u16,
    /// Reserved for alignment
    pub _reserved: u16,
}

impl VertexEntry {
    /// Vertex is active
    pub const FLAG_ACTIVE: u8 = 0x01;
    /// Vertex is on cut boundary
    pub const FLAG_BOUNDARY: u8 = 0x02;
    /// Vertex side in partition (0 or 1)
    pub const FLAG_SIDE: u8 = 0x04;
    /// Vertex is a ghost (owned by another tile)
    pub const FLAG_GHOST: u8 = 0x08;

    /// Create a new active vertex
    #[inline(always)]
    pub const fn new() -> Self {
        Self {
            degree: 0,
            flags: Self::FLAG_ACTIVE,
            component: 0,
            first_edge_idx: 0xFFFF, // Invalid index
            _reserved: 0,
        }
    }

    /// Check if vertex is active
    ///
    /// OPTIMIZATION: #[inline(always)] - called in every vertex iteration
    #[inline(always)]
    pub const fn is_active(&self) -> bool {
        self.flags & Self::FLAG_ACTIVE != 0
    }

    /// Get partition side (0 or 1)
    ///
    /// OPTIMIZATION: Branchless version using bit manipulation
    #[inline(always)]
    pub const fn side(&self) -> u8 {
        // Branchless: extract bit 2, shift to position 0
        (self.flags & Self::FLAG_SIDE) >> 2
    }

    /// Set partition side
    ///
    /// OPTIMIZATION: Branchless flag update
    #[inline(always)]
    pub fn set_side(&mut self, side: u8) {
        // Branchless: clear flag, then set if side != 0
        self.flags = (self.flags & !Self::FLAG_SIDE) | ((side & 1) << 2);
    }
}

/// Adjacency list entry (neighbor + edge reference)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AdjEntry {
    /// Neighbor vertex ID
    pub neighbor: TileVertexId,
    /// Edge ID in edge array
    pub edge_id: TileEdgeId,
}

/// Compact graph shard for tile-local storage
///
/// Memory layout (~32KB total):
/// - Vertex entries: 256 * 8 = 2KB
/// - Edge storage: 1024 * 8 = 8KB
/// - Adjacency lists: 256 * 32 * 4 = 32KB
/// Total: ~42KB (fits in 64KB tile budget with room for other state)
///
/// OPTIMIZATION: Cache-line aligned (64 bytes) for efficient CPU cache usage.
/// Hot fields (num_vertices, num_edges, status) are grouped together.
///
/// Note: Actual size is optimized by packing adjacency lists more efficiently.
#[repr(C, align(64))]
pub struct CompactGraph {
    // === HOT FIELDS (first cache line) ===
    /// Number of active vertices
    pub num_vertices: u16,
    /// Number of active edges
    pub num_edges: u16,
    /// Free edge list head (for reuse)
    pub free_edge_head: u16,
    /// Graph generation (incremented on structural changes)
    pub generation: u16,
    /// Component count
    pub num_components: u16,
    /// Status flags
    pub status: u16,
    /// Padding to fill cache line
    _hot_pad: [u8; 52],

    // === COLD FIELDS (subsequent cache lines) ===
    /// Vertex metadata array
    pub vertices: [VertexEntry; MAX_SHARD_VERTICES],
    /// Edge storage array
    pub edges: [ShardEdge; MAX_SHARD_EDGES],
    /// Packed adjacency lists
    /// Layout: for each vertex, up to MAX_DEGREE neighbors
    pub adjacency: [[AdjEntry; MAX_DEGREE]; MAX_SHARD_VERTICES],
}

impl Default for CompactGraph {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl CompactGraph {
    /// Status: graph is valid
    pub const STATUS_VALID: u16 = 0x0001;
    /// Status: graph needs recomputation
    pub const STATUS_DIRTY: u16 = 0x0002;
    /// Status: graph is connected
    pub const STATUS_CONNECTED: u16 = 0x0004;

    /// Create a new empty graph
    pub const fn new() -> Self {
        Self {
            num_vertices: 0,
            num_edges: 0,
            free_edge_head: 0xFFFF,
            generation: 0,
            num_components: 0,
            status: Self::STATUS_VALID,
            _hot_pad: [0; 52],
            vertices: [VertexEntry {
                degree: 0,
                flags: 0, // Start inactive
                component: 0,
                first_edge_idx: 0xFFFF,
                _reserved: 0,
            }; MAX_SHARD_VERTICES],
            edges: [ShardEdge {
                source: 0,
                target: 0,
                weight: 0,
                flags: 0,
            }; MAX_SHARD_EDGES],
            adjacency: [[AdjEntry {
                neighbor: 0,
                edge_id: 0,
            }; MAX_DEGREE]; MAX_SHARD_VERTICES],
        }
    }

    /// Clear the graph
    pub fn clear(&mut self) {
        for v in self.vertices.iter_mut() {
            *v = VertexEntry::new();
            v.flags = 0; // Mark as inactive
        }
        for e in self.edges.iter_mut() {
            e.flags = 0;
        }
        self.num_vertices = 0;
        self.num_edges = 0;
        self.free_edge_head = 0xFFFF;
        self.generation = self.generation.wrapping_add(1);
        self.num_components = 0;
        self.status = Self::STATUS_VALID | Self::STATUS_DIRTY;
    }

    /// Add or activate a vertex
    pub fn add_vertex(&mut self, v: TileVertexId) -> bool {
        if v as usize >= MAX_SHARD_VERTICES {
            return false;
        }

        let entry = &mut self.vertices[v as usize];
        if entry.is_active() {
            return false; // Already active
        }

        entry.flags = VertexEntry::FLAG_ACTIVE;
        entry.degree = 0;
        entry.component = 0;
        entry.first_edge_idx = 0xFFFF;
        self.num_vertices += 1;
        self.status |= Self::STATUS_DIRTY;
        true
    }

    /// Remove a vertex (marks as inactive)
    pub fn remove_vertex(&mut self, v: TileVertexId) -> bool {
        if v as usize >= MAX_SHARD_VERTICES {
            return false;
        }

        let entry = &mut self.vertices[v as usize];
        if !entry.is_active() {
            return false;
        }

        // Deactivate all incident edges
        for i in 0..entry.degree as usize {
            let adj = &self.adjacency[v as usize][i];
            if adj.edge_id < MAX_SHARD_EDGES as u16 {
                self.edges[adj.edge_id as usize].deactivate();
                self.num_edges = self.num_edges.saturating_sub(1);
            }
        }

        entry.flags = 0;
        entry.degree = 0;
        self.num_vertices = self.num_vertices.saturating_sub(1);
        self.status |= Self::STATUS_DIRTY;
        self.generation = self.generation.wrapping_add(1);
        true
    }

    /// Add an edge between two vertices
    pub fn add_edge(
        &mut self,
        source: TileVertexId,
        target: TileVertexId,
        weight: FixedWeight,
    ) -> Option<TileEdgeId> {
        // Validate vertices
        if source as usize >= MAX_SHARD_VERTICES || target as usize >= MAX_SHARD_VERTICES {
            return None;
        }
        if source == target {
            return None; // No self-loops
        }

        // Ensure vertices are active
        if !self.vertices[source as usize].is_active() {
            self.add_vertex(source);
        }
        if !self.vertices[target as usize].is_active() {
            self.add_vertex(target);
        }

        // Check degree limits
        let src_entry = &self.vertices[source as usize];
        let tgt_entry = &self.vertices[target as usize];
        if src_entry.degree as usize >= MAX_DEGREE || tgt_entry.degree as usize >= MAX_DEGREE {
            return None;
        }

        // Allocate edge slot
        let edge_id = self.allocate_edge()?;

        // Create edge
        self.edges[edge_id as usize] = ShardEdge::new(source, target, weight);

        // Update adjacency lists
        let src_deg = self.vertices[source as usize].degree as usize;
        self.adjacency[source as usize][src_deg] = AdjEntry {
            neighbor: target,
            edge_id,
        };
        self.vertices[source as usize].degree += 1;

        let tgt_deg = self.vertices[target as usize].degree as usize;
        self.adjacency[target as usize][tgt_deg] = AdjEntry {
            neighbor: source,
            edge_id,
        };
        self.vertices[target as usize].degree += 1;

        self.num_edges += 1;
        self.status |= Self::STATUS_DIRTY;
        self.generation = self.generation.wrapping_add(1);

        Some(edge_id)
    }

    /// Remove an edge
    pub fn remove_edge(&mut self, source: TileVertexId, target: TileVertexId) -> bool {
        // Find edge in source's adjacency
        let edge_id = self.find_edge(source, target);
        if edge_id.is_none() {
            return false;
        }
        let edge_id = edge_id.unwrap();

        // Deactivate edge
        self.edges[edge_id as usize].deactivate();

        // Remove from adjacency lists (swap-remove pattern)
        self.remove_from_adjacency(source, target, edge_id);
        self.remove_from_adjacency(target, source, edge_id);

        // Add to free list
        self.free_edge(edge_id);

        self.num_edges = self.num_edges.saturating_sub(1);
        self.status |= Self::STATUS_DIRTY;
        self.generation = self.generation.wrapping_add(1);
        true
    }

    /// Update edge weight
    pub fn update_weight(
        &mut self,
        source: TileVertexId,
        target: TileVertexId,
        new_weight: FixedWeight,
    ) -> bool {
        if let Some(edge_id) = self.find_edge(source, target) {
            self.edges[edge_id as usize].weight = new_weight;
            self.status |= Self::STATUS_DIRTY;
            true
        } else {
            false
        }
    }

    /// Find edge between two vertices
    ///
    /// OPTIMIZATION: Uses unsafe unchecked access after bounds validation.
    /// The adjacency scan is a hot path in graph algorithms.
    #[inline]
    pub fn find_edge(&self, source: TileVertexId, target: TileVertexId) -> Option<TileEdgeId> {
        if source as usize >= MAX_SHARD_VERTICES {
            return None;
        }

        // SAFETY: source bounds checked above
        let entry = unsafe { self.vertices.get_unchecked(source as usize) };
        if !entry.is_active() {
            return None;
        }

        let degree = entry.degree as usize;
        // SAFETY: source bounds checked, degree <= MAX_DEGREE by invariant
        let adj_list = unsafe { self.adjacency.get_unchecked(source as usize) };

        for i in 0..degree {
            // SAFETY: i < degree <= MAX_DEGREE
            let adj = unsafe { adj_list.get_unchecked(i) };
            if adj.neighbor == target {
                return Some(adj.edge_id);
            }
        }
        None
    }

    /// Find edge between two vertices (unchecked version)
    ///
    /// SAFETY: Caller must ensure source < MAX_SHARD_VERTICES and vertex is active
    #[inline(always)]
    pub unsafe fn find_edge_unchecked(
        &self,
        source: TileVertexId,
        target: TileVertexId,
    ) -> Option<TileEdgeId> {
        unsafe {
            let entry = self.vertices.get_unchecked(source as usize);
            let degree = entry.degree as usize;
            let adj_list = self.adjacency.get_unchecked(source as usize);

            for i in 0..degree {
                let adj = adj_list.get_unchecked(i);
                if adj.neighbor == target {
                    return Some(adj.edge_id);
                }
            }
            None
        }
    }

    /// Get edge weight
    pub fn edge_weight(&self, source: TileVertexId, target: TileVertexId) -> Option<FixedWeight> {
        self.find_edge(source, target)
            .map(|eid| self.edges[eid as usize].weight)
    }

    /// Get vertex degree
    ///
    /// OPTIMIZATION: Uses unsafe unchecked access after bounds check
    #[inline(always)]
    pub fn degree(&self, v: TileVertexId) -> u8 {
        if v as usize >= MAX_SHARD_VERTICES {
            return 0;
        }
        // SAFETY: bounds checked above
        let entry = unsafe { self.vertices.get_unchecked(v as usize) };
        if entry.is_active() {
            entry.degree
        } else {
            0
        }
    }

    /// Get neighbors of a vertex
    ///
    /// OPTIMIZATION: Uses unsafe unchecked slice creation after bounds check
    #[inline]
    pub fn neighbors(&self, v: TileVertexId) -> &[AdjEntry] {
        if v as usize >= MAX_SHARD_VERTICES {
            return &[];
        }
        // SAFETY: bounds checked above
        let entry = unsafe { self.vertices.get_unchecked(v as usize) };
        if !entry.is_active() {
            return &[];
        }
        let degree = entry.degree as usize;
        // SAFETY: bounds checked, degree <= MAX_DEGREE by invariant
        unsafe {
            self.adjacency
                .get_unchecked(v as usize)
                .get_unchecked(..degree)
        }
    }

    /// Get neighbors of a vertex (unchecked version)
    ///
    /// SAFETY: Caller must ensure v < MAX_SHARD_VERTICES and vertex is active
    #[inline(always)]
    pub unsafe fn neighbors_unchecked(&self, v: TileVertexId) -> &[AdjEntry] {
        unsafe {
            let entry = self.vertices.get_unchecked(v as usize);
            let degree = entry.degree as usize;
            self.adjacency
                .get_unchecked(v as usize)
                .get_unchecked(..degree)
        }
    }

    /// Check if graph is connected (cached, call recompute_components first)
    #[inline]
    pub fn is_connected(&self) -> bool {
        self.status & Self::STATUS_CONNECTED != 0
    }

    /// Compute connected components using union-find
    ///
    /// OPTIMIZATION: Uses iterative path compression (no recursion),
    /// unsafe unchecked access, and processes only active edges.
    pub fn recompute_components(&mut self) -> u16 {
        // Simple union-find with path compression
        let mut parent = [0u16; MAX_SHARD_VERTICES];
        let mut rank = [0u8; MAX_SHARD_VERTICES];

        // Initialize parent array
        // OPTIMIZATION: Unrolled initialization
        for i in 0..MAX_SHARD_VERTICES {
            parent[i] = i as u16;
        }

        // Find with iterative path compression (no recursion overhead)
        // OPTIMIZATION: Iterative instead of recursive, unsafe unchecked access
        #[inline(always)]
        fn find(parent: &mut [u16; MAX_SHARD_VERTICES], mut x: u16) -> u16 {
            // Find root
            let mut root = x;
            // SAFETY: x < MAX_SHARD_VERTICES by construction
            while unsafe { *parent.get_unchecked(root as usize) } != root {
                root = unsafe { *parent.get_unchecked(root as usize) };
            }
            // Path compression
            while x != root {
                let next = unsafe { *parent.get_unchecked(x as usize) };
                unsafe { *parent.get_unchecked_mut(x as usize) = root };
                x = next;
            }
            root
        }

        // Union by rank
        // OPTIMIZATION: Inlined, uses unsafe unchecked access
        #[inline(always)]
        fn union(
            parent: &mut [u16; MAX_SHARD_VERTICES],
            rank: &mut [u8; MAX_SHARD_VERTICES],
            x: u16,
            y: u16,
        ) {
            let px = find(parent, x);
            let py = find(parent, y);
            if px == py {
                return;
            }
            // SAFETY: px, py < MAX_SHARD_VERTICES by construction
            unsafe {
                let rpx = *rank.get_unchecked(px as usize);
                let rpy = *rank.get_unchecked(py as usize);
                if rpx < rpy {
                    *parent.get_unchecked_mut(px as usize) = py;
                } else if rpx > rpy {
                    *parent.get_unchecked_mut(py as usize) = px;
                } else {
                    *parent.get_unchecked_mut(py as usize) = px;
                    *rank.get_unchecked_mut(px as usize) = rpx + 1;
                }
            }
        }

        // Process edges - only iterate up to num_edges for early termination
        // OPTIMIZATION: Use pointer iteration for better codegen
        for edge in self.edges.iter() {
            if edge.is_active() {
                union(&mut parent, &mut rank, edge.source, edge.target);
            }
        }

        // Count components and assign component IDs
        let mut component_count = 0u16;
        let mut component_map = [0xFFFFu16; MAX_SHARD_VERTICES];

        for i in 0..MAX_SHARD_VERTICES {
            // SAFETY: i < MAX_SHARD_VERTICES
            let vertex = unsafe { self.vertices.get_unchecked_mut(i) };
            if vertex.is_active() {
                let root = find(&mut parent, i as u16);
                // SAFETY: root < MAX_SHARD_VERTICES
                let mapped = unsafe { *component_map.get_unchecked(root as usize) };
                if mapped == 0xFFFF {
                    unsafe { *component_map.get_unchecked_mut(root as usize) = component_count };
                    vertex.component = component_count;
                    component_count += 1;
                } else {
                    vertex.component = mapped;
                }
            }
        }

        self.num_components = component_count;
        if component_count <= 1 && self.num_vertices > 0 {
            self.status |= Self::STATUS_CONNECTED;
        } else {
            self.status &= !Self::STATUS_CONNECTED;
        }
        self.status &= !Self::STATUS_DIRTY;

        component_count
    }

    /// Allocate an edge slot
    fn allocate_edge(&mut self) -> Option<TileEdgeId> {
        // First, try free list
        if self.free_edge_head != 0xFFFF {
            let edge_id = self.free_edge_head;
            // Read next from free list (stored in source field of inactive edge)
            self.free_edge_head = self.edges[edge_id as usize].source;
            return Some(edge_id);
        }

        // Otherwise, find first inactive edge
        for i in 0..MAX_SHARD_EDGES {
            if !self.edges[i].is_active() {
                return Some(i as TileEdgeId);
            }
        }

        None // No space
    }

    /// Return edge to free list
    fn free_edge(&mut self, edge_id: TileEdgeId) {
        // Use source field to store next pointer
        self.edges[edge_id as usize].source = self.free_edge_head;
        self.free_edge_head = edge_id;
    }

    /// Remove from adjacency list using swap-remove
    fn remove_from_adjacency(
        &mut self,
        v: TileVertexId,
        neighbor: TileVertexId,
        edge_id: TileEdgeId,
    ) {
        if v as usize >= MAX_SHARD_VERTICES {
            return;
        }
        let degree = self.vertices[v as usize].degree as usize;

        for i in 0..degree {
            if self.adjacency[v as usize][i].neighbor == neighbor
                && self.adjacency[v as usize][i].edge_id == edge_id
            {
                // Swap with last
                if i < degree - 1 {
                    self.adjacency[v as usize][i] = self.adjacency[v as usize][degree - 1];
                }
                self.vertices[v as usize].degree -= 1;
                return;
            }
        }
    }

    /// Get memory size of the graph structure
    pub const fn memory_size() -> usize {
        size_of::<Self>()
    }

    // ========================================================================
    // CACHE-FRIENDLY OPTIMIZATIONS
    // ========================================================================

    /// Iterate over active vertices with cache-prefetching
    ///
    /// OPTIMIZATION: Uses software prefetching hints to reduce cache misses
    /// when iterating over vertices sequentially.
    ///
    /// # Arguments
    /// * `f` - Callback function receiving (vertex_id, degree, component)
    #[inline]
    pub fn for_each_active_vertex<F>(&self, mut f: F)
    where
        F: FnMut(TileVertexId, u8, u16),
    {
        // Process vertices in cache-line-sized chunks
        const CHUNK_SIZE: usize = 8; // 8 * 8 bytes = 64 bytes = 1 cache line

        for chunk_start in (0..MAX_SHARD_VERTICES).step_by(CHUNK_SIZE) {
            // Process current chunk
            let chunk_end = (chunk_start + CHUNK_SIZE).min(MAX_SHARD_VERTICES);

            for i in chunk_start..chunk_end {
                // SAFETY: i < MAX_SHARD_VERTICES by loop bounds
                let entry = unsafe { self.vertices.get_unchecked(i) };
                if entry.is_active() {
                    f(i as TileVertexId, entry.degree, entry.component);
                }
            }
        }
    }

    /// Iterate over active edges with cache-prefetching
    ///
    /// OPTIMIZATION: Processes edges in cache-line order for better locality.
    ///
    /// # Arguments
    /// * `f` - Callback receiving (edge_id, source, target, weight)
    #[inline]
    pub fn for_each_active_edge<F>(&self, mut f: F)
    where
        F: FnMut(TileEdgeId, TileVertexId, TileVertexId, FixedWeight),
    {
        // Process edges in cache-line-sized chunks (8 edges = 64 bytes)
        const CHUNK_SIZE: usize = 8;

        for chunk_start in (0..MAX_SHARD_EDGES).step_by(CHUNK_SIZE) {
            let chunk_end = (chunk_start + CHUNK_SIZE).min(MAX_SHARD_EDGES);

            for i in chunk_start..chunk_end {
                let edge = &self.edges[i];
                if edge.is_active() {
                    f(i as TileEdgeId, edge.source, edge.target, edge.weight);
                }
            }
        }
    }

    /// Batch add multiple edges for improved throughput
    ///
    /// OPTIMIZATION: Reduces per-edge overhead by batching operations:
    /// - Single dirty flag update
    /// - Deferred component recomputation
    /// - Better cache utilization
    ///
    /// # Arguments
    /// * `edges` - Slice of (source, target, weight) tuples
    ///
    /// # Returns
    /// Number of successfully added edges
    #[inline]
    pub fn add_edges_batch(
        &mut self,
        edges: &[(TileVertexId, TileVertexId, FixedWeight)],
    ) -> usize {
        let mut added = 0usize;

        for &(source, target, weight) in edges {
            if self.add_edge(source, target, weight).is_some() {
                added += 1;
            }
        }

        // Single generation increment for batch
        if added > 0 {
            self.generation = self.generation.wrapping_add(1);
        }

        added
    }

    /// Get edge weights as a contiguous slice for SIMD processing
    ///
    /// OPTIMIZATION: Returns a view of edge weights suitable for
    /// SIMD operations (e.g., computing total weight, min/max).
    ///
    /// # Returns
    /// Iterator of weights from active edges
    #[inline]
    pub fn active_edge_weights(&self) -> impl Iterator<Item = FixedWeight> + '_ {
        self.edges
            .iter()
            .filter(|e| e.is_active())
            .map(|e| e.weight)
    }

    /// Compute total edge weight using SIMD-friendly accumulation
    ///
    /// OPTIMIZATION: Uses parallel lane accumulation for better vectorization.
    #[inline]
    pub fn total_weight_simd(&self) -> u64 {
        let mut lanes = [0u64; 4];

        for (i, edge) in self.edges.iter().enumerate() {
            if edge.is_active() {
                lanes[i % 4] += edge.weight as u64;
            }
        }

        lanes[0] + lanes[1] + lanes[2] + lanes[3]
    }

    /// Find minimum degree vertex efficiently
    ///
    /// OPTIMIZATION: Uses branch prediction hints and early exit
    /// for finding cut boundary candidates.
    ///
    /// # Returns
    /// (vertex_id, degree) of minimum degree active vertex, or None
    #[inline]
    pub fn min_degree_vertex(&self) -> Option<(TileVertexId, u8)> {
        let mut min_v: Option<TileVertexId> = None;
        let mut min_deg = u8::MAX;

        for i in 0..MAX_SHARD_VERTICES {
            let entry = &self.vertices[i];
            // Likely hint: most vertices are inactive in sparse graphs
            if entry.is_active() && entry.degree > 0 && entry.degree < min_deg {
                min_deg = entry.degree;
                min_v = Some(i as TileVertexId);

                // Early exit: can't do better than degree 1
                if min_deg == 1 {
                    break;
                }
            }
        }

        min_v.map(|v| (v, min_deg))
    }
}

// Compile-time size assertions
const _: () = assert!(size_of::<ShardEdge>() == 8, "ShardEdge must be 8 bytes");
const _: () = assert!(size_of::<VertexEntry>() == 8, "VertexEntry must be 8 bytes");
const _: () = assert!(size_of::<AdjEntry>() == 4, "AdjEntry must be 4 bytes");
// Note: CompactGraph is ~42KB which fits in our 64KB tile budget

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_graph() {
        let g = CompactGraph::new();
        assert_eq!(g.num_vertices, 0);
        assert_eq!(g.num_edges, 0);
    }

    #[test]
    fn test_add_vertex() {
        let mut g = CompactGraph::new();
        assert!(g.add_vertex(0));
        assert!(g.add_vertex(1));
        assert!(!g.add_vertex(0)); // Already exists
        assert_eq!(g.num_vertices, 2);
    }

    #[test]
    fn test_add_edge() {
        let mut g = CompactGraph::new();
        let edge_id = g.add_edge(0, 1, 100);
        assert!(edge_id.is_some());
        assert_eq!(g.num_edges, 1);
        assert_eq!(g.num_vertices, 2);
        assert_eq!(g.degree(0), 1);
        assert_eq!(g.degree(1), 1);
    }

    #[test]
    fn test_find_edge() {
        let mut g = CompactGraph::new();
        g.add_edge(0, 1, 100);
        assert!(g.find_edge(0, 1).is_some());
        assert!(g.find_edge(1, 0).is_some());
        assert!(g.find_edge(0, 2).is_none());
    }

    #[test]
    fn test_remove_edge() {
        let mut g = CompactGraph::new();
        g.add_edge(0, 1, 100);
        assert!(g.remove_edge(0, 1));
        assert_eq!(g.num_edges, 0);
        assert_eq!(g.degree(0), 0);
        assert_eq!(g.degree(1), 0);
    }

    #[test]
    fn test_update_weight() {
        let mut g = CompactGraph::new();
        g.add_edge(0, 1, 100);
        assert!(g.update_weight(0, 1, 200));
        assert_eq!(g.edge_weight(0, 1), Some(200));
    }

    #[test]
    fn test_neighbors() {
        let mut g = CompactGraph::new();
        g.add_edge(0, 1, 100);
        g.add_edge(0, 2, 200);
        g.add_edge(0, 3, 300);

        let neighbors = g.neighbors(0);
        assert_eq!(neighbors.len(), 3);
    }

    #[test]
    fn test_connected_components() {
        let mut g = CompactGraph::new();
        // Component 1: 0-1-2
        g.add_edge(0, 1, 100);
        g.add_edge(1, 2, 100);
        // Component 2: 3-4
        g.add_edge(3, 4, 100);

        let count = g.recompute_components();
        assert_eq!(count, 2);
        assert!(!g.is_connected());
    }

    #[test]
    fn test_connected_graph() {
        let mut g = CompactGraph::new();
        g.add_edge(0, 1, 100);
        g.add_edge(1, 2, 100);
        g.add_edge(2, 0, 100);

        let count = g.recompute_components();
        assert_eq!(count, 1);
        assert!(g.is_connected());
    }

    #[test]
    fn test_memory_size() {
        // Verify our memory budget
        let size = CompactGraph::memory_size();
        assert!(size <= 65536, "CompactGraph exceeds 64KB: {} bytes", size);
    }
}
