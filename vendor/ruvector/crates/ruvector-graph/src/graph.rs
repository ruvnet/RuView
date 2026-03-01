//! Graph database implementation with concurrent access and indexing

use crate::edge::Edge;
use crate::error::Result;
use crate::hyperedge::{Hyperedge, HyperedgeId};
use crate::index::{AdjacencyIndex, EdgeTypeIndex, HyperedgeNodeIndex, LabelIndex, PropertyIndex};
use crate::node::Node;
#[cfg(feature = "storage")]
use crate::storage::GraphStorage;
use crate::types::{EdgeId, NodeId, PropertyValue};
use dashmap::DashMap;
#[cfg(feature = "storage")]
use std::path::Path;
use std::sync::Arc;

/// High-performance graph database with concurrent access
pub struct GraphDB {
    /// In-memory node storage (DashMap for lock-free concurrent reads)
    nodes: Arc<DashMap<NodeId, Node>>,
    /// In-memory edge storage
    edges: Arc<DashMap<EdgeId, Edge>>,
    /// In-memory hyperedge storage
    hyperedges: Arc<DashMap<HyperedgeId, Hyperedge>>,
    /// Label index for fast label-based lookups
    label_index: LabelIndex,
    /// Property index for fast property-based lookups
    property_index: PropertyIndex,
    /// Edge type index
    edge_type_index: EdgeTypeIndex,
    /// Adjacency index for neighbor lookups
    adjacency_index: AdjacencyIndex,
    /// Hyperedge node index
    hyperedge_node_index: HyperedgeNodeIndex,
    /// Optional persistent storage
    #[cfg(feature = "storage")]
    storage: Option<GraphStorage>,
}

impl GraphDB {
    /// Create a new in-memory graph database
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(DashMap::new()),
            edges: Arc::new(DashMap::new()),
            hyperedges: Arc::new(DashMap::new()),
            label_index: LabelIndex::new(),
            property_index: PropertyIndex::new(),
            edge_type_index: EdgeTypeIndex::new(),
            adjacency_index: AdjacencyIndex::new(),
            hyperedge_node_index: HyperedgeNodeIndex::new(),
            #[cfg(feature = "storage")]
            storage: None,
        }
    }

    /// Create a new graph database with persistent storage
    #[cfg(feature = "storage")]
    pub fn with_storage<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let storage = GraphStorage::new(path)?;

        let mut db = Self::new();
        db.storage = Some(storage);

        // Load existing data from storage
        db.load_from_storage()?;

        Ok(db)
    }

    /// Load all data from storage into memory
    #[cfg(feature = "storage")]
    fn load_from_storage(&mut self) -> anyhow::Result<()> {
        if let Some(storage) = &self.storage {
            // Load nodes
            for node_id in storage.all_node_ids()? {
                if let Some(node) = storage.get_node(&node_id)? {
                    self.nodes.insert(node_id.clone(), node.clone());
                    self.label_index.add_node(&node);
                    self.property_index.add_node(&node);
                }
            }

            // Load edges
            for edge_id in storage.all_edge_ids()? {
                if let Some(edge) = storage.get_edge(&edge_id)? {
                    self.edges.insert(edge_id.clone(), edge.clone());
                    self.edge_type_index.add_edge(&edge);
                    self.adjacency_index.add_edge(&edge);
                }
            }

            // Load hyperedges
            for hyperedge_id in storage.all_hyperedge_ids()? {
                if let Some(hyperedge) = storage.get_hyperedge(&hyperedge_id)? {
                    self.hyperedges
                        .insert(hyperedge_id.clone(), hyperedge.clone());
                    self.hyperedge_node_index.add_hyperedge(&hyperedge);
                }
            }
        }
        Ok(())
    }

    // Node operations

    /// Create a node
    pub fn create_node(&self, node: Node) -> Result<NodeId> {
        let id = node.id.clone();

        // Update indexes
        self.label_index.add_node(&node);
        self.property_index.add_node(&node);

        // Insert into memory
        self.nodes.insert(id.clone(), node.clone());

        // Persist to storage if available
        #[cfg(feature = "storage")]
        if let Some(storage) = &self.storage {
            storage.insert_node(&node)?;
        }

        Ok(id)
    }

    /// Get a node by ID
    pub fn get_node(&self, id: impl AsRef<str>) -> Option<Node> {
        self.nodes.get(id.as_ref()).map(|entry| entry.clone())
    }

    /// Delete a node
    pub fn delete_node(&self, id: impl AsRef<str>) -> Result<bool> {
        if let Some((_, node)) = self.nodes.remove(id.as_ref()) {
            // Update indexes
            self.label_index.remove_node(&node);
            self.property_index.remove_node(&node);

            // Delete from storage if available
            #[cfg(feature = "storage")]
            if let Some(storage) = &self.storage {
                storage.delete_node(id.as_ref())?;
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get nodes by label
    pub fn get_nodes_by_label(&self, label: &str) -> Vec<Node> {
        self.label_index
            .get_nodes_by_label(label)
            .into_iter()
            .filter_map(|id| self.get_node(&id))
            .collect()
    }

    /// Get nodes by property
    pub fn get_nodes_by_property(&self, key: &str, value: &PropertyValue) -> Vec<Node> {
        self.property_index
            .get_nodes_by_property(key, value)
            .into_iter()
            .filter_map(|id| self.get_node(&id))
            .collect()
    }

    // Edge operations

    /// Create an edge
    pub fn create_edge(&self, edge: Edge) -> Result<EdgeId> {
        let id = edge.id.clone();

        // Verify nodes exist
        if !self.nodes.contains_key(&edge.from) || !self.nodes.contains_key(&edge.to) {
            return Err(crate::error::GraphError::NodeNotFound(
                "Source or target node not found".to_string(),
            ));
        }

        // Update indexes
        self.edge_type_index.add_edge(&edge);
        self.adjacency_index.add_edge(&edge);

        // Insert into memory
        self.edges.insert(id.clone(), edge.clone());

        // Persist to storage if available
        #[cfg(feature = "storage")]
        if let Some(storage) = &self.storage {
            storage.insert_edge(&edge)?;
        }

        Ok(id)
    }

    /// Get an edge by ID
    pub fn get_edge(&self, id: impl AsRef<str>) -> Option<Edge> {
        self.edges.get(id.as_ref()).map(|entry| entry.clone())
    }

    /// Delete an edge
    pub fn delete_edge(&self, id: impl AsRef<str>) -> Result<bool> {
        if let Some((_, edge)) = self.edges.remove(id.as_ref()) {
            // Update indexes
            self.edge_type_index.remove_edge(&edge);
            self.adjacency_index.remove_edge(&edge);

            // Delete from storage if available
            #[cfg(feature = "storage")]
            if let Some(storage) = &self.storage {
                storage.delete_edge(id.as_ref())?;
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get edges by type
    pub fn get_edges_by_type(&self, edge_type: &str) -> Vec<Edge> {
        self.edge_type_index
            .get_edges_by_type(edge_type)
            .into_iter()
            .filter_map(|id| self.get_edge(&id))
            .collect()
    }

    /// Get outgoing edges from a node
    pub fn get_outgoing_edges(&self, node_id: &NodeId) -> Vec<Edge> {
        self.adjacency_index
            .get_outgoing_edges(node_id)
            .into_iter()
            .filter_map(|id| self.get_edge(&id))
            .collect()
    }

    /// Get incoming edges to a node
    pub fn get_incoming_edges(&self, node_id: &NodeId) -> Vec<Edge> {
        self.adjacency_index
            .get_incoming_edges(node_id)
            .into_iter()
            .filter_map(|id| self.get_edge(&id))
            .collect()
    }

    // Hyperedge operations

    /// Create a hyperedge
    pub fn create_hyperedge(&self, hyperedge: Hyperedge) -> Result<HyperedgeId> {
        let id = hyperedge.id.clone();

        // Verify all nodes exist
        for node_id in &hyperedge.nodes {
            if !self.nodes.contains_key(node_id) {
                return Err(crate::error::GraphError::NodeNotFound(format!(
                    "Node {} not found",
                    node_id
                )));
            }
        }

        // Update index
        self.hyperedge_node_index.add_hyperedge(&hyperedge);

        // Insert into memory
        self.hyperedges.insert(id.clone(), hyperedge.clone());

        // Persist to storage if available
        #[cfg(feature = "storage")]
        if let Some(storage) = &self.storage {
            storage.insert_hyperedge(&hyperedge)?;
        }

        Ok(id)
    }

    /// Get a hyperedge by ID
    pub fn get_hyperedge(&self, id: &HyperedgeId) -> Option<Hyperedge> {
        self.hyperedges.get(id).map(|entry| entry.clone())
    }

    /// Get hyperedges containing a node
    pub fn get_hyperedges_by_node(&self, node_id: &NodeId) -> Vec<Hyperedge> {
        self.hyperedge_node_index
            .get_hyperedges_by_node(node_id)
            .into_iter()
            .filter_map(|id| self.get_hyperedge(&id))
            .collect()
    }

    // Statistics

    /// Get the number of nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of edges
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get the number of hyperedges
    pub fn hyperedge_count(&self) -> usize {
        self.hyperedges.len()
    }
}

impl Default for GraphDB {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge::EdgeBuilder;
    use crate::hyperedge::HyperedgeBuilder;
    use crate::node::NodeBuilder;

    #[test]
    fn test_graph_creation() {
        let db = GraphDB::new();
        assert_eq!(db.node_count(), 0);
        assert_eq!(db.edge_count(), 0);
    }

    #[test]
    fn test_node_operations() {
        let db = GraphDB::new();

        let node = NodeBuilder::new()
            .label("Person")
            .property("name", "Alice")
            .build();

        let id = db.create_node(node.clone()).unwrap();
        assert_eq!(db.node_count(), 1);

        let retrieved = db.get_node(&id);
        assert!(retrieved.is_some());

        let deleted = db.delete_node(&id).unwrap();
        assert!(deleted);
        assert_eq!(db.node_count(), 0);
    }

    #[test]
    fn test_edge_operations() {
        let db = GraphDB::new();

        let node1 = NodeBuilder::new().build();
        let node2 = NodeBuilder::new().build();

        let id1 = db.create_node(node1.clone()).unwrap();
        let id2 = db.create_node(node2.clone()).unwrap();

        let edge = EdgeBuilder::new(id1.clone(), id2.clone(), "KNOWS")
            .property("since", 2020i64)
            .build();

        let edge_id = db.create_edge(edge).unwrap();
        assert_eq!(db.edge_count(), 1);

        let retrieved = db.get_edge(&edge_id);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_label_index() {
        let db = GraphDB::new();

        let node1 = NodeBuilder::new().label("Person").build();
        let node2 = NodeBuilder::new().label("Person").build();
        let node3 = NodeBuilder::new().label("Organization").build();

        db.create_node(node1).unwrap();
        db.create_node(node2).unwrap();
        db.create_node(node3).unwrap();

        let people = db.get_nodes_by_label("Person");
        assert_eq!(people.len(), 2);

        let orgs = db.get_nodes_by_label("Organization");
        assert_eq!(orgs.len(), 1);
    }

    #[test]
    fn test_hyperedge_operations() {
        let db = GraphDB::new();

        let node1 = NodeBuilder::new().build();
        let node2 = NodeBuilder::new().build();
        let node3 = NodeBuilder::new().build();

        let id1 = db.create_node(node1).unwrap();
        let id2 = db.create_node(node2).unwrap();
        let id3 = db.create_node(node3).unwrap();

        let hyperedge =
            HyperedgeBuilder::new(vec![id1.clone(), id2.clone(), id3.clone()], "MEETING")
                .description("Team meeting")
                .build();

        let hedge_id = db.create_hyperedge(hyperedge).unwrap();
        assert_eq!(db.hyperedge_count(), 1);

        let hedges = db.get_hyperedges_by_node(&id1);
        assert_eq!(hedges.len(), 1);
    }
}
