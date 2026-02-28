//! SheafEdge: Constraint between nodes with restriction maps
//!
//! An edge in the sheaf graph encodes a constraint between two nodes.
//! The constraint is expressed via two restriction maps:
//!
//! - `rho_source`: Projects the source state to the shared comparison space
//! - `rho_target`: Projects the target state to the shared comparison space
//!
//! The **residual** at an edge is the difference between these projections:
//! ```text
//! r_e = rho_source(x_source) - rho_target(x_target)
//! ```
//!
//! The **weighted residual energy** contributes to global coherence:
//! ```text
//! E_e = weight * ||r_e||^2
//! ```
//!
//! # Performance Optimization
//!
//! Thread-local scratch buffers are used to eliminate per-edge allocations
//! in hot paths. Use `residual_norm_squared_no_alloc` for allocation-free
//! energy computation.

use super::node::NodeId;
use super::restriction::RestrictionMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use uuid::Uuid;

/// Default initial capacity for scratch buffers
const DEFAULT_SCRATCH_CAPACITY: usize = 256;

/// Thread-local scratch buffers for allocation-free edge computations
///
/// These buffers are reused across multiple edge energy calculations
/// to avoid per-edge Vec allocations in hot paths.
struct EdgeScratch {
    /// Buffer for projected source state
    projected_source: Vec<f32>,
    /// Buffer for projected target state
    projected_target: Vec<f32>,
    /// Buffer for residual vector (source - target)
    residual: Vec<f32>,
}

impl EdgeScratch {
    /// Create a new scratch buffer with the given initial capacity
    fn new(capacity: usize) -> Self {
        Self {
            projected_source: Vec::with_capacity(capacity),
            projected_target: Vec::with_capacity(capacity),
            residual: Vec::with_capacity(capacity),
        }
    }

    /// Ensure all buffers have at least the required capacity and set length
    ///
    /// This resizes the vectors to exactly `dim` elements, growing capacity
    /// if needed but never shrinking.
    #[inline]
    fn prepare(&mut self, dim: usize) {
        // Resize to exact dimension, reserving more capacity if needed
        if self.projected_source.capacity() < dim {
            self.projected_source
                .reserve(dim - self.projected_source.len());
        }
        if self.projected_target.capacity() < dim {
            self.projected_target
                .reserve(dim - self.projected_target.len());
        }
        if self.residual.capacity() < dim {
            self.residual.reserve(dim - self.residual.len());
        }

        // Resize to exact length (fills with 0.0 if growing)
        self.projected_source.resize(dim, 0.0);
        self.projected_target.resize(dim, 0.0);
        self.residual.resize(dim, 0.0);
    }
}

thread_local! {
    /// Thread-local scratch buffers for edge computations
    static SCRATCH: RefCell<EdgeScratch> = RefCell::new(EdgeScratch::new(DEFAULT_SCRATCH_CAPACITY));
}

/// Unique identifier for an edge
pub type EdgeId = Uuid;

/// An edge encoding a constraint between two nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafEdge {
    /// Unique edge identifier
    pub id: EdgeId,
    /// Source node identifier
    pub source: NodeId,
    /// Target node identifier
    pub target: NodeId,
    /// Weight for energy calculation (importance of this constraint)
    pub weight: f32,
    /// Restriction map from source to shared comparison space
    pub rho_source: RestrictionMap,
    /// Restriction map from target to shared comparison space
    pub rho_target: RestrictionMap,
    /// Edge type/label for filtering
    pub edge_type: Option<String>,
    /// Namespace for multi-tenant isolation
    pub namespace: Option<String>,
    /// Arbitrary metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl SheafEdge {
    /// Create a new edge with identity restriction maps
    ///
    /// This means both source and target states must match exactly in the
    /// given dimension for the edge to be coherent.
    pub fn identity(source: NodeId, target: NodeId, dim: usize) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            source,
            target,
            weight: 1.0,
            rho_source: RestrictionMap::identity(dim),
            rho_target: RestrictionMap::identity(dim),
            edge_type: None,
            namespace: None,
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new edge with custom restriction maps
    pub fn with_restrictions(
        source: NodeId,
        target: NodeId,
        rho_source: RestrictionMap,
        rho_target: RestrictionMap,
    ) -> Self {
        debug_assert_eq!(
            rho_source.output_dim(),
            rho_target.output_dim(),
            "Restriction maps must have same output dimension"
        );

        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            source,
            target,
            weight: 1.0,
            rho_source,
            rho_target,
            edge_type: None,
            namespace: None,
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Calculate the edge residual (local mismatch)
    ///
    /// The residual is the difference between the projected source and target states:
    /// ```text
    /// r_e = rho_source(x_source) - rho_target(x_target)
    /// ```
    ///
    /// # SIMD Optimization
    ///
    /// The subtraction is performed using SIMD-friendly patterns.
    #[inline]
    pub fn residual(&self, source_state: &[f32], target_state: &[f32]) -> Vec<f32> {
        let projected_source = self.rho_source.apply(source_state);
        let projected_target = self.rho_target.apply(target_state);

        // SIMD-friendly subtraction
        projected_source
            .iter()
            .zip(projected_target.iter())
            .map(|(&a, &b)| a - b)
            .collect()
    }

    /// Calculate the residual norm squared
    ///
    /// This is ||r_e||^2 without the weight factor.
    ///
    /// # SIMD Optimization
    ///
    /// Uses 4-lane accumulation for better vectorization.
    ///
    /// # Note
    ///
    /// This method allocates temporary vectors. For hot paths, prefer
    /// `residual_norm_squared_no_alloc` which uses thread-local scratch buffers.
    #[inline]
    pub fn residual_norm_squared(&self, source_state: &[f32], target_state: &[f32]) -> f32 {
        let residual = self.residual(source_state, target_state);

        // SIMD-friendly: process 4 elements at a time using chunks_exact
        let chunks = residual.chunks_exact(4);
        let remainder = chunks.remainder();

        let mut acc = [0.0f32; 4];
        for chunk in chunks {
            acc[0] += chunk[0] * chunk[0];
            acc[1] += chunk[1] * chunk[1];
            acc[2] += chunk[2] * chunk[2];
            acc[3] += chunk[3] * chunk[3];
        }

        let mut sum = acc[0] + acc[1] + acc[2] + acc[3];
        for &r in remainder {
            sum += r * r;
        }
        sum
    }

    /// Calculate the residual norm squared without allocation
    ///
    /// This is ||r_e||^2 without the weight factor, using thread-local
    /// scratch buffers to avoid per-call allocations.
    ///
    /// # Performance
    ///
    /// This method is optimized for hot paths where many edges are processed
    /// in sequence. It reuses thread-local buffers to eliminate the 2-3 Vec
    /// allocations that would otherwise occur per edge.
    ///
    /// # SIMD Optimization
    ///
    /// Uses 4-lane accumulation for better vectorization.
    ///
    /// # Thread Safety
    ///
    /// Uses thread-local storage, so it's safe to call from multiple threads
    /// concurrently (each thread has its own scratch buffers).
    #[inline]
    pub fn residual_norm_squared_no_alloc(
        &self,
        source_state: &[f32],
        target_state: &[f32],
    ) -> f32 {
        let dim = self.comparison_dim();

        SCRATCH.with(|scratch| {
            let mut scratch = scratch.borrow_mut();
            scratch.prepare(dim);

            // Apply restriction maps into scratch buffers
            self.rho_source
                .apply_into(source_state, &mut scratch.projected_source);
            self.rho_target
                .apply_into(target_state, &mut scratch.projected_target);

            // Compute residual in-place: r = projected_source - projected_target
            for i in 0..dim {
                scratch.residual[i] = scratch.projected_source[i] - scratch.projected_target[i];
            }

            // SIMD-friendly: compute norm squared with 4-lane accumulation
            let chunks = scratch.residual[..dim].chunks_exact(4);
            let remainder = chunks.remainder();

            let mut acc = [0.0f32; 4];
            for chunk in chunks {
                acc[0] += chunk[0] * chunk[0];
                acc[1] += chunk[1] * chunk[1];
                acc[2] += chunk[2] * chunk[2];
                acc[3] += chunk[3] * chunk[3];
            }

            let mut sum = acc[0] + acc[1] + acc[2] + acc[3];
            for &r in remainder {
                sum += r * r;
            }
            sum
        })
    }

    /// Calculate weighted residual energy without allocation
    ///
    /// This is the contribution of this edge to the global coherence energy:
    /// ```text
    /// E_e = weight * ||r_e||^2
    /// ```
    ///
    /// Uses thread-local scratch buffers to avoid per-call allocations.
    /// Preferred over `weighted_residual_energy` in hot paths.
    #[inline]
    pub fn weighted_residual_energy_no_alloc(
        &self,
        source_state: &[f32],
        target_state: &[f32],
    ) -> f32 {
        self.weight * self.residual_norm_squared_no_alloc(source_state, target_state)
    }

    /// Calculate weighted residual energy
    ///
    /// This is the contribution of this edge to the global coherence energy:
    /// ```text
    /// E_e = weight * ||r_e||^2
    /// ```
    #[inline]
    pub fn weighted_residual_energy(&self, source_state: &[f32], target_state: &[f32]) -> f32 {
        self.weight * self.residual_norm_squared(source_state, target_state)
    }

    /// Calculate residual energy and return both the energy and residual vector
    ///
    /// This is more efficient when you need both values.
    #[inline]
    pub fn residual_with_energy(
        &self,
        source_state: &[f32],
        target_state: &[f32],
    ) -> (Vec<f32>, f32) {
        let residual = self.residual(source_state, target_state);

        // SIMD-friendly: process 4 elements at a time using chunks_exact
        let chunks = residual.chunks_exact(4);
        let remainder = chunks.remainder();

        let mut acc = [0.0f32; 4];
        for chunk in chunks {
            acc[0] += chunk[0] * chunk[0];
            acc[1] += chunk[1] * chunk[1];
            acc[2] += chunk[2] * chunk[2];
            acc[3] += chunk[3] * chunk[3];
        }

        let mut norm_sq = acc[0] + acc[1] + acc[2] + acc[3];
        for &r in remainder {
            norm_sq += r * r;
        }
        let energy = self.weight * norm_sq;

        (residual, energy)
    }

    /// Get the output dimension of the restriction maps (comparison space dimension)
    #[inline]
    pub fn comparison_dim(&self) -> usize {
        self.rho_source.output_dim()
    }

    /// Check if this edge is coherent (residual below threshold)
    #[inline]
    pub fn is_coherent(&self, source_state: &[f32], target_state: &[f32], threshold: f32) -> bool {
        self.residual_norm_squared(source_state, target_state) <= threshold * threshold
    }

    /// Update the weight
    pub fn set_weight(&mut self, weight: f32) {
        self.weight = weight;
        self.updated_at = Utc::now();
    }

    /// Update the restriction maps
    pub fn set_restrictions(&mut self, rho_source: RestrictionMap, rho_target: RestrictionMap) {
        debug_assert_eq!(
            rho_source.output_dim(),
            rho_target.output_dim(),
            "Restriction maps must have same output dimension"
        );
        self.rho_source = rho_source;
        self.rho_target = rho_target;
        self.updated_at = Utc::now();
    }

    /// Compute content hash for fingerprinting
    pub fn content_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hasher);
        self.source.hash(&mut hasher);
        self.target.hash(&mut hasher);
        self.weight.to_bits().hash(&mut hasher);
        hasher.finish()
    }
}

/// Builder for constructing SheafEdge instances
#[derive(Debug)]
pub struct SheafEdgeBuilder {
    id: Option<EdgeId>,
    source: NodeId,
    target: NodeId,
    weight: f32,
    rho_source: Option<RestrictionMap>,
    rho_target: Option<RestrictionMap>,
    edge_type: Option<String>,
    namespace: Option<String>,
    metadata: HashMap<String, serde_json::Value>,
}

impl SheafEdgeBuilder {
    /// Create a new builder with required source and target nodes
    pub fn new(source: NodeId, target: NodeId) -> Self {
        Self {
            id: None,
            source,
            target,
            weight: 1.0,
            rho_source: None,
            rho_target: None,
            edge_type: None,
            namespace: None,
            metadata: HashMap::new(),
        }
    }

    /// Set a custom edge ID
    pub fn id(mut self, id: EdgeId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the weight
    pub fn weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }

    /// Set both restriction maps to identity (states must match exactly)
    pub fn identity_restrictions(mut self, dim: usize) -> Self {
        self.rho_source = Some(RestrictionMap::identity(dim));
        self.rho_target = Some(RestrictionMap::identity(dim));
        self
    }

    /// Set the source restriction map
    pub fn rho_source(mut self, rho: RestrictionMap) -> Self {
        self.rho_source = Some(rho);
        self
    }

    /// Set the target restriction map
    pub fn rho_target(mut self, rho: RestrictionMap) -> Self {
        self.rho_target = Some(rho);
        self
    }

    /// Set both restriction maps at once
    pub fn restrictions(mut self, source: RestrictionMap, target: RestrictionMap) -> Self {
        debug_assert_eq!(
            source.output_dim(),
            target.output_dim(),
            "Restriction maps must have same output dimension"
        );
        self.rho_source = Some(source);
        self.rho_target = Some(target);
        self
    }

    /// Set the edge type
    pub fn edge_type(mut self, edge_type: impl Into<String>) -> Self {
        self.edge_type = Some(edge_type.into());
        self
    }

    /// Set the namespace
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Build the edge
    ///
    /// # Panics
    ///
    /// Panics if restriction maps were not provided.
    pub fn build(self) -> SheafEdge {
        let rho_source = self.rho_source.expect("Source restriction map is required");
        let rho_target = self.rho_target.expect("Target restriction map is required");

        debug_assert_eq!(
            rho_source.output_dim(),
            rho_target.output_dim(),
            "Restriction maps must have same output dimension"
        );

        let now = Utc::now();
        SheafEdge {
            id: self.id.unwrap_or_else(Uuid::new_v4),
            source: self.source,
            target: self.target,
            weight: self.weight,
            rho_source,
            rho_target,
            edge_type: self.edge_type,
            namespace: self.namespace,
            metadata: self.metadata,
            created_at: now,
            updated_at: now,
        }
    }

    /// Try to build the edge, returning an error if restrictions are missing
    pub fn try_build(self) -> Result<SheafEdge, &'static str> {
        let rho_source = self
            .rho_source
            .ok_or("Source restriction map is required")?;
        let rho_target = self
            .rho_target
            .ok_or("Target restriction map is required")?;

        if rho_source.output_dim() != rho_target.output_dim() {
            return Err("Restriction maps must have same output dimension");
        }

        let now = Utc::now();
        Ok(SheafEdge {
            id: self.id.unwrap_or_else(Uuid::new_v4),
            source: self.source,
            target: self.target,
            weight: self.weight,
            rho_source,
            rho_target,
            edge_type: self.edge_type,
            namespace: self.namespace,
            metadata: self.metadata,
            created_at: now,
            updated_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_nodes() -> (NodeId, NodeId) {
        (Uuid::new_v4(), Uuid::new_v4())
    }

    #[test]
    fn test_identity_edge() {
        let (source, target) = make_test_nodes();
        let edge = SheafEdge::identity(source, target, 3);

        assert_eq!(edge.source, source);
        assert_eq!(edge.target, target);
        assert_eq!(edge.weight, 1.0);
        assert_eq!(edge.comparison_dim(), 3);
    }

    #[test]
    fn test_identity_residual_matching() {
        let (source, target) = make_test_nodes();
        let edge = SheafEdge::identity(source, target, 3);

        let source_state = vec![1.0, 2.0, 3.0];
        let target_state = vec![1.0, 2.0, 3.0];

        let residual = edge.residual(&source_state, &target_state);
        assert!(residual.iter().all(|&x| x.abs() < 1e-10));
        assert!(edge.residual_norm_squared(&source_state, &target_state) < 1e-10);
    }

    #[test]
    fn test_identity_residual_mismatch() {
        let (source, target) = make_test_nodes();
        let edge = SheafEdge::identity(source, target, 3);

        let source_state = vec![1.0, 2.0, 3.0];
        let target_state = vec![2.0, 2.0, 3.0]; // Differs by 1 in first component

        let residual = edge.residual(&source_state, &target_state);
        assert_eq!(residual, vec![-1.0, 0.0, 0.0]);
        assert!((edge.residual_norm_squared(&source_state, &target_state) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_weighted_energy() {
        let (source, target) = make_test_nodes();
        let mut edge = SheafEdge::identity(source, target, 2);
        edge.set_weight(2.0);

        let source_state = vec![1.0, 0.0];
        let target_state = vec![0.0, 0.0]; // Residual is [1, 0], norm^2 = 1

        let energy = edge.weighted_residual_energy(&source_state, &target_state);
        assert!((energy - 2.0).abs() < 1e-10); // weight * 1 = 2
    }

    #[test]
    fn test_projection_restriction() {
        let (source, target) = make_test_nodes();

        // Source: 4D, project to first 2 dims
        // Target: 2D, identity
        let rho_source = RestrictionMap::projection(vec![0, 1], 4);
        let rho_target = RestrictionMap::identity(2);

        let edge = SheafEdge::with_restrictions(source, target, rho_source, rho_target);

        let source_state = vec![1.0, 2.0, 100.0, 200.0]; // Extra dims ignored
        let target_state = vec![1.0, 2.0];

        let residual = edge.residual(&source_state, &target_state);
        assert!(residual.iter().all(|&x| x.abs() < 1e-10));
    }

    #[test]
    fn test_diagonal_restriction() {
        let (source, target) = make_test_nodes();

        // Source scaled by [2, 2], target by [1, 1]
        // For coherence: 2*source = 1*target, so source = target/2
        let rho_source = RestrictionMap::diagonal(vec![2.0, 2.0]);
        let rho_target = RestrictionMap::identity(2);

        let edge = SheafEdge::with_restrictions(source, target, rho_source, rho_target);

        let source_state = vec![1.0, 1.0];
        let target_state = vec![2.0, 2.0]; // 2*[1,1] = [2,2]

        assert!(edge.residual_norm_squared(&source_state, &target_state) < 1e-10);
    }

    #[test]
    fn test_is_coherent() {
        let (source, target) = make_test_nodes();
        let edge = SheafEdge::identity(source, target, 2);

        let source_state = vec![1.0, 0.0];
        let target_state = vec![1.1, 0.0]; // Small difference

        // Residual is [-0.1, 0], norm = 0.1
        assert!(edge.is_coherent(&source_state, &target_state, 0.2)); // Below threshold
        assert!(!edge.is_coherent(&source_state, &target_state, 0.05)); // Above threshold
    }

    #[test]
    fn test_builder() {
        let (source, target) = make_test_nodes();

        let edge = SheafEdgeBuilder::new(source, target)
            .weight(2.5)
            .identity_restrictions(4)
            .edge_type("citation")
            .namespace("test")
            .metadata("importance", serde_json::json!(0.9))
            .build();

        assert_eq!(edge.weight, 2.5);
        assert_eq!(edge.edge_type, Some("citation".to_string()));
        assert_eq!(edge.namespace, Some("test".to_string()));
        assert!(edge.metadata.contains_key("importance"));
    }

    #[test]
    fn test_residual_with_energy() {
        let (source, target) = make_test_nodes();
        let edge = SheafEdge::identity(source, target, 3);

        let source_state = vec![1.0, 2.0, 3.0];
        let target_state = vec![0.0, 0.0, 0.0];

        let (residual, energy) = edge.residual_with_energy(&source_state, &target_state);

        assert_eq!(residual, vec![1.0, 2.0, 3.0]);
        assert!((energy - 14.0).abs() < 1e-10); // 1 + 4 + 9 = 14
    }

    #[test]
    fn test_content_hash_stability() {
        let (source, target) = make_test_nodes();
        let edge = SheafEdge::identity(source, target, 3);

        let hash1 = edge.content_hash();
        let hash2 = edge.content_hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_residual_norm_squared_no_alloc_identity() {
        let (source, target) = make_test_nodes();
        let edge = SheafEdge::identity(source, target, 3);

        let source_state = vec![1.0, 2.0, 3.0];
        let target_state = vec![1.0, 2.0, 3.0];

        // Should match allocating version
        let alloc_result = edge.residual_norm_squared(&source_state, &target_state);
        let no_alloc_result = edge.residual_norm_squared_no_alloc(&source_state, &target_state);

        assert!((alloc_result - no_alloc_result).abs() < 1e-10);
        assert!(no_alloc_result < 1e-10);
    }

    #[test]
    fn test_residual_norm_squared_no_alloc_mismatch() {
        let (source, target) = make_test_nodes();
        let edge = SheafEdge::identity(source, target, 3);

        let source_state = vec![1.0, 2.0, 3.0];
        let target_state = vec![0.0, 0.0, 0.0];

        // Residual is [1, 2, 3], norm^2 = 1 + 4 + 9 = 14
        let alloc_result = edge.residual_norm_squared(&source_state, &target_state);
        let no_alloc_result = edge.residual_norm_squared_no_alloc(&source_state, &target_state);

        assert!((alloc_result - no_alloc_result).abs() < 1e-10);
        assert!((no_alloc_result - 14.0).abs() < 1e-10);
    }

    #[test]
    fn test_residual_norm_squared_no_alloc_with_projection() {
        let (source, target) = make_test_nodes();

        // Source: 4D, project to first 2 dims
        let rho_source = RestrictionMap::projection(vec![0, 1], 4);
        let rho_target = RestrictionMap::identity(2);

        let edge = SheafEdge::with_restrictions(source, target, rho_source, rho_target);

        let source_state = vec![1.0, 2.0, 100.0, 200.0];
        let target_state = vec![1.0, 2.0];

        let alloc_result = edge.residual_norm_squared(&source_state, &target_state);
        let no_alloc_result = edge.residual_norm_squared_no_alloc(&source_state, &target_state);

        assert!((alloc_result - no_alloc_result).abs() < 1e-10);
        assert!(no_alloc_result < 1e-10);
    }

    #[test]
    fn test_residual_norm_squared_no_alloc_with_diagonal() {
        let (source, target) = make_test_nodes();

        let rho_source = RestrictionMap::diagonal(vec![2.0, 2.0]);
        let rho_target = RestrictionMap::identity(2);

        let edge = SheafEdge::with_restrictions(source, target, rho_source, rho_target);

        let source_state = vec![1.0, 1.0];
        let target_state = vec![2.0, 2.0];

        let alloc_result = edge.residual_norm_squared(&source_state, &target_state);
        let no_alloc_result = edge.residual_norm_squared_no_alloc(&source_state, &target_state);

        assert!((alloc_result - no_alloc_result).abs() < 1e-10);
        assert!(no_alloc_result < 1e-10);
    }

    #[test]
    fn test_weighted_residual_energy_no_alloc() {
        let (source, target) = make_test_nodes();
        let mut edge = SheafEdge::identity(source, target, 2);
        edge.set_weight(2.0);

        let source_state = vec![1.0, 0.0];
        let target_state = vec![0.0, 0.0];

        let alloc_result = edge.weighted_residual_energy(&source_state, &target_state);
        let no_alloc_result = edge.weighted_residual_energy_no_alloc(&source_state, &target_state);

        assert!((alloc_result - no_alloc_result).abs() < 1e-10);
        assert!((no_alloc_result - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_no_alloc_buffer_reuse() {
        // Test that scratch buffers are properly reused across multiple calls
        let (source, target) = make_test_nodes();

        // First call with dim=3
        let edge3 = SheafEdge::identity(source, target, 3);
        let result3 = edge3.residual_norm_squared_no_alloc(&[1.0, 2.0, 3.0], &[0.0, 0.0, 0.0]);
        assert!((result3 - 14.0).abs() < 1e-10);

        // Second call with larger dim=5 (buffers should grow)
        let edge5 = SheafEdge::identity(source, target, 5);
        let result5 = edge5
            .residual_norm_squared_no_alloc(&[1.0, 2.0, 3.0, 4.0, 5.0], &[0.0, 0.0, 0.0, 0.0, 0.0]);
        assert!((result5 - 55.0).abs() < 1e-10); // 1 + 4 + 9 + 16 + 25 = 55

        // Third call back to dim=3 (buffers should shrink length but keep capacity)
        let result3_again =
            edge3.residual_norm_squared_no_alloc(&[1.0, 2.0, 3.0], &[0.0, 0.0, 0.0]);
        assert!((result3_again - 14.0).abs() < 1e-10);
    }

    #[test]
    fn test_no_alloc_large_dimension() {
        // Test with dimension larger than default capacity (256)
        let (source, target) = make_test_nodes();
        let dim = 512;

        let edge = SheafEdge::identity(source, target, dim);
        let source_state: Vec<f32> = (0..dim).map(|i| i as f32).collect();
        let target_state: Vec<f32> = vec![0.0; dim];

        let alloc_result = edge.residual_norm_squared(&source_state, &target_state);
        let no_alloc_result = edge.residual_norm_squared_no_alloc(&source_state, &target_state);

        assert!((alloc_result - no_alloc_result).abs() < 1e-4);
    }
}
