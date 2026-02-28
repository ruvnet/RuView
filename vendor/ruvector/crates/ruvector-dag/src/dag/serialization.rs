//! DAG serialization and deserialization

use serde::{Deserialize, Serialize};

use super::operator_node::OperatorNode;
use super::query_dag::{DagError, QueryDag};

/// Serializable representation of a DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializableDag {
    nodes: Vec<OperatorNode>,
    edges: Vec<(usize, usize)>, // (parent, child) pairs
    root: Option<usize>,
}

/// Trait for DAG serialization
pub trait DagSerializer {
    /// Serialize to JSON string
    fn to_json(&self) -> Result<String, serde_json::Error>;

    /// Serialize to bytes (using bincode-like format via JSON for now)
    fn to_bytes(&self) -> Vec<u8>;
}

/// Trait for DAG deserialization
pub trait DagDeserializer {
    /// Deserialize from JSON string
    fn from_json(json: &str) -> Result<Self, serde_json::Error>
    where
        Self: Sized;

    /// Deserialize from bytes
    fn from_bytes(bytes: &[u8]) -> Result<Self, DagError>
    where
        Self: Sized;
}

impl DagSerializer for QueryDag {
    fn to_json(&self) -> Result<String, serde_json::Error> {
        let nodes: Vec<OperatorNode> = self.nodes.values().cloned().collect();

        let mut edges = Vec::new();
        for (&parent, children) in &self.edges {
            for &child in children {
                edges.push((parent, child));
            }
        }

        let serializable = SerializableDag {
            nodes,
            edges,
            root: self.root,
        };

        serde_json::to_string_pretty(&serializable)
    }

    fn to_bytes(&self) -> Vec<u8> {
        // For now, use JSON as bytes. In production, use bincode or similar
        self.to_json().unwrap_or_default().into_bytes()
    }
}

impl DagDeserializer for QueryDag {
    fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let serializable: SerializableDag = serde_json::from_str(json)?;

        let mut dag = QueryDag::new();

        // Create a mapping from old IDs to new IDs
        let mut id_map = std::collections::HashMap::new();

        // Add all nodes
        for node in serializable.nodes {
            let old_id = node.id;
            let new_id = dag.add_node(node);
            id_map.insert(old_id, new_id);
        }

        // Add all edges using mapped IDs
        for (parent, child) in serializable.edges {
            if let (Some(&new_parent), Some(&new_child)) = (id_map.get(&parent), id_map.get(&child))
            {
                // Ignore errors from edge addition during deserialization
                let _ = dag.add_edge(new_parent, new_child);
            }
        }

        // Map root if it exists
        if let Some(old_root) = serializable.root {
            dag.root = id_map.get(&old_root).copied();
        }

        Ok(dag)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, DagError> {
        let json = String::from_utf8(bytes.to_vec())
            .map_err(|e| DagError::InvalidOperation(format!("Invalid UTF-8: {}", e)))?;

        Self::from_json(&json)
            .map_err(|e| DagError::InvalidOperation(format!("Deserialization failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OperatorNode;

    #[test]
    fn test_json_serialization() {
        let mut dag = QueryDag::new();
        let id1 = dag.add_node(OperatorNode::seq_scan(0, "users"));
        let id2 = dag.add_node(OperatorNode::filter(0, "age > 18"));
        let id3 = dag.add_node(OperatorNode::sort(0, vec!["name".to_string()]));

        dag.add_edge(id1, id2).unwrap();
        dag.add_edge(id2, id3).unwrap();

        // Serialize
        let json = dag.to_json().unwrap();
        assert!(!json.is_empty());

        // Deserialize
        let deserialized = QueryDag::from_json(&json).unwrap();
        assert_eq!(deserialized.node_count(), 3);
        assert_eq!(deserialized.edge_count(), 2);
    }

    #[test]
    fn test_bytes_serialization() {
        let mut dag = QueryDag::new();
        let id1 = dag.add_node(OperatorNode::seq_scan(0, "users"));
        let id2 = dag.add_node(OperatorNode::filter(0, "age > 18"));

        dag.add_edge(id1, id2).unwrap();

        // Serialize to bytes
        let bytes = dag.to_bytes();
        assert!(!bytes.is_empty());

        // Deserialize from bytes
        let deserialized = QueryDag::from_bytes(&bytes).unwrap();
        assert_eq!(deserialized.node_count(), 2);
        assert_eq!(deserialized.edge_count(), 1);
    }

    #[test]
    fn test_complex_dag_roundtrip() {
        let mut dag = QueryDag::new();

        // Create a more complex DAG
        let scan1 = dag.add_node(OperatorNode::seq_scan(0, "users"));
        let scan2 = dag.add_node(OperatorNode::seq_scan(0, "orders"));
        let join = dag.add_node(OperatorNode::hash_join(0, "user_id"));
        let filter = dag.add_node(OperatorNode::filter(0, "total > 100"));
        let sort = dag.add_node(OperatorNode::sort(0, vec!["date".to_string()]));
        let limit = dag.add_node(OperatorNode::limit(0, 10));

        dag.add_edge(scan1, join).unwrap();
        dag.add_edge(scan2, join).unwrap();
        dag.add_edge(join, filter).unwrap();
        dag.add_edge(filter, sort).unwrap();
        dag.add_edge(sort, limit).unwrap();

        // Round trip
        let json = dag.to_json().unwrap();
        let restored = QueryDag::from_json(&json).unwrap();

        assert_eq!(restored.node_count(), dag.node_count());
        assert_eq!(restored.edge_count(), dag.edge_count());
    }

    #[test]
    fn test_empty_dag_serialization() {
        let dag = QueryDag::new();
        let json = dag.to_json().unwrap();
        let restored = QueryDag::from_json(&json).unwrap();

        assert_eq!(restored.node_count(), 0);
        assert_eq!(restored.edge_count(), 0);
    }
}
