// Graph storage structures with concurrent access support

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

/// Node in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: u64,
    pub labels: Vec<String>,
    pub properties: HashMap<String, serde_json::Value>,
}

impl Node {
    pub fn new(id: u64) -> Self {
        Self {
            id,
            labels: Vec::new(),
            properties: HashMap::new(),
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    pub fn with_property(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    pub fn has_label(&self, label: &str) -> bool {
        self.labels.iter().any(|l| l == label)
    }

    pub fn get_property(&self, key: &str) -> Option<&serde_json::Value> {
        self.properties.get(key)
    }
}

/// Edge in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: u64,
    pub source: u64,
    pub target: u64,
    pub edge_type: String,
    pub properties: HashMap<String, serde_json::Value>,
}

impl Edge {
    pub fn new(id: u64, source: u64, target: u64, edge_type: impl Into<String>) -> Self {
        Self {
            id,
            source,
            target,
            edge_type: edge_type.into(),
            properties: HashMap::new(),
        }
    }

    pub fn with_property(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    pub fn get_property(&self, key: &str) -> Option<&serde_json::Value> {
        self.properties.get(key)
    }

    pub fn weight(&self, property: &str) -> f64 {
        self.get_property(property)
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0)
    }
}

/// Node storage with label indexing
pub struct NodeStore {
    nodes: DashMap<u64, Node>,
    label_index: DashMap<String, HashSet<u64>>,
    next_id: AtomicU64,
}

impl NodeStore {
    pub fn new() -> Self {
        Self {
            nodes: DashMap::new(),
            label_index: DashMap::new(),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn insert(&self, node: Node) {
        let id = node.id;

        // Update label index
        for label in &node.labels {
            self.label_index
                .entry(label.clone())
                .or_insert_with(HashSet::new)
                .insert(id);
        }

        self.nodes.insert(id, node);
    }

    pub fn get(&self, id: u64) -> Option<Node> {
        self.nodes.get(&id).map(|n| n.clone())
    }

    pub fn remove(&self, id: u64) -> Option<Node> {
        if let Some((_, node)) = self.nodes.remove(&id) {
            // Remove from label index
            for label in &node.labels {
                if let Some(mut ids) = self.label_index.get_mut(label) {
                    ids.remove(&id);
                }
            }
            Some(node)
        } else {
            None
        }
    }

    pub fn find_by_label(&self, label: &str) -> Vec<Node> {
        self.label_index
            .get(label)
            .map(|ids| ids.iter().filter_map(|id| self.get(*id)).collect())
            .unwrap_or_default()
    }

    pub fn all_nodes(&self) -> Vec<Node> {
        self.nodes.iter().map(|n| n.clone()).collect()
    }

    pub fn count(&self) -> usize {
        self.nodes.len()
    }

    pub fn contains(&self, id: u64) -> bool {
        self.nodes.contains_key(&id)
    }
}

impl Default for NodeStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Edge storage with adjacency list indexing
pub struct EdgeStore {
    edges: DashMap<u64, Edge>,
    // Adjacency list: source_id -> [(target_id, edge_id)]
    outgoing: DashMap<u64, Vec<(u64, u64)>>,
    // Reverse adjacency: target_id -> [(source_id, edge_id)]
    incoming: DashMap<u64, Vec<(u64, u64)>>,
    // Type index: edge_type -> [edge_id]
    type_index: DashMap<String, HashSet<u64>>,
    next_id: AtomicU64,
}

impl EdgeStore {
    pub fn new() -> Self {
        Self {
            edges: DashMap::new(),
            outgoing: DashMap::new(),
            incoming: DashMap::new(),
            type_index: DashMap::new(),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn insert(&self, edge: Edge) {
        let id = edge.id;
        let source = edge.source;
        let target = edge.target;
        let edge_type = edge.edge_type.clone();

        // Update adjacency lists
        self.outgoing
            .entry(source)
            .or_insert_with(Vec::new)
            .push((target, id));

        self.incoming
            .entry(target)
            .or_insert_with(Vec::new)
            .push((source, id));

        // Update type index
        self.type_index
            .entry(edge_type)
            .or_insert_with(HashSet::new)
            .insert(id);

        self.edges.insert(id, edge);
    }

    pub fn get(&self, id: u64) -> Option<Edge> {
        self.edges.get(&id).map(|e| e.clone())
    }

    pub fn remove(&self, id: u64) -> Option<Edge> {
        if let Some((_, edge)) = self.edges.remove(&id) {
            // Remove from adjacency lists
            if let Some(mut out) = self.outgoing.get_mut(&edge.source) {
                out.retain(|(_, eid)| *eid != id);
            }
            if let Some(mut inc) = self.incoming.get_mut(&edge.target) {
                inc.retain(|(_, eid)| *eid != id);
            }

            // Remove from type index
            if let Some(mut ids) = self.type_index.get_mut(&edge.edge_type) {
                ids.remove(&id);
            }

            Some(edge)
        } else {
            None
        }
    }

    pub fn get_outgoing(&self, node_id: u64) -> Vec<Edge> {
        self.outgoing
            .get(&node_id)
            .map(|edges| {
                edges
                    .iter()
                    .filter_map(|(_, edge_id)| self.get(*edge_id))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_incoming(&self, node_id: u64) -> Vec<Edge> {
        self.incoming
            .get(&node_id)
            .map(|edges| {
                edges
                    .iter()
                    .filter_map(|(_, edge_id)| self.get(*edge_id))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_neighbors(&self, node_id: u64) -> Vec<u64> {
        self.outgoing
            .get(&node_id)
            .map(|edges| edges.iter().map(|(target, _)| *target).collect())
            .unwrap_or_default()
    }

    pub fn find_by_type(&self, edge_type: &str) -> Vec<Edge> {
        self.type_index
            .get(edge_type)
            .map(|ids| ids.iter().filter_map(|id| self.get(*id)).collect())
            .unwrap_or_default()
    }

    pub fn all_edges(&self) -> Vec<Edge> {
        self.edges.iter().map(|e| e.clone()).collect()
    }

    pub fn count(&self) -> usize {
        self.edges.len()
    }
}

impl Default for EdgeStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete graph storage
pub struct GraphStore {
    pub nodes: NodeStore,
    pub edges: EdgeStore,
}

impl GraphStore {
    pub fn new() -> Self {
        Self {
            nodes: NodeStore::new(),
            edges: EdgeStore::new(),
        }
    }

    pub fn add_node(
        &self,
        labels: Vec<String>,
        properties: HashMap<String, serde_json::Value>,
    ) -> u64 {
        let id = self.nodes.next_id();
        let mut node = Node::new(id);
        node.labels = labels;
        node.properties = properties;
        self.nodes.insert(node);
        id
    }

    pub fn add_edge(
        &self,
        source: u64,
        target: u64,
        edge_type: String,
        properties: HashMap<String, serde_json::Value>,
    ) -> Result<u64, String> {
        // Validate nodes exist
        if !self.nodes.contains(source) {
            return Err(format!("Source node {} does not exist", source));
        }
        if !self.nodes.contains(target) {
            return Err(format!("Target node {} does not exist", target));
        }

        let id = self.edges.next_id();
        let mut edge = Edge::new(id, source, target, edge_type);
        edge.properties = properties;
        self.edges.insert(edge);
        Ok(id)
    }

    pub fn stats(&self) -> GraphStats {
        GraphStats {
            node_count: self.nodes.count(),
            edge_count: self.edges.count(),
            labels: self
                .nodes
                .label_index
                .iter()
                .map(|e| e.key().clone())
                .collect(),
            edge_types: self
                .edges
                .type_index
                .iter()
                .map(|e| e.key().clone())
                .collect(),
        }
    }
}

impl Default for GraphStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub labels: Vec<String>,
    pub edge_types: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_operations() {
        let store = NodeStore::new();

        let node = Node::new(1)
            .with_label("Person")
            .with_property("name", "Alice");

        store.insert(node.clone());

        let retrieved = store.get(1).unwrap();
        assert_eq!(retrieved.id, 1);
        assert!(retrieved.has_label("Person"));
        assert_eq!(
            retrieved.get_property("name").unwrap().as_str().unwrap(),
            "Alice"
        );

        let persons = store.find_by_label("Person");
        assert_eq!(persons.len(), 1);
    }

    #[test]
    fn test_edge_operations() {
        let store = EdgeStore::new();

        let edge = Edge::new(1, 10, 20, "KNOWS").with_property("since", 2020);

        store.insert(edge);

        let outgoing = store.get_outgoing(10);
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].target, 20);

        let neighbors = store.get_neighbors(10);
        assert_eq!(neighbors, vec![20]);
    }

    #[test]
    fn test_graph_store() {
        let graph = GraphStore::new();

        let n1 = graph.add_node(
            vec!["Person".to_string()],
            HashMap::from([("name".to_string(), "Alice".into())]),
        );

        let n2 = graph.add_node(
            vec!["Person".to_string()],
            HashMap::from([("name".to_string(), "Bob".into())]),
        );

        let e1 = graph
            .add_edge(
                n1,
                n2,
                "KNOWS".to_string(),
                HashMap::from([("since".to_string(), 2020.into())]),
            )
            .unwrap();

        assert_eq!(graph.nodes.count(), 2);
        assert_eq!(graph.edges.count(), 1);

        let stats = graph.stats();
        assert_eq!(stats.node_count, 2);
        assert_eq!(stats.edge_count, 1);
        assert!(stats.labels.contains(&"Person".to_string()));
        assert!(stats.edge_types.contains(&"KNOWS".to_string()));
    }
}
