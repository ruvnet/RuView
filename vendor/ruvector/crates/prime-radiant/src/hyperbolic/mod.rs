//! Hyperbolic Coherence Module
//!
//! Hierarchy-aware coherence computation using hyperbolic geometry.
//! Leverages `ruvector-hyperbolic-hnsw` for Poincare ball operations.
//!
//! # Features
//!
//! - Depth-aware energy weighting: deeper nodes get higher violation weights
//! - Poincare ball projection for hierarchy-aware storage
//! - Curvature-adaptive residual computation
//! - Sharded hyperbolic index for scalability
//!
//! # Mathematical Foundation
//!
//! In the Poincare ball model, distance from origin correlates with hierarchy depth.
//! Nodes closer to the boundary (|x| -> 1) are "deeper" in the hierarchy.
//!
//! Energy weighting: E_weighted = w_e * |r_e|^2 * depth_weight(e)
//! where depth_weight = 1 + ln(max(avg_depth, 1))

mod adapter;
mod config;
mod depth;
mod energy;

pub use adapter::HyperbolicAdapter;
pub use config::HyperbolicCoherenceConfig;
pub use depth::{DepthComputer, HierarchyLevel};
pub use energy::{HyperbolicEnergy, WeightedResidual};

use std::collections::HashMap;

/// Node identifier type alias
pub type NodeId = u64;

/// Edge identifier type alias
pub type EdgeId = u64;

/// Result type for hyperbolic coherence operations
pub type Result<T> = std::result::Result<T, HyperbolicCoherenceError>;

/// Errors that can occur in hyperbolic coherence computation
#[derive(Debug, Clone, thiserror::Error)]
pub enum HyperbolicCoherenceError {
    /// Node not found in the index
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),

    /// Invalid vector dimension
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Curvature out of valid range
    #[error("Invalid curvature: {0} (must be negative)")]
    InvalidCurvature(f32),

    /// Projection failed (vector outside ball)
    #[error("Projection failed: vector norm {0} exceeds ball radius")]
    ProjectionFailed(f32),

    /// Underlying HNSW error
    #[error("HNSW error: {0}")]
    HnswError(String),

    /// Empty collection
    #[error("Empty collection")]
    EmptyCollection,
}

/// Main hyperbolic coherence engine
///
/// Computes hierarchy-aware coherence energy using the Poincare ball model.
/// Deeper nodes (further from origin) receive higher weights for violations,
/// encoding the intuition that deeper hierarchical nodes should be more consistent.
#[derive(Debug)]
pub struct HyperbolicCoherence {
    /// Configuration
    config: HyperbolicCoherenceConfig,
    /// Adapter to underlying hyperbolic HNSW
    adapter: HyperbolicAdapter,
    /// Depth computer
    depth: DepthComputer,
    /// Node states (node_id -> state vector)
    node_states: HashMap<NodeId, Vec<f32>>,
    /// Node depths (cached)
    node_depths: HashMap<NodeId, f32>,
}

impl HyperbolicCoherence {
    /// Create a new hyperbolic coherence engine
    pub fn new(config: HyperbolicCoherenceConfig) -> Self {
        let adapter = HyperbolicAdapter::new(config.clone());
        let depth = DepthComputer::new(config.curvature);

        Self {
            config,
            adapter,
            depth,
            node_states: HashMap::new(),
            node_depths: HashMap::new(),
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(HyperbolicCoherenceConfig::default())
    }

    /// Insert a node state
    pub fn insert_node(&mut self, node_id: NodeId, state: Vec<f32>) -> Result<()> {
        // Validate dimension
        if !self.node_states.is_empty() {
            let expected_dim = self.config.dimension;
            if state.len() != expected_dim {
                return Err(HyperbolicCoherenceError::DimensionMismatch {
                    expected: expected_dim,
                    actual: state.len(),
                });
            }
        }

        // Project to Poincare ball
        let projected = self.adapter.project_to_ball(&state)?;

        // Compute and cache depth
        let depth = self.depth.compute_depth(&projected);
        self.node_depths.insert(node_id, depth);

        // Store in adapter and local cache
        self.adapter.insert(node_id, projected.clone())?;
        self.node_states.insert(node_id, projected);

        Ok(())
    }

    /// Update a node state
    pub fn update_node(&mut self, node_id: NodeId, state: Vec<f32>) -> Result<()> {
        if !self.node_states.contains_key(&node_id) {
            return Err(HyperbolicCoherenceError::NodeNotFound(node_id));
        }

        // Project and update
        let projected = self.adapter.project_to_ball(&state)?;
        let depth = self.depth.compute_depth(&projected);

        self.node_depths.insert(node_id, depth);
        self.adapter.update(node_id, projected.clone())?;
        self.node_states.insert(node_id, projected);

        Ok(())
    }

    /// Get node state
    pub fn get_node(&self, node_id: NodeId) -> Option<&Vec<f32>> {
        self.node_states.get(&node_id)
    }

    /// Get node depth
    pub fn get_depth(&self, node_id: NodeId) -> Option<f32> {
        self.node_depths.get(&node_id).copied()
    }

    /// Compute depth-weighted energy for an edge
    ///
    /// The energy is weighted by the average depth of the connected nodes.
    /// Deeper nodes receive higher violation weights.
    pub fn weighted_edge_energy(
        &self,
        source_id: NodeId,
        target_id: NodeId,
        residual: &[f32],
        base_weight: f32,
    ) -> Result<WeightedResidual> {
        let source_depth = self
            .node_depths
            .get(&source_id)
            .ok_or(HyperbolicCoherenceError::NodeNotFound(source_id))?;
        let target_depth = self
            .node_depths
            .get(&target_id)
            .ok_or(HyperbolicCoherenceError::NodeNotFound(target_id))?;

        let avg_depth = (source_depth + target_depth) / 2.0;

        // Depth weight: higher for deeper nodes
        let depth_weight = self.config.depth_weight_fn(avg_depth);

        // Residual norm squared
        let residual_norm_sq: f32 = residual.iter().map(|x| x * x).sum();

        let weighted_energy = base_weight * residual_norm_sq * depth_weight;

        Ok(WeightedResidual {
            source_id,
            target_id,
            source_depth: *source_depth,
            target_depth: *target_depth,
            depth_weight,
            residual_norm_sq,
            base_weight,
            weighted_energy,
        })
    }

    /// Compute total hyperbolic energy for a set of edges
    pub fn compute_total_energy(
        &self,
        edges: &[(NodeId, NodeId, Vec<f32>, f32)], // (source, target, residual, weight)
    ) -> Result<HyperbolicEnergy> {
        if edges.is_empty() {
            return Ok(HyperbolicEnergy::empty());
        }

        let mut edge_energies = Vec::with_capacity(edges.len());
        let mut total_energy = 0.0f32;
        let mut max_depth = 0.0f32;
        let mut min_depth = f32::MAX;

        for (source, target, residual, weight) in edges {
            let weighted = self.weighted_edge_energy(*source, *target, residual, *weight)?;
            total_energy += weighted.weighted_energy;
            max_depth = max_depth.max(weighted.source_depth.max(weighted.target_depth));
            min_depth = min_depth.min(weighted.source_depth.min(weighted.target_depth));
            edge_energies.push(weighted);
        }

        Ok(HyperbolicEnergy {
            total_energy,
            edge_energies,
            curvature: self.config.curvature,
            max_depth,
            min_depth,
            num_edges: edges.len(),
        })
    }

    /// Find similar nodes in hyperbolic space
    pub fn find_similar(&self, query: &[f32], k: usize) -> Result<Vec<(NodeId, f32)>> {
        let projected = self.adapter.project_to_ball(query)?;
        self.adapter.search(&projected, k)
    }

    /// Get hierarchy level for a node based on depth
    pub fn hierarchy_level(&self, node_id: NodeId) -> Result<HierarchyLevel> {
        let depth = self
            .node_depths
            .get(&node_id)
            .ok_or(HyperbolicCoherenceError::NodeNotFound(node_id))?;

        Ok(self.depth.classify_level(*depth))
    }

    /// Compute Frechet mean of a set of nodes
    pub fn frechet_mean(&self, node_ids: &[NodeId]) -> Result<Vec<f32>> {
        if node_ids.is_empty() {
            return Err(HyperbolicCoherenceError::EmptyCollection);
        }

        let states: Vec<&Vec<f32>> = node_ids
            .iter()
            .filter_map(|id| self.node_states.get(id))
            .collect();

        if states.is_empty() {
            return Err(HyperbolicCoherenceError::EmptyCollection);
        }

        self.adapter.frechet_mean(&states)
    }

    /// Get configuration
    pub fn config(&self) -> &HyperbolicCoherenceConfig {
        &self.config
    }

    /// Get number of nodes
    pub fn num_nodes(&self) -> usize {
        self.node_states.len()
    }

    /// Get curvature
    pub fn curvature(&self) -> f32 {
        self.config.curvature
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_coherence() {
        let config = HyperbolicCoherenceConfig {
            dimension: 4,
            curvature: -1.0,
            ..Default::default()
        };
        let mut coherence = HyperbolicCoherence::new(config);

        // Insert nodes
        coherence.insert_node(1, vec![0.1, 0.1, 0.1, 0.1]).unwrap();
        coherence.insert_node(2, vec![0.2, 0.2, 0.2, 0.2]).unwrap();
        coherence.insert_node(3, vec![0.5, 0.5, 0.5, 0.5]).unwrap();

        assert_eq!(coherence.num_nodes(), 3);

        // Node 3 should be deeper (further from origin)
        let depth1 = coherence.get_depth(1).unwrap();
        let depth3 = coherence.get_depth(3).unwrap();
        assert!(depth3 > depth1);
    }

    #[test]
    fn test_weighted_energy() {
        let config = HyperbolicCoherenceConfig {
            dimension: 4,
            curvature: -1.0,
            ..Default::default()
        };
        let mut coherence = HyperbolicCoherence::new(config);

        coherence.insert_node(1, vec![0.1, 0.1, 0.1, 0.1]).unwrap();
        coherence.insert_node(2, vec![0.5, 0.5, 0.5, 0.5]).unwrap();

        let residual = vec![0.1, 0.1, 0.1, 0.1];
        let weighted = coherence
            .weighted_edge_energy(1, 2, &residual, 1.0)
            .unwrap();

        assert!(weighted.weighted_energy > 0.0);
        assert!(weighted.depth_weight > 1.0); // Should have depth scaling
    }

    #[test]
    fn test_hierarchy_levels() {
        let config = HyperbolicCoherenceConfig {
            dimension: 4,
            curvature: -1.0,
            ..Default::default()
        };
        let mut coherence = HyperbolicCoherence::new(config);

        coherence
            .insert_node(1, vec![0.05, 0.05, 0.05, 0.05])
            .unwrap();
        coherence.insert_node(2, vec![0.7, 0.7, 0.0, 0.0]).unwrap();

        let level1 = coherence.hierarchy_level(1).unwrap();
        let level2 = coherence.hierarchy_level(2).unwrap();

        // Node 1 should be at higher level (closer to root)
        assert!(matches!(
            level1,
            HierarchyLevel::Root | HierarchyLevel::High
        ));
        // Node 2 should be deeper
        assert!(matches!(
            level2,
            HierarchyLevel::Deep | HierarchyLevel::VeryDeep
        ));
    }
}
