//! Node delta operations
//!
//! Represents changes to nodes in a graph.

use std::collections::HashMap;

use ruvector_delta_core::{Delta, VectorDelta};

use crate::{NodeId, PropertyOp, PropertyValue};

/// A property delta for nodes
#[derive(Debug, Clone)]
pub struct PropertyDelta {
    /// Property key
    pub key: String,
    /// Operation
    pub operation: PropertyOp,
}

impl PropertyDelta {
    /// Create a set operation
    pub fn set(key: impl Into<String>, value: PropertyValue) -> Self {
        Self {
            key: key.into(),
            operation: PropertyOp::Set(value),
        }
    }

    /// Create a remove operation
    pub fn remove(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            operation: PropertyOp::Remove,
        }
    }

    /// Create a vector delta operation
    pub fn vector_delta(key: impl Into<String>, delta: VectorDelta) -> Self {
        Self {
            key: key.into(),
            operation: PropertyOp::VectorDelta(delta),
        }
    }
}

/// Delta for a node
#[derive(Debug, Clone)]
pub struct NodeDelta {
    /// Property deltas
    pub property_deltas: Vec<PropertyDelta>,
    /// Label changes (add/remove)
    pub label_adds: Vec<String>,
    /// Labels to remove
    pub label_removes: Vec<String>,
}

impl NodeDelta {
    /// Create empty delta
    pub fn new() -> Self {
        Self {
            property_deltas: Vec::new(),
            label_adds: Vec::new(),
            label_removes: Vec::new(),
        }
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.property_deltas.is_empty()
            && self.label_adds.is_empty()
            && self.label_removes.is_empty()
    }

    /// Add a property set
    pub fn set_property(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        self.property_deltas.push(PropertyDelta::set(key, value));
        self
    }

    /// Add a property removal
    pub fn remove_property(mut self, key: impl Into<String>) -> Self {
        self.property_deltas.push(PropertyDelta::remove(key));
        self
    }

    /// Add a vector delta for an embedding property
    pub fn vector_delta(mut self, key: impl Into<String>, delta: VectorDelta) -> Self {
        self.property_deltas
            .push(PropertyDelta::vector_delta(key, delta));
        self
    }

    /// Add a label
    pub fn add_label(mut self, label: impl Into<String>) -> Self {
        self.label_adds.push(label.into());
        self
    }

    /// Remove a label
    pub fn remove_label(mut self, label: impl Into<String>) -> Self {
        self.label_removes.push(label.into());
        self
    }

    /// Compose with another delta
    pub fn compose(mut self, other: NodeDelta) -> Self {
        // Merge property deltas (later overrides earlier)
        let mut prop_map: HashMap<String, PropertyDelta> = HashMap::new();
        for pd in self.property_deltas {
            prop_map.insert(pd.key.clone(), pd);
        }
        for pd in other.property_deltas {
            // For vector deltas, compose them
            if let PropertyOp::VectorDelta(vd2) = &pd.operation {
                if let Some(existing) = prop_map.get(&pd.key) {
                    if let PropertyOp::VectorDelta(vd1) = &existing.operation {
                        let composed = vd1.clone().compose(vd2.clone());
                        prop_map.insert(
                            pd.key.clone(),
                            PropertyDelta::vector_delta(pd.key.clone(), composed),
                        );
                        continue;
                    }
                }
            }
            prop_map.insert(pd.key.clone(), pd);
        }
        self.property_deltas = prop_map.into_values().collect();

        // Merge label changes
        let mut adds: std::collections::HashSet<String> = self.label_adds.into_iter().collect();
        let mut removes: std::collections::HashSet<String> =
            self.label_removes.into_iter().collect();

        for label in other.label_adds {
            removes.remove(&label);
            adds.insert(label);
        }
        for label in other.label_removes {
            adds.remove(&label);
            removes.insert(label);
        }

        self.label_adds = adds.into_iter().collect();
        self.label_removes = removes.into_iter().collect();

        self
    }

    /// Compute inverse
    pub fn inverse(&self) -> Self {
        // Swap adds and removes for labels
        // Property deltas can't be fully inverted without originals
        Self {
            property_deltas: Vec::new(),
            label_adds: self.label_removes.clone(),
            label_removes: self.label_adds.clone(),
        }
    }
}

impl Default for NodeDelta {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for node deltas
pub struct NodeDeltaBuilder {
    delta: NodeDelta,
}

impl NodeDeltaBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            delta: NodeDelta::new(),
        }
    }

    /// Set a property
    pub fn set(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        self.delta
            .property_deltas
            .push(PropertyDelta::set(key, value));
        self
    }

    /// Remove a property
    pub fn remove(mut self, key: impl Into<String>) -> Self {
        self.delta.property_deltas.push(PropertyDelta::remove(key));
        self
    }

    /// Apply vector delta
    pub fn vector(mut self, key: impl Into<String>, delta: VectorDelta) -> Self {
        self.delta
            .property_deltas
            .push(PropertyDelta::vector_delta(key, delta));
        self
    }

    /// Add a label
    pub fn add_label(mut self, label: impl Into<String>) -> Self {
        self.delta.label_adds.push(label.into());
        self
    }

    /// Remove a label
    pub fn remove_label(mut self, label: impl Into<String>) -> Self {
        self.delta.label_removes.push(label.into());
        self
    }

    /// Build the delta
    pub fn build(self) -> NodeDelta {
        self.delta
    }
}

impl Default for NodeDeltaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_delta_builder() {
        let delta = NodeDeltaBuilder::new()
            .set("name", PropertyValue::String("Alice".into()))
            .set("age", PropertyValue::Int(30))
            .add_label("Person")
            .build();

        assert_eq!(delta.property_deltas.len(), 2);
        assert_eq!(delta.label_adds.len(), 1);
    }

    #[test]
    fn test_node_delta_compose() {
        let d1 = NodeDelta::new()
            .set_property("a", PropertyValue::Int(1))
            .add_label("Label1");

        let d2 = NodeDelta::new()
            .set_property("b", PropertyValue::Int(2))
            .remove_label("Label1")
            .add_label("Label2");

        let composed = d1.compose(d2);

        assert_eq!(composed.property_deltas.len(), 2);
        assert!(composed.label_adds.contains(&"Label2".to_string()));
        assert!(!composed.label_adds.contains(&"Label1".to_string()));
    }

    #[test]
    fn test_vector_delta_compose() {
        let vd1 = VectorDelta::from_dense(vec![1.0, 0.0, 0.0]);
        let vd2 = VectorDelta::from_dense(vec![0.0, 1.0, 0.0]);

        let d1 = NodeDelta::new().vector_delta("embedding", vd1);
        let d2 = NodeDelta::new().vector_delta("embedding", vd2);

        let composed = d1.compose(d2);

        assert_eq!(composed.property_deltas.len(), 1);

        if let PropertyOp::VectorDelta(vd) = &composed.property_deltas[0].operation {
            // Should be composed delta
            assert!(!vd.is_identity());
        } else {
            panic!("Expected vector delta");
        }
    }
}
