//! Knowledge graph integration for OSpipe.
//!
//! Provides entity extraction from captured text and stores entity relationships
//! in a [`ruvector_graph::GraphDB`] (native) or a lightweight in-memory stub (WASM).
//!
//! ## Usage
//!
//! ```rust,no_run
//! use ospipe::graph::KnowledgeGraph;
//!
//! let mut kg = KnowledgeGraph::new();
//! let ids = kg.ingest_frame_entities("frame-001", "Meeting with John Smith at https://meet.example.com").unwrap();
//! let people = kg.find_by_label("Person");
//! ```

pub mod entity_extractor;

use crate::error::Result;
use std::collections::HashMap;

/// A lightweight entity representation returned by query methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    /// Unique identifier for this entity.
    pub id: String,
    /// Category label (e.g. "Person", "Url", "Mention", "Email", "Frame").
    pub label: String,
    /// Human-readable name or value.
    pub name: String,
    /// Additional key-value properties.
    pub properties: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Native implementation (backed by ruvector-graph)
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
mod inner {
    use super::*;
    use crate::error::OsPipeError;
    use ruvector_graph::{EdgeBuilder, GraphDB, NodeBuilder, PropertyValue};

    /// A knowledge graph that stores entity relationships extracted from captured
    /// frames. On native targets this is backed by [`ruvector_graph::GraphDB`].
    pub struct KnowledgeGraph {
        db: GraphDB,
    }

    impl KnowledgeGraph {
        /// Create a new, empty knowledge graph.
        pub fn new() -> Self {
            Self { db: GraphDB::new() }
        }

        /// Add an entity node to the graph.
        ///
        /// Returns the newly created node ID.
        pub fn add_entity(
            &self,
            label: &str,
            name: &str,
            properties: HashMap<String, String>,
        ) -> Result<String> {
            let mut builder = NodeBuilder::new().label(label).property("name", name);

            for (k, v) in &properties {
                builder = builder.property(k.as_str(), v.as_str());
            }

            let node = builder.build();
            let id = self
                .db
                .create_node(node)
                .map_err(|e| OsPipeError::Storage(format!("graph: {}", e)))?;
            Ok(id)
        }

        /// Create a directed relationship (edge) between two entities.
        ///
        /// Both `from_id` and `to_id` must refer to existing nodes.
        /// Returns the edge ID.
        pub fn add_relationship(
            &self,
            from_id: &str,
            to_id: &str,
            rel_type: &str,
        ) -> Result<String> {
            let edge = EdgeBuilder::new(from_id.to_string(), to_id.to_string(), rel_type).build();
            let id = self
                .db
                .create_edge(edge)
                .map_err(|e| OsPipeError::Storage(format!("graph: {}", e)))?;
            Ok(id)
        }

        /// Find all entities that carry `label`.
        pub fn find_by_label(&self, label: &str) -> Vec<Entity> {
            self.db
                .get_nodes_by_label(label)
                .into_iter()
                .map(|n| node_to_entity(&n))
                .collect()
        }

        /// Find all entities directly connected to `entity_id` (both outgoing and
        /// incoming edges).
        pub fn neighbors(&self, entity_id: &str) -> Vec<Entity> {
            let mut seen = std::collections::HashSet::new();
            let mut result = Vec::new();

            let node_id = entity_id.to_string();

            // Outgoing neighbours.
            for edge in self.db.get_outgoing_edges(&node_id) {
                if seen.insert(edge.to.clone()) {
                    if let Some(node) = self.db.get_node(&edge.to) {
                        result.push(node_to_entity(&node));
                    }
                }
            }

            // Incoming neighbours.
            for edge in self.db.get_incoming_edges(&node_id) {
                if seen.insert(edge.from.clone()) {
                    if let Some(node) = self.db.get_node(&edge.from) {
                        result.push(node_to_entity(&node));
                    }
                }
            }

            result
        }

        /// Run heuristic NER on `text` and return extracted `(label, name)` pairs.
        pub fn extract_entities(text: &str) -> Vec<(String, String)> {
            entity_extractor::extract_entities(text)
        }

        /// Extract entities from `text`, create nodes for each, link them to the
        /// given `frame_id` node (creating the frame node if it does not yet exist),
        /// and return the IDs of all newly created entity nodes.
        pub fn ingest_frame_entities(&self, frame_id: &str, text: &str) -> Result<Vec<String>> {
            // Ensure frame node exists.
            let frame_node_id = if self.db.get_node(frame_id).is_some() {
                frame_id.to_string()
            } else {
                let node = NodeBuilder::new()
                    .id(frame_id)
                    .label("Frame")
                    .property("name", frame_id)
                    .build();
                self.db
                    .create_node(node)
                    .map_err(|e| OsPipeError::Storage(format!("graph: {}", e)))?
            };

            let extracted = entity_extractor::extract_entities(text);
            let mut entity_ids = Vec::with_capacity(extracted.len());

            for (label, name) in &extracted {
                let entity_id = self.add_entity(label, name, HashMap::new())?;
                self.add_relationship(&frame_node_id, &entity_id, "CONTAINS")?;
                entity_ids.push(entity_id);
            }

            Ok(entity_ids)
        }
    }

    impl Default for KnowledgeGraph {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Convert a `ruvector_graph::Node` into the crate-public `Entity` type.
    fn node_to_entity(node: &ruvector_graph::Node) -> Entity {
        let label = node
            .labels
            .first()
            .map_or_else(String::new, |l| l.name.clone());

        let name = match node.get_property("name") {
            Some(PropertyValue::String(s)) => s.clone(),
            _ => String::new(),
        };

        let mut properties = HashMap::new();
        for (k, v) in &node.properties {
            if k == "name" {
                continue;
            }
            let v_str = match v {
                PropertyValue::String(s) => s.clone(),
                PropertyValue::Integer(i) => i.to_string(),
                PropertyValue::Float(f) => f.to_string(),
                PropertyValue::Boolean(b) => b.to_string(),
                _ => format!("{:?}", v),
            };
            properties.insert(k.clone(), v_str);
        }

        Entity {
            id: node.id.clone(),
            label,
            name,
            properties,
        }
    }
}

// ---------------------------------------------------------------------------
// WASM fallback (lightweight in-memory stub)
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
mod inner {
    use super::*;

    struct StoredNode {
        id: String,
        label: String,
        name: String,
        properties: HashMap<String, String>,
    }

    struct StoredEdge {
        _id: String,
        from: String,
        to: String,
        _rel_type: String,
    }

    /// A knowledge graph backed by simple `Vec` storage for WASM targets.
    pub struct KnowledgeGraph {
        nodes: Vec<StoredNode>,
        edges: Vec<StoredEdge>,
        next_id: u64,
    }

    impl KnowledgeGraph {
        pub fn new() -> Self {
            Self {
                nodes: Vec::new(),
                edges: Vec::new(),
                next_id: 0,
            }
        }

        pub fn add_entity(
            &mut self,
            label: &str,
            name: &str,
            properties: HashMap<String, String>,
        ) -> Result<String> {
            let id = format!("wasm-{}", self.next_id);
            self.next_id += 1;
            self.nodes.push(StoredNode {
                id: id.clone(),
                label: label.to_string(),
                name: name.to_string(),
                properties,
            });
            Ok(id)
        }

        pub fn add_relationship(
            &mut self,
            from_id: &str,
            to_id: &str,
            rel_type: &str,
        ) -> Result<String> {
            let id = format!("wasm-e-{}", self.next_id);
            self.next_id += 1;
            self.edges.push(StoredEdge {
                _id: id.clone(),
                from: from_id.to_string(),
                to: to_id.to_string(),
                _rel_type: rel_type.to_string(),
            });
            Ok(id)
        }

        pub fn find_by_label(&self, label: &str) -> Vec<Entity> {
            self.nodes
                .iter()
                .filter(|n| n.label == label)
                .map(|n| Entity {
                    id: n.id.clone(),
                    label: n.label.clone(),
                    name: n.name.clone(),
                    properties: n.properties.clone(),
                })
                .collect()
        }

        pub fn neighbors(&self, entity_id: &str) -> Vec<Entity> {
            let mut ids = std::collections::HashSet::new();
            for e in &self.edges {
                if e.from == entity_id {
                    ids.insert(e.to.clone());
                }
                if e.to == entity_id {
                    ids.insert(e.from.clone());
                }
            }
            self.nodes
                .iter()
                .filter(|n| ids.contains(&n.id))
                .map(|n| Entity {
                    id: n.id.clone(),
                    label: n.label.clone(),
                    name: n.name.clone(),
                    properties: n.properties.clone(),
                })
                .collect()
        }

        pub fn extract_entities(text: &str) -> Vec<(String, String)> {
            entity_extractor::extract_entities(text)
        }

        pub fn ingest_frame_entities(&mut self, frame_id: &str, text: &str) -> Result<Vec<String>> {
            // Ensure frame node.
            let frame_exists = self.nodes.iter().any(|n| n.id == frame_id);
            let frame_node_id = if frame_exists {
                frame_id.to_string()
            } else {
                let id = frame_id.to_string();
                self.nodes.push(StoredNode {
                    id: id.clone(),
                    label: "Frame".to_string(),
                    name: frame_id.to_string(),
                    properties: HashMap::new(),
                });
                id
            };

            let extracted = entity_extractor::extract_entities(text);
            let mut entity_ids = Vec::with_capacity(extracted.len());
            for (label, name) in &extracted {
                let eid = self.add_entity(label, name, HashMap::new())?;
                self.add_relationship(&frame_node_id, &eid, "CONTAINS")?;
                entity_ids.push(eid);
            }
            Ok(entity_ids)
        }
    }

    impl Default for KnowledgeGraph {
        fn default() -> Self {
            Self::new()
        }
    }
}

// Re-export the platform-appropriate implementation.
pub use inner::KnowledgeGraph;
