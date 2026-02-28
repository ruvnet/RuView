//! Index structures for fast node and edge lookups
//!
//! Provides label indexes, property indexes, and edge type indexes for efficient querying

use crate::edge::Edge;
use crate::hyperedge::{Hyperedge, HyperedgeId};
use crate::node::Node;
use crate::types::{EdgeId, NodeId, PropertyValue};
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::Arc;

/// Label index for nodes (maps labels to node IDs)
#[derive(Debug, Clone)]
pub struct LabelIndex {
    /// Label -> Set of node IDs
    index: Arc<DashMap<String, HashSet<NodeId>>>,
}

impl LabelIndex {
    /// Create a new label index
    pub fn new() -> Self {
        Self {
            index: Arc::new(DashMap::new()),
        }
    }

    /// Add a node to the index
    pub fn add_node(&self, node: &Node) {
        for label in &node.labels {
            self.index
                .entry(label.name.clone())
                .or_insert_with(HashSet::new)
                .insert(node.id.clone());
        }
    }

    /// Remove a node from the index
    pub fn remove_node(&self, node: &Node) {
        for label in &node.labels {
            if let Some(mut set) = self.index.get_mut(&label.name) {
                set.remove(&node.id);
            }
        }
    }

    /// Get all nodes with a specific label
    pub fn get_nodes_by_label(&self, label: &str) -> Vec<NodeId> {
        self.index
            .get(label)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all labels in the index
    pub fn all_labels(&self) -> Vec<String> {
        self.index.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Count nodes with a specific label
    pub fn count_by_label(&self, label: &str) -> usize {
        self.index.get(label).map(|set| set.len()).unwrap_or(0)
    }

    /// Clear the index
    pub fn clear(&self) {
        self.index.clear();
    }
}

impl Default for LabelIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Property index for nodes (maps property keys to values to node IDs)
#[derive(Debug, Clone)]
pub struct PropertyIndex {
    /// Property key -> Property value -> Set of node IDs
    index: Arc<DashMap<String, DashMap<String, HashSet<NodeId>>>>,
}

impl PropertyIndex {
    /// Create a new property index
    pub fn new() -> Self {
        Self {
            index: Arc::new(DashMap::new()),
        }
    }

    /// Add a node to the index
    pub fn add_node(&self, node: &Node) {
        for (key, value) in &node.properties {
            let value_str = self.property_value_to_string(value);
            self.index
                .entry(key.clone())
                .or_insert_with(DashMap::new)
                .entry(value_str)
                .or_insert_with(HashSet::new)
                .insert(node.id.clone());
        }
    }

    /// Remove a node from the index
    pub fn remove_node(&self, node: &Node) {
        for (key, value) in &node.properties {
            let value_str = self.property_value_to_string(value);
            if let Some(value_map) = self.index.get(key) {
                if let Some(mut set) = value_map.get_mut(&value_str) {
                    set.remove(&node.id);
                }
            }
        }
    }

    /// Get nodes by property key-value pair
    pub fn get_nodes_by_property(&self, key: &str, value: &PropertyValue) -> Vec<NodeId> {
        let value_str = self.property_value_to_string(value);
        self.index
            .get(key)
            .and_then(|value_map| {
                value_map
                    .get(&value_str)
                    .map(|set| set.iter().cloned().collect())
            })
            .unwrap_or_default()
    }

    /// Get all nodes that have a specific property key (regardless of value)
    pub fn get_nodes_with_property(&self, key: &str) -> Vec<NodeId> {
        self.index
            .get(key)
            .map(|value_map| {
                let mut result = HashSet::new();
                for entry in value_map.iter() {
                    for id in entry.value().iter() {
                        result.insert(id.clone());
                    }
                }
                result.into_iter().collect()
            })
            .unwrap_or_default()
    }

    /// Get all property keys in the index
    pub fn all_property_keys(&self) -> Vec<String> {
        self.index.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Clear the index
    pub fn clear(&self) {
        self.index.clear();
    }

    /// Convert property value to string for indexing
    fn property_value_to_string(&self, value: &PropertyValue) -> String {
        match value {
            PropertyValue::Null => "null".to_string(),
            PropertyValue::Boolean(b) => b.to_string(),
            PropertyValue::Integer(i) => i.to_string(),
            PropertyValue::Float(f) => f.to_string(),
            PropertyValue::String(s) => s.clone(),
            PropertyValue::Array(_) | PropertyValue::List(_) => format!("{:?}", value),
            PropertyValue::Map(_) => format!("{:?}", value),
        }
    }
}

impl Default for PropertyIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Edge type index (maps edge types to edge IDs)
#[derive(Debug, Clone)]
pub struct EdgeTypeIndex {
    /// Edge type -> Set of edge IDs
    index: Arc<DashMap<String, HashSet<EdgeId>>>,
}

impl EdgeTypeIndex {
    /// Create a new edge type index
    pub fn new() -> Self {
        Self {
            index: Arc::new(DashMap::new()),
        }
    }

    /// Add an edge to the index
    pub fn add_edge(&self, edge: &Edge) {
        self.index
            .entry(edge.edge_type.clone())
            .or_insert_with(HashSet::new)
            .insert(edge.id.clone());
    }

    /// Remove an edge from the index
    pub fn remove_edge(&self, edge: &Edge) {
        if let Some(mut set) = self.index.get_mut(&edge.edge_type) {
            set.remove(&edge.id);
        }
    }

    /// Get all edges of a specific type
    pub fn get_edges_by_type(&self, edge_type: &str) -> Vec<EdgeId> {
        self.index
            .get(edge_type)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all edge types
    pub fn all_edge_types(&self) -> Vec<String> {
        self.index.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Count edges of a specific type
    pub fn count_by_type(&self, edge_type: &str) -> usize {
        self.index.get(edge_type).map(|set| set.len()).unwrap_or(0)
    }

    /// Clear the index
    pub fn clear(&self) {
        self.index.clear();
    }
}

impl Default for EdgeTypeIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Adjacency index for fast neighbor lookups
#[derive(Debug, Clone)]
pub struct AdjacencyIndex {
    /// Node ID -> Set of outgoing edge IDs
    outgoing: Arc<DashMap<NodeId, HashSet<EdgeId>>>,
    /// Node ID -> Set of incoming edge IDs
    incoming: Arc<DashMap<NodeId, HashSet<EdgeId>>>,
}

impl AdjacencyIndex {
    /// Create a new adjacency index
    pub fn new() -> Self {
        Self {
            outgoing: Arc::new(DashMap::new()),
            incoming: Arc::new(DashMap::new()),
        }
    }

    /// Add an edge to the index
    pub fn add_edge(&self, edge: &Edge) {
        self.outgoing
            .entry(edge.from.clone())
            .or_insert_with(HashSet::new)
            .insert(edge.id.clone());

        self.incoming
            .entry(edge.to.clone())
            .or_insert_with(HashSet::new)
            .insert(edge.id.clone());
    }

    /// Remove an edge from the index
    pub fn remove_edge(&self, edge: &Edge) {
        if let Some(mut set) = self.outgoing.get_mut(&edge.from) {
            set.remove(&edge.id);
        }
        if let Some(mut set) = self.incoming.get_mut(&edge.to) {
            set.remove(&edge.id);
        }
    }

    /// Get all outgoing edges from a node
    pub fn get_outgoing_edges(&self, node_id: &NodeId) -> Vec<EdgeId> {
        self.outgoing
            .get(node_id)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all incoming edges to a node
    pub fn get_incoming_edges(&self, node_id: &NodeId) -> Vec<EdgeId> {
        self.incoming
            .get(node_id)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get all edges connected to a node (both incoming and outgoing)
    pub fn get_all_edges(&self, node_id: &NodeId) -> Vec<EdgeId> {
        let mut edges = self.get_outgoing_edges(node_id);
        edges.extend(self.get_incoming_edges(node_id));
        edges
    }

    /// Get degree (number of outgoing edges)
    pub fn out_degree(&self, node_id: &NodeId) -> usize {
        self.outgoing.get(node_id).map(|set| set.len()).unwrap_or(0)
    }

    /// Get in-degree (number of incoming edges)
    pub fn in_degree(&self, node_id: &NodeId) -> usize {
        self.incoming.get(node_id).map(|set| set.len()).unwrap_or(0)
    }

    /// Clear the index
    pub fn clear(&self) {
        self.outgoing.clear();
        self.incoming.clear();
    }
}

impl Default for AdjacencyIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Hyperedge node index (maps nodes to hyperedges they participate in)
#[derive(Debug, Clone)]
pub struct HyperedgeNodeIndex {
    /// Node ID -> Set of hyperedge IDs
    index: Arc<DashMap<NodeId, HashSet<HyperedgeId>>>,
}

impl HyperedgeNodeIndex {
    /// Create a new hyperedge node index
    pub fn new() -> Self {
        Self {
            index: Arc::new(DashMap::new()),
        }
    }

    /// Add a hyperedge to the index
    pub fn add_hyperedge(&self, hyperedge: &Hyperedge) {
        for node_id in &hyperedge.nodes {
            self.index
                .entry(node_id.clone())
                .or_insert_with(HashSet::new)
                .insert(hyperedge.id.clone());
        }
    }

    /// Remove a hyperedge from the index
    pub fn remove_hyperedge(&self, hyperedge: &Hyperedge) {
        for node_id in &hyperedge.nodes {
            if let Some(mut set) = self.index.get_mut(node_id) {
                set.remove(&hyperedge.id);
            }
        }
    }

    /// Get all hyperedges containing a node
    pub fn get_hyperedges_by_node(&self, node_id: &NodeId) -> Vec<HyperedgeId> {
        self.index
            .get(node_id)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Clear the index
    pub fn clear(&self) {
        self.index.clear();
    }
}

impl Default for HyperedgeNodeIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeBuilder;

    #[test]
    fn test_label_index() {
        let index = LabelIndex::new();

        let node1 = NodeBuilder::new().label("Person").label("User").build();

        let node2 = NodeBuilder::new().label("Person").build();

        index.add_node(&node1);
        index.add_node(&node2);

        let people = index.get_nodes_by_label("Person");
        assert_eq!(people.len(), 2);

        let users = index.get_nodes_by_label("User");
        assert_eq!(users.len(), 1);

        assert_eq!(index.count_by_label("Person"), 2);
    }

    #[test]
    fn test_property_index() {
        let index = PropertyIndex::new();

        let node1 = NodeBuilder::new()
            .property("name", "Alice")
            .property("age", 30i64)
            .build();

        let node2 = NodeBuilder::new()
            .property("name", "Bob")
            .property("age", 30i64)
            .build();

        index.add_node(&node1);
        index.add_node(&node2);

        let alice =
            index.get_nodes_by_property("name", &PropertyValue::String("Alice".to_string()));
        assert_eq!(alice.len(), 1);

        let age_30 = index.get_nodes_by_property("age", &PropertyValue::Integer(30));
        assert_eq!(age_30.len(), 2);

        let with_age = index.get_nodes_with_property("age");
        assert_eq!(with_age.len(), 2);
    }

    #[test]
    fn test_edge_type_index() {
        let index = EdgeTypeIndex::new();

        let edge1 = Edge::create("n1".to_string(), "n2".to_string(), "KNOWS");
        let edge2 = Edge::create("n2".to_string(), "n3".to_string(), "KNOWS");
        let edge3 = Edge::create("n1".to_string(), "n3".to_string(), "WORKS_WITH");

        index.add_edge(&edge1);
        index.add_edge(&edge2);
        index.add_edge(&edge3);

        let knows_edges = index.get_edges_by_type("KNOWS");
        assert_eq!(knows_edges.len(), 2);

        let works_with_edges = index.get_edges_by_type("WORKS_WITH");
        assert_eq!(works_with_edges.len(), 1);

        assert_eq!(index.all_edge_types().len(), 2);
    }

    #[test]
    fn test_adjacency_index() {
        let index = AdjacencyIndex::new();

        let edge1 = Edge::create("n1".to_string(), "n2".to_string(), "KNOWS");
        let edge2 = Edge::create("n1".to_string(), "n3".to_string(), "KNOWS");
        let edge3 = Edge::create("n2".to_string(), "n1".to_string(), "KNOWS");

        index.add_edge(&edge1);
        index.add_edge(&edge2);
        index.add_edge(&edge3);

        assert_eq!(index.out_degree(&"n1".to_string()), 2);
        assert_eq!(index.in_degree(&"n1".to_string()), 1);

        let outgoing = index.get_outgoing_edges(&"n1".to_string());
        assert_eq!(outgoing.len(), 2);

        let incoming = index.get_incoming_edges(&"n1".to_string());
        assert_eq!(incoming.len(), 1);
    }
}
