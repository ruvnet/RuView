//! In-memory property graph storage for WASM-compatible Cypher execution

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

pub type NodeId = String;
pub type EdgeId = String;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),
    #[error("Edge not found: {0}")]
    EdgeNotFound(EdgeId),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

/// Property value that can be stored in nodes/edges
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    Map(HashMap<String, Value>),
}

impl Value {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Integer(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Boolean(b)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Integer(n)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

/// Node in the property graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub labels: Vec<String>,
    pub properties: HashMap<String, Value>,
}

impl Node {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            labels: Vec::new(),
            properties: HashMap::new(),
        }
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.labels.push(label);
        self
    }

    pub fn with_labels(mut self, labels: Vec<String>) -> Self {
        self.labels = labels;
        self
    }

    pub fn with_property(mut self, key: String, value: Value) -> Self {
        self.properties.insert(key, value);
        self
    }

    pub fn has_label(&self, label: &str) -> bool {
        self.labels.iter().any(|l| l == label)
    }

    pub fn get_property(&self, key: &str) -> Option<&Value> {
        self.properties.get(key)
    }

    pub fn set_property(&mut self, key: String, value: Value) {
        self.properties.insert(key, value);
    }

    pub fn remove_property(&mut self, key: &str) -> Option<Value> {
        self.properties.remove(key)
    }

    pub fn add_label(&mut self, label: String) {
        if !self.has_label(&label) {
            self.labels.push(label);
        }
    }

    pub fn remove_label(&mut self, label: &str) {
        self.labels.retain(|l| l != label);
    }
}

/// Edge/Relationship in the property graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: String,
    pub properties: HashMap<String, Value>,
}

impl Edge {
    pub fn new(id: EdgeId, from: NodeId, to: NodeId, edge_type: String) -> Self {
        Self {
            id,
            from,
            to,
            edge_type,
            properties: HashMap::new(),
        }
    }

    pub fn with_property(mut self, key: String, value: Value) -> Self {
        self.properties.insert(key, value);
        self
    }

    pub fn get_property(&self, key: &str) -> Option<&Value> {
        self.properties.get(key)
    }

    pub fn set_property(&mut self, key: String, value: Value) {
        self.properties.insert(key, value);
    }
}

/// In-memory property graph store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyGraph {
    nodes: HashMap<NodeId, Node>,
    edges: HashMap<EdgeId, Edge>,
    // Indexes for faster lookups
    label_index: HashMap<String, Vec<NodeId>>,
    edge_type_index: HashMap<String, Vec<EdgeId>>,
    outgoing_edges: HashMap<NodeId, Vec<EdgeId>>,
    incoming_edges: HashMap<NodeId, Vec<EdgeId>>,
    next_node_id: usize,
    next_edge_id: usize,
}

impl PropertyGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            label_index: HashMap::new(),
            edge_type_index: HashMap::new(),
            outgoing_edges: HashMap::new(),
            incoming_edges: HashMap::new(),
            next_node_id: 0,
            next_edge_id: 0,
        }
    }

    /// Generate a unique node ID
    pub fn generate_node_id(&mut self) -> NodeId {
        let id = format!("n{}", self.next_node_id);
        self.next_node_id += 1;
        id
    }

    /// Generate a unique edge ID
    pub fn generate_edge_id(&mut self) -> EdgeId {
        let id = format!("e{}", self.next_edge_id);
        self.next_edge_id += 1;
        id
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, node: Node) -> NodeId {
        let id = node.id.clone();

        // Update label index
        for label in &node.labels {
            self.label_index
                .entry(label.clone())
                .or_insert_with(Vec::new)
                .push(id.clone());
        }

        self.nodes.insert(id.clone(), node);
        id
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Get a mutable reference to a node
    pub fn get_node_mut(&mut self, id: &NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    /// Find nodes by label
    pub fn find_nodes_by_label(&self, label: &str) -> Vec<&Node> {
        if let Some(node_ids) = self.label_index.get(label) {
            node_ids
                .iter()
                .filter_map(|id| self.nodes.get(id))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Find all nodes matching a predicate
    pub fn find_nodes<F>(&self, predicate: F) -> Vec<&Node>
    where
        F: Fn(&Node) -> bool,
    {
        self.nodes.values().filter(|n| predicate(n)).collect()
    }

    /// Add an edge to the graph
    pub fn add_edge(&mut self, edge: Edge) -> Result<EdgeId, GraphError> {
        // Verify nodes exist
        if !self.nodes.contains_key(&edge.from) {
            return Err(GraphError::NodeNotFound(edge.from.clone()));
        }
        if !self.nodes.contains_key(&edge.to) {
            return Err(GraphError::NodeNotFound(edge.to.clone()));
        }

        let id = edge.id.clone();
        let from = edge.from.clone();
        let to = edge.to.clone();
        let edge_type = edge.edge_type.clone();

        // Update indexes
        self.edge_type_index
            .entry(edge_type)
            .or_insert_with(Vec::new)
            .push(id.clone());

        self.outgoing_edges
            .entry(from)
            .or_insert_with(Vec::new)
            .push(id.clone());

        self.incoming_edges
            .entry(to)
            .or_insert_with(Vec::new)
            .push(id.clone());

        self.edges.insert(id.clone(), edge);
        Ok(id)
    }

    /// Get an edge by ID
    pub fn get_edge(&self, id: &EdgeId) -> Option<&Edge> {
        self.edges.get(id)
    }

    /// Get outgoing edges from a node
    pub fn get_outgoing_edges(&self, node_id: &NodeId) -> Vec<&Edge> {
        if let Some(edge_ids) = self.outgoing_edges.get(node_id) {
            edge_ids
                .iter()
                .filter_map(|id| self.edges.get(id))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get incoming edges to a node
    pub fn get_incoming_edges(&self, node_id: &NodeId) -> Vec<&Edge> {
        if let Some(edge_ids) = self.incoming_edges.get(node_id) {
            edge_ids
                .iter()
                .filter_map(|id| self.edges.get(id))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all edges of a specific type
    pub fn find_edges_by_type(&self, edge_type: &str) -> Vec<&Edge> {
        if let Some(edge_ids) = self.edge_type_index.get(edge_type) {
            edge_ids
                .iter()
                .filter_map(|id| self.edges.get(id))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Delete a node and its connected edges
    pub fn delete_node(&mut self, id: &NodeId) -> Result<(), GraphError> {
        let node = self
            .nodes
            .remove(id)
            .ok_or_else(|| GraphError::NodeNotFound(id.clone()))?;

        // Remove from label index
        for label in &node.labels {
            if let Some(ids) = self.label_index.get_mut(label) {
                ids.retain(|nid| nid != id);
            }
        }

        // Remove connected edges
        if let Some(edge_ids) = self.outgoing_edges.remove(id) {
            for edge_id in edge_ids {
                self.edges.remove(&edge_id);
            }
        }

        if let Some(edge_ids) = self.incoming_edges.remove(id) {
            for edge_id in edge_ids {
                self.edges.remove(&edge_id);
            }
        }

        Ok(())
    }

    /// Delete an edge
    pub fn delete_edge(&mut self, id: &EdgeId) -> Result<(), GraphError> {
        let edge = self
            .edges
            .remove(id)
            .ok_or_else(|| GraphError::EdgeNotFound(id.clone()))?;

        // Remove from type index
        if let Some(ids) = self.edge_type_index.get_mut(&edge.edge_type) {
            ids.retain(|eid| eid != id);
        }

        // Remove from node edge lists
        if let Some(ids) = self.outgoing_edges.get_mut(&edge.from) {
            ids.retain(|eid| eid != id);
        }

        if let Some(ids) = self.incoming_edges.get_mut(&edge.to) {
            ids.retain(|eid| eid != id);
        }

        Ok(())
    }

    /// Get statistics about the graph
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            node_count: self.nodes.len(),
            edge_count: self.edges.len(),
            label_count: self.label_index.len(),
            edge_type_count: self.edge_type_index.len(),
        }
    }

    /// Get all nodes in the graph
    pub fn all_nodes(&self) -> Vec<&Node> {
        self.nodes.values().collect()
    }

    /// Get all edges in the graph
    pub fn all_edges(&self) -> Vec<&Edge> {
        self.edges.values().collect()
    }
}

impl Default for PropertyGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub label_count: usize,
    pub edge_type_count: usize,
}
