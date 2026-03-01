//! Coherence Engine - Core computation aggregate
//!
//! The CoherenceEngine is the primary aggregate for computing sheaf Laplacian coherence.
//! It maintains:
//! - Sheaf graph structure (nodes with states, edges with restriction maps)
//! - Residual cache for incremental computation
//! - Fingerprinting for staleness detection
//!
//! # Key Formula
//!
//! E(S) = sum(w_e * |r_e|^2) where r_e = rho_u(x_u) - rho_v(x_v)
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::coherence::{CoherenceEngine, CoherenceConfig};
//!
//! let mut engine = CoherenceEngine::new(CoherenceConfig::default());
//!
//! // Add nodes with state vectors
//! engine.add_node("belief_1", vec![1.0, 0.5, 0.3]);
//! engine.add_node("belief_2", vec![0.9, 0.6, 0.2]);
//!
//! // Add edge with constraint (restriction map)
//! engine.add_edge("belief_1", "belief_2", 1.0, None);
//!
//! // Compute global coherence energy
//! let energy = engine.compute_energy();
//! println!("Total energy: {}", energy.total_energy);
//! ```

use super::energy::{
    compute_norm_sq, compute_residual, CoherenceEnergy, EdgeEnergy, EdgeId, ScopeId,
};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

/// Unique identifier for a node in the sheaf graph
pub type NodeId = String;

/// Errors that can occur in the coherence engine
#[derive(Debug, Error)]
pub enum CoherenceError {
    /// Node not found in the graph
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// Edge not found in the graph
    #[error("Edge not found: {0}")]
    EdgeNotFound(String),

    /// Duplicate node ID
    #[error("Node already exists: {0}")]
    NodeExists(String),

    /// Duplicate edge
    #[error("Edge already exists between {0} and {1}")]
    EdgeExists(String, String),

    /// Dimension mismatch
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Invalid restriction map
    #[error("Invalid restriction map: {0}")]
    InvalidRestrictionMap(String),
}

/// Result type for coherence operations
pub type Result<T> = std::result::Result<T, CoherenceError>;

/// Configuration for the coherence engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceConfig {
    /// Default edge weight when not specified
    pub default_edge_weight: f32,
    /// Parallel threshold (use parallel computation above this edge count)
    pub parallel_threshold: usize,
    /// Whether to cache residuals for incremental updates
    pub cache_residuals: bool,
    /// Maximum cache size (in number of edges)
    pub max_cache_size: usize,
    /// Default state dimension (for identity restriction maps)
    pub default_dimension: usize,
}

impl Default for CoherenceConfig {
    fn default() -> Self {
        Self {
            default_edge_weight: 1.0,
            parallel_threshold: 100,
            cache_residuals: true,
            max_cache_size: 100_000,
            default_dimension: 256,
        }
    }
}

/// State of a node in the sheaf graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    /// Node identifier
    pub id: NodeId,
    /// State vector (stalk of the sheaf)
    pub state: Vec<f32>,
    /// Metadata for filtering and governance
    pub metadata: HashMap<String, String>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Scope/namespace this node belongs to
    pub scope: Option<ScopeId>,
    /// Version for optimistic concurrency
    pub version: u64,
}

impl NodeState {
    /// Create a new node state
    pub fn new(id: impl Into<NodeId>, state: Vec<f32>) -> Self {
        Self {
            id: id.into(),
            state,
            metadata: HashMap::new(),
            updated_at: Utc::now(),
            scope: None,
            version: 1,
        }
    }

    /// Set metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set scope
    pub fn with_scope(mut self, scope: impl Into<ScopeId>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// Get the dimension of the state vector
    #[inline]
    pub fn dimension(&self) -> usize {
        self.state.len()
    }

    /// Compute a fingerprint for this node state
    pub fn fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hasher);
        self.version.hash(&mut hasher);
        // Hash the state bytes
        for val in &self.state {
            val.to_bits().hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// A sheaf node wraps node state with graph connectivity info
#[derive(Debug, Clone)]
pub struct SheafNode {
    /// The node state
    pub state: NodeState,
    /// Incident edge IDs
    pub edges: Vec<EdgeId>,
}

impl SheafNode {
    /// Create a new sheaf node
    pub fn new(state: NodeState) -> Self {
        Self {
            state,
            edges: Vec::new(),
        }
    }

    /// Add an incident edge
    pub fn add_edge(&mut self, edge_id: EdgeId) {
        if !self.edges.contains(&edge_id) {
            self.edges.push(edge_id);
        }
    }

    /// Remove an incident edge
    pub fn remove_edge(&mut self, edge_id: &str) {
        self.edges.retain(|e| e != edge_id);
    }
}

/// Linear restriction map: Ax + b
///
/// Maps a node's state to the shared edge space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestrictionMap {
    /// Linear transformation matrix (row-major, output_dim x input_dim)
    pub matrix: Vec<f32>,
    /// Bias vector
    pub bias: Vec<f32>,
    /// Input dimension (source state dimension)
    pub input_dim: usize,
    /// Output dimension (shared edge space dimension)
    pub output_dim: usize,
}

impl RestrictionMap {
    /// Create an identity restriction map (no transformation)
    pub fn identity(dim: usize) -> Self {
        let mut matrix = vec![0.0; dim * dim];
        for i in 0..dim {
            matrix[i * dim + i] = 1.0;
        }
        Self {
            matrix,
            bias: vec![0.0; dim],
            input_dim: dim,
            output_dim: dim,
        }
    }

    /// Create a projection map that selects specific dimensions
    pub fn projection(input_dim: usize, selected_dims: &[usize]) -> Self {
        let output_dim = selected_dims.len();
        let mut matrix = vec![0.0; output_dim * input_dim];

        for (row, &dim) in selected_dims.iter().enumerate() {
            if dim < input_dim {
                matrix[row * input_dim + dim] = 1.0;
            }
        }

        Self {
            matrix,
            bias: vec![0.0; output_dim],
            input_dim,
            output_dim,
        }
    }

    /// Create a random restriction map (for learned initialization)
    pub fn random(input_dim: usize, output_dim: usize, seed: u64) -> Self {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

        let scale = (2.0 / (input_dim + output_dim) as f32).sqrt();
        let matrix: Vec<f32> = (0..output_dim * input_dim)
            .map(|_| rng.gen_range(-scale..scale))
            .collect();

        Self {
            matrix,
            bias: vec![0.0; output_dim],
            input_dim,
            output_dim,
        }
    }

    /// Apply the restriction map: y = Ax + b
    #[inline]
    pub fn apply(&self, x: &[f32]) -> Vec<f32> {
        debug_assert_eq!(
            x.len(),
            self.input_dim,
            "Input dimension mismatch: expected {}, got {}",
            self.input_dim,
            x.len()
        );

        let mut result = self.bias.clone();

        // Matrix-vector multiplication
        #[cfg(feature = "simd")]
        {
            self.apply_simd(x, &mut result);
        }
        #[cfg(not(feature = "simd"))]
        {
            self.apply_scalar(x, &mut result);
        }

        result
    }

    /// Apply restriction map into pre-allocated buffer (zero allocation hot path)
    #[inline]
    pub fn apply_into(&self, x: &[f32], result: &mut [f32]) {
        debug_assert_eq!(x.len(), self.input_dim);
        debug_assert_eq!(result.len(), self.output_dim);

        result.copy_from_slice(&self.bias);

        #[cfg(feature = "simd")]
        {
            self.apply_simd(x, result);
        }
        #[cfg(not(feature = "simd"))]
        {
            self.apply_scalar(x, result);
        }
    }

    /// Scalar matrix-vector multiplication with loop unrolling
    #[cfg(not(feature = "simd"))]
    #[inline]
    fn apply_scalar(&self, x: &[f32], result: &mut [f32]) {
        // Process 4 rows at a time for ILP
        let row_chunks = self.output_dim / 4;
        let row_rem = self.output_dim % 4;

        for chunk in 0..row_chunks {
            let base = chunk * 4;
            let row0 = base * self.input_dim;
            let row1 = (base + 1) * self.input_dim;
            let row2 = (base + 2) * self.input_dim;
            let row3 = (base + 3) * self.input_dim;

            for col in 0..self.input_dim {
                let xv = x[col];
                result[base] += self.matrix[row0 + col] * xv;
                result[base + 1] += self.matrix[row1 + col] * xv;
                result[base + 2] += self.matrix[row2 + col] * xv;
                result[base + 3] += self.matrix[row3 + col] * xv;
            }
        }

        // Handle remainder rows
        for row in (self.output_dim - row_rem)..self.output_dim {
            let row_offset = row * self.input_dim;
            for col in 0..self.input_dim {
                result[row] += self.matrix[row_offset + col] * x[col];
            }
        }
    }

    /// SIMD-optimized matrix-vector multiplication
    #[cfg(feature = "simd")]
    fn apply_simd(&self, x: &[f32], result: &mut [f32]) {
        use wide::f32x8;

        for row in 0..self.output_dim {
            let row_offset = row * self.input_dim;
            let row_slice = &self.matrix[row_offset..row_offset + self.input_dim];

            let chunks_m = row_slice.chunks_exact(8);
            let chunks_x = x.chunks_exact(8);

            let mut sum = f32x8::ZERO;

            for (chunk_m, chunk_x) in chunks_m.zip(chunks_x) {
                let vm = f32x8::from(<[f32; 8]>::try_from(chunk_m).unwrap());
                let vx = f32x8::from(<[f32; 8]>::try_from(chunk_x).unwrap());
                sum += vm * vx;
            }

            result[row] += sum.reduce_add();

            // Handle remainder
            let remainder_start = (self.input_dim / 8) * 8;
            for col in remainder_start..self.input_dim {
                result[row] += row_slice[col] * x[col];
            }
        }
    }
}

/// An edge encoding a constraint between two nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafEdge {
    /// Edge identifier
    pub id: EdgeId,
    /// Source node
    pub source: NodeId,
    /// Target node
    pub target: NodeId,
    /// Weight for energy calculation
    pub weight: f32,
    /// Restriction map from source to shared space
    pub rho_source: RestrictionMap,
    /// Restriction map from target to shared space
    pub rho_target: RestrictionMap,
    /// Scope this edge belongs to
    pub scope: Option<ScopeId>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl SheafEdge {
    /// Create a new sheaf edge with identity restriction maps
    pub fn new(
        id: impl Into<EdgeId>,
        source: impl Into<NodeId>,
        target: impl Into<NodeId>,
        weight: f32,
        dim: usize,
    ) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            weight,
            rho_source: RestrictionMap::identity(dim),
            rho_target: RestrictionMap::identity(dim),
            scope: None,
            created_at: Utc::now(),
        }
    }

    /// Create edge with custom restriction maps
    pub fn with_restriction_maps(
        id: impl Into<EdgeId>,
        source: impl Into<NodeId>,
        target: impl Into<NodeId>,
        weight: f32,
        rho_source: RestrictionMap,
        rho_target: RestrictionMap,
    ) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            weight,
            rho_source,
            rho_target,
            scope: None,
            created_at: Utc::now(),
        }
    }

    /// Set the scope
    pub fn with_scope(mut self, scope: impl Into<ScopeId>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// Calculate the edge residual: r_e = rho_u(x_u) - rho_v(x_v)
    #[inline]
    pub fn residual(&self, source_state: &[f32], target_state: &[f32]) -> Vec<f32> {
        let projected_source = self.rho_source.apply(source_state);
        let projected_target = self.rho_target.apply(target_state);

        compute_residual(&projected_source, &projected_target)
    }

    /// Calculate weighted residual energy: w_e * |r_e|^2
    #[inline]
    pub fn weighted_residual_energy(&self, source: &[f32], target: &[f32]) -> f32 {
        let r = self.residual(source, target);
        let norm_sq = compute_norm_sq(&r);
        self.weight * norm_sq
    }

    /// Calculate weighted residual energy with pre-allocated buffers (zero allocation)
    /// This is the preferred method for hot paths in batch computation.
    #[inline]
    pub fn weighted_residual_energy_into(
        &self,
        source: &[f32],
        target: &[f32],
        source_buf: &mut [f32],
        target_buf: &mut [f32],
    ) -> f32 {
        self.rho_source.apply_into(source, source_buf);
        self.rho_target.apply_into(target, target_buf);

        // Compute norm squared directly without allocating residual
        super::energy::compute_residual_norm_sq(source_buf, target_buf) * self.weight
    }

    /// Create an EdgeEnergy from this edge
    pub fn to_edge_energy(&self, source_state: &[f32], target_state: &[f32]) -> EdgeEnergy {
        let residual = self.residual(source_state, target_state);
        EdgeEnergy::new(
            self.id.clone(),
            self.source.clone(),
            self.target.clone(),
            residual,
            self.weight,
        )
    }
}

/// Cached residual for incremental computation
#[derive(Debug, Clone)]
struct CachedResidual {
    residual: Vec<f32>,
    energy: f32,
    source_version: u64,
    target_version: u64,
}

/// The main coherence computation engine
pub struct CoherenceEngine {
    /// Configuration
    config: CoherenceConfig,
    /// Nodes in the graph (thread-safe)
    nodes: DashMap<NodeId, SheafNode>,
    /// Edges in the graph (thread-safe)
    edges: DashMap<EdgeId, SheafEdge>,
    /// Edge-to-scope mapping
    edge_scopes: DashMap<EdgeId, ScopeId>,
    /// Cached residuals for incremental computation
    residual_cache: DashMap<EdgeId, CachedResidual>,
    /// Global fingerprint (changes on any modification)
    global_fingerprint: AtomicU64,
    /// Last computed energy
    last_energy: RwLock<Option<CoherenceEnergy>>,
    /// Statistics
    stats: RwLock<EngineStats>,
}

/// Statistics about engine operation
#[derive(Debug, Clone, Default)]
struct EngineStats {
    node_count: usize,
    edge_count: usize,
    cache_hits: u64,
    cache_misses: u64,
    full_computations: u64,
    incremental_updates: u64,
}

impl CoherenceEngine {
    /// Create a new coherence engine with configuration
    pub fn new(config: CoherenceConfig) -> Self {
        Self {
            config,
            nodes: DashMap::new(),
            edges: DashMap::new(),
            edge_scopes: DashMap::new(),
            residual_cache: DashMap::new(),
            global_fingerprint: AtomicU64::new(0),
            last_energy: RwLock::new(None),
            stats: RwLock::new(EngineStats::default()),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&self, id: impl Into<NodeId>, state: Vec<f32>) -> Result<()> {
        let id = id.into();

        if self.nodes.contains_key(&id) {
            return Err(CoherenceError::NodeExists(id));
        }

        let node_state = NodeState::new(id.clone(), state);
        let node = SheafNode::new(node_state);

        self.nodes.insert(id, node);
        self.increment_fingerprint();
        self.stats.write().node_count += 1;

        Ok(())
    }

    /// Add a node with full state
    pub fn add_node_state(&self, state: NodeState) -> Result<()> {
        let id = state.id.clone();

        if self.nodes.contains_key(&id) {
            return Err(CoherenceError::NodeExists(id));
        }

        let node = SheafNode::new(state);
        self.nodes.insert(id, node);
        self.increment_fingerprint();
        self.stats.write().node_count += 1;

        Ok(())
    }

    /// Update a node's state
    pub fn update_node(&self, id: &str, new_state: Vec<f32>) -> Result<()> {
        let mut node = self
            .nodes
            .get_mut(id)
            .ok_or_else(|| CoherenceError::NodeNotFound(id.to_string()))?;

        node.state.state = new_state;
        node.state.updated_at = Utc::now();
        node.state.version += 1;

        self.increment_fingerprint();
        self.invalidate_edges_for_node(id);

        Ok(())
    }

    /// Remove a node (and all incident edges)
    pub fn remove_node(&self, id: &str) -> Result<NodeState> {
        let (_, node) = self
            .nodes
            .remove(id)
            .ok_or_else(|| CoherenceError::NodeNotFound(id.to_string()))?;

        // Remove all incident edges
        for edge_id in &node.edges {
            self.edges.remove(edge_id);
            self.edge_scopes.remove(edge_id);
            self.residual_cache.remove(edge_id);
            self.stats.write().edge_count = self.stats.read().edge_count.saturating_sub(1);
        }

        self.increment_fingerprint();
        self.stats.write().node_count = self.stats.read().node_count.saturating_sub(1);

        Ok(node.state)
    }

    /// Add an edge between two nodes
    pub fn add_edge(
        &self,
        source: impl Into<NodeId>,
        target: impl Into<NodeId>,
        weight: f32,
        scope: Option<ScopeId>,
    ) -> Result<EdgeId> {
        let source = source.into();
        let target = target.into();

        // Check nodes exist
        if !self.nodes.contains_key(&source) {
            return Err(CoherenceError::NodeNotFound(source));
        }
        if !self.nodes.contains_key(&target) {
            return Err(CoherenceError::NodeNotFound(target));
        }

        // Get dimension
        let dim = self
            .nodes
            .get(&source)
            .map(|n| n.state.dimension())
            .unwrap_or(self.config.default_dimension);

        // Generate edge ID
        let edge_id = format!("{}:{}", source, target);

        if self.edges.contains_key(&edge_id) {
            return Err(CoherenceError::EdgeExists(source, target));
        }

        let mut edge = SheafEdge::new(&edge_id, &source, &target, weight, dim);
        if let Some(s) = scope.clone() {
            edge = edge.with_scope(s.clone());
            self.edge_scopes.insert(edge_id.clone(), s);
        }

        self.edges.insert(edge_id.clone(), edge);

        // Update node edge lists
        if let Some(mut node) = self.nodes.get_mut(&source) {
            node.add_edge(edge_id.clone());
        }
        if let Some(mut node) = self.nodes.get_mut(&target) {
            node.add_edge(edge_id.clone());
        }

        self.increment_fingerprint();
        self.stats.write().edge_count += 1;

        Ok(edge_id)
    }

    /// Add an edge with custom restriction maps
    pub fn add_edge_with_maps(
        &self,
        source: impl Into<NodeId>,
        target: impl Into<NodeId>,
        weight: f32,
        rho_source: RestrictionMap,
        rho_target: RestrictionMap,
        scope: Option<ScopeId>,
    ) -> Result<EdgeId> {
        let source = source.into();
        let target = target.into();

        // Check nodes exist
        if !self.nodes.contains_key(&source) {
            return Err(CoherenceError::NodeNotFound(source));
        }
        if !self.nodes.contains_key(&target) {
            return Err(CoherenceError::NodeNotFound(target));
        }

        // Generate edge ID
        let edge_id = format!("{}:{}", source, target);

        if self.edges.contains_key(&edge_id) {
            return Err(CoherenceError::EdgeExists(source, target));
        }

        let mut edge = SheafEdge::with_restriction_maps(
            &edge_id, &source, &target, weight, rho_source, rho_target,
        );
        if let Some(s) = scope.clone() {
            edge = edge.with_scope(s.clone());
            self.edge_scopes.insert(edge_id.clone(), s);
        }

        self.edges.insert(edge_id.clone(), edge);

        // Update node edge lists
        if let Some(mut node) = self.nodes.get_mut(&source) {
            node.add_edge(edge_id.clone());
        }
        if let Some(mut node) = self.nodes.get_mut(&target) {
            node.add_edge(edge_id.clone());
        }

        self.increment_fingerprint();
        self.stats.write().edge_count += 1;

        Ok(edge_id)
    }

    /// Remove an edge
    pub fn remove_edge(&self, edge_id: &str) -> Result<SheafEdge> {
        let (_, edge) = self
            .edges
            .remove(edge_id)
            .ok_or_else(|| CoherenceError::EdgeNotFound(edge_id.to_string()))?;

        // Update node edge lists
        if let Some(mut node) = self.nodes.get_mut(&edge.source) {
            node.remove_edge(edge_id);
        }
        if let Some(mut node) = self.nodes.get_mut(&edge.target) {
            node.remove_edge(edge_id);
        }

        self.edge_scopes.remove(edge_id);
        self.residual_cache.remove(edge_id);
        self.increment_fingerprint();
        self.stats.write().edge_count = self.stats.read().edge_count.saturating_sub(1);

        Ok(edge)
    }

    /// Compute global coherence energy: E(S) = sum(w_e * |r_e|^2)
    pub fn compute_energy(&self) -> CoherenceEnergy {
        let fingerprint = self.current_fingerprint();

        // Check if we have a valid cached result
        {
            let last = self.last_energy.read();
            if let Some(ref energy) = *last {
                if energy.fingerprint == fingerprint {
                    return energy.clone();
                }
            }
        }

        // Compute fresh
        let edge_energies = self.compute_all_edge_energies();
        let scope_mapping = self.get_scope_mapping();
        let node_count = self.nodes.len();

        let energy = CoherenceEnergy::new(edge_energies, &scope_mapping, node_count, fingerprint);

        // Cache result
        *self.last_energy.write() = Some(energy.clone());
        self.stats.write().full_computations += 1;

        energy
    }

    /// Compute energy for a specific edge
    pub fn compute_edge_energy(&self, edge_id: &str) -> Result<EdgeEnergy> {
        let edge = self
            .edges
            .get(edge_id)
            .ok_or_else(|| CoherenceError::EdgeNotFound(edge_id.to_string()))?;

        let source_node = self
            .nodes
            .get(&edge.source)
            .ok_or_else(|| CoherenceError::NodeNotFound(edge.source.clone()))?;
        let target_node = self
            .nodes
            .get(&edge.target)
            .ok_or_else(|| CoherenceError::NodeNotFound(edge.target.clone()))?;

        Ok(edge.to_edge_energy(&source_node.state.state, &target_node.state.state))
    }

    /// Get edges incident to a node
    pub fn edges_incident_to(&self, node_id: &str) -> Vec<EdgeId> {
        self.nodes
            .get(node_id)
            .map(|n| n.edges.clone())
            .unwrap_or_default()
    }

    /// Get the current fingerprint
    #[inline]
    pub fn current_fingerprint(&self) -> String {
        self.global_fingerprint.load(Ordering::SeqCst).to_string()
    }

    /// Get node count
    #[inline]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get edge count
    #[inline]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Check if the engine has any nodes
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &str) -> Option<NodeState> {
        self.nodes.get(id).map(|n| n.state.clone())
    }

    /// Get an edge by ID
    pub fn get_edge(&self, id: &str) -> Option<SheafEdge> {
        self.edges.get(id).map(|e| e.clone())
    }

    // Private methods

    fn compute_all_edge_energies(&self) -> HashMap<EdgeId, EdgeEnergy> {
        let edge_count = self.edges.len();

        // Pre-allocate HashMap with known capacity
        let mut result = HashMap::with_capacity(edge_count);

        // Collect edges for processing
        let edges: Vec<_> = self.edges.iter().collect();

        // Choose parallel or sequential based on size
        #[cfg(feature = "parallel")]
        if edge_count >= self.config.parallel_threshold {
            let parallel_results: Vec<_> = edges
                .par_iter()
                .filter_map(|edge_ref| {
                    let edge = edge_ref.value();
                    self.compute_edge_energy_internal(edge)
                        .map(|e| (edge.id.clone(), e))
                })
                .collect();

            result.extend(parallel_results);
            return result;
        }

        // Sequential path - use pre-allocated buffers for zero-allocation hot loop
        let state_dim = self.config.default_dimension;
        let mut source_buf = vec![0.0f32; state_dim];
        let mut target_buf = vec![0.0f32; state_dim];

        for edge_ref in &edges {
            let edge = edge_ref.value();
            if let Some(energy) =
                self.compute_edge_energy_with_buffers(edge, &mut source_buf, &mut target_buf)
            {
                result.insert(edge.id.clone(), energy);
            }
        }

        result
    }

    /// Compute edge energy with pre-allocated buffers (zero allocation hot path)
    #[inline]
    fn compute_edge_energy_with_buffers(
        &self,
        edge: &SheafEdge,
        source_buf: &mut Vec<f32>,
        target_buf: &mut Vec<f32>,
    ) -> Option<EdgeEnergy> {
        let source_node = self.nodes.get(&edge.source)?;
        let target_node = self.nodes.get(&edge.target)?;

        let source_state = &source_node.state.state;
        let target_state = &target_node.state.state;

        // Resize buffers if needed
        let out_dim = edge.rho_source.output_dim;
        if source_buf.len() < out_dim {
            source_buf.resize(out_dim, 0.0);
            target_buf.resize(out_dim, 0.0);
        }

        // Use zero-allocation path
        let energy = edge.weighted_residual_energy_into(
            source_state,
            target_state,
            &mut source_buf[..out_dim],
            &mut target_buf[..out_dim],
        );

        // Create lightweight EdgeEnergy without storing residual
        Some(EdgeEnergy::new_lightweight(
            edge.id.clone(),
            edge.source.clone(),
            edge.target.clone(),
            energy / edge.weight, // Recover norm_sq
            edge.weight,
        ))
    }

    fn compute_edge_energy_internal(&self, edge: &SheafEdge) -> Option<EdgeEnergy> {
        let source_node = self.nodes.get(&edge.source)?;
        let target_node = self.nodes.get(&edge.target)?;

        // Check cache if enabled
        if self.config.cache_residuals {
            if let Some(cached) = self.residual_cache.get(&edge.id) {
                if cached.source_version == source_node.state.version
                    && cached.target_version == target_node.state.version
                {
                    // Cache hit
                    return Some(EdgeEnergy::new(
                        edge.id.clone(),
                        edge.source.clone(),
                        edge.target.clone(),
                        cached.residual.clone(),
                        edge.weight,
                    ));
                }
            }
        }

        // Compute fresh
        let energy = edge.to_edge_energy(&source_node.state.state, &target_node.state.state);

        // Update cache
        if self.config.cache_residuals {
            let cached = CachedResidual {
                residual: energy.residual.clone(),
                energy: energy.energy,
                source_version: source_node.state.version,
                target_version: target_node.state.version,
            };
            self.residual_cache.insert(edge.id.clone(), cached);
        }

        Some(energy)
    }

    fn get_scope_mapping(&self) -> HashMap<EdgeId, ScopeId> {
        self.edge_scopes
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    fn increment_fingerprint(&self) {
        self.global_fingerprint.fetch_add(1, Ordering::SeqCst);
    }

    fn invalidate_edges_for_node(&self, node_id: &str) {
        if let Some(node) = self.nodes.get(node_id) {
            for edge_id in &node.edges {
                self.residual_cache.remove(edge_id);
            }
        }
    }
}

impl Default for CoherenceEngine {
    fn default() -> Self {
        Self::new(CoherenceConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = CoherenceEngine::default();
        assert!(engine.is_empty());
        assert_eq!(engine.node_count(), 0);
        assert_eq!(engine.edge_count(), 0);
    }

    #[test]
    fn test_add_nodes() {
        let engine = CoherenceEngine::default();

        engine.add_node("n1", vec![1.0, 0.5]).unwrap();
        engine.add_node("n2", vec![0.9, 0.6]).unwrap();

        assert_eq!(engine.node_count(), 2);

        // Duplicate should fail
        let result = engine.add_node("n1", vec![0.0, 0.0]);
        assert!(matches!(result, Err(CoherenceError::NodeExists(_))));
    }

    #[test]
    fn test_add_edges() {
        let engine = CoherenceEngine::default();

        engine.add_node("n1", vec![1.0, 0.5]).unwrap();
        engine.add_node("n2", vec![0.9, 0.6]).unwrap();

        let edge_id = engine.add_edge("n1", "n2", 1.0, None).unwrap();
        assert_eq!(edge_id, "n1:n2");
        assert_eq!(engine.edge_count(), 1);

        // Duplicate should fail
        let result = engine.add_edge("n1", "n2", 2.0, None);
        assert!(matches!(result, Err(CoherenceError::EdgeExists(_, _))));
    }

    #[test]
    fn test_compute_energy() {
        let engine = CoherenceEngine::default();

        // Identical states = zero energy
        engine.add_node("n1", vec![1.0, 0.0]).unwrap();
        engine.add_node("n2", vec![1.0, 0.0]).unwrap();
        engine.add_edge("n1", "n2", 1.0, None).unwrap();

        let energy = engine.compute_energy();
        assert_eq!(energy.total_energy, 0.0);
        assert_eq!(energy.edge_count, 1);
    }

    #[test]
    fn test_compute_energy_nonzero() {
        let engine = CoherenceEngine::default();

        // Different states = nonzero energy
        engine.add_node("n1", vec![1.0, 0.0]).unwrap();
        engine.add_node("n2", vec![0.0, 1.0]).unwrap();
        engine.add_edge("n1", "n2", 1.0, None).unwrap();

        let energy = engine.compute_energy();
        // residual = [1.0, -1.0], |r|^2 = 2.0, energy = 1.0 * 2.0 = 2.0
        assert_eq!(energy.total_energy, 2.0);
    }

    #[test]
    fn test_update_node() {
        let engine = CoherenceEngine::default();

        engine.add_node("n1", vec![1.0, 0.0]).unwrap();
        engine.add_node("n2", vec![0.0, 1.0]).unwrap();
        engine.add_edge("n1", "n2", 1.0, None).unwrap();

        let energy1 = engine.compute_energy();
        assert!(energy1.total_energy > 0.0);

        // Update to match
        engine.update_node("n2", vec![1.0, 0.0]).unwrap();

        let energy2 = engine.compute_energy();
        assert_eq!(energy2.total_energy, 0.0);
    }

    #[test]
    fn test_restriction_map_identity() {
        let rho = RestrictionMap::identity(3);
        let x = vec![1.0, 2.0, 3.0];
        let y = rho.apply(&x);

        assert_eq!(y, x);
    }

    #[test]
    fn test_restriction_map_projection() {
        let rho = RestrictionMap::projection(4, &[0, 2]);
        let x = vec![1.0, 2.0, 3.0, 4.0];
        let y = rho.apply(&x);

        assert_eq!(y.len(), 2);
        assert_eq!(y[0], 1.0);
        assert_eq!(y[1], 3.0);
    }

    #[test]
    fn test_sheaf_edge_residual() {
        let edge = SheafEdge::new("e1", "n1", "n2", 2.0, 2);

        let source = vec![1.0, 0.5];
        let target = vec![0.5, 0.5];

        let residual = edge.residual(&source, &target);
        assert_eq!(residual.len(), 2);
        assert!((residual[0] - 0.5).abs() < 1e-6);
        assert!((residual[1] - 0.0).abs() < 1e-6);

        let energy = edge.weighted_residual_energy(&source, &target);
        // |r|^2 = 0.25, energy = 2.0 * 0.25 = 0.5
        assert!((energy - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_scoped_edges() {
        let engine = CoherenceEngine::default();

        engine.add_node("n1", vec![1.0]).unwrap();
        engine.add_node("n2", vec![0.5]).unwrap();
        engine.add_node("n3", vec![0.3]).unwrap();

        engine
            .add_edge("n1", "n2", 1.0, Some("scope_a".to_string()))
            .unwrap();
        engine
            .add_edge("n2", "n3", 1.0, Some("scope_b".to_string()))
            .unwrap();

        let energy = engine.compute_energy();

        assert_eq!(energy.scope_energies.len(), 2);
        assert!(energy.scope_energies.contains_key("scope_a"));
        assert!(energy.scope_energies.contains_key("scope_b"));
    }

    #[test]
    fn test_fingerprint_changes() {
        let engine = CoherenceEngine::default();

        let fp1 = engine.current_fingerprint();

        engine.add_node("n1", vec![1.0]).unwrap();
        let fp2 = engine.current_fingerprint();
        assert_ne!(fp1, fp2);

        engine.update_node("n1", vec![2.0]).unwrap();
        let fp3 = engine.current_fingerprint();
        assert_ne!(fp2, fp3);
    }

    #[test]
    fn test_remove_node() {
        let engine = CoherenceEngine::default();

        engine.add_node("n1", vec![1.0]).unwrap();
        engine.add_node("n2", vec![0.5]).unwrap();
        engine.add_edge("n1", "n2", 1.0, None).unwrap();

        assert_eq!(engine.node_count(), 2);
        assert_eq!(engine.edge_count(), 1);

        engine.remove_node("n1").unwrap();

        assert_eq!(engine.node_count(), 1);
        assert_eq!(engine.edge_count(), 0);
    }
}
