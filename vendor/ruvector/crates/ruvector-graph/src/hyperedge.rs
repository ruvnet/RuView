//! N-ary relationship support (hyperedges)
//!
//! Extends the basic edge model to support relationships connecting multiple nodes

use crate::types::{NodeId, Properties, PropertyValue};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// Unique identifier for a hyperedge
pub type HyperedgeId = String;

/// Hyperedge connecting multiple nodes (N-ary relationship)
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct Hyperedge {
    /// Unique identifier
    pub id: HyperedgeId,
    /// Node IDs connected by this hyperedge
    pub nodes: Vec<NodeId>,
    /// Hyperedge type/label (e.g., "MEETING", "COLLABORATION")
    pub edge_type: String,
    /// Natural language description of the relationship
    pub description: Option<String>,
    /// Property key-value pairs
    pub properties: Properties,
    /// Confidence/weight (0.0-1.0)
    pub confidence: f32,
}

impl Hyperedge {
    /// Create a new hyperedge with generated UUID
    pub fn new<S: Into<String>>(nodes: Vec<NodeId>, edge_type: S) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            nodes,
            edge_type: edge_type.into(),
            description: None,
            properties: Properties::new(),
            confidence: 1.0,
        }
    }

    /// Create a new hyperedge with specific ID
    pub fn with_id<S: Into<String>>(id: HyperedgeId, nodes: Vec<NodeId>, edge_type: S) -> Self {
        Self {
            id,
            nodes,
            edge_type: edge_type.into(),
            description: None,
            properties: Properties::new(),
            confidence: 1.0,
        }
    }

    /// Get the order of the hyperedge (number of nodes)
    pub fn order(&self) -> usize {
        self.nodes.len()
    }

    /// Check if hyperedge contains a specific node
    pub fn contains_node(&self, node_id: &NodeId) -> bool {
        self.nodes.contains(node_id)
    }

    /// Check if hyperedge contains all specified nodes
    pub fn contains_all_nodes(&self, node_ids: &[NodeId]) -> bool {
        node_ids.iter().all(|id| self.contains_node(id))
    }

    /// Check if hyperedge contains any of the specified nodes
    pub fn contains_any_node(&self, node_ids: &[NodeId]) -> bool {
        node_ids.iter().any(|id| self.contains_node(id))
    }

    /// Get unique nodes (removes duplicates)
    pub fn unique_nodes(&self) -> HashSet<&NodeId> {
        self.nodes.iter().collect()
    }

    /// Set the description
    pub fn set_description<S: Into<String>>(&mut self, description: S) -> &mut Self {
        self.description = Some(description.into());
        self
    }

    /// Set the confidence
    pub fn set_confidence(&mut self, confidence: f32) -> &mut Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set a property
    pub fn set_property<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: Into<String>,
        V: Into<PropertyValue>,
    {
        self.properties.insert(key.into(), value.into());
        self
    }

    /// Get a property
    pub fn get_property(&self, key: &str) -> Option<&PropertyValue> {
        self.properties.get(key)
    }

    /// Remove a property
    pub fn remove_property(&mut self, key: &str) -> Option<PropertyValue> {
        self.properties.remove(key)
    }

    /// Check if hyperedge has a property
    pub fn has_property(&self, key: &str) -> bool {
        self.properties.contains_key(key)
    }

    /// Get all property keys
    pub fn property_keys(&self) -> Vec<&String> {
        self.properties.keys().collect()
    }

    /// Clear all properties
    pub fn clear_properties(&mut self) {
        self.properties.clear();
    }

    /// Get the number of properties
    pub fn property_count(&self) -> usize {
        self.properties.len()
    }
}

/// Builder for creating hyperedges with fluent API
pub struct HyperedgeBuilder {
    hyperedge: Hyperedge,
}

impl HyperedgeBuilder {
    /// Create a new builder
    pub fn new<S: Into<String>>(nodes: Vec<NodeId>, edge_type: S) -> Self {
        Self {
            hyperedge: Hyperedge::new(nodes, edge_type),
        }
    }

    /// Create builder with specific ID
    pub fn with_id<S: Into<String>>(id: HyperedgeId, nodes: Vec<NodeId>, edge_type: S) -> Self {
        Self {
            hyperedge: Hyperedge::with_id(id, nodes, edge_type),
        }
    }

    /// Set description
    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.hyperedge.set_description(description);
        self
    }

    /// Set confidence
    pub fn confidence(mut self, confidence: f32) -> Self {
        self.hyperedge.set_confidence(confidence);
        self
    }

    /// Set a property
    pub fn property<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<PropertyValue>,
    {
        self.hyperedge.set_property(key, value);
        self
    }

    /// Build the hyperedge
    pub fn build(self) -> Hyperedge {
        self.hyperedge
    }
}

/// Hyperedge role assignment for directed N-ary relationships
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct HyperedgeWithRoles {
    /// Base hyperedge
    pub hyperedge: Hyperedge,
    /// Role assignments: node_id -> role
    pub roles: std::collections::HashMap<NodeId, String>,
}

impl HyperedgeWithRoles {
    /// Create a new hyperedge with roles
    pub fn new(hyperedge: Hyperedge) -> Self {
        Self {
            hyperedge,
            roles: std::collections::HashMap::new(),
        }
    }

    /// Assign a role to a node
    pub fn assign_role<S: Into<String>>(&mut self, node_id: NodeId, role: S) -> &mut Self {
        self.roles.insert(node_id, role.into());
        self
    }

    /// Get the role of a node
    pub fn get_role(&self, node_id: &NodeId) -> Option<&String> {
        self.roles.get(node_id)
    }

    /// Get all nodes with a specific role
    pub fn nodes_with_role(&self, role: &str) -> Vec<&NodeId> {
        self.roles
            .iter()
            .filter(|(_, r)| r.as_str() == role)
            .map(|(id, _)| id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hyperedge_creation() {
        let nodes = vec![
            "node1".to_string(),
            "node2".to_string(),
            "node3".to_string(),
        ];
        let hedge = Hyperedge::new(nodes, "MEETING");

        assert!(!hedge.id.is_empty());
        assert_eq!(hedge.order(), 3);
        assert_eq!(hedge.edge_type, "MEETING");
        assert_eq!(hedge.confidence, 1.0);
    }

    #[test]
    fn test_hyperedge_contains() {
        let nodes = vec![
            "node1".to_string(),
            "node2".to_string(),
            "node3".to_string(),
        ];
        let hedge = Hyperedge::new(nodes, "MEETING");

        assert!(hedge.contains_node(&"node1".to_string()));
        assert!(hedge.contains_node(&"node2".to_string()));
        assert!(!hedge.contains_node(&"node4".to_string()));

        assert!(hedge.contains_all_nodes(&["node1".to_string(), "node2".to_string()]));
        assert!(!hedge.contains_all_nodes(&["node1".to_string(), "node4".to_string()]));

        assert!(hedge.contains_any_node(&["node1".to_string(), "node4".to_string()]));
        assert!(!hedge.contains_any_node(&["node4".to_string(), "node5".to_string()]));
    }

    #[test]
    fn test_hyperedge_builder() {
        let nodes = vec!["node1".to_string(), "node2".to_string()];
        let hedge = HyperedgeBuilder::new(nodes, "COLLABORATION")
            .description("Team collaboration on project X")
            .confidence(0.95)
            .property("project", "X")
            .property("duration", 30i64)
            .build();

        assert_eq!(hedge.edge_type, "COLLABORATION");
        assert_eq!(hedge.confidence, 0.95);
        assert!(hedge.description.is_some());
        assert_eq!(
            hedge.get_property("project"),
            Some(&PropertyValue::String("X".to_string()))
        );
    }

    #[test]
    fn test_hyperedge_with_roles() {
        let nodes = vec![
            "alice".to_string(),
            "bob".to_string(),
            "charlie".to_string(),
        ];
        let hedge = Hyperedge::new(nodes, "MEETING");

        let mut hedge_with_roles = HyperedgeWithRoles::new(hedge);
        hedge_with_roles.assign_role("alice".to_string(), "organizer");
        hedge_with_roles.assign_role("bob".to_string(), "participant");
        hedge_with_roles.assign_role("charlie".to_string(), "participant");

        assert_eq!(
            hedge_with_roles.get_role(&"alice".to_string()),
            Some(&"organizer".to_string())
        );

        let participants = hedge_with_roles.nodes_with_role("participant");
        assert_eq!(participants.len(), 2);
    }

    #[test]
    fn test_unique_nodes() {
        let nodes = vec![
            "node1".to_string(),
            "node2".to_string(),
            "node1".to_string(), // duplicate
        ];
        let hedge = Hyperedge::new(nodes, "TEST");

        let unique = hedge.unique_nodes();
        assert_eq!(unique.len(), 2);
    }
}
