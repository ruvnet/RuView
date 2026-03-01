//! Pseudo-deterministic canonical minimum cut via cactus representation.
//!
//! Provides reproducible, auditable min-cut results where the same graph
//! always produces the same canonical cut, regardless of construction order.
//!
//! # Overview
//!
//! A *canonical* min-cut is a uniquely selected minimum cut chosen by a
//! deterministic tie-breaking rule. The cactus graph encodes all minimum cuts
//! of a weighted graph in a compact tree-of-cycles structure. By rooting
//! the cactus at the vertex containing the lexicographically smallest
//! original vertex and selecting the leftmost branch, we obtain a
//! cut that is invariant under any permutation of input order.
//!
//! # Example
//!
//! ```rust,ignore
//! use ruvector_mincut::canonical::{CanonicalMinCutImpl, CanonicalMinCut};
//! use ruvector_mincut::{MinCutBuilder, DynamicMinCut};
//!
//! let mc = MinCutBuilder::new()
//!     .exact()
//!     .with_edges(vec![(1, 2, 1.0), (2, 3, 1.0), (3, 1, 1.0)])
//!     .build()
//!     .unwrap();
//!
//! let canonical = CanonicalMinCutImpl::from_dynamic(mc);
//! let result = canonical.canonical_cut();
//! println!("Canonical cut value: {}", result.value);
//! ```

#[cfg(test)]
mod tests;

use crate::algorithm::{self, MinCutConfig};
use crate::graph::{DynamicGraph, VertexId, Weight};

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// FixedWeight -- deterministic 32.32 fixed-point weight
// ---------------------------------------------------------------------------

/// Deterministic fixed-point weight for reproducible comparison.
///
/// Uses a 32.32 format where the upper 32 bits represent the integer part
/// and the lower 32 bits represent the fractional part. This avoids
/// floating-point non-determinism across platforms.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct FixedWeight(u64);

impl FixedWeight {
    /// Number of fractional bits in the 32.32 format.
    const FRAC_BITS: u32 = 32;

    /// Convert from `f64` to `FixedWeight`.
    ///
    /// Clamps negative values to zero.
    #[must_use]
    pub fn from_f64(val: f64) -> Self {
        let clamped = if val < 0.0 { 0.0 } else { val };
        let scaled = clamped * (1u64 << Self::FRAC_BITS) as f64;
        Self(scaled as u64)
    }

    /// Convert back to `f64`.
    #[must_use]
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / (1u64 << Self::FRAC_BITS) as f64
    }

    /// Saturating add.
    #[must_use]
    pub fn add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    /// Saturating subtract.
    #[must_use]
    pub fn sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    /// Zero weight.
    #[must_use]
    pub fn zero() -> Self {
        Self(0)
    }

    /// Raw inner value.
    #[must_use]
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for FixedWeight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.6}", self.to_f64())
    }
}

// ---------------------------------------------------------------------------
// Cactus graph types
// ---------------------------------------------------------------------------

/// A vertex in the cactus graph.
///
/// Each cactus vertex represents a subset of original graph vertices that
/// were contracted together during Gomory-Hu / cactus construction.
#[derive(Debug, Clone)]
pub struct CactusVertex {
    /// Identifier of this cactus vertex.
    pub id: u16,
    /// Original graph vertices that map to this cactus vertex.
    pub original_vertices: Vec<usize>,
    /// Parent in the rooted cactus (None for root).
    pub parent: Option<u16>,
}

/// An edge in the cactus graph.
#[derive(Debug, Clone)]
pub struct CactusEdge {
    /// Source cactus vertex.
    pub source: u16,
    /// Target cactus vertex.
    pub target: u16,
    /// Weight in deterministic fixed-point format.
    pub weight: FixedWeight,
    /// Whether this edge lies on a cycle in the cactus.
    pub is_cycle_edge: bool,
}

/// A cycle in the cactus graph.
///
/// In a cactus, every edge belongs to at most one simple cycle.
#[derive(Debug, Clone)]
pub struct CactusCycle {
    /// Vertices forming this cycle (in order).
    pub vertices: Vec<u16>,
    /// Indices into `CactusGraph::edges` for the edges of this cycle.
    pub edges: Vec<usize>,
}

/// Compact cactus representation encoding all minimum cuts.
///
/// The cactus graph has the property that every minimum (s,t)-cut in the
/// original graph corresponds to removing a single edge or splitting a
/// cycle in the cactus.
#[derive(Debug, Clone)]
pub struct CactusGraph {
    /// Cactus vertices.
    pub vertices: Vec<CactusVertex>,
    /// Cactus edges.
    pub edges: Vec<CactusEdge>,
    /// Cycles in the cactus.
    pub cycles: Vec<CactusCycle>,
    /// Map from original vertex id to cactus vertex id.
    pub vertex_map: HashMap<usize, u16>,
    /// Root of the rooted cactus.
    pub root: u16,
    /// Number of cactus vertices.
    pub n_vertices: u16,
    /// Number of cactus edges.
    pub n_edges: u16,
}

impl CactusGraph {
    /// Build a cactus representation from a `DynamicGraph`.
    ///
    /// Uses a simplified Stoer-Wagner-like approach to identify minimum
    /// cuts and then builds the cactus structure from them.
    pub fn build_from_graph(graph: &DynamicGraph) -> Self {
        let vertices_ids = graph.vertices();
        let edges_list = graph.edges();

        // Handle trivial cases
        if vertices_ids.is_empty() {
            return Self::empty();
        }

        if vertices_ids.len() == 1 {
            return Self::singleton(vertices_ids[0] as usize);
        }

        // Build adjacency for Stoer-Wagner
        let mut adj: HashMap<usize, HashMap<usize, f64>> = HashMap::new();
        for &v in &vertices_ids {
            adj.entry(v as usize).or_default();
        }
        for e in &edges_list {
            *adj.entry(e.source as usize)
                .or_default()
                .entry(e.target as usize)
                .or_insert(0.0) += e.weight;
            *adj.entry(e.target as usize)
                .or_default()
                .entry(e.source as usize)
                .or_insert(0.0) += e.weight;
        }

        // Run Stoer-Wagner to find global min-cut value and all min-cut
        // partitions (simplified: we find the min-cut value and one
        // partition, then enumerate by vertex removal).
        let (min_cut_value, min_cut_partitions) = Self::stoer_wagner_all_cuts(&adj);

        // Build cactus from discovered min-cuts
        Self::build_cactus_from_cuts(&vertices_ids, &adj, min_cut_value, &min_cut_partitions)
    }

    /// Root the cactus at the vertex containing the lexicographically
    /// smallest original vertex.
    pub fn root_at_lex_smallest(&mut self) {
        if self.vertices.is_empty() {
            return;
        }

        // Find cactus vertex with the smallest original vertex
        let mut best_cactus_id = self.vertices[0].id;
        let mut best_orig = usize::MAX;
        for cv in &self.vertices {
            for &orig in &cv.original_vertices {
                if orig < best_orig {
                    best_orig = orig;
                    best_cactus_id = cv.id;
                }
            }
        }

        if best_cactus_id == self.root {
            return; // Already rooted correctly
        }

        self.root = best_cactus_id;

        // Rebuild parent pointers via BFS from new root
        let adj = self.adjacency_list();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(self.root);
        visited.insert(self.root);

        // Clear all parent pointers
        for cv in &mut self.vertices {
            cv.parent = None;
        }

        while let Some(u) = queue.pop_front() {
            if let Some(neighbors) = adj.get(&u) {
                for &v in neighbors {
                    if visited.insert(v) {
                        if let Some(cv) = self.vertices.iter_mut().find(|c| c.id == v) {
                            cv.parent = Some(u);
                        }
                        queue.push_back(v);
                    }
                }
            }
        }
    }

    /// Extract the canonical minimum cut.
    ///
    /// The canonical cut is obtained by choosing the lexicographically
    /// smallest partition among all minimum cuts.
    pub fn canonical_cut(&self) -> CanonicalCutResult {
        let all_cuts = self.enumerate_min_cuts();

        if all_cuts.is_empty() {
            // No cuts found -- graph has 0 or 1 vertex
            return CanonicalCutResult {
                value: f64::INFINITY,
                partition: (Vec::new(), Vec::new()),
                cut_edges: Vec::new(),
                canonical_key: [0u8; 32],
            };
        }

        // Select lexicographically smallest partition
        // First normalize: smaller side first, sorted within each side
        let mut best: Option<(Vec<usize>, Vec<usize>)> = None;

        for (mut s, mut t) in all_cuts {
            s.sort_unstable();
            t.sort_unstable();

            // Ensure smaller side is first; break ties lexicographically
            if s.len() > t.len() || (s.len() == t.len() && s > t) {
                std::mem::swap(&mut s, &mut t);
            }

            if let Some((ref bs, _)) = best {
                if s < *bs {
                    best = Some((s, t));
                }
            } else {
                best = Some((s, t));
            }
        }

        let (part_s, part_t) = best.unwrap();

        // Compute cut value from the partition
        let cut_value = self.compute_cut_value_from_partition(&part_s);

        // Compute cut edges
        let cut_edges = self.compute_cut_edges(&part_s);

        // Compute canonical key
        let canonical_key = Self::compute_canonical_key(&part_s);

        CanonicalCutResult {
            value: cut_value,
            partition: (part_s, part_t),
            cut_edges,
            canonical_key,
        }
    }

    /// Enumerate all minimum cut partitions from the cactus structure.
    ///
    /// Each tree edge and each cycle split yields a distinct minimum cut.
    pub fn enumerate_min_cuts(&self) -> Vec<(Vec<usize>, Vec<usize>)> {
        let mut result = Vec::new();

        if self.vertices.is_empty() {
            return result;
        }

        let adj = self.adjacency_list();

        // For each non-cycle edge: removing it splits the cactus into
        // two connected subtrees.
        for (idx, edge) in self.edges.iter().enumerate() {
            if edge.is_cycle_edge {
                continue;
            }
            let (side_a, side_b) = self.split_at_edge(edge.source, edge.target, &adj);
            let orig_a = self.collect_original_vertices(&side_a);
            let orig_b = self.collect_original_vertices(&side_b);
            if !orig_a.is_empty() && !orig_b.is_empty() {
                result.push((orig_a, orig_b));
            }
        }

        // For each cycle: removing any single edge of the cycle yields a
        // tree edge, giving a min-cut.
        for cycle in &self.cycles {
            for &edge_idx in &cycle.edges {
                if edge_idx >= self.edges.len() {
                    continue;
                }
                let e = &self.edges[edge_idx];
                let (side_a, side_b) = self.split_at_edge(e.source, e.target, &adj);
                let orig_a = self.collect_original_vertices(&side_a);
                let orig_b = self.collect_original_vertices(&side_b);
                if !orig_a.is_empty() && !orig_b.is_empty() {
                    result.push((orig_a, orig_b));
                }
            }
        }

        // If no edges at all, produce a trivial partition
        if result.is_empty() && self.vertices.len() >= 2 {
            let all_orig: Vec<usize> = self
                .vertices
                .iter()
                .flat_map(|v| v.original_vertices.iter().copied())
                .collect();
            if all_orig.len() >= 2 {
                result.push((vec![all_orig[0]], all_orig[1..].to_vec()));
            }
        }

        result
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            edges: Vec::new(),
            cycles: Vec::new(),
            vertex_map: HashMap::new(),
            root: 0,
            n_vertices: 0,
            n_edges: 0,
        }
    }

    fn singleton(v: usize) -> Self {
        let cv = CactusVertex {
            id: 0,
            original_vertices: vec![v],
            parent: None,
        };
        let mut vertex_map = HashMap::new();
        vertex_map.insert(v, 0);
        Self {
            vertices: vec![cv],
            edges: Vec::new(),
            cycles: Vec::new(),
            vertex_map,
            root: 0,
            n_vertices: 1,
            n_edges: 0,
        }
    }

    /// Stoer-Wagner algorithm that returns global min-cut value and all
    /// minimum-phase cuts whose value equals the global minimum.
    ///
    /// Tight dense implementation using flat arrays with no HashMap overhead.
    /// For n <= 256 vertices the dense approach is fastest due to cache locality.
    fn stoer_wagner_all_cuts(
        adj: &HashMap<usize, HashMap<usize, f64>>,
    ) -> (f64, Vec<(Vec<usize>, Vec<usize>)>) {
        let n = adj.len();
        if n <= 1 {
            return (f64::INFINITY, Vec::new());
        }

        // Build compact index mapping using Vec instead of HashMap
        let node_ids: Vec<usize> = {
            let mut v: Vec<usize> = adj.keys().copied().collect();
            v.sort_unstable();
            v
        };

        let max_id = *node_ids.last().unwrap();
        let mut id_to_idx = vec![usize::MAX; max_id + 1];
        for (i, &nid) in node_ids.iter().enumerate() {
            id_to_idx[nid] = i;
        }

        // Flat weight matrix (dense, row-major, contiguous allocation)
        let mut w: Vec<f64> = vec![0.0; n * n];
        for (&u, nbrs) in adj {
            let ui = id_to_idx[u];
            let row = ui * n;
            for (&v, &wt) in nbrs {
                let vi = id_to_idx[v];
                w[row + vi] = wt;
            }
        }

        // Track which original vertices are merged into each super-node
        let mut merged: Vec<Vec<usize>> = node_ids.iter().map(|&v| vec![v]).collect();
        // Compact active-list (only iterate active nodes)
        let mut active_list: Vec<usize> = (0..n).collect();
        let mut active_pos: Vec<usize> = (0..n).collect();
        let mut n_active = n;

        let mut global_min = f64::INFINITY;
        let mut best_partitions: Vec<(Vec<usize>, Vec<usize>)> = Vec::new();

        // Reusable per-phase buffers
        let mut key: Vec<f64> = vec![0.0; n];
        let mut in_a: Vec<bool> = vec![false; n];

        for _phase in 0..(n - 1) {
            if n_active <= 1 {
                break;
            }

            // Reset per-phase state using active_list (touching only n_active nodes)
            for k in 0..n_active {
                let j = active_list[k];
                in_a[j] = false;
                key[j] = 0.0;
            }

            // Start with first active node
            let first = active_list[0];
            in_a[first] = true;
            // Initialize keys from first's row
            let first_row = first * n;
            for k in 0..n_active {
                let j = active_list[k];
                key[j] = w[first_row + j];
            }

            let mut prev = first;
            let mut last = first;

            for _step in 1..n_active {
                // Find max key among active nodes not in A
                let mut best = usize::MAX;
                let mut best_key = -1.0f64;
                for k in 0..n_active {
                    let j = active_list[k];
                    if !in_a[j] && key[j] > best_key {
                        best_key = key[j];
                        best = j;
                    }
                }

                if best == usize::MAX {
                    break;
                }

                in_a[best] = true;
                prev = last;
                last = best;

                // Update keys from best's row (only active nodes not in A)
                let best_row = best * n;
                for k in 0..n_active {
                    let j = active_list[k];
                    if !in_a[j] {
                        key[j] += w[best_row + j];
                    }
                }
            }

            // Cut-of-the-phase: key[last]
            let cut_value = key[last];

            if cut_value < global_min - 1e-12 {
                global_min = cut_value;
                best_partitions.clear();
                let part_s: Vec<usize> = merged[last].clone();
                let part_t: Vec<usize> = (0..n_active)
                    .map(|k| active_list[k])
                    .filter(|&i| i != last)
                    .flat_map(|i| merged[i].iter().copied())
                    .collect();
                best_partitions.push((part_s, part_t));
            } else if (cut_value - global_min).abs() < 1e-12 {
                let part_s: Vec<usize> = merged[last].clone();
                let part_t: Vec<usize> = (0..n_active)
                    .map(|k| active_list[k])
                    .filter(|&i| i != last)
                    .flat_map(|i| merged[i].iter().copied())
                    .collect();
                best_partitions.push((part_s, part_t));
            }

            // Merge last into prev: move last's merged list to prev
            let last_merged = std::mem::take(&mut merged[last]);
            merged[prev].extend(last_merged);

            // Update weight matrix: merge last's row/col into prev's
            let prev_row = prev * n;
            let last_row = last * n;
            for k in 0..n_active {
                let j = active_list[k];
                if j != last {
                    w[prev_row + j] += w[last_row + j];
                    w[j * n + prev] += w[j * n + last];
                }
            }

            // Remove last from active_list using swap-remove (O(1))
            let pos = active_pos[last];
            n_active -= 1;
            if pos < n_active {
                let swapped = active_list[n_active];
                active_list[pos] = swapped;
                active_pos[swapped] = pos;
            }
            active_list.truncate(n_active);
        }

        (global_min, best_partitions)
    }

    /// Build cactus from discovered min-cut partitions.
    fn build_cactus_from_cuts(
        vertices_ids: &[VertexId],
        adj: &HashMap<usize, HashMap<usize, f64>>,
        min_cut_value: f64,
        partitions: &[(Vec<usize>, Vec<usize>)],
    ) -> Self {
        if partitions.is_empty() {
            // No min-cuts => all vertices in one cactus node
            let all: Vec<usize> = vertices_ids.iter().map(|&v| v as usize).collect();
            let cv = CactusVertex {
                id: 0,
                original_vertices: all.clone(),
                parent: None,
            };
            let mut vertex_map = HashMap::new();
            for &v in &all {
                vertex_map.insert(v, 0);
            }
            return Self {
                vertices: vec![cv],
                edges: Vec::new(),
                cycles: Vec::new(),
                vertex_map,
                root: 0,
                n_vertices: 1,
                n_edges: 0,
            };
        }

        // Group original vertices into equivalence classes based on
        // which side of each cut they fall on. Vertices that are always
        // on the same side across all min-cuts belong to the same cactus node.
        let all_verts: BTreeSet<usize> = vertices_ids.iter().map(|&v| v as usize).collect();

        // Pre-compute HashSets for each partition's side_a for O(1) lookups
        let partition_sets: Vec<HashSet<usize>> = partitions
            .iter()
            .map(|(side_a, _)| side_a.iter().copied().collect())
            .collect();

        // Assign a signature to each vertex: for each partition, is the
        // vertex in side A (true) or side B (false)?
        let mut signatures: HashMap<usize, Vec<bool>> = HashMap::new();
        for &v in &all_verts {
            let mut sig = Vec::with_capacity(partitions.len());
            for set in &partition_sets {
                sig.push(set.contains(&v));
            }
            signatures.insert(v, sig);
        }

        // Group by signature
        let mut groups: HashMap<Vec<bool>, Vec<usize>> = HashMap::new();
        for (v, sig) in &signatures {
            groups.entry(sig.clone()).or_default().push(*v);
        }

        // Sort vertices within each group for determinism
        for g in groups.values_mut() {
            g.sort_unstable();
        }

        // Assign cactus vertex IDs
        let mut cactus_vertices: Vec<CactusVertex> = Vec::new();
        let mut vertex_map: HashMap<usize, u16> = HashMap::new();
        let mut sorted_groups: Vec<Vec<usize>> = groups.values().cloned().collect();
        sorted_groups.sort_by(|a, b| a.first().cmp(&b.first()));

        for (i, group) in sorted_groups.iter().enumerate() {
            let cid = i as u16;
            cactus_vertices.push(CactusVertex {
                id: cid,
                original_vertices: group.clone(),
                parent: None,
            });
            for &v in group {
                vertex_map.insert(v, cid);
            }
        }

        let n_cactus = cactus_vertices.len() as u16;

        // Build cactus edges: two cactus vertices are connected if there
        // exists a min-cut that separates them and they are "adjacent"
        // in the cut structure.
        let mut cactus_edges: Vec<CactusEdge> = Vec::new();
        let mut edge_set: HashSet<(u16, u16)> = HashSet::new();

        // Compute edge weight between cactus vertex groups by summing
        // original edge weights crossing them.
        for i in 0..cactus_vertices.len() {
            for j in (i + 1)..cactus_vertices.len() {
                let ci = cactus_vertices[i].id;
                let cj = cactus_vertices[j].id;

                // Check if there's a min-cut separating these groups
                let mut separates = false;
                for set in &partition_sets {
                    let i_in_a = set.contains(&cactus_vertices[i].original_vertices[0]);
                    let j_in_a = set.contains(&cactus_vertices[j].original_vertices[0]);
                    if i_in_a != j_in_a {
                        separates = true;
                        break;
                    }
                }

                if !separates {
                    continue;
                }

                // Compute crossing weight
                let mut crossing = 0.0f64;
                for &u in &cactus_vertices[i].original_vertices {
                    if let Some(nbrs) = adj.get(&u) {
                        for &v in &cactus_vertices[j].original_vertices {
                            if let Some(&w) = nbrs.get(&v) {
                                crossing += w;
                            }
                        }
                    }
                }

                if crossing > 0.0 {
                    let key = if ci < cj { (ci, cj) } else { (cj, ci) };
                    if edge_set.insert(key) {
                        cactus_edges.push(CactusEdge {
                            source: ci,
                            target: cj,
                            weight: FixedWeight::from_f64(crossing),
                            is_cycle_edge: false,
                        });
                    }
                }
            }
        }

        let n_edges = cactus_edges.len() as u16;

        // Detect cycles in the cactus (simple cycle detection via DFS)
        let cycles = Self::detect_cycles(&cactus_vertices, &mut cactus_edges);

        // Root at lex-smallest
        let mut cactus = Self {
            vertices: cactus_vertices,
            edges: cactus_edges,
            cycles,
            vertex_map,
            root: 0,
            n_vertices: n_cactus,
            n_edges,
        };
        cactus.root_at_lex_smallest();
        cactus
    }

    /// Simple cycle detection in the cactus graph.
    fn detect_cycles(vertices: &[CactusVertex], edges: &mut [CactusEdge]) -> Vec<CactusCycle> {
        if vertices.is_empty() || edges.is_empty() {
            return Vec::new();
        }

        let mut adj: HashMap<u16, Vec<(u16, usize)>> = HashMap::new();
        for (idx, e) in edges.iter().enumerate() {
            adj.entry(e.source).or_default().push((e.target, idx));
            adj.entry(e.target).or_default().push((e.source, idx));
        }

        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut parent: HashMap<u16, u16> = HashMap::new();
        let mut parent_edge: HashMap<u16, usize> = HashMap::new();
        let mut stack: Vec<(u16, Option<u16>)> = Vec::new();

        // Start DFS from vertex 0
        if let Some(start) = vertices.first() {
            stack.push((start.id, None));
        }

        while let Some((u, from)) = stack.pop() {
            if visited.contains(&u) {
                continue;
            }
            visited.insert(u);
            if let Some(p) = from {
                parent.insert(u, p);
            }

            if let Some(neighbors) = adj.get(&u) {
                for &(v, edge_idx) in neighbors {
                    if !visited.contains(&v) {
                        parent_edge.insert(v, edge_idx);
                        stack.push((v, Some(u)));
                    } else if from != Some(v) {
                        // Back edge: found a cycle
                        let mut cycle_verts = vec![u, v];
                        let mut cycle_edges = vec![edge_idx];

                        // Trace back from u to v via parent pointers
                        let mut cur = u;
                        while cur != v {
                            if let Some(&p) = parent.get(&cur) {
                                if let Some(&pe) = parent_edge.get(&cur) {
                                    cycle_edges.push(pe);
                                }
                                if p != v {
                                    cycle_verts.push(p);
                                }
                                cur = p;
                            } else {
                                break;
                            }
                        }

                        // Mark edges as cycle edges
                        for &ei in &cycle_edges {
                            if ei < edges.len() {
                                edges[ei].is_cycle_edge = true;
                            }
                        }

                        cycles.push(CactusCycle {
                            vertices: cycle_verts,
                            edges: cycle_edges,
                        });
                    }
                }
            }
        }

        cycles
    }

    /// Build adjacency list for the cactus.
    fn adjacency_list(&self) -> HashMap<u16, Vec<u16>> {
        let mut adj: HashMap<u16, Vec<u16>> = HashMap::new();
        for cv in &self.vertices {
            adj.entry(cv.id).or_default();
        }
        for e in &self.edges {
            adj.entry(e.source).or_default().push(e.target);
            adj.entry(e.target).or_default().push(e.source);
        }
        adj
    }

    /// Split cactus into two components by removing edge (u, v).
    fn split_at_edge(
        &self,
        u: u16,
        v: u16,
        adj: &HashMap<u16, Vec<u16>>,
    ) -> (HashSet<u16>, HashSet<u16>) {
        // BFS from u, excluding edge (u, v)
        let mut side_u: HashSet<u16> = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(u);
        side_u.insert(u);

        while let Some(cur) = queue.pop_front() {
            if let Some(neighbors) = adj.get(&cur) {
                for &next in neighbors {
                    if side_u.contains(&next) {
                        continue;
                    }
                    // Skip the removed edge
                    if (cur == u && next == v) || (cur == v && next == u) {
                        continue;
                    }
                    side_u.insert(next);
                    queue.push_back(next);
                }
            }
        }

        let side_v: HashSet<u16> = self
            .vertices
            .iter()
            .map(|cv| cv.id)
            .filter(|id| !side_u.contains(id))
            .collect();

        (side_u, side_v)
    }

    /// Collect original vertices from a set of cactus vertex IDs.
    fn collect_original_vertices(&self, cactus_ids: &HashSet<u16>) -> Vec<usize> {
        let mut result: Vec<usize> = self
            .vertices
            .iter()
            .filter(|cv| cactus_ids.contains(&cv.id))
            .flat_map(|cv| cv.original_vertices.iter().copied())
            .collect();
        result.sort_unstable();
        result
    }

    /// Compute cut value from a partition (sum of crossing edge weights).
    fn compute_cut_value_from_partition(&self, part_s: &[usize]) -> f64 {
        let s_set: HashSet<usize> = part_s.iter().copied().collect();
        // Build id -> index map for O(1) lookup
        let id_map: HashMap<u16, usize> = self
            .vertices
            .iter()
            .enumerate()
            .map(|(i, cv)| (cv.id, i))
            .collect();
        let mut total = 0.0f64;

        for e in &self.edges {
            let src_in_s = id_map
                .get(&e.source)
                .map(|&i| {
                    self.vertices[i]
                        .original_vertices
                        .iter()
                        .any(|v| s_set.contains(v))
                })
                .unwrap_or(false);
            let tgt_in_s = id_map
                .get(&e.target)
                .map(|&i| {
                    self.vertices[i]
                        .original_vertices
                        .iter()
                        .any(|v| s_set.contains(v))
                })
                .unwrap_or(false);

            if src_in_s != tgt_in_s {
                total += e.weight.to_f64();
            }
        }

        total
    }

    /// Compute cut edges (original graph edges) for a partition.
    fn compute_cut_edges(&self, part_s: &[usize]) -> Vec<(usize, usize, f64)> {
        let s_set: HashSet<usize> = part_s.iter().copied().collect();
        // Build id -> index map for O(1) lookup
        let id_map: HashMap<u16, usize> = self
            .vertices
            .iter()
            .enumerate()
            .map(|(i, cv)| (cv.id, i))
            .collect();
        let mut cut_edges = Vec::new();

        for e in &self.edges {
            let src_idx = id_map.get(&e.source).copied();
            let tgt_idx = id_map.get(&e.target).copied();

            let src_in_s = src_idx
                .map(|i| {
                    self.vertices[i]
                        .original_vertices
                        .iter()
                        .any(|v| s_set.contains(v))
                })
                .unwrap_or(false);
            let tgt_in_s = tgt_idx
                .map(|i| {
                    self.vertices[i]
                        .original_vertices
                        .iter()
                        .any(|v| s_set.contains(v))
                })
                .unwrap_or(false);

            if src_in_s != tgt_in_s {
                // Add representative edge
                let su = src_idx.and_then(|i| self.vertices[i].original_vertices.first().copied());
                let tv = tgt_idx.and_then(|i| self.vertices[i].original_vertices.first().copied());
                if let (Some(su), Some(tv)) = (su, tv) {
                    cut_edges.push((su, tv, e.weight.to_f64()));
                }
            }
        }

        cut_edges
    }

    /// Compute a deterministic canonical key from a partition using SipHash.
    fn compute_canonical_key(partition: &[usize]) -> [u8; 32] {
        let mut sorted = partition.to_vec();
        sorted.sort_unstable();

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        sorted.hash(&mut hasher);
        let hash1 = hasher.finish();

        let mut hasher2 = std::collections::hash_map::DefaultHasher::new();
        sorted.len().hash(&mut hasher2);
        for &v in &sorted {
            v.hash(&mut hasher2);
        }
        let hash2 = hasher2.finish();

        // Fill 32 bytes from two 64-bit hashes, repeated with mixing
        let mut key = [0u8; 32];
        key[0..8].copy_from_slice(&hash1.to_le_bytes());
        key[8..16].copy_from_slice(&hash2.to_le_bytes());
        // Mix for bytes 16..32
        let mixed1 = hash1.wrapping_mul(0x517cc1b727220a95) ^ hash2;
        let mixed2 = hash2.wrapping_mul(0x6c62272e07bb0142) ^ hash1;
        key[16..24].copy_from_slice(&mixed1.to_le_bytes());
        key[24..32].copy_from_slice(&mixed2.to_le_bytes());

        key
    }
}

// ---------------------------------------------------------------------------
// CanonicalCutResult
// ---------------------------------------------------------------------------

/// Result of a canonical minimum cut query.
///
/// Contains the cut value, the canonical partition, cut edges, and a
/// deterministic hash key that uniquely identifies this canonical cut.
#[derive(Debug, Clone)]
pub struct CanonicalCutResult {
    /// The minimum cut value.
    pub value: f64,
    /// The canonical partition (S, T) with S being the lexicographically
    /// smaller side.
    pub partition: (Vec<usize>, Vec<usize>),
    /// Edges in the cut as (source, target, weight) triples.
    pub cut_edges: Vec<(usize, usize, f64)>,
    /// Deterministic hash of the sorted smaller partition.
    pub canonical_key: [u8; 32],
}

// ---------------------------------------------------------------------------
// WitnessReceipt
// ---------------------------------------------------------------------------

/// An immutable receipt attesting to a canonical min-cut at a given epoch.
///
/// Can be used for audit trails and reproducibility verification.
#[derive(Debug, Clone)]
pub struct WitnessReceipt {
    /// Epoch (logical timestamp) at which this receipt was produced.
    pub epoch: u64,
    /// Hash of the canonical cut partition.
    pub cut_hash: [u8; 32],
    /// The cut value.
    pub cut_value: f64,
    /// Number of edges in the cut.
    pub edge_count: usize,
    /// Wall-clock timestamp in nanoseconds since Unix epoch.
    pub timestamp_ns: u64,
}

// ---------------------------------------------------------------------------
// CanonicalMinCut trait
// ---------------------------------------------------------------------------

/// Trait extending `DynamicMinCut` with canonical cut capabilities.
///
/// Implementors provide reproducible, deterministic min-cut results
/// backed by a cactus graph representation.
pub trait CanonicalMinCut {
    /// Compute the canonical minimum cut.
    fn canonical_cut(&self) -> CanonicalCutResult;

    /// Build and return the cactus graph for the current state.
    fn cactus_graph(&self) -> CactusGraph;

    /// Generate a witness receipt for the current canonical cut.
    fn witness_receipt(&self) -> WitnessReceipt;

    /// Insert an edge and return the new canonical min-cut value.
    fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) -> crate::Result<f64>;

    /// Delete an edge and return the new canonical min-cut value.
    fn delete_edge(&mut self, u: VertexId, v: VertexId) -> crate::Result<f64>;

    /// Get the current minimum cut value.
    fn min_cut_value(&self) -> f64;

    /// Number of vertices.
    fn num_vertices(&self) -> usize;

    /// Number of edges.
    fn num_edges(&self) -> usize;

    /// Check if graph is connected.
    fn is_connected(&self) -> bool;
}

// ---------------------------------------------------------------------------
// CanonicalMinCutImpl
// ---------------------------------------------------------------------------

/// Concrete implementation of `CanonicalMinCut`.
///
/// Wraps an inner `DynamicMinCut` and lazily builds a `CactusGraph`
/// when the canonical cut is requested. The cactus is invalidated on
/// any structural change (edge insert / delete).
pub struct CanonicalMinCutImpl {
    /// Underlying dynamic min-cut engine.
    inner: algorithm::DynamicMinCut,
    /// Cached cactus graph (rebuilt when dirty).
    cactus: Option<CactusGraph>,
    /// Logical epoch counter, incremented on each mutation.
    epoch: u64,
    /// Whether the cached cactus is stale.
    dirty: bool,
}

impl CanonicalMinCutImpl {
    /// Create from an existing `DynamicMinCut`.
    pub fn from_dynamic(inner: algorithm::DynamicMinCut) -> Self {
        Self {
            inner,
            cactus: None,
            epoch: 0,
            dirty: true,
        }
    }

    /// Create a new empty canonical min-cut structure.
    pub fn new() -> Self {
        Self {
            inner: algorithm::DynamicMinCut::new(MinCutConfig::default()),
            cactus: None,
            epoch: 0,
            dirty: true,
        }
    }

    /// Create from edges.
    pub fn with_edges(edges: Vec<(VertexId, VertexId, Weight)>) -> crate::Result<Self> {
        let inner = algorithm::MinCutBuilder::new()
            .exact()
            .with_edges(edges)
            .build()?;
        Ok(Self {
            inner,
            cactus: None,
            epoch: 0,
            dirty: true,
        })
    }

    /// Ensure the cactus is up to date.
    fn ensure_cactus(&mut self) {
        if !self.dirty && self.cactus.is_some() {
            return;
        }

        let graph = self.inner.graph();
        let g = graph.read();
        let mut cactus = CactusGraph::build_from_graph(&g);
        drop(g);
        cactus.root_at_lex_smallest();
        self.cactus = Some(cactus);
        self.dirty = false;
    }
}

impl Default for CanonicalMinCutImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl CanonicalMinCut for CanonicalMinCutImpl {
    fn canonical_cut(&self) -> CanonicalCutResult {
        // We need a mutable borrow to ensure the cactus, but the trait
        // signature is &self. Work around by using interior mutability
        // pattern (rebuild inline if needed).
        let graph = self.inner.graph();
        let g = graph.read();

        if let Some(ref cactus) = self.cactus {
            if !self.dirty {
                return cactus.canonical_cut();
            }
        }

        // Rebuild
        let mut cactus = CactusGraph::build_from_graph(&g);
        drop(g);
        cactus.root_at_lex_smallest();
        cactus.canonical_cut()
    }

    fn cactus_graph(&self) -> CactusGraph {
        let graph = self.inner.graph();
        let g = graph.read();

        if let Some(ref cactus) = self.cactus {
            if !self.dirty {
                return cactus.clone();
            }
        }

        let mut cactus = CactusGraph::build_from_graph(&g);
        drop(g);
        cactus.root_at_lex_smallest();
        cactus
    }

    fn witness_receipt(&self) -> WitnessReceipt {
        let result = self.canonical_cut();
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        WitnessReceipt {
            epoch: self.epoch,
            cut_hash: result.canonical_key,
            cut_value: result.value,
            edge_count: result.cut_edges.len(),
            timestamp_ns: ts,
        }
    }

    fn insert_edge(&mut self, u: VertexId, v: VertexId, weight: Weight) -> crate::Result<f64> {
        let val = self.inner.insert_edge(u, v, weight)?;
        self.epoch += 1;
        self.dirty = true;
        self.cactus = None;
        Ok(val)
    }

    fn delete_edge(&mut self, u: VertexId, v: VertexId) -> crate::Result<f64> {
        let val = self.inner.delete_edge(u, v)?;
        self.epoch += 1;
        self.dirty = true;
        self.cactus = None;
        Ok(val)
    }

    fn min_cut_value(&self) -> f64 {
        self.inner.min_cut_value()
    }

    fn num_vertices(&self) -> usize {
        self.inner.num_vertices()
    }

    fn num_edges(&self) -> usize {
        self.inner.num_edges()
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }
}
