//! Edge delta operations
//!
//! Represents changes to edges in a graph.

use std::collections::HashMap;

use ruvector_delta_core::VectorDelta;

use crate::{EdgeId, NodeId, PropertyOp, PropertyValue};

/// An operation on an edge
#[derive(Debug, Clone)]
pub enum EdgeOp {
    /// Add a new edge
    Add {
        source: NodeId,
        target: NodeId,
        edge_type: String,
        properties: HashMap<String, PropertyValue>,
    },
    /// Remove an edge
    Remove,
    /// Update edge properties
    Update(Vec<PropertyDelta>),
    /// Change edge type
    Retype { new_type: String },
}

/// A property delta for edges
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
}

/// Delta for an edge
#[derive(Debug, Clone)]
pub struct EdgeDelta {
    /// Property deltas
    pub property_deltas: Vec<PropertyDelta>,
    /// Weight change (if applicable)
    pub weight_delta: Option<f64>,
    /// Type change
    pub type_change: Option<String>,
}

impl EdgeDelta {
    /// Create empty delta
    pub fn new() -> Self {
        Self {
            property_deltas: Vec::new(),
            weight_delta: None,
            type_change: None,
        }
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.property_deltas.is_empty() && self.weight_delta.is_none() && self.type_change.is_none()
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

    /// Set weight delta
    pub fn with_weight_delta(mut self, delta: f64) -> Self {
        self.weight_delta = Some(delta);
        self
    }

    /// Set type change
    pub fn with_type_change(mut self, new_type: impl Into<String>) -> Self {
        self.type_change = Some(new_type.into());
        self
    }

    /// Compose with another delta
    pub fn compose(mut self, other: EdgeDelta) -> Self {
        // Merge property deltas
        let mut prop_map: HashMap<String, PropertyDelta> = HashMap::new();
        for pd in self.property_deltas {
            prop_map.insert(pd.key.clone(), pd);
        }
        for pd in other.property_deltas {
            prop_map.insert(pd.key.clone(), pd);
        }
        self.property_deltas = prop_map.into_values().collect();

        // Compose weight delta
        if let Some(od) = other.weight_delta {
            self.weight_delta = Some(self.weight_delta.unwrap_or(0.0) + od);
        }

        // Take latest type change
        if other.type_change.is_some() {
            self.type_change = other.type_change;
        }

        self
    }

    /// Compute inverse
    pub fn inverse(&self) -> Self {
        Self {
            property_deltas: Vec::new(), // Can't invert without originals
            weight_delta: self.weight_delta.map(|d| -d),
            type_change: None, // Can't invert without original type
        }
    }
}

impl Default for EdgeDelta {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for edge operations
pub struct EdgeDeltaBuilder {
    delta: EdgeDelta,
}

impl EdgeDeltaBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            delta: EdgeDelta::new(),
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

    /// Set weight delta
    pub fn weight(mut self, delta: f64) -> Self {
        self.delta.weight_delta = Some(delta);
        self
    }

    /// Set type change
    pub fn retype(mut self, new_type: impl Into<String>) -> Self {
        self.delta.type_change = Some(new_type.into());
        self
    }

    /// Build the delta
    pub fn build(self) -> EdgeDelta {
        self.delta
    }
}

impl Default for EdgeDeltaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_delta_builder() {
        let delta = EdgeDeltaBuilder::new()
            .set("weight", PropertyValue::Float(1.5))
            .set("label", PropertyValue::String("test".into()))
            .build();

        assert_eq!(delta.property_deltas.len(), 2);
    }

    #[test]
    fn test_edge_delta_compose() {
        let d1 = EdgeDelta::new()
            .set_property("a", PropertyValue::Int(1))
            .with_weight_delta(1.0);

        let d2 = EdgeDelta::new()
            .set_property("b", PropertyValue::Int(2))
            .with_weight_delta(0.5);

        let composed = d1.compose(d2);

        assert_eq!(composed.property_deltas.len(), 2);
        assert!((composed.weight_delta.unwrap() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn test_edge_delta_inverse() {
        let delta = EdgeDelta::new().with_weight_delta(2.0);
        let inverse = delta.inverse();

        assert!((inverse.weight_delta.unwrap() - (-2.0)).abs() < 1e-6);
    }
}
