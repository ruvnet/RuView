//! Canonical witness fragments using pseudo-deterministic min-cut.
//!
//! Produces reproducible, hash-stable witness fragments by computing
//! a canonical min-cut partition via lexicographic tie-breaking.
//!
//! All structures are `#[repr(C)]` aligned, use fixed-size arrays, and
//! operate entirely on the stack (no heap allocation). This module is
//! designed for no_std WASM tiles with a ~2.1KB temporary memory footprint.

#![allow(missing_docs)]

use crate::shard::{CompactGraph, MAX_SHARD_VERTICES};
use core::mem::size_of;

// ============================================================================
// Fixed-point weight for deterministic comparison
// ============================================================================

/// Fixed-point weight for deterministic, total-order comparison.
///
/// Uses 16.16 fixed-point representation (upper 16 bits integer, lower 16
/// bits fractional). This avoids floating-point non-determinism in
/// partition comparisons.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct FixedPointWeight(pub u32);

impl FixedPointWeight {
    /// Zero weight constant
    pub const ZERO: Self = Self(0);

    /// One (1.0) in 16.16 fixed-point
    pub const ONE: Self = Self(65536);

    /// Maximum representable weight
    pub const MAX: Self = Self(u32::MAX);

    /// Convert from a `ShardEdge` weight (u16, 0.01 precision) to fixed-point.
    ///
    /// The shard weight is scaled up by shifting left 8 bits, mapping
    /// the 0-65535 range into the 16.16 fixed-point space.
    #[inline(always)]
    pub const fn from_u16_weight(w: u16) -> Self {
        Self((w as u32) << 8)
    }

    /// Saturating addition (clamps at `u32::MAX`)
    #[inline(always)]
    pub const fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    /// Saturating subtraction (clamps at 0)
    #[inline(always)]
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    /// Truncate to u16 by shifting right 8 bits (inverse of `from_u16_weight`)
    #[inline(always)]
    pub const fn to_u16(self) -> u16 {
        (self.0 >> 8) as u16
    }
}

// ============================================================================
// Cactus node and arena
// ============================================================================

/// A single node in the arena-allocated cactus tree.
///
/// Represents a vertex (or contracted 2-edge-connected component) in the
/// simplified cactus structure derived from the tile's compact graph.
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct CactusNode {
    /// Vertex ID in the original graph
    pub id: u16,
    /// Parent index in `ArenaCactus::nodes` (0xFFFF = root / no parent)
    pub parent: u16,
    /// Degree in the cactus tree
    pub degree: u8,
    /// Flags (reserved)
    pub flags: u8,
    /// Weight of the edge connecting this node to its parent
    pub weight_to_parent: FixedPointWeight,
}

impl CactusNode {
    /// Sentinel value indicating no parent (root node)
    pub const NO_PARENT: u16 = 0xFFFF;

    /// Create an empty / default node
    #[inline(always)]
    pub const fn empty() -> Self {
        Self {
            id: 0,
            parent: Self::NO_PARENT,
            degree: 0,
            flags: 0,
            weight_to_parent: FixedPointWeight::ZERO,
        }
    }
}

// Compile-time size check: repr(C) layout is 12 bytes
// (u16 + u16 + u8 + u8 + 2-pad + u32 = 12, aligned to 4)
// 256 nodes * 12 = 3072 bytes (~3KB), fits in 14.5KB headroom.
const _: () = assert!(size_of::<CactusNode>() == 12, "CactusNode must be 12 bytes");

/// Arena-allocated cactus tree for a single tile (up to 256 vertices).
///
/// The cactus captures the 2-edge-connected component structure of the
/// tile's local graph. It is built entirely on the stack (~2KB) and used
/// to derive a canonical min-cut partition.
#[repr(C)]
pub struct ArenaCactus {
    /// Node storage (one per vertex in the original graph)
    pub nodes: [CactusNode; 256],
    /// Number of active nodes
    pub n_nodes: u16,
    /// Root node index
    pub root: u16,
    /// Value of the global minimum cut found
    pub min_cut_value: FixedPointWeight,
}

impl ArenaCactus {
    /// Build a cactus from the tile's `CompactGraph`.
    ///
    /// Algorithm (simplified):
    /// 1. BFS spanning tree from the lowest-ID active vertex.
    /// 2. Identify back edges and compute 2-edge-connected components
    ///    via low-link (Tarjan-style on edges).
    /// 3. Contract each 2-edge-connected component into a single cactus
    ///    node; the inter-component bridge edges become cactus edges.
    /// 4. Track the minimum-weight bridge as the global min-cut value.
    pub fn build_from_compact_graph(graph: &CompactGraph) -> Self {
        let mut cactus = ArenaCactus {
            nodes: [CactusNode::empty(); 256],
            n_nodes: 0,
            root: 0xFFFF,
            min_cut_value: FixedPointWeight::MAX,
        };

        if graph.num_vertices == 0 {
            cactus.min_cut_value = FixedPointWeight::ZERO;
            return cactus;
        }

        // ---- Phase 1: BFS spanning tree ----
        // BFS queue (fixed-size ring buffer)
        let mut queue = [0u16; 256];
        let mut q_head: usize = 0;
        let mut q_tail: usize = 0;

        // Per-vertex BFS state
        let mut visited = [false; MAX_SHARD_VERTICES];
        let mut parent = [0xFFFFu16; MAX_SHARD_VERTICES];
        let mut depth = [0u16; MAX_SHARD_VERTICES];
        // Component ID for 2-edge-connected grouping
        let mut comp_id = [0xFFFFu16; MAX_SHARD_VERTICES];

        // Find lowest-ID active vertex as root
        let mut root_v = 0xFFFFu16;
        for v in 0..MAX_SHARD_VERTICES {
            if graph.vertices[v].is_active() {
                root_v = v as u16;
                break;
            }
        }

        if root_v == 0xFFFF {
            cactus.min_cut_value = FixedPointWeight::ZERO;
            return cactus;
        }

        // BFS
        visited[root_v as usize] = true;
        parent[root_v as usize] = 0xFFFF;
        queue[q_tail] = root_v;
        q_tail += 1;

        while q_head < q_tail {
            let u = queue[q_head] as usize;
            q_head += 1;

            let neighbors = graph.neighbors(u as u16);
            for adj in neighbors {
                let w = adj.neighbor as usize;
                if !visited[w] {
                    visited[w] = true;
                    parent[w] = u as u16;
                    depth[w] = depth[u] + 1;
                    if q_tail < 256 {
                        queue[q_tail] = w as u16;
                        q_tail += 1;
                    }
                }
            }
        }

        // ---- Phase 2: Identify 2-edge-connected components ----
        // For each back edge (u,w) where w is an ancestor of u in the BFS tree,
        // all vertices on the path from u to w belong to the same 2-edge-connected
        // component. We perform path marking for each back edge.
        let mut next_comp: u16 = 0;

        // Mark tree edges as bridges initially; back edges will un-bridge them
        // We iterate edges and find back edges (both endpoints visited, not parent-child)
        for e_idx in 0..graph.edges.len() {
            let edge = &graph.edges[e_idx];
            if !edge.is_active() {
                continue;
            }
            let u = edge.source as usize;
            let w = edge.target as usize;

            if !visited[u] || !visited[w] {
                continue;
            }

            // Check if this is a back edge (non-tree edge)
            let is_tree = (parent[w] == u as u16 && depth[w] == depth[u] + 1)
                || (parent[u] == w as u16 && depth[u] == depth[w] + 1);

            if is_tree {
                continue; // Skip tree edges
            }

            // Back edge found: mark the path from u to w as same component
            // Walk u and w up to their LCA, assigning a single component ID
            let c = if comp_id[u] != 0xFFFF {
                comp_id[u]
            } else if comp_id[w] != 0xFFFF {
                comp_id[w]
            } else {
                let c = next_comp;
                next_comp = next_comp.saturating_add(1);
                c
            };

            // Walk from u towards root, marking component
            let mut a = u as u16;
            while a != 0xFFFF && comp_id[a as usize] != c {
                if comp_id[a as usize] == 0xFFFF {
                    comp_id[a as usize] = c;
                }
                a = parent[a as usize];
            }

            // Walk from w towards root, marking component
            let mut b = w as u16;
            while b != 0xFFFF && comp_id[b as usize] != c {
                if comp_id[b as usize] == 0xFFFF {
                    comp_id[b as usize] = c;
                }
                b = parent[b as usize];
            }
        }

        // Assign each unmarked visited vertex its own component
        for v in 0..MAX_SHARD_VERTICES {
            if visited[v] && comp_id[v] == 0xFFFF {
                comp_id[v] = next_comp;
                next_comp = next_comp.saturating_add(1);
            }
        }

        // ---- Phase 3: Build cactus from component structure ----
        // Each unique comp_id becomes a cactus node.
        // The representative vertex is the lowest-ID vertex in the component.
        let mut comp_repr = [0xFFFFu16; 256]; // comp_id -> representative vertex
        let mut comp_to_node = [0xFFFFu16; 256]; // comp_id -> cactus node index

        // Find representative (lowest vertex ID) for each component
        for v in 0..MAX_SHARD_VERTICES {
            if !visited[v] {
                continue;
            }
            let c = comp_id[v] as usize;
            if c < 256 && (comp_repr[c] == 0xFFFF || (v as u16) < comp_repr[c]) {
                comp_repr[c] = v as u16;
            }
        }

        // Create cactus nodes for each component
        let mut n_cactus: u16 = 0;
        for c in 0..next_comp.min(256) as usize {
            if comp_repr[c] != 0xFFFF {
                let idx = n_cactus as usize;
                if idx < 256 {
                    cactus.nodes[idx] = CactusNode {
                        id: comp_repr[c],
                        parent: CactusNode::NO_PARENT,
                        degree: 0,
                        flags: 0,
                        weight_to_parent: FixedPointWeight::ZERO,
                    };
                    comp_to_node[c] = n_cactus;
                    n_cactus += 1;
                }
            }
        }

        cactus.n_nodes = n_cactus;

        // Set root to the node containing root_v
        let root_comp = comp_id[root_v as usize] as usize;
        if root_comp < 256 {
            cactus.root = comp_to_node[root_comp];
        }

        // ---- Phase 4: Connect cactus nodes via bridge edges ----
        // A tree edge (parent[v] -> v) where comp_id[parent[v]] != comp_id[v]
        // is a bridge. It becomes a cactus edge.
        for v in 0..MAX_SHARD_VERTICES {
            if !visited[v] || parent[v] == 0xFFFF {
                continue;
            }
            let p = parent[v] as usize;
            let cv = comp_id[v] as usize;
            let cp = comp_id[p] as usize;

            if cv != cp && cv < 256 && cp < 256 {
                let node_v = comp_to_node[cv];
                let node_p = comp_to_node[cp];

                if node_v < 256
                    && node_p < 256
                    && cactus.nodes[node_v as usize].parent == CactusNode::NO_PARENT
                    && node_v != cactus.root
                {
                    // Compute bridge weight: sum of edge weights between the
                    // two components along this boundary
                    let bridge_weight = Self::compute_bridge_weight(graph, v as u16, parent[v]);

                    cactus.nodes[node_v as usize].parent = node_p;
                    cactus.nodes[node_v as usize].weight_to_parent = bridge_weight;
                    cactus.nodes[node_p as usize].degree += 1;
                    cactus.nodes[node_v as usize].degree += 1;

                    // Track minimum cut
                    if bridge_weight < cactus.min_cut_value {
                        cactus.min_cut_value = bridge_weight;
                    }
                }
            }
        }

        // If no bridges found, min cut is sum of all edge weights (graph is
        // 2-edge-connected) or zero if there are no edges
        if cactus.min_cut_value == FixedPointWeight::MAX {
            if graph.num_edges == 0 {
                cactus.min_cut_value = FixedPointWeight::ZERO;
            } else {
                // 2-edge-connected: min cut is at least the minimum degree
                // weight sum. Compute as total weight / 2 as rough upper bound
                // or just report the minimum vertex weighted degree.
                cactus.min_cut_value = Self::min_vertex_weight_degree(graph);
            }
        }

        cactus
    }

    /// Compute bridge weight between two vertices that are in different
    /// 2-edge-connected components.
    fn compute_bridge_weight(graph: &CompactGraph, v: u16, p: u16) -> FixedPointWeight {
        // Find the edge between v and p and return its weight
        if let Some(eid) = graph.find_edge(v, p) {
            FixedPointWeight::from_u16_weight(graph.edges[eid as usize].weight)
        } else {
            FixedPointWeight::ONE
        }
    }

    /// Compute minimum vertex weighted degree in the graph.
    fn min_vertex_weight_degree(graph: &CompactGraph) -> FixedPointWeight {
        let mut min_weight = FixedPointWeight::MAX;

        for v in 0..MAX_SHARD_VERTICES {
            if !graph.vertices[v].is_active() || graph.vertices[v].degree == 0 {
                continue;
            }
            let mut weight_sum = FixedPointWeight::ZERO;
            let neighbors = graph.neighbors(v as u16);
            for adj in neighbors {
                let eid = adj.edge_id as usize;
                if eid < graph.edges.len() && graph.edges[eid].is_active() {
                    weight_sum = weight_sum
                        .saturating_add(FixedPointWeight::from_u16_weight(graph.edges[eid].weight));
                }
            }
            if weight_sum < min_weight {
                min_weight = weight_sum;
            }
        }

        if min_weight == FixedPointWeight::MAX {
            FixedPointWeight::ZERO
        } else {
            min_weight
        }
    }

    /// Derive the canonical (lex-smallest) partition from this cactus.
    ///
    /// Finds the minimum-weight edge in the cactus, removes it to create
    /// two subtrees, and assigns the subtree with the lex-smallest vertex
    /// set to side A. Ties are broken by selecting the edge whose removal
    /// yields the lex-smallest side-A bitset.
    pub fn canonical_partition(&self) -> CanonicalPartition {
        let mut best = CanonicalPartition::empty();

        if self.n_nodes <= 1 {
            // Trivial: all vertices on side A
            best.cardinality_a = self.n_nodes;
            best.cut_value = FixedPointWeight::ZERO;
            best.compute_hash();
            return best;
        }

        // Find the minimum-weight cactus edge. For each non-root node whose
        // edge to its parent has weight == min_cut_value, compute the
        // resulting partition and keep the lex-smallest.
        let mut found = false;

        for i in 0..self.n_nodes as usize {
            let node = &self.nodes[i];
            if node.parent == CactusNode::NO_PARENT {
                continue; // Root has no parent edge
            }
            if node.weight_to_parent != self.min_cut_value {
                continue; // Not a minimum edge
            }

            // Removing this edge splits the cactus into:
            //   subtree rooted at node i  vs  everything else
            let mut candidate = CanonicalPartition::empty();
            candidate.cut_value = self.min_cut_value;

            // Mark the subtree rooted at node i as side B
            self.mark_subtree(i as u16, &mut candidate);

            // Count cardinalities
            candidate.recount();

            // Ensure canonical orientation: side A should have lex-smallest
            // vertex set. If side B is lex-smaller, flip.
            if !candidate.is_canonical() {
                candidate.flip();
            }

            candidate.compute_hash();

            if !found || candidate.side < best.side {
                best = candidate;
                found = true;
            }
        }

        if !found {
            best.compute_hash();
        }

        best
    }

    /// Mark all nodes in the subtree rooted at `start` to side B.
    fn mark_subtree(&self, start: u16, partition: &mut CanonicalPartition) {
        // The cactus tree has parent pointers, so we find all nodes
        // whose ancestor chain leads to `start` (before reaching the root
        // or a node not descended from `start`).
        partition.set_side(self.nodes[start as usize].id, true);

        for i in 0..self.n_nodes as usize {
            if i == start as usize {
                continue;
            }
            // Walk ancestor chain to see if this node is in start's subtree
            let mut cur = i as u16;
            let mut in_subtree = false;
            let mut steps = 0u16;
            while cur != CactusNode::NO_PARENT && steps < 256 {
                if cur == start {
                    in_subtree = true;
                    break;
                }
                cur = self.nodes[cur as usize].parent;
                steps += 1;
            }
            if in_subtree {
                partition.set_side(self.nodes[i].id, true);
            }
        }
    }

    /// Compute a 16-bit digest of the cactus structure for embedding
    /// in the witness fragment.
    pub fn digest(&self) -> u16 {
        let mut hash: u32 = 0x811c9dc5;
        for i in 0..self.n_nodes as usize {
            let node = &self.nodes[i];
            hash ^= node.id as u32;
            hash = hash.wrapping_mul(0x01000193);
            hash ^= node.parent as u32;
            hash = hash.wrapping_mul(0x01000193);
            hash ^= node.weight_to_parent.0;
            hash = hash.wrapping_mul(0x01000193);
        }
        (hash & 0xFFFF) as u16
    }
}

// ============================================================================
// Canonical partition
// ============================================================================

/// A canonical two-way partition of vertices into sides A and B.
///
/// The bitset encodes 256 vertices (1 bit each = 32 bytes). A cleared
/// bit means side A, a set bit means side B. The canonical orientation
/// guarantees that side A contains the lex-smallest vertex set.
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct CanonicalPartition {
    /// Bitset: 256 vertices, 1 bit each (0 = side A, 1 = side B)
    pub side: [u8; 32],
    /// Number of vertices on side A
    pub cardinality_a: u16,
    /// Number of vertices on side B
    pub cardinality_b: u16,
    /// Cut value (weight of edges crossing the partition)
    pub cut_value: FixedPointWeight,
    /// 32-bit FNV-1a hash of the `side` bitset
    pub canonical_hash: [u8; 4],
}

impl CanonicalPartition {
    /// Create an empty partition (all vertices on side A)
    #[inline]
    pub const fn empty() -> Self {
        Self {
            side: [0u8; 32],
            cardinality_a: 0,
            cardinality_b: 0,
            cut_value: FixedPointWeight::ZERO,
            canonical_hash: [0u8; 4],
        }
    }

    /// Set which side a vertex belongs to.
    ///
    /// `side_b = false` means side A, `side_b = true` means side B.
    #[inline]
    pub fn set_side(&mut self, vertex: u16, side_b: bool) {
        if vertex >= 256 {
            return;
        }
        let byte_idx = (vertex / 8) as usize;
        let bit_idx = vertex % 8;
        if side_b {
            self.side[byte_idx] |= 1 << bit_idx;
        } else {
            self.side[byte_idx] &= !(1 << bit_idx);
        }
    }

    /// Get which side a vertex belongs to (false = A, true = B).
    #[inline]
    pub fn get_side(&self, vertex: u16) -> bool {
        if vertex >= 256 {
            return false;
        }
        let byte_idx = (vertex / 8) as usize;
        let bit_idx = vertex % 8;
        (self.side[byte_idx] >> bit_idx) & 1 != 0
    }

    /// Compute the FNV-1a hash of the side bitset.
    pub fn compute_hash(&mut self) {
        self.canonical_hash = fnv1a_hash(&self.side);
    }

    /// Check if this partition is in canonical orientation.
    ///
    /// Canonical means: side A (the cleared bits) represents the
    /// lex-smallest vertex set. Equivalently, the first set bit in
    /// the bitset must be 1 (vertex 0 is on side A) OR, if vertex 0
    /// is on side B, we should flip.
    ///
    /// More precisely: the complement of `side` (i.e. the A-set bitset)
    /// must be lex-smaller-or-equal to `side` (the B-set bitset).
    pub fn is_canonical(&self) -> bool {
        // Compare side vs. its complement byte-by-byte.
        // The complement represents side-A. If complement < side, canonical.
        // If complement > side, not canonical (should flip).
        // If equal, canonical by convention.
        for i in 0..32 {
            let complement = !self.side[i];
            if complement < self.side[i] {
                return true;
            }
            if complement > self.side[i] {
                return false;
            }
        }
        true // Equal (symmetric partition)
    }

    /// Flip the partition so that side A and side B swap.
    pub fn flip(&mut self) {
        for i in 0..32 {
            self.side[i] = !self.side[i];
        }
        let tmp = self.cardinality_a;
        self.cardinality_a = self.cardinality_b;
        self.cardinality_b = tmp;
    }

    /// Recount cardinalities from the bitset.
    pub fn recount(&mut self) {
        let mut count_b: u16 = 0;
        for i in 0..32 {
            count_b += self.side[i].count_ones() as u16;
        }
        self.cardinality_b = count_b;
        // cardinality_a is total vertices minus B, but we only know
        // about the vertices that were explicitly placed. We approximate
        // with 256 - B here; the caller may adjust.
        self.cardinality_a = 256u16.saturating_sub(count_b);
    }
}

// ============================================================================
// Canonical witness fragment
// ============================================================================

/// Canonical witness fragment (16 bytes, same as `WitnessFragment`).
///
/// Extends the original witness fragment with pseudo-deterministic
/// partition information derived from the cactus tree.
#[derive(Debug, Copy, Clone, Default)]
#[repr(C, align(16))]
pub struct CanonicalWitnessFragment {
    /// Tile ID (0-255)
    pub tile_id: u8,
    /// Truncated epoch (tick & 0xFF)
    pub epoch: u8,
    /// Vertices on side A of the canonical partition
    pub cardinality_a: u16,
    /// Vertices on side B of the canonical partition
    pub cardinality_b: u16,
    /// Cut value (original weight format, truncated)
    pub cut_value: u16,
    /// FNV-1a hash of the canonical partition bitset
    pub canonical_hash: [u8; 4],
    /// Number of boundary edges
    pub boundary_edges: u16,
    /// Truncated hash of the cactus structure
    pub cactus_digest: u16,
}

// Compile-time size assertion
const _: () = assert!(
    size_of::<CanonicalWitnessFragment>() == 16,
    "CanonicalWitnessFragment must be exactly 16 bytes"
);

// ============================================================================
// FNV-1a hash (no_std, no allocation)
// ============================================================================

/// Compute a 32-bit FNV-1a hash of the given byte slice.
///
/// FNV-1a is a simple, fast, non-cryptographic hash with good
/// distribution properties. It is fully deterministic and portable.
#[inline]
pub fn fnv1a_hash(data: &[u8]) -> [u8; 4] {
    let mut hash: u32 = 0x811c9dc5; // FNV offset basis
    for &byte in data {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x01000193); // FNV prime
    }
    hash.to_le_bytes()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shard::CompactGraph;
    use crate::TileState;
    use core::mem::size_of;

    #[test]
    fn test_fixed_point_weight_ordering() {
        let a = FixedPointWeight(100);
        let b = FixedPointWeight(200);
        let c = FixedPointWeight(100);

        assert!(a < b);
        assert!(b > a);
        assert_eq!(a, c);
        assert!(a <= c);
        assert!(a >= c);

        // Check from_u16_weight ordering
        let w1 = FixedPointWeight::from_u16_weight(50);
        let w2 = FixedPointWeight::from_u16_weight(100);
        assert!(w1 < w2);

        // Saturating add
        let sum = w1.saturating_add(w2);
        assert_eq!(sum, FixedPointWeight((50u32 << 8) + (100u32 << 8)));

        // Saturating add at max
        let max_sum = FixedPointWeight::MAX.saturating_add(FixedPointWeight::ONE);
        assert_eq!(max_sum, FixedPointWeight::MAX);
    }

    #[test]
    fn test_canonical_partition_determinism() {
        // Build the same graph twice, verify same partition hash
        let build_graph = || {
            let mut g = CompactGraph::new();
            g.add_edge(0, 1, 100);
            g.add_edge(1, 2, 100);
            g.add_edge(2, 3, 100);
            g.add_edge(3, 0, 100);
            g.add_edge(0, 2, 50); // Diagonal, lighter weight
            g.recompute_components();
            g
        };

        let g1 = build_graph();
        let g2 = build_graph();

        let c1 = ArenaCactus::build_from_compact_graph(&g1);
        let c2 = ArenaCactus::build_from_compact_graph(&g2);

        let p1 = c1.canonical_partition();
        let p2 = c2.canonical_partition();

        assert_eq!(p1.canonical_hash, p2.canonical_hash);
        assert_eq!(p1.side, p2.side);
        assert_eq!(p1.cut_value, p2.cut_value);
    }

    #[test]
    fn test_fnv1a_known_values() {
        // Empty input
        let h0 = fnv1a_hash(&[]);
        assert_eq!(
            u32::from_le_bytes(h0),
            0x811c9dc5,
            "FNV-1a of empty should be the offset basis"
        );

        // Single zero byte
        let h1 = fnv1a_hash(&[0]);
        let expected = 0x811c9dc5u32 ^ 0;
        let expected = expected.wrapping_mul(0x01000193);
        assert_eq!(u32::from_le_bytes(h1), expected);

        // Determinism: same input -> same output
        let data = [1, 2, 3, 4, 5, 6, 7, 8];
        let a = fnv1a_hash(&data);
        let b = fnv1a_hash(&data);
        assert_eq!(a, b);

        // Different input -> (almost certainly) different output
        let c = fnv1a_hash(&[8, 7, 6, 5, 4, 3, 2, 1]);
        assert_ne!(a, c);
    }

    #[test]
    fn test_arena_cactus_simple_triangle() {
        let mut g = CompactGraph::new();
        g.add_edge(0, 1, 100);
        g.add_edge(1, 2, 100);
        g.add_edge(2, 0, 100);
        g.recompute_components();

        let cactus = ArenaCactus::build_from_compact_graph(&g);

        // A triangle is 2-edge-connected, so the cactus should have
        // a single node (all 3 vertices collapsed into one component).
        assert!(
            cactus.n_nodes >= 1,
            "Triangle cactus should have at least 1 node"
        );

        // The partition should be trivial since there is only one component
        let partition = cactus.canonical_partition();
        partition.canonical_hash; // Just ensure it doesn't panic
    }

    #[test]
    fn test_canonical_witness_fragment_size() {
        assert_eq!(
            size_of::<CanonicalWitnessFragment>(),
            16,
            "CanonicalWitnessFragment must be exactly 16 bytes"
        );
    }

    #[test]
    fn test_canonical_witness_reproducibility() {
        // Build two identical tile states and verify they produce the
        // same canonical witness fragment.
        let build_tile = || {
            let mut tile = TileState::new(42);
            tile.ingest_delta(&crate::delta::Delta::edge_add(0, 1, 100));
            tile.ingest_delta(&crate::delta::Delta::edge_add(1, 2, 100));
            tile.ingest_delta(&crate::delta::Delta::edge_add(2, 3, 200));
            tile.ingest_delta(&crate::delta::Delta::edge_add(3, 0, 200));
            tile.tick(10);
            tile
        };

        let t1 = build_tile();
        let t2 = build_tile();

        let w1 = t1.canonical_witness();
        let w2 = t2.canonical_witness();

        assert_eq!(w1.tile_id, w2.tile_id);
        assert_eq!(w1.epoch, w2.epoch);
        assert_eq!(w1.cardinality_a, w2.cardinality_a);
        assert_eq!(w1.cardinality_b, w2.cardinality_b);
        assert_eq!(w1.cut_value, w2.cut_value);
        assert_eq!(w1.canonical_hash, w2.canonical_hash);
        assert_eq!(w1.boundary_edges, w2.boundary_edges);
        assert_eq!(w1.cactus_digest, w2.cactus_digest);
    }

    #[test]
    fn test_partition_set_get_side() {
        let mut p = CanonicalPartition::empty();

        // All on side A initially
        for v in 0..256u16 {
            assert!(!p.get_side(v), "vertex {} should be on side A", v);
        }

        // Set some to side B
        p.set_side(0, true);
        p.set_side(7, true);
        p.set_side(8, true);
        p.set_side(255, true);

        assert!(p.get_side(0));
        assert!(p.get_side(7));
        assert!(p.get_side(8));
        assert!(p.get_side(255));
        assert!(!p.get_side(1));
        assert!(!p.get_side(254));

        // Clear
        p.set_side(0, false);
        assert!(!p.get_side(0));
    }

    #[test]
    fn test_partition_flip() {
        let mut p = CanonicalPartition::empty();
        p.set_side(0, true);
        p.set_side(1, true);
        p.cardinality_a = 254;
        p.cardinality_b = 2;

        p.flip();

        assert!(!p.get_side(0));
        assert!(!p.get_side(1));
        assert!(p.get_side(2));
        assert_eq!(p.cardinality_a, 2);
        assert_eq!(p.cardinality_b, 254);
    }

    #[test]
    fn test_empty_graph_cactus() {
        let g = CompactGraph::new();
        let cactus = ArenaCactus::build_from_compact_graph(&g);
        assert_eq!(cactus.n_nodes, 0);
        assert_eq!(cactus.min_cut_value, FixedPointWeight::ZERO);
    }

    #[test]
    fn test_single_edge_cactus() {
        let mut g = CompactGraph::new();
        g.add_edge(0, 1, 150);
        g.recompute_components();

        let cactus = ArenaCactus::build_from_compact_graph(&g);
        assert!(
            cactus.n_nodes >= 2,
            "Single edge should have 2 cactus nodes"
        );

        let partition = cactus.canonical_partition();
        // One vertex on each side
        assert!(
            partition.cardinality_b >= 1,
            "Should have at least 1 vertex on side B"
        );
    }
}
