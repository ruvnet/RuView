//! Node implementation

use crate::types::{Label, NodeId, Properties, PropertyValue};
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct Node {
    pub id: NodeId,
    pub labels: Vec<Label>,
    pub properties: Properties,
}

impl Node {
    pub fn new(id: NodeId, labels: Vec<Label>, properties: Properties) -> Self {
        Self {
            id,
            labels,
            properties,
        }
    }

    /// Check if node has a specific label
    pub fn has_label(&self, label_name: &str) -> bool {
        self.labels.iter().any(|l| l.name == label_name)
    }

    /// Get a property value by key
    pub fn get_property(&self, key: &str) -> Option<&PropertyValue> {
        self.properties.get(key)
    }

    /// Set a property value
    pub fn set_property(&mut self, key: impl Into<String>, value: PropertyValue) {
        self.properties.insert(key.into(), value);
    }

    /// Add a label to the node
    pub fn add_label(&mut self, label: impl Into<String>) {
        self.labels.push(Label::new(label));
    }

    /// Remove a label from the node
    pub fn remove_label(&mut self, label_name: &str) -> bool {
        let len_before = self.labels.len();
        self.labels.retain(|l| l.name != label_name);
        self.labels.len() < len_before
    }
}

/// Builder for constructing Node instances
#[derive(Debug, Clone, Default)]
pub struct NodeBuilder {
    id: Option<NodeId>,
    labels: Vec<Label>,
    properties: Properties,
}

impl NodeBuilder {
    /// Create a new node builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the node ID
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Add a label to the node
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(Label::new(label));
        self
    }

    /// Add multiple labels to the node
    pub fn labels(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for label in labels {
            self.labels.push(Label::new(label));
        }
        self
    }

    /// Add a property to the node
    pub fn property<V: Into<PropertyValue>>(mut self, key: impl Into<String>, value: V) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    /// Add multiple properties to the node
    pub fn properties(mut self, props: Properties) -> Self {
        self.properties.extend(props);
        self
    }

    /// Build the node
    pub fn build(self) -> Node {
        Node {
            id: self.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            labels: self.labels,
            properties: self.properties,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_builder() {
        let node = NodeBuilder::new()
            .label("Person")
            .property("name", "Alice")
            .property("age", 30i64)
            .build();

        assert!(node.has_label("Person"));
        assert!(!node.has_label("Organization"));
        assert_eq!(
            node.get_property("name"),
            Some(&PropertyValue::String("Alice".to_string()))
        );
    }

    #[test]
    fn test_node_has_label() {
        let node = NodeBuilder::new().label("Person").label("Employee").build();

        assert!(node.has_label("Person"));
        assert!(node.has_label("Employee"));
        assert!(!node.has_label("Company"));
    }

    #[test]
    fn test_node_modify_labels() {
        let mut node = NodeBuilder::new().label("Person").build();

        node.add_label("Employee");
        assert!(node.has_label("Employee"));

        let removed = node.remove_label("Person");
        assert!(removed);
        assert!(!node.has_label("Person"));
    }
}
