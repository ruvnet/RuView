//! Edge (relationship) implementation

use crate::types::{EdgeId, NodeId, Properties, PropertyValue};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct Edge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: String,
    pub properties: Properties,
}

impl Edge {
    /// Create a new edge with all fields
    pub fn new(
        id: EdgeId,
        from: NodeId,
        to: NodeId,
        edge_type: String,
        properties: Properties,
    ) -> Self {
        Self {
            id,
            from,
            to,
            edge_type,
            properties,
        }
    }

    /// Create a new edge with auto-generated ID and empty properties
    pub fn create(from: NodeId, to: NodeId, edge_type: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            from,
            to,
            edge_type: edge_type.into(),
            properties: HashMap::new(),
        }
    }

    /// Get a property value by key
    pub fn get_property(&self, key: &str) -> Option<&PropertyValue> {
        self.properties.get(key)
    }

    /// Set a property value
    pub fn set_property(&mut self, key: impl Into<String>, value: PropertyValue) {
        self.properties.insert(key.into(), value);
    }
}

/// Builder for constructing Edge instances
#[derive(Debug, Clone)]
pub struct EdgeBuilder {
    id: Option<EdgeId>,
    from: NodeId,
    to: NodeId,
    edge_type: String,
    properties: Properties,
}

impl EdgeBuilder {
    /// Create a new edge builder with required fields
    pub fn new(from: NodeId, to: NodeId, edge_type: impl Into<String>) -> Self {
        Self {
            id: None,
            from,
            to,
            edge_type: edge_type.into(),
            properties: HashMap::new(),
        }
    }

    /// Set a custom edge ID
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Add a property to the edge
    pub fn property<V: Into<PropertyValue>>(mut self, key: impl Into<String>, value: V) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    /// Add multiple properties to the edge
    pub fn properties(mut self, props: Properties) -> Self {
        self.properties.extend(props);
        self
    }

    /// Build the edge
    pub fn build(self) -> Edge {
        Edge {
            id: self.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            from: self.from,
            to: self.to,
            edge_type: self.edge_type,
            properties: self.properties,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_builder() {
        let edge = EdgeBuilder::new("node1".to_string(), "node2".to_string(), "KNOWS")
            .property("since", 2020i64)
            .build();

        assert_eq!(edge.from, "node1");
        assert_eq!(edge.to, "node2");
        assert_eq!(edge.edge_type, "KNOWS");
        assert_eq!(
            edge.get_property("since"),
            Some(&PropertyValue::Integer(2020))
        );
    }

    #[test]
    fn test_edge_create() {
        let edge = Edge::create("a".to_string(), "b".to_string(), "FOLLOWS");
        assert_eq!(edge.from, "a");
        assert_eq!(edge.to, "b");
        assert_eq!(edge.edge_type, "FOLLOWS");
        assert!(edge.properties.is_empty());
    }

    #[test]
    fn test_edge_new() {
        let edge = Edge::new(
            "e1".to_string(),
            "n1".to_string(),
            "n2".to_string(),
            "LIKES".to_string(),
            HashMap::new(),
        );
        assert_eq!(edge.id, "e1");
        assert_eq!(edge.edge_type, "LIKES");
    }
}
