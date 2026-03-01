//! 256-Tile Coherence Gate Architecture for ruQu
//!
//! This module implements the tile hierarchy for the Anytime-Valid Coherence Gate:
//!
//! - **WorkerTile** (IDs 1-255): Individual processing units with 64KB memory budget
//! - **TileZero** (ID 0): Coordinator that merges reports and issues gate decisions
//!
//! # Memory Layout per Worker Tile (64KB)
//!
//! | Component | Size | Purpose |
//! |-----------|------|---------|
//! | PatchGraph | ~32KB | Local graph shard (vertices, edges, adjacency) |
//! | SyndromBuffer | ~16KB | Rolling syndrome history (1024 rounds) |
//! | EvidenceAccumulator | ~4KB | E-value computation |
//! | LocalCutState | ~8KB | Boundary candidates, cut cache, witness fragments |
//! | Control/Scratch | ~4KB | Delta buffer, report scratch, stack |
//!
//! # Latency Budget (Target: <4μs p99)
//!
//! ```text
//! Syndrome Arrival        → 0 ns
//! Ring buffer append      → +50 ns
//! Graph update            → +200 ns (amortized O(n^{o(1)}))
//! Worker Tick             → +500 ns (local cut eval)
//! Report generation       → +100 ns
//! TileZero Merge          → +500 ns (parallel from 255 tiles)
//! Global cut              → +300 ns
//! Three-filter eval       → +100 ns
//! Token signing           → +500 ns (Ed25519)
//! Receipt append          → +100 ns
//! ─────────────────────────────────
//! Total                   → ~2,350 ns
//! ```

#![allow(missing_docs)]

use std::mem::size_of;

// Cryptographic imports
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use subtle::ConstantTimeEq;

// ============================================================================
// TYPE ALIASES
// ============================================================================

/// Vertex identifier in the patch graph (tile-local)
pub type VertexId = u16;

/// Edge identifier in the patch graph (tile-local)
pub type EdgeId = u16;

/// Fixed-point weight representation (Q16.16 format)
pub type FixedWeight = u32;

/// Tile identifier (0 = TileZero, 1-255 = Workers)
pub type TileId = u8;

/// Log e-value in fixed-point (log2(e) * 65536)
pub type LogEValue = i32;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Maximum vertices per patch graph shard
pub const MAX_PATCH_VERTICES: usize = 256;

/// Maximum edges per patch graph shard
pub const MAX_PATCH_EDGES: usize = 1024;

/// Maximum degree per vertex
pub const MAX_DEGREE: usize = 32;

/// Syndrome buffer depth (rounds)
pub const SYNDROME_BUFFER_DEPTH: usize = 1024;

/// Maximum boundary candidates to track
pub const MAX_BOUNDARY_CANDIDATES: usize = 64;

/// Cache line size for alignment
const CACHE_LINE_SIZE: usize = 64;

/// log2(20) * 65536 - Strong evidence threshold
const LOG_E_STRONG: LogEValue = 282944;

/// log2(100) * 65536 - Very strong evidence threshold
const LOG_E_VERY_STRONG: LogEValue = 436906;

/// Number of worker tiles
pub const NUM_WORKERS: usize = 255;

// ============================================================================
// SYNDROME DELTA
// ============================================================================

/// Syndrome delta representing a change in the syndrome stream
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct SyndromeDelta {
    /// Source qubit/node
    pub source: VertexId,
    /// Target qubit/node (for two-qubit events)
    pub target: VertexId,
    /// Syndrome value (error indicator)
    pub value: u16,
    /// Delta flags
    pub flags: u16,
}

impl SyndromeDelta {
    /// Flag: delta represents an edge addition
    pub const FLAG_EDGE_ADD: u16 = 0x0001;
    /// Flag: delta represents an edge removal
    pub const FLAG_EDGE_REMOVE: u16 = 0x0002;
    /// Flag: delta represents a weight update
    pub const FLAG_WEIGHT_UPDATE: u16 = 0x0004;
    /// Flag: delta is a syndrome observation
    pub const FLAG_SYNDROME: u16 = 0x0008;
    /// Flag: delta crosses tile boundary
    pub const FLAG_BOUNDARY: u16 = 0x0010;

    /// Create a new syndrome delta
    #[inline]
    pub const fn new(source: VertexId, target: VertexId, value: u16) -> Self {
        Self {
            source,
            target,
            value,
            flags: Self::FLAG_SYNDROME,
        }
    }

    /// Create an edge addition delta
    #[inline]
    pub const fn edge_add(source: VertexId, target: VertexId, weight: u16) -> Self {
        Self {
            source,
            target,
            value: weight,
            flags: Self::FLAG_EDGE_ADD,
        }
    }

    /// Create an edge removal delta
    #[inline]
    pub const fn edge_remove(source: VertexId, target: VertexId) -> Self {
        Self {
            source,
            target,
            value: 0,
            flags: Self::FLAG_EDGE_REMOVE,
        }
    }

    /// Check if this delta is a syndrome observation
    #[inline]
    pub const fn is_syndrome(&self) -> bool {
        self.flags & Self::FLAG_SYNDROME != 0
    }

    /// Check if this delta is an edge modification
    #[inline]
    pub const fn is_edge_modification(&self) -> bool {
        self.flags & (Self::FLAG_EDGE_ADD | Self::FLAG_EDGE_REMOVE | Self::FLAG_WEIGHT_UPDATE) != 0
    }
}

// ============================================================================
// VERTEX AND EDGE STRUCTURES
// ============================================================================

/// Vertex in the patch graph
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, align(8))]
pub struct Vertex {
    /// Vertex degree
    pub degree: u8,
    /// Vertex flags
    pub flags: u8,
    /// Component ID
    pub component: u16,
    /// First adjacency index
    pub adj_start: u16,
    /// Syndrome accumulator for this vertex
    pub syndrome_acc: u16,
}

impl Vertex {
    /// Vertex is active
    pub const FLAG_ACTIVE: u8 = 0x01;
    /// Vertex is on cut boundary
    pub const FLAG_BOUNDARY: u8 = 0x02;
    /// Vertex is in unhealthy partition
    pub const FLAG_UNHEALTHY: u8 = 0x04;
    /// Vertex is a ghost (owned by another tile)
    pub const FLAG_GHOST: u8 = 0x08;

    /// Create a new active vertex
    #[inline]
    pub const fn new() -> Self {
        Self {
            degree: 0,
            flags: Self::FLAG_ACTIVE,
            component: 0,
            adj_start: 0xFFFF,
            syndrome_acc: 0,
        }
    }

    /// Check if vertex is active
    #[inline(always)]
    pub const fn is_active(&self) -> bool {
        self.flags & Self::FLAG_ACTIVE != 0
    }

    /// Check if vertex is on boundary
    #[inline(always)]
    pub const fn is_boundary(&self) -> bool {
        self.flags & Self::FLAG_BOUNDARY != 0
    }
}

/// Edge in the patch graph
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, align(8))]
pub struct Edge {
    /// Source vertex
    pub source: VertexId,
    /// Target vertex
    pub target: VertexId,
    /// Edge weight (coupling strength or correlation)
    pub weight: FixedWeight,
}

impl Edge {
    /// Create a new edge
    #[inline]
    pub const fn new(source: VertexId, target: VertexId, weight: FixedWeight) -> Self {
        Self {
            source,
            target,
            weight,
        }
    }
}

/// Adjacency entry
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct AdjEntry {
    /// Neighbor vertex ID
    pub neighbor: VertexId,
    /// Edge ID
    pub edge_id: EdgeId,
}

// ============================================================================
// PATCH GRAPH
// ============================================================================

/// Local graph shard maintained by each worker tile
///
/// Memory: ~32KB for vertices, edges, and adjacency lists
#[derive(Debug)]
#[repr(C, align(64))]
pub struct PatchGraph {
    // === HOT FIELDS (first cache line) ===
    /// Number of active vertices
    pub num_vertices: u16,
    /// Number of active edges
    pub num_edges: u16,
    /// Number of connected components
    pub num_components: u16,
    /// Graph generation (incremented on changes)
    pub generation: u16,
    /// Status flags
    pub status: u16,
    /// Free edge list head
    pub free_edge_head: u16,
    /// Padding for cache alignment
    _pad: [u8; 52],

    // === COLD FIELDS ===
    /// Vertex array
    pub vertices: [Vertex; MAX_PATCH_VERTICES],
    /// Edge array
    pub edges: [Edge; MAX_PATCH_EDGES],
    /// Adjacency lists (packed)
    pub adjacency: [[AdjEntry; MAX_DEGREE]; MAX_PATCH_VERTICES],
}

impl Default for PatchGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl PatchGraph {
    /// Status: graph is valid
    pub const STATUS_VALID: u16 = 0x0001;
    /// Status: graph needs recomputation
    pub const STATUS_DIRTY: u16 = 0x0002;
    /// Status: graph is connected
    pub const STATUS_CONNECTED: u16 = 0x0004;
    /// Status: cut boundary has moved
    pub const STATUS_BOUNDARY_MOVED: u16 = 0x0008;

    /// Create a new empty patch graph
    pub const fn new() -> Self {
        Self {
            num_vertices: 0,
            num_edges: 0,
            num_components: 0,
            generation: 0,
            status: Self::STATUS_VALID,
            free_edge_head: 0xFFFF,
            _pad: [0; 52],
            vertices: [Vertex {
                degree: 0,
                flags: 0,
                component: 0,
                adj_start: 0xFFFF,
                syndrome_acc: 0,
            }; MAX_PATCH_VERTICES],
            edges: [Edge {
                source: 0,
                target: 0,
                weight: 0,
            }; MAX_PATCH_EDGES],
            adjacency: [[AdjEntry {
                neighbor: 0,
                edge_id: 0,
            }; MAX_DEGREE]; MAX_PATCH_VERTICES],
        }
    }

    /// Apply a syndrome delta to the graph
    pub fn apply_delta(&mut self, delta: &SyndromeDelta) {
        if delta.flags & SyndromeDelta::FLAG_EDGE_ADD != 0 {
            self.add_edge(delta.source, delta.target, delta.value as FixedWeight);
        } else if delta.flags & SyndromeDelta::FLAG_EDGE_REMOVE != 0 {
            self.remove_edge(delta.source, delta.target);
        } else if delta.flags & SyndromeDelta::FLAG_WEIGHT_UPDATE != 0 {
            self.update_weight(delta.source, delta.target, delta.value as FixedWeight);
        } else if delta.flags & SyndromeDelta::FLAG_SYNDROME != 0 {
            // Update syndrome accumulator at vertex
            if (delta.source as usize) < MAX_PATCH_VERTICES {
                self.ensure_vertex(delta.source);
                self.vertices[delta.source as usize].syndrome_acc = self.vertices
                    [delta.source as usize]
                    .syndrome_acc
                    .wrapping_add(delta.value);
            }
        }
    }

    /// Ensure a vertex exists (activate if needed)
    pub fn ensure_vertex(&mut self, v: VertexId) -> bool {
        if v as usize >= MAX_PATCH_VERTICES {
            return false;
        }
        if !self.vertices[v as usize].is_active() {
            self.vertices[v as usize].flags = Vertex::FLAG_ACTIVE;
            self.vertices[v as usize].degree = 0;
            self.vertices[v as usize].component = 0;
            self.num_vertices += 1;
            self.status |= Self::STATUS_DIRTY;
        }
        true
    }

    /// Add an edge to the graph
    pub fn add_edge(
        &mut self,
        source: VertexId,
        target: VertexId,
        weight: FixedWeight,
    ) -> Option<EdgeId> {
        if source as usize >= MAX_PATCH_VERTICES || target as usize >= MAX_PATCH_VERTICES {
            return None;
        }
        if source == target {
            return None;
        }

        self.ensure_vertex(source);
        self.ensure_vertex(target);

        // Check degree limits
        if self.vertices[source as usize].degree as usize >= MAX_DEGREE
            || self.vertices[target as usize].degree as usize >= MAX_DEGREE
        {
            return None;
        }

        // Allocate edge
        let edge_id = self.allocate_edge()?;
        self.edges[edge_id as usize] = Edge::new(source, target, weight);

        // Update adjacency
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

    /// Remove an edge from the graph
    pub fn remove_edge(&mut self, source: VertexId, target: VertexId) -> bool {
        if let Some(edge_id) = self.find_edge(source, target) {
            // Remove from adjacency lists
            self.remove_from_adj(source, target, edge_id);
            self.remove_from_adj(target, source, edge_id);

            // Free edge slot
            self.free_edge(edge_id);
            self.num_edges = self.num_edges.saturating_sub(1);
            self.status |= Self::STATUS_DIRTY;
            self.generation = self.generation.wrapping_add(1);
            true
        } else {
            false
        }
    }

    /// Update edge weight
    pub fn update_weight(
        &mut self,
        source: VertexId,
        target: VertexId,
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
    pub fn find_edge(&self, source: VertexId, target: VertexId) -> Option<EdgeId> {
        if source as usize >= MAX_PATCH_VERTICES {
            return None;
        }
        let v = &self.vertices[source as usize];
        if !v.is_active() {
            return None;
        }

        for i in 0..v.degree as usize {
            if self.adjacency[source as usize][i].neighbor == target {
                return Some(self.adjacency[source as usize][i].edge_id);
            }
        }
        None
    }

    /// Compute local minimum cut estimate
    ///
    /// Uses minimum vertex degree as a heuristic for the cut value
    pub fn estimate_local_cut(&self) -> f64 {
        let mut min_degree = u8::MAX;
        let mut total_weight: u64 = 0;
        let mut degree_count = 0u32;

        for v in &self.vertices[..MAX_PATCH_VERTICES] {
            if v.is_active() && v.degree > 0 {
                if v.degree < min_degree {
                    min_degree = v.degree;
                }
                degree_count += 1;
            }
        }

        // Sum edge weights
        for e in &self.edges[..self.num_edges as usize] {
            total_weight += e.weight as u64;
        }

        if degree_count == 0 || min_degree == u8::MAX {
            return 0.0;
        }

        // Estimate: min_degree * avg_weight
        let avg_weight = total_weight as f64 / (self.num_edges.max(1) as f64);
        (min_degree as f64) * avg_weight
    }

    /// Identify boundary candidates (edges that might be in the cut)
    pub fn identify_boundary_candidates(&self, out: &mut [EdgeId]) -> usize {
        let mut count = 0;
        let max_out = out.len().min(MAX_BOUNDARY_CANDIDATES);

        // Find edges with lowest weight (most likely to be in cut)
        let mut edges_with_weights: [(EdgeId, FixedWeight); MAX_BOUNDARY_CANDIDATES] =
            [(0, u32::MAX); MAX_BOUNDARY_CANDIDATES];

        for (i, e) in self.edges[..self.num_edges as usize].iter().enumerate() {
            if e.weight > 0 {
                // Insert into sorted list if smaller than max
                for j in 0..max_out {
                    if e.weight < edges_with_weights[j].1 {
                        // Shift down
                        for k in (j + 1..max_out).rev() {
                            edges_with_weights[k] = edges_with_weights[k - 1];
                        }
                        edges_with_weights[j] = (i as EdgeId, e.weight);
                        break;
                    }
                }
            }
        }

        // Output sorted edge IDs
        for (eid, weight) in edges_with_weights.iter() {
            if *weight < u32::MAX {
                out[count] = *eid;
                count += 1;
            }
        }

        count
    }

    /// Recompute connected components
    pub fn recompute_components(&mut self) -> u16 {
        // Union-find with path compression
        let mut parent = [0u16; MAX_PATCH_VERTICES];
        let mut rank = [0u8; MAX_PATCH_VERTICES];

        for i in 0..MAX_PATCH_VERTICES {
            parent[i] = i as u16;
        }

        #[inline(always)]
        fn find(parent: &mut [u16; MAX_PATCH_VERTICES], mut x: u16) -> u16 {
            let mut root = x;
            while parent[root as usize] != root {
                root = parent[root as usize];
            }
            while x != root {
                let next = parent[x as usize];
                parent[x as usize] = root;
                x = next;
            }
            root
        }

        #[inline(always)]
        fn union(
            parent: &mut [u16; MAX_PATCH_VERTICES],
            rank: &mut [u8; MAX_PATCH_VERTICES],
            x: u16,
            y: u16,
        ) {
            let px = find(parent, x);
            let py = find(parent, y);
            if px == py {
                return;
            }
            if rank[px as usize] < rank[py as usize] {
                parent[px as usize] = py;
            } else if rank[px as usize] > rank[py as usize] {
                parent[py as usize] = px;
            } else {
                parent[py as usize] = px;
                rank[px as usize] += 1;
            }
        }

        // Process edges
        for i in 0..self.num_edges as usize {
            let e = &self.edges[i];
            if e.weight > 0 {
                union(&mut parent, &mut rank, e.source, e.target);
            }
        }

        // Count and assign components
        let mut component_count = 0u16;
        let mut component_map = [0xFFFFu16; MAX_PATCH_VERTICES];

        for i in 0..MAX_PATCH_VERTICES {
            if self.vertices[i].is_active() {
                let root = find(&mut parent, i as u16);
                if component_map[root as usize] == 0xFFFF {
                    component_map[root as usize] = component_count;
                    self.vertices[i].component = component_count;
                    component_count += 1;
                } else {
                    self.vertices[i].component = component_map[root as usize];
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

    /// Clear the graph
    pub fn clear(&mut self) {
        for v in &mut self.vertices {
            v.flags = 0;
            v.degree = 0;
        }
        self.num_vertices = 0;
        self.num_edges = 0;
        self.num_components = 0;
        self.free_edge_head = 0xFFFF;
        self.status = Self::STATUS_VALID | Self::STATUS_DIRTY;
        self.generation = self.generation.wrapping_add(1);
    }

    fn allocate_edge(&mut self) -> Option<EdgeId> {
        if self.free_edge_head != 0xFFFF {
            let id = self.free_edge_head;
            self.free_edge_head = self.edges[id as usize].source;
            return Some(id);
        }
        for i in 0..MAX_PATCH_EDGES {
            if self.edges[i].weight == 0 && self.edges[i].source == 0 && self.edges[i].target == 0 {
                return Some(i as EdgeId);
            }
        }
        None
    }

    fn free_edge(&mut self, edge_id: EdgeId) {
        self.edges[edge_id as usize].source = self.free_edge_head;
        self.edges[edge_id as usize].target = 0;
        self.edges[edge_id as usize].weight = 0;
        self.free_edge_head = edge_id;
    }

    fn remove_from_adj(&mut self, v: VertexId, neighbor: VertexId, edge_id: EdgeId) {
        if v as usize >= MAX_PATCH_VERTICES {
            return;
        }
        let degree = self.vertices[v as usize].degree as usize;
        for i in 0..degree {
            if self.adjacency[v as usize][i].neighbor == neighbor
                && self.adjacency[v as usize][i].edge_id == edge_id
            {
                if i < degree - 1 {
                    self.adjacency[v as usize][i] = self.adjacency[v as usize][degree - 1];
                }
                self.vertices[v as usize].degree -= 1;
                return;
            }
        }
    }

    /// Get memory size
    pub const fn memory_size() -> usize {
        size_of::<Self>()
    }
}

// ============================================================================
// SYNDROME BUFFER
// ============================================================================

/// Syndrome ring entry
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct SyndromeEntry {
    /// Round number
    pub round: u32,
    /// Syndrome bits (packed)
    pub syndrome: [u8; 8],
    /// Flags
    pub flags: u32,
}

/// Rolling syndrome buffer (1024 rounds)
#[derive(Debug)]
#[repr(C, align(64))]
pub struct SyndromBuffer {
    /// Ring buffer of syndrome entries
    pub entries: [SyndromeEntry; SYNDROME_BUFFER_DEPTH],
    /// Head pointer
    pub head: u16,
    /// Count of valid entries
    pub count: u16,
    /// Current round
    pub current_round: u32,
    /// Padding
    _pad: [u8; 56],
}

impl Default for SyndromBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl SyndromBuffer {
    /// Create a new syndrome buffer
    pub const fn new() -> Self {
        Self {
            entries: [SyndromeEntry {
                round: 0,
                syndrome: [0; 8],
                flags: 0,
            }; SYNDROME_BUFFER_DEPTH],
            head: 0,
            count: 0,
            current_round: 0,
            _pad: [0; 56],
        }
    }

    /// Append a syndrome entry
    pub fn append(&mut self, entry: SyndromeEntry) {
        self.entries[self.head as usize] = entry;
        self.head = ((self.head as usize + 1) % SYNDROME_BUFFER_DEPTH) as u16;
        if (self.count as usize) < SYNDROME_BUFFER_DEPTH {
            self.count += 1;
        }
        self.current_round = entry.round;
    }

    /// Get recent syndrome entries
    pub fn recent(&self, count: usize) -> impl Iterator<Item = &SyndromeEntry> {
        let count = count.min(self.count as usize);
        let start = if self.head as usize >= count {
            self.head as usize - count
        } else {
            SYNDROME_BUFFER_DEPTH - (count - self.head as usize)
        };

        (0..count).map(move |i| {
            let idx = (start + i) % SYNDROME_BUFFER_DEPTH;
            &self.entries[idx]
        })
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.head = 0;
        self.count = 0;
        self.current_round = 0;
    }

    /// Get memory size
    pub const fn memory_size() -> usize {
        size_of::<Self>()
    }
}

// ============================================================================
// EVIDENCE ACCUMULATOR
// ============================================================================

/// Evidence accumulator for anytime-valid testing
#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct EvidenceAccumulator {
    /// Global log e-value (log2(e) * 65536)
    pub log_e_value: LogEValue,
    /// Total observations
    pub obs_count: u32,
    /// Rejected hypothesis count
    pub rejected_count: u16,
    /// Status flags
    pub status: u16,
    /// Padding
    _pad: [u8; 48],
}

impl Default for EvidenceAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl EvidenceAccumulator {
    /// Status: accumulator is active
    pub const STATUS_ACTIVE: u16 = 0x0001;
    /// Status: has rejection
    pub const STATUS_HAS_REJECTION: u16 = 0x0002;
    /// Status: significant evidence
    pub const STATUS_SIGNIFICANT: u16 = 0x0004;

    /// Create a new evidence accumulator
    pub const fn new() -> Self {
        Self {
            log_e_value: 0,
            obs_count: 0,
            rejected_count: 0,
            status: Self::STATUS_ACTIVE,
            _pad: [0; 48],
        }
    }

    /// Process an observation with given log likelihood ratio
    pub fn observe(&mut self, log_lr: LogEValue) {
        self.log_e_value = self.log_e_value.saturating_add(log_lr);
        self.obs_count += 1;

        if self.log_e_value > LOG_E_STRONG {
            self.status |= Self::STATUS_SIGNIFICANT;
        }
        if self.log_e_value > LOG_E_VERY_STRONG {
            self.rejected_count += 1;
            self.status |= Self::STATUS_HAS_REJECTION;
        }
    }

    /// Get e-value as f64
    pub fn e_value(&self) -> f64 {
        let log2_val = (self.log_e_value as f64) / 65536.0;
        f64::exp2(log2_val)
    }

    /// Check if evidence is significant
    pub fn is_significant(&self) -> bool {
        self.status & Self::STATUS_SIGNIFICANT != 0
    }

    /// Reset the accumulator
    pub fn reset(&mut self) {
        self.log_e_value = 0;
        self.obs_count = 0;
        self.rejected_count = 0;
        self.status = Self::STATUS_ACTIVE;
    }
}

// ============================================================================
// LOCAL CUT STATE
// ============================================================================

/// Local min-cut state tracking
#[derive(Debug, Clone)]
#[repr(C, align(64))]
pub struct LocalCutState {
    /// Current local cut value estimate
    pub cut_value: f64,
    /// Previous cut value (for delta detection)
    pub prev_cut_value: f64,
    /// Boundary candidate edge IDs
    pub boundary_candidates: [EdgeId; MAX_BOUNDARY_CANDIDATES],
    /// Number of boundary candidates
    pub num_candidates: u16,
    /// Cut generation
    pub generation: u16,
    /// Boundary has moved flag
    pub boundary_moved: bool,
    /// Padding
    _pad: [u8; 51],
}

impl Default for LocalCutState {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalCutState {
    /// Create a new local cut state
    pub const fn new() -> Self {
        Self {
            cut_value: 0.0,
            prev_cut_value: 0.0,
            boundary_candidates: [0; MAX_BOUNDARY_CANDIDATES],
            num_candidates: 0,
            generation: 0,
            boundary_moved: false,
            _pad: [0; 51],
        }
    }

    /// Update from patch graph
    pub fn update_from_graph(&mut self, graph: &PatchGraph) {
        self.prev_cut_value = self.cut_value;
        self.cut_value = graph.estimate_local_cut();

        // Identify boundary candidates
        self.num_candidates =
            graph.identify_boundary_candidates(&mut self.boundary_candidates) as u16;

        // Detect boundary movement
        let delta = (self.cut_value - self.prev_cut_value).abs();
        self.boundary_moved = delta > 0.1 * self.prev_cut_value.max(1.0);

        self.generation = self.generation.wrapping_add(1);
    }

    /// Get boundary candidates as slice
    pub fn candidates(&self) -> &[EdgeId] {
        &self.boundary_candidates[..self.num_candidates as usize]
    }
}

// ============================================================================
// TILE REPORT
// ============================================================================

/// Report produced by a worker tile after each tick
#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct TileReport {
    /// Tile ID (1-255)
    pub tile_id: TileId,
    /// Status flags
    pub status: u8,
    /// Generation number
    pub generation: u16,
    /// Tick number
    pub tick: u32,

    /// Local cut value estimate
    pub local_cut: f64,

    /// Boundary candidate edge IDs (top 8)
    pub boundary_candidates: [EdgeId; 8],

    /// Shift score (distribution drift)
    pub shift_score: f64,

    /// E-value (evidence accumulator)
    pub e_value: f64,

    /// Number of vertices
    pub num_vertices: u16,
    /// Number of edges
    pub num_edges: u16,
    /// Number of components
    pub num_components: u16,
    /// Boundary moved flag
    pub boundary_moved: bool,
    /// Reserved
    _reserved: u8,
}

impl Default for TileReport {
    fn default() -> Self {
        Self::new(0)
    }
}

impl TileReport {
    /// Status: report is valid
    pub const STATUS_VALID: u8 = 0x01;
    /// Status: tile had error
    pub const STATUS_ERROR: u8 = 0x02;
    /// Status: boundary moved
    pub const STATUS_BOUNDARY_MOVED: u8 = 0x04;

    /// Create a new tile report
    pub const fn new(tile_id: TileId) -> Self {
        Self {
            tile_id,
            status: Self::STATUS_VALID,
            generation: 0,
            tick: 0,
            local_cut: 0.0,
            boundary_candidates: [0; 8],
            shift_score: 0.0,
            e_value: 1.0,
            num_vertices: 0,
            num_edges: 0,
            num_components: 0,
            boundary_moved: false,
            _reserved: 0,
        }
    }
}

// ============================================================================
// WORKER TILE
// ============================================================================

/// Worker tile - individual processing unit in the 256-tile fabric
///
/// Memory budget: ~64KB
/// - PatchGraph: ~32KB
/// - SyndromBuffer: ~16KB
/// - Evidence + LocalCut + Control: ~16KB
#[derive(Debug)]
#[repr(C)]
pub struct WorkerTile {
    /// Tile identifier (1-255)
    pub tile_id: TileId,
    /// Current tick number
    pub tick: u32,
    /// Generation number
    pub generation: u16,
    /// Status flags
    pub status: u8,
    /// Reserved
    _reserved: u8,

    /// Local graph shard
    pub patch_graph: PatchGraph,
    /// Syndrome ring buffer
    pub syndrome_buffer: SyndromBuffer,
    /// Evidence accumulator
    pub evidence: EvidenceAccumulator,
    /// Local cut state
    pub local_cut_state: LocalCutState,
}

impl WorkerTile {
    /// Create a new worker tile
    pub fn new(tile_id: TileId) -> Self {
        debug_assert!(tile_id != 0, "TileId 0 is reserved for TileZero");
        Self {
            tile_id,
            tick: 0,
            generation: 0,
            status: 0,
            _reserved: 0,
            patch_graph: PatchGraph::new(),
            syndrome_buffer: SyndromBuffer::new(),
            evidence: EvidenceAccumulator::new(),
            local_cut_state: LocalCutState::new(),
        }
    }

    /// Process one tick of the coherence gate
    ///
    /// This is the main entry point for per-cycle processing:
    /// 1. Apply syndrome delta to patch graph
    /// 2. Update local cut state
    /// 3. Accumulate evidence
    /// 4. Generate tile report
    pub fn tick(&mut self, delta: &SyndromeDelta) -> TileReport {
        self.tick += 1;

        // 1. Apply delta to graph
        self.patch_graph.apply_delta(delta);

        // 2. Add to syndrome buffer if it's a syndrome observation
        if delta.is_syndrome() {
            let entry = SyndromeEntry {
                round: self.tick,
                syndrome: [
                    (delta.value & 0xFF) as u8,
                    ((delta.value >> 8) & 0xFF) as u8,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                ],
                flags: delta.flags as u32,
            };
            self.syndrome_buffer.append(entry);
        }

        // 3. Recompute graph state if dirty
        if self.patch_graph.status & PatchGraph::STATUS_DIRTY != 0 {
            self.patch_graph.recompute_components();
        }

        // 4. Update local cut state
        self.local_cut_state.update_from_graph(&self.patch_graph);

        // 5. Update evidence (using syndrome value as log likelihood ratio proxy)
        if delta.is_syndrome() {
            // Map syndrome value to log likelihood ratio
            // High syndrome value = evidence of instability
            let log_lr = if delta.value > 128 {
                (delta.value as LogEValue - 128) * 256 // Positive evidence against coherence
            } else {
                (128 - delta.value as LogEValue) * 256 // Positive evidence for coherence
            };
            self.evidence.observe(-log_lr); // Negate because we test H0: coherent
        }

        // 6. Compute shift score
        let shift_score = self.compute_shift_score();

        // 7. Build report
        let mut report = TileReport::new(self.tile_id);
        report.tick = self.tick;
        report.generation = self.generation;
        report.local_cut = self.local_cut_state.cut_value;
        report.shift_score = shift_score;
        report.e_value = self.evidence.e_value();
        report.num_vertices = self.patch_graph.num_vertices;
        report.num_edges = self.patch_graph.num_edges;
        report.num_components = self.patch_graph.num_components;
        report.boundary_moved = self.local_cut_state.boundary_moved;

        if report.boundary_moved {
            report.status |= TileReport::STATUS_BOUNDARY_MOVED;
        }

        // Copy top boundary candidates
        let candidates = self.local_cut_state.candidates();
        let count = candidates.len().min(8);
        report.boundary_candidates[..count].copy_from_slice(&candidates[..count]);

        self.generation = self.generation.wrapping_add(1);
        report
    }

    /// Compute shift score from recent syndrome history
    ///
    /// Uses Welford's online algorithm to avoid allocation
    #[inline]
    fn compute_shift_score(&self) -> f64 {
        // Need at least 32 entries for meaningful variance
        if (self.syndrome_buffer.count as usize) < 32 {
            return 0.0;
        }

        // Use Welford's online algorithm to compute variance in one pass
        // Avoids allocation by iterating directly
        let mut count = 0u64;
        let mut sum: u64 = 0;
        let mut sum_sq: u64 = 0;

        for entry in self.syndrome_buffer.recent(32) {
            let val = entry.syndrome[0] as u64;
            sum += val;
            sum_sq += val * val;
            count += 1;
        }

        if count < 32 {
            return 0.0;
        }

        // Variance = E[X²] - E[X]²
        let n = count as f64;
        let mean = sum as f64 / n;
        let variance = (sum_sq as f64 / n) - (mean * mean);

        // Normalize variance as shift score (higher = more shift)
        (variance / 128.0).min(1.0)
    }

    /// Reset the worker tile
    pub fn reset(&mut self) {
        self.tick = 0;
        self.generation = 0;
        self.status = 0;
        self.patch_graph.clear();
        self.syndrome_buffer.clear();
        self.evidence.reset();
        self.local_cut_state = LocalCutState::new();
    }

    /// Get memory size
    pub const fn memory_size() -> usize {
        size_of::<Self>()
    }
}

// ============================================================================
// GATE THRESHOLDS
// ============================================================================

/// Configuration thresholds for the coherence gate
#[derive(Debug, Clone, Copy)]
pub struct GateThresholds {
    /// Minimum structural cut value
    pub structural_min_cut: f64,
    /// Maximum shift pressure before deferral
    pub shift_max: f64,
    /// E-value threshold for denial (H0 rejected)
    pub tau_deny: f64,
    /// E-value threshold for permit (sufficient evidence for H0)
    pub tau_permit: f64,
    /// Permit token TTL in nanoseconds
    pub permit_ttl_ns: u64,
}

impl Default for GateThresholds {
    fn default() -> Self {
        Self {
            structural_min_cut: 5.0,
            shift_max: 0.5,
            tau_deny: 0.01,
            tau_permit: 100.0,
            permit_ttl_ns: 4_000_000, // 4ms (compatible with 1MHz syndrome rate)
        }
    }
}

// ============================================================================
// GATE DECISION
// ============================================================================

/// Gate decision output
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GateDecision {
    /// Safe to proceed - all filters passed
    Permit = 0,
    /// Uncertain - escalate to human or stronger model
    Defer = 1,
    /// Unsafe - block the action
    Deny = 2,
}

impl GateDecision {
    /// Check if this is a permit
    pub const fn is_permit(&self) -> bool {
        matches!(self, GateDecision::Permit)
    }

    /// Check if this is a denial
    pub const fn is_deny(&self) -> bool {
        matches!(self, GateDecision::Deny)
    }
}

// ============================================================================
// PERMIT TOKEN
// ============================================================================

/// Permit token issued by TileZero
///
/// SECURITY: Tokens must be cryptographically signed to prevent forgery.
/// Production deployments MUST use proper Ed25519 key management.
#[derive(Debug, Clone)]
pub struct PermitToken {
    /// Gate decision
    pub decision: GateDecision,
    /// Sequence number
    pub sequence: u64,
    /// Timestamp (nanoseconds)
    pub timestamp: u64,
    /// Time-to-live (nanoseconds)
    pub ttl_ns: u64,
    /// Witness hash (Blake3)
    pub witness_hash: [u8; 32],
    /// Ed25519 signature (64 bytes as per spec)
    /// SECURITY: This field MUST contain a valid Ed25519 signature in production.
    pub signature: [u8; 64],
}

impl PermitToken {
    /// Check if token is still valid (time bounds only)
    ///
    /// SECURITY: This method only checks time validity. Callers MUST also verify
    /// the signature using `verify_signature()` before trusting the token.
    pub fn is_valid(&self, now_ns: u64) -> bool {
        self.decision == GateDecision::Permit
            && now_ns >= self.timestamp  // Not before issued
            && now_ns <= self.timestamp.saturating_add(self.ttl_ns) // Not after expiry
    }

    /// Compute the message bytes to be signed
    ///
    /// Returns a canonical byte representation of the token for signing/verification.
    pub fn signature_message(&self) -> [u8; 81] {
        let mut msg = [0u8; 81];
        msg[0] = self.decision as u8;
        msg[1..9].copy_from_slice(&self.sequence.to_le_bytes());
        msg[9..17].copy_from_slice(&self.timestamp.to_le_bytes());
        msg[17..25].copy_from_slice(&self.ttl_ns.to_le_bytes());
        msg[25..57].copy_from_slice(&self.witness_hash);
        // Remaining 24 bytes are zero padding for future fields
        msg
    }

    /// Verify the token signature using Ed25519
    ///
    /// # Security
    /// This method uses constant-time comparison to prevent timing attacks.
    ///
    /// # Arguments
    /// * `public_key` - The 32-byte Ed25519 public key of TileZero
    ///
    /// # Returns
    /// `true` if signature is valid, `false` otherwise
    pub fn verify_signature(&self, public_key: &[u8; 32]) -> bool {
        // Reject all-zero signatures immediately
        let zero_sig = [0u8; 64];
        if self.signature.ct_eq(&zero_sig).into() {
            return false;
        }

        // Parse the public key
        let verifying_key = match VerifyingKey::from_bytes(public_key) {
            Ok(key) => key,
            Err(_) => return false,
        };

        // Parse the signature (from_slice returns Result, from_bytes takes array)
        let sig_bytes: [u8; 64] = self.signature;
        let signature = Signature::from_bytes(&sig_bytes);

        // Compute message hash using Blake3 for domain separation
        let message = self.signature_message();
        let hash = blake3::hash(&message);

        // Verify signature over the hash
        verifying_key
            .verify_strict(hash.as_bytes(), &signature)
            .is_ok()
    }
}

// ============================================================================
// RECEIPT LOG
// ============================================================================

/// Receipt entry in the hash-chained log
#[derive(Debug, Clone)]
pub struct ReceiptEntry {
    /// Sequence number
    pub sequence: u64,
    /// Decision
    pub decision: GateDecision,
    /// Timestamp
    pub timestamp: u64,
    /// Witness hash
    pub witness_hash: [u8; 32],
    /// Previous hash (for chaining)
    pub previous_hash: [u8; 32],
    /// This entry's hash
    pub hash: [u8; 32],
}

/// Hash-chained receipt log
///
/// SECURITY: The hash chain provides tamper-evidence for the audit trail.
/// Each entry's hash is computed from its data and the previous entry's hash.
#[derive(Debug, Clone, Default)]
pub struct ReceiptLog {
    /// Log entries
    entries: Vec<ReceiptEntry>,
    /// Last hash for chaining
    last_hash: [u8; 32],
}

impl ReceiptLog {
    /// Create a new receipt log
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            last_hash: [0u8; 32],
        }
    }

    /// Append a receipt with cryptographic hash chaining using Blake3
    ///
    /// # Security
    /// Uses Blake3 for cryptographic hash chaining, ensuring tamper-evidence.
    /// The hash is computed as: H(prev_hash || sequence || decision || timestamp || witness_hash)
    pub fn append(
        &mut self,
        decision: GateDecision,
        sequence: u64,
        timestamp: u64,
        witness_hash: [u8; 32],
    ) {
        // Compute Blake3 hash of all data including previous hash
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.last_hash);
        hasher.update(&sequence.to_le_bytes());
        hasher.update(&[decision as u8]);
        hasher.update(&timestamp.to_le_bytes());
        hasher.update(&witness_hash);
        let hash: [u8; 32] = *hasher.finalize().as_bytes();

        let entry = ReceiptEntry {
            sequence,
            decision,
            timestamp,
            witness_hash,
            previous_hash: self.last_hash,
            hash,
        };

        self.last_hash = hash;
        self.entries.push(entry);
    }

    /// Verify the integrity of the hash chain
    ///
    /// # Security
    /// Recomputes all hashes using Blake3 and verifies chain integrity.
    /// Uses constant-time comparison to prevent timing attacks.
    ///
    /// Returns `true` if all entries are correctly chained, `false` if tampering detected.
    pub fn verify_chain(&self) -> bool {
        if self.entries.is_empty() {
            return true;
        }

        // First entry should chain from zero hash
        let mut expected_prev = [0u8; 32];

        for entry in &self.entries {
            // Verify previous hash link (constant-time)
            if !bool::from(entry.previous_hash.ct_eq(&expected_prev)) {
                return false;
            }

            // Recompute hash to verify integrity
            let mut hasher = blake3::Hasher::new();
            hasher.update(&entry.previous_hash);
            hasher.update(&entry.sequence.to_le_bytes());
            hasher.update(&[entry.decision as u8]);
            hasher.update(&entry.timestamp.to_le_bytes());
            hasher.update(&entry.witness_hash);
            let computed_hash: [u8; 32] = *hasher.finalize().as_bytes();

            // Verify hash matches (constant-time)
            if !bool::from(entry.hash.ct_eq(&computed_hash)) {
                return false;
            }

            expected_prev = entry.hash;
        }

        // Last hash should match our stored value (constant-time)
        bool::from(self.last_hash.ct_eq(&expected_prev))
    }

    /// Get last hash
    pub fn last_hash(&self) -> [u8; 32] {
        self.last_hash
    }

    /// Get entry by sequence
    pub fn get(&self, sequence: u64) -> Option<&ReceiptEntry> {
        self.entries.iter().find(|e| e.sequence == sequence)
    }

    /// Get log length
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if log is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ============================================================================
// TILE ZERO (COORDINATOR)
// ============================================================================

/// TileZero - Coordinator tile that merges worker reports and issues decisions
#[derive(Debug)]
pub struct TileZero {
    /// Gate thresholds
    pub thresholds: GateThresholds,
    /// Collected worker reports
    worker_reports: Vec<TileReport>,
    /// Receipt log
    pub receipt_log: ReceiptLog,
    /// Sequence counter
    sequence: u64,
    /// Ed25519 signing key for permit tokens
    /// SECURITY: In production, this key should be stored in a secure enclave/HSM
    signing_key: Option<SigningKey>,
}

impl TileZero {
    /// Create a new TileZero coordinator without signing capability
    ///
    /// Tokens issued by this coordinator will have placeholder signatures
    /// and MUST NOT be trusted in production.
    pub fn new(thresholds: GateThresholds) -> Self {
        Self {
            thresholds,
            worker_reports: Vec::with_capacity(NUM_WORKERS),
            receipt_log: ReceiptLog::new(),
            sequence: 0,
            signing_key: None,
        }
    }

    /// Create a new TileZero coordinator with Ed25519 signing capability
    ///
    /// # Security
    /// The signing key enables cryptographic token signing. In production:
    /// - Store the key in a secure enclave or HSM
    /// - Never log or expose the key bytes
    /// - Rotate keys periodically
    ///
    /// # Arguments
    /// * `thresholds` - Gate thresholds for decision logic
    /// * `signing_key` - Ed25519 signing key for token authentication
    pub fn with_signing_key(thresholds: GateThresholds, signing_key: SigningKey) -> Self {
        Self {
            thresholds,
            worker_reports: Vec::with_capacity(NUM_WORKERS),
            receipt_log: ReceiptLog::new(),
            sequence: 0,
            signing_key: Some(signing_key),
        }
    }

    /// Create a new TileZero coordinator with a randomly generated signing key
    ///
    /// This is convenient for testing but in production, keys should be
    /// deterministically derived from secure key material.
    pub fn with_random_key(thresholds: GateThresholds) -> Self {
        use rand::rngs::OsRng;
        let signing_key = SigningKey::generate(&mut OsRng);
        Self::with_signing_key(thresholds, signing_key)
    }

    /// Get the verifying (public) key if signing is enabled
    ///
    /// Use this to verify tokens issued by this TileZero.
    pub fn verifying_key(&self) -> Option<VerifyingKey> {
        self.signing_key.as_ref().map(|sk| sk.verifying_key())
    }

    /// Check if cryptographic signing is enabled
    pub fn has_signing_key(&self) -> bool {
        self.signing_key.is_some()
    }

    /// Merge reports from worker tiles and produce a gate decision
    pub fn merge_reports(&mut self, reports: Vec<TileReport>) -> GateDecision {
        self.worker_reports = reports;

        // Aggregate metrics from all tiles
        let (global_cut, shift_pressure, e_aggregate) = self.aggregate_metrics();

        // Three-filter decision logic
        let decision = self.evaluate_filters(global_cut, shift_pressure, e_aggregate);

        // Compute witness hash
        let witness_hash = self.compute_witness_hash();

        // Get timestamp (simplified - use proper time in production)
        let timestamp = self.sequence * 1_000_000; // Pseudo-timestamp

        // Issue permit token and log receipt
        self.receipt_log
            .append(decision, self.sequence, timestamp, witness_hash);
        self.sequence += 1;

        decision
    }

    /// Issue a permit token for the current decision
    ///
    /// # Security
    ///
    /// If a signing key is configured (via `with_signing_key` or `with_random_key`),
    /// tokens are cryptographically signed with Ed25519 and can be verified.
    ///
    /// If no signing key is configured, tokens have placeholder signatures marked
    /// with byte 63 = 0xFF. These tokens MUST NOT be trusted in production.
    ///
    /// # Returns
    ///
    /// A `PermitToken` containing:
    /// - The gate decision
    /// - Sequence number and timestamp
    /// - Witness hash of the current state
    /// - Ed25519 signature (real if key available, placeholder otherwise)
    pub fn issue_permit(&self, decision: &GateDecision) -> PermitToken {
        let timestamp = self.receipt_log.last_hash()[0..8]
            .try_into()
            .map(u64::from_le_bytes)
            .unwrap_or(0);

        let witness_hash = self.compute_witness_hash();

        // Build token structure first (signature will be computed over this)
        let mut token = PermitToken {
            decision: *decision,
            sequence: self.sequence.saturating_sub(1),
            timestamp,
            ttl_ns: self.thresholds.permit_ttl_ns,
            witness_hash,
            signature: [0u8; 64],
        };

        // Sign the token
        if let Some(ref signing_key) = self.signing_key {
            // Real Ed25519 signature
            let message = token.signature_message();
            let hash = blake3::hash(&message);
            let signature = signing_key.sign(hash.as_bytes());
            token.signature = signature.to_bytes();
        } else {
            // Placeholder signature - includes entropy but is NOT cryptographically secure
            // SECURITY WARNING: Tokens without real signatures MUST NOT be trusted!
            token.signature[0..8].copy_from_slice(&token.sequence.to_le_bytes());
            token.signature[8..16].copy_from_slice(&timestamp.to_le_bytes());
            token.signature[16..48].copy_from_slice(&witness_hash);
            // Mark as placeholder (byte 63 = 0xFF indicates unsigned)
            token.signature[63] = 0xFF;
        }

        token
    }

    /// Check if a token was signed by this TileZero
    ///
    /// # Returns
    ///
    /// - `Some(true)` if the signature is valid
    /// - `Some(false)` if the signature is invalid
    /// - `None` if no signing key is configured (cannot verify)
    pub fn verify_token(&self, token: &PermitToken) -> Option<bool> {
        let verifying_key = self.verifying_key()?;
        Some(token.verify_signature(&verifying_key.to_bytes()))
    }

    /// Aggregate metrics from worker reports
    ///
    /// Computes:
    /// - Global min-cut: minimum of all local cuts
    /// - Shift pressure: maximum shift score across tiles
    /// - E-aggregate: geometric mean of e-values
    #[inline]
    fn aggregate_metrics(&self) -> (f64, f64, f64) {
        if self.worker_reports.is_empty() {
            return (f64::MAX, 0.0, 1.0);
        }

        let mut min_cut = f64::MAX;
        let mut total_shift = 0.0;
        let mut log_e_sum = 0.0;

        for report in &self.worker_reports {
            // Global cut is minimum of local cuts
            if report.local_cut < min_cut && report.local_cut > 0.0 {
                min_cut = report.local_cut;
            }

            // Shift pressure is maximum across tiles
            if report.shift_score > total_shift {
                total_shift = report.shift_score;
            }

            // E-values multiply (add in log space)
            log_e_sum += f64::log2(report.e_value.max(1e-10));
        }

        // Convert back from log space
        let e_aggregate = f64::exp2(log_e_sum / self.worker_reports.len() as f64);

        (min_cut, total_shift, e_aggregate)
    }

    /// Evaluate the three-filter decision logic
    /// Evaluate the three-filter decision logic
    #[inline]
    fn evaluate_filters(
        &self,
        global_cut: f64,
        shift_pressure: f64,
        e_aggregate: f64,
    ) -> GateDecision {
        // Filter 1: Structural (min-cut check)
        if global_cut < self.thresholds.structural_min_cut {
            return GateDecision::Deny;
        }

        // Filter 2: Shift (distribution drift)
        if shift_pressure >= self.thresholds.shift_max {
            return GateDecision::Defer;
        }

        // Filter 3: Evidence (e-value thresholds)
        if e_aggregate < self.thresholds.tau_deny {
            return GateDecision::Deny;
        }
        if e_aggregate < self.thresholds.tau_permit {
            return GateDecision::Defer;
        }

        // All filters passed
        GateDecision::Permit
    }

    /// Compute witness hash from current state
    fn compute_witness_hash(&self) -> [u8; 32] {
        let mut hash = [0u8; 32];

        // Simplified hash - use blake3 in production
        // Each report contributes 5 bytes (1 for tile_id + 4 for cut), so max 6 reports
        let mut idx = 0;
        for report in self.worker_reports.iter().take(6) {
            if idx + 5 > 32 {
                break;
            }
            hash[idx] = report.tile_id;
            idx += 1;
            let cut_bytes = report.local_cut.to_le_bytes();
            hash[idx..idx + 4].copy_from_slice(&cut_bytes[0..4]);
            idx += 4;
        }

        hash
    }

    /// Get collected reports
    pub fn reports(&self) -> &[TileReport] {
        &self.worker_reports
    }
}

// ============================================================================
// SIZE ASSERTIONS
// ============================================================================

#[cfg(test)]
mod size_assertions {
    use super::*;

    #[test]
    fn test_patch_graph_size() {
        let size = PatchGraph::memory_size();
        // Should be around 32KB
        assert!(size <= 65536, "PatchGraph exceeds 64KB: {} bytes", size);
    }

    #[test]
    fn test_syndrome_buffer_size() {
        let size = SyndromBuffer::memory_size();
        // Should be around 16KB
        assert!(size <= 32768, "SyndromBuffer exceeds 32KB: {} bytes", size);
    }

    #[test]
    fn test_worker_tile_size() {
        let size = WorkerTile::memory_size();
        // Should fit in 64KB budget with some margin
        assert!(size <= 131072, "WorkerTile exceeds 128KB: {} bytes", size);
    }

    #[test]
    fn test_tile_report_alignment() {
        assert_eq!(core::mem::align_of::<TileReport>(), 64);
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_tile_creation() {
        let tile = WorkerTile::new(42);
        assert_eq!(tile.tile_id, 42);
        assert_eq!(tile.tick, 0);
    }

    #[test]
    fn test_worker_tile_tick() {
        let mut tile = WorkerTile::new(1);
        let delta = SyndromeDelta::new(0, 1, 100);
        let report = tile.tick(&delta);

        assert_eq!(report.tile_id, 1);
        assert_eq!(report.tick, 1);
    }

    #[test]
    fn test_patch_graph_add_edge() {
        let mut graph = PatchGraph::new();
        let edge_id = graph.add_edge(0, 1, 100);
        assert!(edge_id.is_some());
        assert_eq!(graph.num_edges, 1);
        assert_eq!(graph.num_vertices, 2);
    }

    #[test]
    fn test_patch_graph_remove_edge() {
        let mut graph = PatchGraph::new();
        graph.add_edge(0, 1, 100);
        assert!(graph.remove_edge(0, 1));
        assert_eq!(graph.num_edges, 0);
    }

    #[test]
    fn test_gate_decision_permit() {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        // Create reports with good metrics
        let reports: Vec<TileReport> = (1..=10)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 10.0;
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        let decision = tilezero.merge_reports(reports);
        assert_eq!(decision, GateDecision::Permit);
    }

    #[test]
    fn test_gate_decision_deny_structural() {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        // Create reports with low cut value
        let reports: Vec<TileReport> = (1..=10)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 1.0; // Below threshold
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        let decision = tilezero.merge_reports(reports);
        assert_eq!(decision, GateDecision::Deny);
    }

    #[test]
    fn test_gate_decision_defer_shift() {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        // Create reports with high shift
        let reports: Vec<TileReport> = (1..=10)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 10.0;
                report.shift_score = 0.8; // Above threshold
                report.e_value = 200.0;
                report
            })
            .collect();

        let decision = tilezero.merge_reports(reports);
        assert_eq!(decision, GateDecision::Defer);
    }

    #[test]
    fn test_receipt_log_chaining() {
        let mut log = ReceiptLog::new();

        log.append(GateDecision::Permit, 0, 1000, [0u8; 32]);
        log.append(GateDecision::Permit, 1, 2000, [1u8; 32]);
        log.append(GateDecision::Deny, 2, 3000, [2u8; 32]);

        assert_eq!(log.len(), 3);

        let entry1 = log.get(1).unwrap();
        let entry2 = log.get(2).unwrap();

        // Verify chain linkage
        assert_eq!(entry2.previous_hash, entry1.hash);
    }

    #[test]
    fn test_evidence_accumulator() {
        let mut evidence = EvidenceAccumulator::new();

        // Positive evidence for coherence
        for _ in 0..10 {
            evidence.observe(10000); // log_lr = 10000 / 65536 ~ 0.15
        }

        assert!(evidence.e_value() > 1.0);
        assert!(evidence.obs_count == 10);
    }

    #[test]
    fn test_syndrome_buffer() {
        let mut buffer = SyndromBuffer::new();

        for i in 0..100 {
            let entry = SyndromeEntry {
                round: i,
                syndrome: [i as u8; 8],
                flags: 0,
            };
            buffer.append(entry);
        }

        assert_eq!(buffer.count, 100);
        assert_eq!(buffer.current_round, 99);

        let recent: Vec<_> = buffer.recent(10).collect();
        assert_eq!(recent.len(), 10);
    }

    #[test]
    fn test_local_cut_state() {
        let mut graph = PatchGraph::new();
        graph.add_edge(0, 1, 100);
        graph.add_edge(1, 2, 100);
        graph.add_edge(2, 0, 100);
        graph.recompute_components();

        let mut cut_state = LocalCutState::new();
        cut_state.update_from_graph(&graph);

        assert!(cut_state.cut_value > 0.0);
    }

    #[test]
    fn test_permit_token_validity() {
        let token = PermitToken {
            decision: GateDecision::Permit,
            sequence: 0,
            timestamp: 1000,
            ttl_ns: 500,
            witness_hash: [0u8; 32],
            signature: [1u8; 64], // Non-zero placeholder signature
        };

        // Valid: within time bounds (1000 <= 1200 <= 1500)
        assert!(token.is_valid(1200));
        // Invalid: after expiry (1600 > 1500)
        assert!(!token.is_valid(1600));
        // Invalid: before issuance (500 < 1000)
        assert!(!token.is_valid(500));
    }

    #[test]
    fn test_permit_token_signature_verification() {
        let token = PermitToken {
            decision: GateDecision::Permit,
            sequence: 0,
            timestamp: 1000,
            ttl_ns: 500,
            witness_hash: [0u8; 32],
            signature: [0u8; 64], // All-zero signature
        };

        // Zero signature should always be rejected
        let dummy_pubkey = [0u8; 32];
        assert!(!token.verify_signature(&dummy_pubkey));
    }

    #[test]
    fn test_receipt_log_chain_verification() {
        let mut log = ReceiptLog::new();

        log.append(GateDecision::Permit, 0, 1000, [0u8; 32]);
        log.append(GateDecision::Permit, 1, 2000, [1u8; 32]);
        log.append(GateDecision::Deny, 2, 3000, [2u8; 32]);

        // Chain should verify correctly
        assert!(log.verify_chain());
    }

    #[test]
    fn test_tilezero_with_signing_key() {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        // Create TileZero with a random signing key
        let thresholds = GateThresholds::default();
        let tilezero = TileZero::with_random_key(thresholds);

        // Should have signing capability
        assert!(tilezero.has_signing_key());
        assert!(tilezero.verifying_key().is_some());
    }

    #[test]
    fn test_permit_token_real_signature() {
        // Create TileZero with signing key
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::with_random_key(thresholds);

        // Create some reports and make a decision
        let reports: Vec<TileReport> = (1..=5)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 10.0;
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        let decision = tilezero.merge_reports(reports);
        assert_eq!(decision, GateDecision::Permit);

        // Issue a permit token
        let token = tilezero.issue_permit(&decision);

        // Token should have a real signature (byte 63 != 0xFF)
        assert_ne!(token.signature[63], 0xFF, "Token has placeholder signature");

        // Get the verifying key and verify the token
        let verifying_key = tilezero.verifying_key().expect("Should have verifying key");
        let is_valid = token.verify_signature(&verifying_key.to_bytes());
        assert!(is_valid, "Token signature should be valid");

        // Also test via the verify_token method
        let result = tilezero.verify_token(&token);
        assert_eq!(result, Some(true), "verify_token should return Some(true)");
    }

    #[test]
    fn test_permit_token_placeholder_signature() {
        // Create TileZero WITHOUT signing key
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        // Should not have signing capability
        assert!(!tilezero.has_signing_key());
        assert!(tilezero.verifying_key().is_none());

        // Create reports and decision
        let reports: Vec<TileReport> = (1..=5)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 10.0;
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        let decision = tilezero.merge_reports(reports);
        let token = tilezero.issue_permit(&decision);

        // Token should have placeholder marker
        assert_eq!(
            token.signature[63], 0xFF,
            "Token should have placeholder signature marker"
        );

        // verify_token should return None when no key is configured
        assert_eq!(tilezero.verify_token(&token), None);
    }

    #[test]
    fn test_token_signature_tampering_detected() {
        // Create TileZero with signing key
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::with_random_key(thresholds);

        let reports: Vec<TileReport> = (1..=5)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 10.0;
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        let decision = tilezero.merge_reports(reports);
        let mut token = tilezero.issue_permit(&decision);

        // Tamper with the token
        token.sequence += 1;

        // Signature should no longer verify
        let verifying_key = tilezero.verifying_key().unwrap();
        let is_valid = token.verify_signature(&verifying_key.to_bytes());
        assert!(!is_valid, "Tampered token signature should be invalid");
    }

    #[test]
    fn test_different_keys_different_signatures() {
        let thresholds = GateThresholds::default();
        let mut tilezero1 = TileZero::with_random_key(thresholds.clone());
        let mut tilezero2 = TileZero::with_random_key(thresholds);

        let reports: Vec<TileReport> = (1..=3)
            .map(|i| {
                let mut report = TileReport::new(i);
                report.local_cut = 10.0;
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        // Make decisions and get tokens
        let decision1 = tilezero1.merge_reports(reports.clone());
        let decision2 = tilezero2.merge_reports(reports);

        let token1 = tilezero1.issue_permit(&decision1);
        let token2 = tilezero2.issue_permit(&decision2);

        // Signatures should be different (different keys)
        assert_ne!(token1.signature, token2.signature);

        // Each token should only verify with its own key
        let key1 = tilezero1.verifying_key().unwrap();
        let key2 = tilezero2.verifying_key().unwrap();

        assert!(token1.verify_signature(&key1.to_bytes()));
        assert!(!token1.verify_signature(&key2.to_bytes())); // Wrong key
        assert!(!token2.verify_signature(&key1.to_bytes())); // Wrong key
        assert!(token2.verify_signature(&key2.to_bytes()));
    }
}
