//! Knowledge Substrate: Sheaf Graph Data Structures
//!
//! This module implements the mathematical foundation for coherence computation
//! using sheaf theory. The key abstractions are:
//!
//! - **SheafNode**: Vertices carrying fixed-dimensional state vectors (stalks)
//! - **SheafEdge**: Edges encoding constraints via restriction maps
//! - **RestrictionMap**: Linear transforms defining how states constrain each other
//! - **SheafGraph**: The aggregate root managing the complete graph structure
//!
//! # Mathematical Foundation
//!
//! A sheaf on a graph assigns:
//! - A vector space F(v) to each vertex v (the "stalk")
//! - A linear map ρ: F(u) → F(e) for each edge e incident to u (the "restriction")
//!
//! The **residual** at an edge measures local inconsistency:
//! ```text
//! r_e = ρ_source(x_source) - ρ_target(x_target)
//! ```
//!
//! The **coherence energy** is the global inconsistency measure:
//! ```text
//! E(S) = Σ w_e ||r_e||²
//! ```
//!
//! # Domain Agnostic Design
//!
//! The same substrate supports multiple domains:
//!
//! | Domain | Nodes | Edges | Residual Interpretation |
//! |--------|-------|-------|------------------------|
//! | AI Agents | Facts, beliefs | Citations, implication | Contradiction energy |
//! | Finance | Trades, positions | Market dependencies | Regime mismatch |
//! | Medical | Vitals, diagnoses | Physiological causality | Clinical disagreement |
//! | Robotics | Sensors, goals | Physics, kinematics | Motion impossibility |
//!
//! # Performance Features
//!
//! - SIMD-optimized residual calculation
//! - Incremental fingerprint updates
//! - Thread-safe with rayon parallelization
//! - Cache-aligned data structures

pub mod edge;
pub mod graph;
pub mod node;
pub mod restriction;

// Re-exports
pub use edge::{EdgeId, SheafEdge, SheafEdgeBuilder};
pub use graph::{
    CoherenceEnergy, CoherenceFingerprint, GraphStats, IncrementalCoherence, Namespace, ScopeId,
    SheafGraph, SheafGraphBuilder,
};
pub use node::{NodeId, NodeMetadata, SheafNode, SheafNodeBuilder, StateVector};
pub use restriction::{
    CsrMatrix, MatrixStorage, RestrictionMap, RestrictionMapBuilder, RestrictionMapError,
};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A subgraph extracted from a SheafGraph for localized computation
///
/// Useful for:
/// - Computing energy in a neighborhood
/// - Isolating incoherent regions
/// - Parallel processing of graph partitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafSubgraph {
    /// Nodes in the subgraph
    pub nodes: HashMap<NodeId, SheafNode>,
    /// Edges in the subgraph (only edges between nodes in the subgraph)
    pub edges: HashMap<EdgeId, SheafEdge>,
    /// Optional center node (for neighborhood subgraphs)
    pub center: Option<NodeId>,
    /// Number of hops from center (if applicable)
    pub hops: Option<usize>,
}

impl SheafSubgraph {
    /// Create a new empty subgraph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            center: None,
            hops: None,
        }
    }

    /// Create a subgraph centered on a node
    pub fn centered(center: NodeId, hops: usize) -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            center: Some(center),
            hops: Some(hops),
        }
    }

    /// Add a node to the subgraph
    pub fn add_node(&mut self, node: SheafNode) {
        self.nodes.insert(node.id, node);
    }

    /// Add an edge to the subgraph
    pub fn add_edge(&mut self, edge: SheafEdge) {
        self.edges.insert(edge.id, edge);
    }

    /// Check if the subgraph contains a node
    pub fn has_node(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id)
    }

    /// Check if the subgraph contains an edge
    pub fn has_edge(&self, id: EdgeId) -> bool {
        self.edges.contains_key(&id)
    }

    /// Get the number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of edges
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Compute total coherence energy within the subgraph
    pub fn compute_energy(&self) -> f32 {
        let mut total = 0.0;

        for edge in self.edges.values() {
            if let (Some(source), Some(target)) =
                (self.nodes.get(&edge.source), self.nodes.get(&edge.target))
            {
                total +=
                    edge.weighted_residual_energy(source.state.as_slice(), target.state.as_slice());
            }
        }

        total
    }

    /// Extract a subgraph from a SheafGraph around a center node
    pub fn from_graph(graph: &SheafGraph, center: NodeId, hops: usize) -> Self {
        let mut subgraph = Self::centered(center, hops);

        // BFS to collect nodes within hops distance
        let mut visited = std::collections::HashSet::new();
        let mut frontier = vec![center];
        let mut depth = 0;

        while depth <= hops && !frontier.is_empty() {
            let mut next_frontier = Vec::new();

            for node_id in frontier {
                if visited.contains(&node_id) {
                    continue;
                }
                visited.insert(node_id);

                // Add node to subgraph
                if let Some(node) = graph.get_node(node_id) {
                    subgraph.add_node(node);
                }

                // Explore neighbors if within hop limit
                if depth < hops {
                    for edge_id in graph.edges_incident_to(node_id) {
                        if let Some(edge) = graph.get_edge(edge_id) {
                            let neighbor = if edge.source == node_id {
                                edge.target
                            } else {
                                edge.source
                            };

                            if !visited.contains(&neighbor) {
                                next_frontier.push(neighbor);
                            }
                        }
                    }
                }
            }

            frontier = next_frontier;
            depth += 1;
        }

        // Add edges between nodes in the subgraph
        for node_id in &visited {
            for edge_id in graph.edges_incident_to(*node_id) {
                if let Some(edge) = graph.get_edge(edge_id) {
                    // Only add if both endpoints are in the subgraph
                    if visited.contains(&edge.source) && visited.contains(&edge.target) {
                        if !subgraph.has_edge(edge_id) {
                            subgraph.add_edge(edge);
                        }
                    }
                }
            }
        }

        subgraph
    }
}

impl Default for SheafSubgraph {
    fn default() -> Self {
        Self::new()
    }
}
