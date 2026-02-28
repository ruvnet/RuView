//! SheafGraph: Aggregate root for the sheaf-theoretic knowledge substrate
//!
//! The `SheafGraph` is the central data structure for coherence computation.
//! It manages:
//!
//! - Nodes with state vectors (stalks of the sheaf)
//! - Edges with restriction maps (constraints)
//! - Namespaces for multi-tenant isolation
//! - Incremental coherence energy computation
//! - Graph fingerprinting for change detection
//!
//! # Coherence Computation
//!
//! Global coherence energy is computed as:
//! ```text
//! E(S) = Σ w_e ||r_e||²
//! ```
//!
//! Where:
//! - `w_e` is the edge weight
//! - `r_e = ρ_source(x_source) - ρ_target(x_target)` is the residual
//!
//! # Thread Safety
//!
//! The graph is designed for concurrent access:
//! - Read operations use DashMap for lock-free concurrent reads
//! - Write operations update thread-safe counters
//! - Parallel energy computation uses rayon

use super::edge::{EdgeId, SheafEdge};
use super::node::{NodeId, SheafNode};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;

/// Namespace identifier for multi-tenant isolation
pub type Namespace = String;

/// Scope identifier for energy aggregation
pub type ScopeId = String;

/// Coherence fingerprint for change detection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CoherenceFingerprint {
    /// Hash of graph structure (nodes + edges)
    pub structure_hash: u64,
    /// Hash of all state vectors
    pub state_hash: u64,
    /// Generation counter
    pub generation: u64,
}

impl CoherenceFingerprint {
    /// Create a new fingerprint
    pub fn new(structure_hash: u64, state_hash: u64, generation: u64) -> Self {
        Self {
            structure_hash,
            state_hash,
            generation,
        }
    }

    /// Combine hashes into a single value
    pub fn combined(&self) -> u64 {
        self.structure_hash
            .wrapping_mul(31)
            .wrapping_add(self.state_hash)
            .wrapping_mul(31)
            .wrapping_add(self.generation)
    }

    /// Check if fingerprint has changed
    pub fn has_changed(&self, other: &Self) -> bool {
        self.combined() != other.combined()
    }
}

/// Global coherence energy with breakdown by edge and scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceEnergy {
    /// Total system energy (lower = more coherent)
    pub total_energy: f32,
    /// Per-edge energies for localization
    pub edge_energies: HashMap<EdgeId, f32>,
    /// Energy aggregated by scope/namespace
    pub scope_energies: HashMap<ScopeId, f32>,
    /// Number of edges contributing to energy
    pub edge_count: usize,
    /// Computation timestamp
    pub computed_at: DateTime<Utc>,
    /// Fingerprint at computation time
    pub fingerprint: CoherenceFingerprint,
}

impl CoherenceEnergy {
    /// Create an empty energy result
    pub fn empty() -> Self {
        Self {
            total_energy: 0.0,
            edge_energies: HashMap::new(),
            scope_energies: HashMap::new(),
            edge_count: 0,
            computed_at: Utc::now(),
            fingerprint: CoherenceFingerprint::new(0, 0, 0),
        }
    }

    /// Get energy for a specific scope
    pub fn scope_energy(&self, scope: &str) -> f32 {
        self.scope_energies.get(scope).copied().unwrap_or(0.0)
    }

    /// Get the top N highest-energy edges
    pub fn top_edges(&self, n: usize) -> Vec<(EdgeId, f32)> {
        let mut edges: Vec<_> = self.edge_energies.iter().map(|(&k, &v)| (k, v)).collect();
        edges.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        edges.truncate(n);
        edges
    }

    /// Check if total energy is below threshold (system is coherent)
    pub fn is_coherent(&self, threshold: f32) -> bool {
        self.total_energy <= threshold
    }

    /// Get edges with energy above threshold
    pub fn incoherent_edges(&self, threshold: f32) -> Vec<(EdgeId, f32)> {
        self.edge_energies
            .iter()
            .filter(|(_, &e)| e > threshold)
            .map(|(&k, &v)| (k, v))
            .collect()
    }
}

/// Incremental coherence computation state
#[derive(Debug)]
pub struct IncrementalCoherence {
    /// Stored per-edge residual norms squared
    residual_norms: DashMap<EdgeId, f32>,
    /// Subgraph energy summaries by scope
    scope_summaries: DashMap<ScopeId, f32>,
    /// Global fingerprint for staleness detection
    fingerprint: RwLock<CoherenceFingerprint>,
    /// Dirty edges that need recomputation
    dirty_edges: DashMap<EdgeId, ()>,
}

impl IncrementalCoherence {
    /// Create new incremental state
    pub fn new() -> Self {
        Self {
            residual_norms: DashMap::new(),
            scope_summaries: DashMap::new(),
            fingerprint: RwLock::new(CoherenceFingerprint::new(0, 0, 0)),
            dirty_edges: DashMap::new(),
        }
    }

    /// Mark an edge as dirty (needs recomputation)
    pub fn mark_dirty(&self, edge_id: EdgeId) {
        self.dirty_edges.insert(edge_id, ());
    }

    /// Mark edges incident to a node as dirty
    pub fn mark_node_dirty(&self, graph: &SheafGraph, node_id: NodeId) {
        for edge_id in graph.edges_incident_to(node_id) {
            self.dirty_edges.insert(edge_id, ());
        }
    }

    /// Update cached residual for an edge
    pub fn update_residual(&self, edge_id: EdgeId, norm_sq: f32) {
        self.residual_norms.insert(edge_id, norm_sq);
        self.dirty_edges.remove(&edge_id);
    }

    /// Get cached residual norm (if not dirty)
    pub fn get_residual(&self, edge_id: &EdgeId) -> Option<f32> {
        if self.dirty_edges.contains_key(edge_id) {
            None
        } else {
            self.residual_norms.get(edge_id).map(|r| *r)
        }
    }

    /// Check if any edges are dirty
    pub fn has_dirty_edges(&self) -> bool {
        !self.dirty_edges.is_empty()
    }

    /// Get count of dirty edges
    pub fn dirty_count(&self) -> usize {
        self.dirty_edges.len()
    }

    /// Clear all dirty flags
    pub fn clear_dirty(&self) {
        self.dirty_edges.clear();
    }

    /// Update fingerprint
    pub fn update_fingerprint(&self, fingerprint: CoherenceFingerprint) {
        *self.fingerprint.write() = fingerprint;
    }

    /// Get current fingerprint
    pub fn fingerprint(&self) -> CoherenceFingerprint {
        *self.fingerprint.read()
    }
}

impl Default for IncrementalCoherence {
    fn default() -> Self {
        Self::new()
    }
}

/// Adjacency index for fast neighbor lookups
#[derive(Debug, Default)]
struct AdjacencyIndex {
    /// Edges incident to each node (outgoing and incoming combined)
    node_edges: DashMap<NodeId, HashSet<EdgeId>>,
}

impl AdjacencyIndex {
    fn new() -> Self {
        Self::default()
    }

    fn add_edge(&self, edge: &SheafEdge) {
        self.node_edges
            .entry(edge.source)
            .or_insert_with(HashSet::new)
            .insert(edge.id);
        self.node_edges
            .entry(edge.target)
            .or_insert_with(HashSet::new)
            .insert(edge.id);
    }

    fn remove_edge(&self, edge: &SheafEdge) {
        if let Some(mut edges) = self.node_edges.get_mut(&edge.source) {
            edges.remove(&edge.id);
        }
        if let Some(mut edges) = self.node_edges.get_mut(&edge.target) {
            edges.remove(&edge.id);
        }
    }

    fn edges_for_node(&self, node_id: NodeId) -> Vec<EdgeId> {
        self.node_edges
            .get(&node_id)
            .map(|edges| edges.iter().copied().collect())
            .unwrap_or_default()
    }

    fn remove_node(&self, node_id: NodeId) {
        self.node_edges.remove(&node_id);
    }
}

/// The sheaf graph: aggregate root for coherence computation
pub struct SheafGraph {
    /// Node storage (thread-safe)
    nodes: Arc<DashMap<NodeId, SheafNode>>,
    /// Edge storage (thread-safe)
    edges: Arc<DashMap<EdgeId, SheafEdge>>,
    /// Adjacency index for fast lookups
    adjacency: AdjacencyIndex,
    /// Namespace registry
    namespaces: DashMap<Namespace, HashSet<NodeId>>,
    /// Generation counter for fingerprinting
    generation: AtomicU64,
    /// Incremental coherence state
    incremental: IncrementalCoherence,
    /// Default namespace
    default_namespace: String,
}

impl SheafGraph {
    /// Create a new empty sheaf graph
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(DashMap::new()),
            edges: Arc::new(DashMap::new()),
            adjacency: AdjacencyIndex::new(),
            namespaces: DashMap::new(),
            generation: AtomicU64::new(0),
            incremental: IncrementalCoherence::new(),
            default_namespace: "default".to_string(),
        }
    }

    /// Create a graph with a specific default namespace
    pub fn with_namespace(namespace: impl Into<String>) -> Self {
        Self {
            default_namespace: namespace.into(),
            ..Self::new()
        }
    }

    // ========================================================================
    // Node Operations
    // ========================================================================

    /// Add a node to the graph
    pub fn add_node(&self, node: SheafNode) -> NodeId {
        let id = node.id;
        let namespace = node
            .metadata
            .namespace
            .clone()
            .unwrap_or_else(|| self.default_namespace.clone());

        // Add to namespace index
        self.namespaces
            .entry(namespace)
            .or_insert_with(HashSet::new)
            .insert(id);

        // Insert node
        self.nodes.insert(id, node);
        self.increment_generation();

        id
    }

    /// Get a node by ID (clones the node)
    pub fn get_node(&self, id: NodeId) -> Option<SheafNode> {
        self.nodes.get(&id).map(|n| n.clone())
    }

    /// Get a reference to a node without cloning
    ///
    /// Returns a DashMap reference guard for read-only access.
    /// More efficient than `get_node()` when you only need to read.
    #[inline]
    pub fn get_node_ref(
        &self,
        id: NodeId,
    ) -> Option<dashmap::mapref::one::Ref<'_, NodeId, SheafNode>> {
        self.nodes.get(&id)
    }

    /// Execute a closure with a reference to a node (zero-copy read)
    ///
    /// More efficient than get_node() when you only need to read node data.
    #[inline]
    pub fn with_node<R>(&self, id: NodeId, f: impl FnOnce(&SheafNode) -> R) -> Option<R> {
        self.nodes.get(&id).map(|n| f(&n))
    }

    /// Get a reference to a node (for reading state)
    pub fn node_state(&self, id: NodeId) -> Option<Vec<f32>> {
        self.nodes.get(&id).map(|n| n.state.as_slice().to_vec())
    }

    /// Update a node's state
    pub fn update_node_state(&self, id: NodeId, new_state: &[f32]) -> bool {
        if let Some(mut node) = self.nodes.get_mut(&id) {
            node.update_state_from_slice(new_state);
            self.incremental.mark_node_dirty(self, id);
            self.increment_generation();
            true
        } else {
            false
        }
    }

    /// Remove a node (and all incident edges)
    pub fn remove_node(&self, id: NodeId) -> Option<SheafNode> {
        // First remove all incident edges
        let incident_edges = self.edges_incident_to(id);
        for edge_id in incident_edges {
            self.remove_edge(edge_id);
        }

        // Remove from namespace index
        if let Some((_, node)) = self.nodes.remove(&id) {
            let namespace = node
                .metadata
                .namespace
                .clone()
                .unwrap_or_else(|| self.default_namespace.clone());

            if let Some(mut ns_nodes) = self.namespaces.get_mut(&namespace) {
                ns_nodes.remove(&id);
            }

            self.adjacency.remove_node(id);
            self.increment_generation();
            Some(node)
        } else {
            None
        }
    }

    /// Check if a node exists
    pub fn has_node(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id)
    }

    /// Get count of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Iterate over all node IDs
    pub fn node_ids(&self) -> Vec<NodeId> {
        self.nodes.iter().map(|r| *r.key()).collect()
    }

    /// Get nodes in a namespace
    pub fn nodes_in_namespace(&self, namespace: &str) -> Vec<NodeId> {
        self.namespaces
            .get(namespace)
            .map(|ns| ns.iter().copied().collect())
            .unwrap_or_default()
    }

    // ========================================================================
    // Edge Operations
    // ========================================================================

    /// Add an edge to the graph
    pub fn add_edge(&self, edge: SheafEdge) -> Result<EdgeId, &'static str> {
        // Verify nodes exist
        if !self.has_node(edge.source) {
            return Err("Source node does not exist");
        }
        if !self.has_node(edge.target) {
            return Err("Target node does not exist");
        }

        let id = edge.id;

        // Update adjacency index
        self.adjacency.add_edge(&edge);

        // Insert edge
        self.edges.insert(id, edge);
        self.incremental.mark_dirty(id);
        self.increment_generation();

        Ok(id)
    }

    /// Get an edge by ID
    pub fn get_edge(&self, id: EdgeId) -> Option<SheafEdge> {
        self.edges.get(&id).map(|e| e.clone())
    }

    /// Remove an edge
    pub fn remove_edge(&self, id: EdgeId) -> Option<SheafEdge> {
        if let Some((_, edge)) = self.edges.remove(&id) {
            self.adjacency.remove_edge(&edge);
            self.incremental.residual_norms.remove(&id);
            self.increment_generation();
            Some(edge)
        } else {
            None
        }
    }

    /// Update an edge's weight
    pub fn update_edge_weight(&self, id: EdgeId, weight: f32) -> bool {
        if let Some(mut edge) = self.edges.get_mut(&id) {
            edge.set_weight(weight);
            self.incremental.mark_dirty(id);
            self.increment_generation();
            true
        } else {
            false
        }
    }

    /// Check if an edge exists
    pub fn has_edge(&self, id: EdgeId) -> bool {
        self.edges.contains_key(&id)
    }

    /// Get count of edges
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Iterate over all edge IDs
    pub fn edge_ids(&self) -> Vec<EdgeId> {
        self.edges.iter().map(|r| *r.key()).collect()
    }

    /// Get edges incident to a node
    pub fn edges_incident_to(&self, node_id: NodeId) -> Vec<EdgeId> {
        self.adjacency.edges_for_node(node_id)
    }

    // ========================================================================
    // Coherence Computation
    // ========================================================================

    /// Compute global coherence energy
    ///
    /// This computes E(S) = Σ w_e ||r_e||² across all edges.
    ///
    /// # Thread Safety
    ///
    /// Uses rayon for parallel computation when the `parallel` feature is enabled.
    pub fn compute_energy(&self) -> CoherenceEnergy {
        let fingerprint = self.compute_fingerprint();

        #[cfg(feature = "parallel")]
        let edge_energies: HashMap<EdgeId, f32> = {
            use rayon::prelude::*;
            self.edges
                .iter()
                .par_bridge()
                .filter_map(|entry| {
                    let edge = entry.value();
                    let source_state = self.nodes.get(&edge.source)?;
                    let target_state = self.nodes.get(&edge.target)?;
                    let energy = edge.weighted_residual_energy(
                        source_state.state.as_slice(),
                        target_state.state.as_slice(),
                    );
                    Some((*entry.key(), energy))
                })
                .collect()
        };

        #[cfg(not(feature = "parallel"))]
        let edge_energies: HashMap<EdgeId, f32> = self
            .edges
            .iter()
            .filter_map(|entry| {
                let edge = entry.value();
                let source_state = self.nodes.get(&edge.source)?;
                let target_state = self.nodes.get(&edge.target)?;
                let energy = edge.weighted_residual_energy(
                    source_state.state.as_slice(),
                    target_state.state.as_slice(),
                );
                Some((*entry.key(), energy))
            })
            .collect();

        let total_energy: f32 = edge_energies.values().sum();
        let scope_energies = self.aggregate_by_scope(&edge_energies);

        // Update incremental cache
        for (&id, &energy) in &edge_energies {
            self.incremental.update_residual(id, energy);
        }
        self.incremental.update_fingerprint(fingerprint);

        CoherenceEnergy {
            total_energy,
            edge_energies,
            scope_energies,
            edge_count: self.edges.len(),
            computed_at: Utc::now(),
            fingerprint,
        }
    }

    /// Compute energy incrementally (only for dirty edges)
    ///
    /// This is more efficient when only a few nodes have changed.
    pub fn compute_energy_incremental(&self) -> CoherenceEnergy {
        if !self.incremental.has_dirty_edges() {
            // No changes, return cached result
            let mut edge_energies = HashMap::new();
            for entry in self.incremental.residual_norms.iter() {
                edge_energies.insert(*entry.key(), *entry.value());
            }

            let total_energy: f32 = edge_energies.values().sum();
            let scope_energies = self.aggregate_by_scope(&edge_energies);

            return CoherenceEnergy {
                total_energy,
                edge_energies,
                scope_energies,
                edge_count: self.edges.len(),
                computed_at: Utc::now(),
                fingerprint: self.incremental.fingerprint(),
            };
        }

        // Recompute only dirty edges
        let dirty_ids: Vec<EdgeId> = self
            .incremental
            .dirty_edges
            .iter()
            .map(|r| *r.key())
            .collect();

        for edge_id in dirty_ids {
            if let Some(edge) = self.edges.get(&edge_id) {
                if let (Some(source), Some(target)) =
                    (self.nodes.get(&edge.source), self.nodes.get(&edge.target))
                {
                    let energy = edge
                        .weighted_residual_energy(source.state.as_slice(), target.state.as_slice());
                    self.incremental.update_residual(edge_id, energy);
                }
            }
        }

        // Build full result from cache
        let mut edge_energies = HashMap::new();
        for entry in self.incremental.residual_norms.iter() {
            edge_energies.insert(*entry.key(), *entry.value());
        }

        let total_energy: f32 = edge_energies.values().sum();
        let scope_energies = self.aggregate_by_scope(&edge_energies);
        let fingerprint = self.compute_fingerprint();
        self.incremental.update_fingerprint(fingerprint);

        CoherenceEnergy {
            total_energy,
            edge_energies,
            scope_energies,
            edge_count: self.edges.len(),
            computed_at: Utc::now(),
            fingerprint,
        }
    }

    /// Compute energy for a specific node's neighborhood
    pub fn compute_local_energy(&self, node_id: NodeId) -> f32 {
        let incident_edges = self.edges_incident_to(node_id);
        let mut total = 0.0;

        for edge_id in incident_edges {
            if let Some(edge) = self.edges.get(&edge_id) {
                if let (Some(source), Some(target)) =
                    (self.nodes.get(&edge.source), self.nodes.get(&edge.target))
                {
                    total += edge
                        .weighted_residual_energy(source.state.as_slice(), target.state.as_slice());
                }
            }
        }

        total
    }

    /// Aggregate edge energies by scope (namespace)
    fn aggregate_by_scope(&self, edge_energies: &HashMap<EdgeId, f32>) -> HashMap<ScopeId, f32> {
        let mut scope_energies: HashMap<ScopeId, f32> = HashMap::new();

        for (&edge_id, &energy) in edge_energies {
            if let Some(edge) = self.edges.get(&edge_id) {
                let scope = edge
                    .namespace
                    .clone()
                    .unwrap_or_else(|| self.default_namespace.clone());
                *scope_energies.entry(scope).or_insert(0.0) += energy;
            }
        }

        scope_energies
    }

    // ========================================================================
    // Fingerprinting
    // ========================================================================

    /// Compute graph fingerprint for change detection
    pub fn compute_fingerprint(&self) -> CoherenceFingerprint {
        use std::hash::{Hash, Hasher};
        let mut structure_hasher = std::collections::hash_map::DefaultHasher::new();
        let mut state_hasher = std::collections::hash_map::DefaultHasher::new();

        // Hash structure (node IDs and edge connections)
        let mut node_ids: Vec<_> = self.nodes.iter().map(|r| *r.key()).collect();
        node_ids.sort();
        for id in &node_ids {
            id.hash(&mut structure_hasher);
        }

        let mut edge_ids: Vec<_> = self.edges.iter().map(|r| *r.key()).collect();
        edge_ids.sort();
        for id in &edge_ids {
            id.hash(&mut structure_hasher);
            if let Some(edge) = self.edges.get(id) {
                edge.source.hash(&mut structure_hasher);
                edge.target.hash(&mut structure_hasher);
            }
        }

        // Hash state vectors
        for id in &node_ids {
            if let Some(node) = self.nodes.get(id) {
                state_hasher.write_u64(node.state.content_hash());
                state_hasher.write_u64(node.version);
            }
        }

        CoherenceFingerprint {
            structure_hash: structure_hasher.finish(),
            state_hash: state_hasher.finish(),
            generation: self.generation.load(Ordering::SeqCst),
        }
    }

    /// Check if graph has changed since given fingerprint
    pub fn has_changed_since(&self, fingerprint: &CoherenceFingerprint) -> bool {
        self.generation.load(Ordering::SeqCst) != fingerprint.generation
            || self.compute_fingerprint().has_changed(fingerprint)
    }

    /// Get current generation
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Increment generation counter
    fn increment_generation(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get graph statistics
    pub fn stats(&self) -> GraphStats {
        let node_count = self.nodes.len();
        let edge_count = self.edges.len();

        // Compute degree distribution
        let mut total_degree = 0usize;
        let mut max_degree = 0usize;

        for entry in self.adjacency.node_edges.iter() {
            let degree = entry.value().len();
            total_degree += degree;
            max_degree = max_degree.max(degree);
        }

        let avg_degree = if node_count > 0 {
            total_degree as f64 / node_count as f64
        } else {
            0.0
        };

        GraphStats {
            node_count,
            edge_count,
            namespace_count: self.namespaces.len(),
            avg_degree,
            max_degree,
            dirty_edges: self.incremental.dirty_count(),
            generation: self.generation(),
        }
    }
}

impl Default for SheafGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// Graph statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    /// Number of nodes
    pub node_count: usize,
    /// Number of edges
    pub edge_count: usize,
    /// Number of namespaces
    pub namespace_count: usize,
    /// Average node degree
    pub avg_degree: f64,
    /// Maximum node degree
    pub max_degree: usize,
    /// Number of dirty edges (pending recomputation)
    pub dirty_edges: usize,
    /// Generation counter
    pub generation: u64,
}

/// Builder for constructing SheafGraph with initial data
pub struct SheafGraphBuilder {
    graph: SheafGraph,
}

impl SheafGraphBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            graph: SheafGraph::new(),
        }
    }

    /// Set default namespace
    pub fn default_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.graph.default_namespace = namespace.into();
        self
    }

    /// Add a node
    pub fn node(self, node: SheafNode) -> Self {
        self.graph.add_node(node);
        self
    }

    /// Add multiple nodes
    pub fn nodes(self, nodes: impl IntoIterator<Item = SheafNode>) -> Self {
        for node in nodes {
            self.graph.add_node(node);
        }
        self
    }

    /// Add an edge (panics if nodes don't exist)
    pub fn edge(self, edge: SheafEdge) -> Self {
        self.graph.add_edge(edge).expect("Failed to add edge");
        self
    }

    /// Add multiple edges
    pub fn edges(self, edges: impl IntoIterator<Item = SheafEdge>) -> Self {
        for edge in edges {
            self.graph.add_edge(edge).expect("Failed to add edge");
        }
        self
    }

    /// Build the graph
    pub fn build(self) -> SheafGraph {
        self.graph
    }
}

impl Default for SheafGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::substrate::edge::SheafEdgeBuilder;
    use crate::substrate::node::{SheafNodeBuilder, StateVector};
    use crate::substrate::restriction::RestrictionMap;

    fn make_test_graph() -> SheafGraph {
        let graph = SheafGraph::new();

        // Create three nodes with states
        let node1 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 0.0, 0.0])
            .namespace("test")
            .build();
        let node2 = SheafNodeBuilder::new()
            .state_from_slice(&[0.0, 1.0, 0.0])
            .namespace("test")
            .build();
        let node3 = SheafNodeBuilder::new()
            .state_from_slice(&[0.0, 0.0, 1.0])
            .namespace("test")
            .build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);
        let id3 = graph.add_node(node3);

        // Create edges with identity restrictions
        let edge12 = SheafEdgeBuilder::new(id1, id2)
            .identity_restrictions(3)
            .namespace("test")
            .build();
        let edge23 = SheafEdgeBuilder::new(id2, id3)
            .identity_restrictions(3)
            .namespace("test")
            .build();
        let edge31 = SheafEdgeBuilder::new(id3, id1)
            .identity_restrictions(3)
            .namespace("test")
            .build();

        graph.add_edge(edge12).unwrap();
        graph.add_edge(edge23).unwrap();
        graph.add_edge(edge31).unwrap();

        graph
    }

    #[test]
    fn test_graph_creation() {
        let graph = SheafGraph::new();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_add_node() {
        let graph = SheafGraph::new();
        let node = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 2.0, 3.0])
            .build();

        let id = graph.add_node(node);
        assert!(graph.has_node(id));
        assert_eq!(graph.node_count(), 1);
    }

    #[test]
    fn test_add_edge() {
        let graph = SheafGraph::new();

        let node1 = SheafNodeBuilder::new().state_from_slice(&[1.0]).build();
        let node2 = SheafNodeBuilder::new().state_from_slice(&[2.0]).build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);

        let edge = SheafEdgeBuilder::new(id1, id2)
            .identity_restrictions(1)
            .build();

        let edge_id = graph.add_edge(edge).unwrap();
        assert!(graph.has_edge(edge_id));
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn test_edge_without_nodes_fails() {
        let graph = SheafGraph::new();
        let fake_id = Uuid::new_v4();

        let edge = SheafEdgeBuilder::new(fake_id, fake_id)
            .identity_restrictions(1)
            .build();

        let result = graph.add_edge(edge);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_node() {
        let graph = make_test_graph();
        let node_ids = graph.node_ids();

        let removed = graph.remove_node(node_ids[0]);
        assert!(removed.is_some());
        assert!(!graph.has_node(node_ids[0]));
        assert_eq!(graph.node_count(), 2);
        // Edges incident to removed node should also be removed
        assert!(graph.edge_count() < 3);
    }

    #[test]
    fn test_update_node_state() {
        let graph = SheafGraph::new();
        let node = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 2.0])
            .build();
        let id = graph.add_node(node);

        assert!(graph.update_node_state(id, &[3.0, 4.0]));

        let state = graph.node_state(id).unwrap();
        assert_eq!(state, vec![3.0, 4.0]);
    }

    #[test]
    fn test_compute_energy() {
        let graph = make_test_graph();
        let energy = graph.compute_energy();

        // With orthogonal states and identity restrictions, all edges should have energy
        assert!(energy.total_energy > 0.0);
        assert_eq!(energy.edge_count, 3);
        assert_eq!(energy.edge_energies.len(), 3);
    }

    #[test]
    fn test_coherent_graph() {
        let graph = SheafGraph::new();

        // Create nodes with identical states
        let node1 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 1.0, 1.0])
            .build();
        let node2 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 1.0, 1.0])
            .build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);

        let edge = SheafEdgeBuilder::new(id1, id2)
            .identity_restrictions(3)
            .build();
        graph.add_edge(edge).unwrap();

        let energy = graph.compute_energy();
        assert!(energy.total_energy < 1e-10);
        assert!(energy.is_coherent(0.01));
    }

    #[test]
    fn test_incremental_energy() {
        let graph = make_test_graph();

        // First computation
        let energy1 = graph.compute_energy();

        // No changes - incremental should return same result
        let energy2 = graph.compute_energy_incremental();
        assert!((energy1.total_energy - energy2.total_energy).abs() < 1e-10);

        // Update a node to a value that creates more coherence (closer to neighbors)
        let node_ids = graph.node_ids();
        // Update node1 from [1,0,0] to [0.5, 0.5, 0] - closer to node2's [0,1,0]
        graph.update_node_state(node_ids[0], &[0.5, 0.5, 0.0]);

        // Incremental should detect dirty edges
        assert!(graph.incremental.has_dirty_edges());

        let energy3 = graph.compute_energy_incremental();

        // After clearing dirty edges, subsequent call returns cached result
        let energy4 = graph.compute_energy_incremental();
        assert!((energy3.total_energy - energy4.total_energy).abs() < 1e-10);

        // Verify energy was recomputed (not necessarily changed significantly,
        // but the mechanism should work)
        assert!(energy3.edge_energies.len() == energy1.edge_energies.len());
    }

    #[test]
    fn test_local_energy() {
        let graph = make_test_graph();
        let node_ids = graph.node_ids();

        let local_energy = graph.compute_local_energy(node_ids[0]);
        assert!(local_energy > 0.0);

        // Local energy should be less than or equal to total
        // (node has 2 incident edges out of 3)
        let total = graph.compute_energy().total_energy;
        assert!(local_energy <= total);
    }

    #[test]
    fn test_fingerprint() {
        let graph = make_test_graph();

        let fp1 = graph.compute_fingerprint();
        let fp2 = graph.compute_fingerprint();

        // Same graph should have same fingerprint
        assert_eq!(fp1.combined(), fp2.combined());

        // Update should change fingerprint
        let node_ids = graph.node_ids();
        graph.update_node_state(node_ids[0], &[2.0, 0.0, 0.0]);

        let fp3 = graph.compute_fingerprint();
        assert!(fp1.has_changed(&fp3));
    }

    #[test]
    fn test_edges_incident_to() {
        let graph = make_test_graph();
        let node_ids = graph.node_ids();

        let edges = graph.edges_incident_to(node_ids[0]);
        // Each node in a triangle has 2 incident edges
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn test_namespaces() {
        let graph = SheafGraph::new();

        let node1 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0])
            .namespace("ns1")
            .build();
        let node2 = SheafNodeBuilder::new()
            .state_from_slice(&[2.0])
            .namespace("ns2")
            .build();

        graph.add_node(node1);
        graph.add_node(node2);

        assert_eq!(graph.nodes_in_namespace("ns1").len(), 1);
        assert_eq!(graph.nodes_in_namespace("ns2").len(), 1);
    }

    #[test]
    fn test_builder() {
        let node1 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 2.0])
            .build();
        let node2 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 2.0])
            .build();
        let id1 = node1.id;
        let id2 = node2.id;

        let edge = SheafEdgeBuilder::new(id1, id2)
            .identity_restrictions(2)
            .build();

        let graph = SheafGraphBuilder::new()
            .default_namespace("test")
            .node(node1)
            .node(node2)
            .edge(edge)
            .build();

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn test_graph_stats() {
        let graph = make_test_graph();
        let stats = graph.stats();

        assert_eq!(stats.node_count, 3);
        assert_eq!(stats.edge_count, 3);
        assert!((stats.avg_degree - 2.0).abs() < 0.01); // Triangle: each node has degree 2
        assert_eq!(stats.max_degree, 2);
    }

    #[test]
    fn test_scope_energies() {
        let graph = SheafGraph::new();

        // Create nodes in different namespaces
        let node1 = SheafNodeBuilder::new()
            .state_from_slice(&[1.0])
            .namespace("scope_a")
            .build();
        let node2 = SheafNodeBuilder::new()
            .state_from_slice(&[2.0])
            .namespace("scope_a")
            .build();
        let node3 = SheafNodeBuilder::new()
            .state_from_slice(&[3.0])
            .namespace("scope_b")
            .build();

        let id1 = graph.add_node(node1);
        let id2 = graph.add_node(node2);
        let id3 = graph.add_node(node3);

        // Edge in scope_a
        let edge1 = SheafEdgeBuilder::new(id1, id2)
            .identity_restrictions(1)
            .namespace("scope_a")
            .build();
        // Edge in scope_b
        let edge2 = SheafEdgeBuilder::new(id2, id3)
            .identity_restrictions(1)
            .namespace("scope_b")
            .build();

        graph.add_edge(edge1).unwrap();
        graph.add_edge(edge2).unwrap();

        let energy = graph.compute_energy();
        assert!(energy.scope_energies.contains_key("scope_a"));
        assert!(energy.scope_energies.contains_key("scope_b"));
    }
}
