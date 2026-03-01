//! # RuVector Delta Graph
//!
//! Delta operations for graph structures, supporting incremental updates
//! to edges and node properties.
//!
//! ## Key Features
//!
//! - Edge delta operations (add, remove, update weight)
//! - Node property deltas (including vector properties)
//! - Delta-aware graph traversal
//! - Efficient batch updates

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod edge_delta;
pub mod error;
pub mod node_delta;
pub mod traversal;

use std::collections::{HashMap, HashSet};

use ruvector_delta_core::{Delta, DeltaStream, VectorDelta};
use smallvec::SmallVec;

pub use edge_delta::{EdgeDelta, EdgeOp};
pub use error::{GraphDeltaError, Result};
pub use node_delta::{NodeDelta, PropertyDelta};
pub use traversal::{DeltaAwareTraversal, TraversalMode};

/// A node ID type
pub type NodeId = String;

/// An edge ID type
pub type EdgeId = String;

/// Property value types
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// Null/missing value
    Null,
    /// Boolean
    Bool(bool),
    /// Integer
    Int(i64),
    /// Float
    Float(f64),
    /// String
    String(String),
    /// Vector (for embeddings)
    Vector(Vec<f32>),
    /// List of values
    List(Vec<PropertyValue>),
    /// Map of values
    Map(HashMap<String, PropertyValue>),
}

impl Default for PropertyValue {
    fn default() -> Self {
        Self::Null
    }
}

/// A graph delta representing changes to graph structure
#[derive(Debug, Clone)]
pub struct GraphDelta {
    /// Node additions
    pub node_adds: Vec<(NodeId, HashMap<String, PropertyValue>)>,
    /// Node removals
    pub node_removes: Vec<NodeId>,
    /// Node property updates
    pub node_updates: Vec<(NodeId, NodeDelta)>,
    /// Edge additions
    pub edge_adds: Vec<EdgeAddition>,
    /// Edge removals
    pub edge_removes: Vec<EdgeId>,
    /// Edge updates
    pub edge_updates: Vec<(EdgeId, EdgeDelta)>,
}

/// An edge addition
#[derive(Debug, Clone)]
pub struct EdgeAddition {
    /// Edge ID
    pub id: EdgeId,
    /// Source node
    pub source: NodeId,
    /// Target node
    pub target: NodeId,
    /// Edge type/label
    pub edge_type: String,
    /// Edge properties
    pub properties: HashMap<String, PropertyValue>,
}

impl GraphDelta {
    /// Create an empty graph delta
    pub fn new() -> Self {
        Self {
            node_adds: Vec::new(),
            node_removes: Vec::new(),
            node_updates: Vec::new(),
            edge_adds: Vec::new(),
            edge_removes: Vec::new(),
            edge_updates: Vec::new(),
        }
    }

    /// Check if delta is empty
    pub fn is_empty(&self) -> bool {
        self.node_adds.is_empty()
            && self.node_removes.is_empty()
            && self.node_updates.is_empty()
            && self.edge_adds.is_empty()
            && self.edge_removes.is_empty()
            && self.edge_updates.is_empty()
    }

    /// Get total number of operations
    pub fn operation_count(&self) -> usize {
        self.node_adds.len()
            + self.node_removes.len()
            + self.node_updates.len()
            + self.edge_adds.len()
            + self.edge_removes.len()
            + self.edge_updates.len()
    }

    /// Add a node addition
    pub fn add_node(&mut self, id: NodeId, properties: HashMap<String, PropertyValue>) {
        self.node_adds.push((id, properties));
    }

    /// Add a node removal
    pub fn remove_node(&mut self, id: NodeId) {
        self.node_removes.push(id);
    }

    /// Add a node update
    pub fn update_node(&mut self, id: NodeId, delta: NodeDelta) {
        self.node_updates.push((id, delta));
    }

    /// Add an edge
    pub fn add_edge(&mut self, edge: EdgeAddition) {
        self.edge_adds.push(edge);
    }

    /// Remove an edge
    pub fn remove_edge(&mut self, id: EdgeId) {
        self.edge_removes.push(id);
    }

    /// Update an edge
    pub fn update_edge(&mut self, id: EdgeId, delta: EdgeDelta) {
        self.edge_updates.push((id, delta));
    }

    /// Compose with another graph delta
    pub fn compose(mut self, other: GraphDelta) -> Self {
        // Handle conflicting operations
        let removed_nodes: HashSet<_> = other.node_removes.iter().collect();
        let removed_edges: HashSet<_> = other.edge_removes.iter().collect();

        // Filter out adds that are removed
        self.node_adds.retain(|(id, _)| !removed_nodes.contains(id));
        self.edge_adds.retain(|e| !removed_edges.contains(&e.id));

        // Merge adds
        self.node_adds.extend(other.node_adds);
        self.node_removes.extend(other.node_removes);
        self.edge_adds.extend(other.edge_adds);
        self.edge_removes.extend(other.edge_removes);

        // Compose node updates
        let mut node_update_map: HashMap<NodeId, NodeDelta> = HashMap::new();
        for (id, delta) in self.node_updates {
            node_update_map.insert(id, delta);
        }
        for (id, delta) in other.node_updates {
            node_update_map
                .entry(id)
                .and_modify(|existing| *existing = existing.clone().compose(delta.clone()))
                .or_insert(delta);
        }
        self.node_updates = node_update_map.into_iter().collect();

        // Compose edge updates
        let mut edge_update_map: HashMap<EdgeId, EdgeDelta> = HashMap::new();
        for (id, delta) in self.edge_updates {
            edge_update_map.insert(id, delta);
        }
        for (id, delta) in other.edge_updates {
            edge_update_map
                .entry(id)
                .and_modify(|existing| *existing = existing.clone().compose(delta.clone()))
                .or_insert(delta);
        }
        self.edge_updates = edge_update_map.into_iter().collect();

        self
    }

    /// Compute inverse delta
    pub fn inverse(&self) -> Self {
        // Note: Full inverse requires knowing original state
        // This is a partial inverse that works for simple cases
        Self {
            node_adds: Vec::new(), // Would need originals to undo removes
            node_removes: self.node_adds.iter().map(|(id, _)| id.clone()).collect(),
            node_updates: self
                .node_updates
                .iter()
                .map(|(id, d)| (id.clone(), d.inverse()))
                .collect(),
            edge_adds: Vec::new(),
            edge_removes: self.edge_adds.iter().map(|e| e.id.clone()).collect(),
            edge_updates: self
                .edge_updates
                .iter()
                .map(|(id, d)| (id.clone(), d.inverse()))
                .collect(),
        }
    }

    /// Get affected node IDs
    pub fn affected_nodes(&self) -> HashSet<NodeId> {
        let mut nodes = HashSet::new();

        for (id, _) in &self.node_adds {
            nodes.insert(id.clone());
        }
        for id in &self.node_removes {
            nodes.insert(id.clone());
        }
        for (id, _) in &self.node_updates {
            nodes.insert(id.clone());
        }
        for edge in &self.edge_adds {
            nodes.insert(edge.source.clone());
            nodes.insert(edge.target.clone());
        }

        nodes
    }

    /// Get affected edge IDs
    pub fn affected_edges(&self) -> HashSet<EdgeId> {
        let mut edges = HashSet::new();

        for edge in &self.edge_adds {
            edges.insert(edge.id.clone());
        }
        for id in &self.edge_removes {
            edges.insert(id.clone());
        }
        for (id, _) in &self.edge_updates {
            edges.insert(id.clone());
        }

        edges
    }
}

impl Default for GraphDelta {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for graph deltas
pub struct GraphDeltaBuilder {
    delta: GraphDelta,
}

impl GraphDeltaBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            delta: GraphDelta::new(),
        }
    }

    /// Add a node
    pub fn add_node(mut self, id: impl Into<NodeId>) -> Self {
        self.delta.add_node(id.into(), HashMap::new());
        self
    }

    /// Add a node with properties
    pub fn add_node_with_props(
        mut self,
        id: impl Into<NodeId>,
        props: HashMap<String, PropertyValue>,
    ) -> Self {
        self.delta.add_node(id.into(), props);
        self
    }

    /// Remove a node
    pub fn remove_node(mut self, id: impl Into<NodeId>) -> Self {
        self.delta.remove_node(id.into());
        self
    }

    /// Add an edge
    pub fn add_edge(
        mut self,
        id: impl Into<EdgeId>,
        source: impl Into<NodeId>,
        target: impl Into<NodeId>,
        edge_type: impl Into<String>,
    ) -> Self {
        self.delta.add_edge(EdgeAddition {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            edge_type: edge_type.into(),
            properties: HashMap::new(),
        });
        self
    }

    /// Remove an edge
    pub fn remove_edge(mut self, id: impl Into<EdgeId>) -> Self {
        self.delta.remove_edge(id.into());
        self
    }

    /// Build the delta
    pub fn build(self) -> GraphDelta {
        self.delta
    }
}

impl Default for GraphDeltaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Stream of graph deltas for event sourcing
pub struct GraphDeltaStream {
    stream: DeltaStream<WrappedGraphDelta>,
}

// Wrapper to implement Delta trait
#[derive(Clone)]
struct WrappedGraphDelta(GraphDelta);

impl Delta for WrappedGraphDelta {
    type Base = GraphState;
    type Error = GraphDeltaError;

    fn compute(_old: &Self::Base, _new: &Self::Base) -> Self {
        // Would diff two graph states
        WrappedGraphDelta(GraphDelta::new())
    }

    fn apply(&self, base: &mut Self::Base) -> std::result::Result<(), Self::Error> {
        base.apply_delta(&self.0)
    }

    fn compose(self, other: Self) -> Self {
        WrappedGraphDelta(self.0.compose(other.0))
    }

    fn inverse(&self) -> Self {
        WrappedGraphDelta(self.0.inverse())
    }

    fn is_identity(&self) -> bool {
        self.0.is_empty()
    }

    fn byte_size(&self) -> usize {
        // Approximate size
        std::mem::size_of::<GraphDelta>() + self.0.operation_count() * 100
    }
}

impl Default for WrappedGraphDelta {
    fn default() -> Self {
        Self(GraphDelta::new())
    }
}

/// Simplified graph state for delta application
#[derive(Debug, Clone, Default)]
pub struct GraphState {
    /// Nodes with properties
    pub nodes: HashMap<NodeId, HashMap<String, PropertyValue>>,
    /// Edges
    pub edges: HashMap<EdgeId, (NodeId, NodeId, String, HashMap<String, PropertyValue>)>,
}

impl GraphState {
    /// Create empty state
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a graph delta
    pub fn apply_delta(&mut self, delta: &GraphDelta) -> Result<()> {
        // Remove nodes
        for id in &delta.node_removes {
            self.nodes.remove(id);
            // Also remove incident edges
            self.edges.retain(|_, (s, t, _, _)| s != id && t != id);
        }

        // Remove edges
        for id in &delta.edge_removes {
            self.edges.remove(id);
        }

        // Add nodes
        for (id, props) in &delta.node_adds {
            self.nodes.insert(id.clone(), props.clone());
        }

        // Add edges
        for edge in &delta.edge_adds {
            self.edges.insert(
                edge.id.clone(),
                (
                    edge.source.clone(),
                    edge.target.clone(),
                    edge.edge_type.clone(),
                    edge.properties.clone(),
                ),
            );
        }

        // Update nodes
        for (id, node_delta) in &delta.node_updates {
            if let Some(props) = self.nodes.get_mut(id) {
                for prop_delta in &node_delta.property_deltas {
                    match &prop_delta.operation {
                        PropertyOp::Set(value) => {
                            props.insert(prop_delta.key.clone(), value.clone());
                        }
                        PropertyOp::Remove => {
                            props.remove(&prop_delta.key);
                        }
                        PropertyOp::VectorDelta(vd) => {
                            if let Some(PropertyValue::Vector(v)) = props.get_mut(&prop_delta.key) {
                                vd.apply(v)
                                    .map_err(|e| GraphDeltaError::DeltaError(format!("{:?}", e)))?;
                            }
                        }
                    }
                }
            }
        }

        // Update edges
        for (id, edge_delta) in &delta.edge_updates {
            if let Some((_, _, _, props)) = self.edges.get_mut(id) {
                for prop_delta in &edge_delta.property_deltas {
                    match &prop_delta.operation {
                        PropertyOp::Set(value) => {
                            props.insert(prop_delta.key.clone(), value.clone());
                        }
                        PropertyOp::Remove => {
                            props.remove(&prop_delta.key);
                        }
                        PropertyOp::VectorDelta(_) => {
                            // Edges typically don't have vector properties
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get edge count
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

/// Property operation types
#[derive(Debug, Clone)]
pub enum PropertyOp {
    /// Set property value
    Set(PropertyValue),
    /// Remove property
    Remove,
    /// Apply vector delta to vector property
    VectorDelta(VectorDelta),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_delta_builder() {
        let delta = GraphDeltaBuilder::new()
            .add_node("a")
            .add_node("b")
            .add_edge("e1", "a", "b", "KNOWS")
            .build();

        assert_eq!(delta.node_adds.len(), 2);
        assert_eq!(delta.edge_adds.len(), 1);
    }

    #[test]
    fn test_graph_state_apply() {
        let mut state = GraphState::new();

        let delta = GraphDeltaBuilder::new()
            .add_node("a")
            .add_node("b")
            .add_edge("e1", "a", "b", "KNOWS")
            .build();

        state.apply_delta(&delta).unwrap();

        assert_eq!(state.node_count(), 2);
        assert_eq!(state.edge_count(), 1);
    }

    #[test]
    fn test_delta_compose() {
        let d1 = GraphDeltaBuilder::new().add_node("a").add_node("b").build();

        let d2 = GraphDeltaBuilder::new()
            .remove_node("b")
            .add_node("c")
            .build();

        let composed = d1.compose(d2);

        // "a" added, "b" removed, "c" added
        assert_eq!(composed.node_adds.len(), 2); // a and c
        assert_eq!(composed.node_removes.len(), 1); // b
    }

    #[test]
    fn test_affected_nodes() {
        let delta = GraphDeltaBuilder::new()
            .add_node("a")
            .add_edge("e1", "b", "c", "RELATES")
            .build();

        let affected = delta.affected_nodes();

        assert!(affected.contains("a"));
        assert!(affected.contains("b"));
        assert!(affected.contains("c"));
    }
}
