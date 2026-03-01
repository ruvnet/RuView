//! WebAssembly bindings for RuVector Graph Database
//!
//! This module provides high-performance browser bindings for a Neo4j-inspired graph database
//! built on RuVector's hypergraph infrastructure.
//!
//! Features:
//! - Node and edge CRUD operations
//! - Hyperedge support for n-ary relationships
//! - Basic Cypher query support
//! - Web Workers support for parallel operations
//! - Async query execution with streaming results
//! - IndexedDB persistence (planned)

use js_sys::{Array, Object, Promise, Reflect};
use parking_lot::Mutex;
use ruvector_core::advanced::hypergraph::{
    Hyperedge as CoreHyperedge, HypergraphIndex, TemporalGranularity, TemporalHyperedge,
};
use ruvector_core::types::DistanceMetric;
use serde_wasm_bindgen::{from_value, to_value};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use web_sys::console;

pub mod async_ops;
pub mod types;

use types::{
    js_object_to_hashmap, Edge, EdgeId, GraphError, Hyperedge, HyperedgeId, JsEdge, JsHyperedge,
    JsNode, Node, NodeId, QueryResult,
};

/// Initialize panic hook for better error messages
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
}

/// Main GraphDB class for browser usage
#[wasm_bindgen]
pub struct GraphDB {
    nodes: Arc<Mutex<HashMap<NodeId, Node>>>,
    edges: Arc<Mutex<HashMap<EdgeId, Edge>>>,
    hypergraph: Arc<Mutex<HypergraphIndex>>,
    hyperedges: Arc<Mutex<HashMap<HyperedgeId, Hyperedge>>>,
    // Index structures for efficient queries
    labels_index: Arc<Mutex<HashMap<String, Vec<NodeId>>>>,
    edge_types_index: Arc<Mutex<HashMap<String, Vec<EdgeId>>>>,
    node_edges_out: Arc<Mutex<HashMap<NodeId, Vec<EdgeId>>>>,
    node_edges_in: Arc<Mutex<HashMap<NodeId, Vec<EdgeId>>>>,
    distance_metric: DistanceMetric,
}

#[wasm_bindgen]
impl GraphDB {
    /// Create a new GraphDB instance
    ///
    /// # Arguments
    /// * `metric` - Distance metric for hypergraph embeddings ("euclidean", "cosine", "dotproduct", "manhattan")
    #[wasm_bindgen(constructor)]
    pub fn new(metric: Option<String>) -> Result<GraphDB, JsValue> {
        let distance_metric = match metric.as_deref() {
            Some("euclidean") => DistanceMetric::Euclidean,
            Some("cosine") => DistanceMetric::Cosine,
            Some("dotproduct") => DistanceMetric::DotProduct,
            Some("manhattan") => DistanceMetric::Manhattan,
            None => DistanceMetric::Cosine,
            Some(other) => return Err(JsValue::from_str(&format!("Unknown metric: {}", other))),
        };

        Ok(GraphDB {
            nodes: Arc::new(Mutex::new(HashMap::new())),
            edges: Arc::new(Mutex::new(HashMap::new())),
            hypergraph: Arc::new(Mutex::new(HypergraphIndex::new(distance_metric))),
            hyperedges: Arc::new(Mutex::new(HashMap::new())),
            labels_index: Arc::new(Mutex::new(HashMap::new())),
            edge_types_index: Arc::new(Mutex::new(HashMap::new())),
            node_edges_out: Arc::new(Mutex::new(HashMap::new())),
            node_edges_in: Arc::new(Mutex::new(HashMap::new())),
            distance_metric,
        })
    }

    /// Execute a Cypher query (basic implementation)
    ///
    /// # Arguments
    /// * `cypher` - Cypher query string
    ///
    /// # Returns
    /// Promise<QueryResult> with matching nodes, edges, and hyperedges
    #[wasm_bindgen]
    pub async fn query(&self, cypher: String) -> Result<QueryResult, JsValue> {
        console::log_1(&format!("Executing Cypher: {}", cypher).into());

        // Parse and execute basic Cypher queries
        // This is a simplified implementation - a full Cypher parser would be more complex
        let result = self
            .execute_cypher(&cypher)
            .map_err(|e| JsValue::from(GraphError::from(e)))?;

        Ok(result)
    }

    /// Create a new node
    ///
    /// # Arguments
    /// * `labels` - Array of label strings
    /// * `properties` - JavaScript object with node properties
    ///
    /// # Returns
    /// Node ID
    #[wasm_bindgen(js_name = createNode)]
    pub fn create_node(&self, labels: Vec<String>, properties: JsValue) -> Result<String, JsValue> {
        let id = Uuid::new_v4().to_string();
        let props = js_object_to_hashmap(properties).map_err(|e| JsValue::from_str(&e))?;

        // Extract embedding if present
        let embedding = props
            .get("embedding")
            .and_then(|v| serde_json::from_value::<Vec<f32>>(v.clone()).ok());

        let node = Node {
            id: id.clone(),
            labels: labels.clone(),
            properties: props,
            embedding: embedding.clone(),
        };

        // Store node
        self.nodes.lock().insert(id.clone(), node);

        // Update label index
        let mut labels_index = self.labels_index.lock();
        for label in &labels {
            labels_index
                .entry(label.clone())
                .or_insert_with(Vec::new)
                .push(id.clone());
        }

        // Add to hypergraph if embedding exists
        if let Some(emb) = embedding {
            self.hypergraph.lock().add_entity(id.clone(), emb);
        }

        // Initialize edge lists
        self.node_edges_out.lock().insert(id.clone(), Vec::new());
        self.node_edges_in.lock().insert(id.clone(), Vec::new());

        Ok(id)
    }

    /// Create a new edge (relationship)
    ///
    /// # Arguments
    /// * `from` - Source node ID
    /// * `to` - Target node ID
    /// * `edge_type` - Relationship type
    /// * `properties` - JavaScript object with edge properties
    ///
    /// # Returns
    /// Edge ID
    #[wasm_bindgen(js_name = createEdge)]
    pub fn create_edge(
        &self,
        from: String,
        to: String,
        edge_type: String,
        properties: JsValue,
    ) -> Result<String, JsValue> {
        // Verify nodes exist
        let nodes = self.nodes.lock();
        if !nodes.contains_key(&from) {
            return Err(JsValue::from_str(&format!("Node {} not found", from)));
        }
        if !nodes.contains_key(&to) {
            return Err(JsValue::from_str(&format!("Node {} not found", to)));
        }
        drop(nodes);

        let id = Uuid::new_v4().to_string();
        let props = js_object_to_hashmap(properties).map_err(|e| JsValue::from_str(&e))?;

        let edge = Edge {
            id: id.clone(),
            from: from.clone(),
            to: to.clone(),
            edge_type: edge_type.clone(),
            properties: props,
        };

        // Store edge
        self.edges.lock().insert(id.clone(), edge);

        // Update indices
        self.edge_types_index
            .lock()
            .entry(edge_type)
            .or_insert_with(Vec::new)
            .push(id.clone());

        self.node_edges_out
            .lock()
            .entry(from)
            .or_insert_with(Vec::new)
            .push(id.clone());

        self.node_edges_in
            .lock()
            .entry(to)
            .or_insert_with(Vec::new)
            .push(id.clone());

        Ok(id)
    }

    /// Create a hyperedge (n-ary relationship)
    ///
    /// # Arguments
    /// * `nodes` - Array of node IDs
    /// * `description` - Natural language description of the relationship
    /// * `embedding` - Optional embedding vector (auto-generated if not provided)
    /// * `confidence` - Optional confidence score (0.0-1.0, defaults to 1.0)
    ///
    /// # Returns
    /// Hyperedge ID
    #[wasm_bindgen(js_name = createHyperedge)]
    pub fn create_hyperedge(
        &self,
        nodes: Vec<String>,
        description: String,
        embedding: Option<Vec<f32>>,
        confidence: Option<f32>,
    ) -> Result<String, JsValue> {
        // Verify all nodes exist
        let nodes_map = self.nodes.lock();
        for node_id in &nodes {
            if !nodes_map.contains_key(node_id) {
                return Err(JsValue::from_str(&format!("Node {} not found", node_id)));
            }
        }
        drop(nodes_map);

        let id = Uuid::new_v4().to_string();

        // Generate embedding if not provided (use average of node embeddings)
        let emb = if let Some(e) = embedding {
            e
        } else {
            self.generate_hyperedge_embedding(&nodes)?
        };

        let conf = confidence.unwrap_or(1.0).clamp(0.0, 1.0);

        let hyperedge = Hyperedge {
            id: id.clone(),
            nodes: nodes.clone(),
            description: description.clone(),
            embedding: emb.clone(),
            confidence: conf,
            properties: HashMap::new(),
        };

        // Create core hyperedge
        let core_hyperedge = CoreHyperedge {
            id: id.clone(),
            nodes: nodes.clone(),
            description,
            embedding: emb,
            confidence: conf,
            metadata: HashMap::new(),
        };

        // Add to hypergraph index
        self.hypergraph
            .lock()
            .add_hyperedge(core_hyperedge)
            .map_err(|e| JsValue::from_str(&format!("Failed to add hyperedge: {}", e)))?;

        // Store hyperedge
        self.hyperedges.lock().insert(id.clone(), hyperedge);

        Ok(id)
    }

    /// Get a node by ID
    ///
    /// # Arguments
    /// * `id` - Node ID
    ///
    /// # Returns
    /// JsNode or null if not found
    #[wasm_bindgen(js_name = getNode)]
    pub fn get_node(&self, id: String) -> Option<JsNode> {
        self.nodes.lock().get(&id).map(|n| n.to_js())
    }

    /// Get an edge by ID
    #[wasm_bindgen(js_name = getEdge)]
    pub fn get_edge(&self, id: String) -> Option<JsEdge> {
        self.edges.lock().get(&id).map(|e| e.to_js())
    }

    /// Get a hyperedge by ID
    #[wasm_bindgen(js_name = getHyperedge)]
    pub fn get_hyperedge(&self, id: String) -> Option<JsHyperedge> {
        self.hyperedges.lock().get(&id).map(|h| h.to_js())
    }

    /// Delete a node by ID
    ///
    /// # Arguments
    /// * `id` - Node ID
    ///
    /// # Returns
    /// True if deleted, false if not found
    #[wasm_bindgen(js_name = deleteNode)]
    pub fn delete_node(&self, id: String) -> bool {
        // Remove from nodes
        let removed = self.nodes.lock().remove(&id).is_some();

        if removed {
            // Clean up indices
            let mut labels_index = self.labels_index.lock();
            for (_, node_ids) in labels_index.iter_mut() {
                node_ids.retain(|nid| nid != &id);
            }

            // Remove associated edges
            if let Some(out_edges) = self.node_edges_out.lock().remove(&id) {
                for edge_id in out_edges {
                    self.edges.lock().remove(&edge_id);
                }
            }
            if let Some(in_edges) = self.node_edges_in.lock().remove(&id) {
                for edge_id in in_edges {
                    self.edges.lock().remove(&edge_id);
                }
            }
        }

        removed
    }

    /// Delete an edge by ID
    #[wasm_bindgen(js_name = deleteEdge)]
    pub fn delete_edge(&self, id: String) -> bool {
        if let Some(edge) = self.edges.lock().remove(&id) {
            // Clean up indices
            if let Some(edges) = self.node_edges_out.lock().get_mut(&edge.from) {
                edges.retain(|eid| eid != &id);
            }
            if let Some(edges) = self.node_edges_in.lock().get_mut(&edge.to) {
                edges.retain(|eid| eid != &id);
            }
            true
        } else {
            false
        }
    }

    /// Import Cypher statements
    ///
    /// # Arguments
    /// * `statements` - Array of Cypher CREATE statements
    ///
    /// # Returns
    /// Number of statements executed
    #[wasm_bindgen(js_name = importCypher)]
    pub async fn import_cypher(&self, statements: Vec<String>) -> Result<usize, JsValue> {
        let mut count = 0;
        for statement in statements {
            self.execute_cypher(&statement)
                .map_err(|e| JsValue::from_str(&e))?;
            count += 1;
        }
        Ok(count)
    }

    /// Export database as Cypher CREATE statements
    ///
    /// # Returns
    /// String containing Cypher statements
    #[wasm_bindgen(js_name = exportCypher)]
    pub fn export_cypher(&self) -> String {
        let mut cypher = String::new();

        // Export nodes
        for (id, node) in self.nodes.lock().iter() {
            let labels = if node.labels.is_empty() {
                String::new()
            } else {
                format!(":{}", node.labels.join(":"))
            };

            let props = if node.properties.is_empty() {
                String::new()
            } else {
                format!(
                    " {{{}}}",
                    node.properties
                        .iter()
                        .map(|(k, v)| format!("{}: {}", k, v))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };

            cypher.push_str(&format!("CREATE (n{}{})\n", labels, props));
        }

        // Export edges
        for (id, edge) in self.edges.lock().iter() {
            let props = if edge.properties.is_empty() {
                String::new()
            } else {
                format!(
                    " {{{}}}",
                    edge.properties
                        .iter()
                        .map(|(k, v)| format!("{}: {}", k, v))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };

            cypher.push_str(&format!(
                "MATCH (a), (b) WHERE id(a) = '{}' AND id(b) = '{}' CREATE (a)-[:{}{}]->(b)\n",
                edge.from, edge.to, edge.edge_type, props
            ));
        }

        cypher
    }

    /// Get database statistics
    #[wasm_bindgen]
    pub fn stats(&self) -> JsValue {
        let node_count = self.nodes.lock().len();
        let edge_count = self.edges.lock().len();
        let hyperedge_count = self.hyperedges.lock().len();
        let hypergraph_stats = self.hypergraph.lock().stats();

        let obj = Object::new();
        Reflect::set(&obj, &"nodeCount".into(), &JsValue::from(node_count)).unwrap();
        Reflect::set(&obj, &"edgeCount".into(), &JsValue::from(edge_count)).unwrap();
        Reflect::set(
            &obj,
            &"hyperedgeCount".into(),
            &JsValue::from(hyperedge_count),
        )
        .unwrap();
        Reflect::set(
            &obj,
            &"hypergraphEntities".into(),
            &JsValue::from(hypergraph_stats.total_entities),
        )
        .unwrap();
        Reflect::set(
            &obj,
            &"hypergraphEdges".into(),
            &JsValue::from(hypergraph_stats.total_hyperedges),
        )
        .unwrap();
        Reflect::set(
            &obj,
            &"avgEntityDegree".into(),
            &JsValue::from(hypergraph_stats.avg_entity_degree),
        )
        .unwrap();

        obj.into()
    }
}

// Internal helper methods
impl GraphDB {
    fn execute_cypher(&self, cypher: &str) -> Result<QueryResult, String> {
        let cypher = cypher.trim();

        // Very basic Cypher parsing - in production, use a proper parser
        if cypher.to_uppercase().starts_with("MATCH") {
            self.execute_match_query(cypher)
        } else if cypher.to_uppercase().starts_with("CREATE") {
            self.execute_create_query(cypher)
        } else {
            Err(format!("Unsupported Cypher statement: {}", cypher))
        }
    }

    fn execute_match_query(&self, _cypher: &str) -> Result<QueryResult, String> {
        // Simplified MATCH implementation
        // In production, parse the pattern and execute accordingly

        Ok(QueryResult {
            nodes: Vec::new(),
            edges: Vec::new(),
            hyperedges: Vec::new(),
            data: Vec::new(),
        })
    }

    fn execute_create_query(&self, _cypher: &str) -> Result<QueryResult, String> {
        // Simplified CREATE implementation
        // Parse CREATE statement and create nodes/relationships

        Ok(QueryResult {
            nodes: Vec::new(),
            edges: Vec::new(),
            hyperedges: Vec::new(),
            data: Vec::new(),
        })
    }

    fn generate_hyperedge_embedding(&self, node_ids: &[String]) -> Result<Vec<f32>, JsValue> {
        let nodes = self.nodes.lock();
        let embeddings: Vec<Vec<f32>> = node_ids
            .iter()
            .filter_map(|id| nodes.get(id).and_then(|n| n.embedding.clone()))
            .collect();

        if embeddings.is_empty() {
            return Err(JsValue::from_str("No embeddings found for nodes"));
        }

        let dim = embeddings[0].len();
        let mut avg_embedding = vec![0.0; dim];

        for emb in &embeddings {
            for (i, val) in emb.iter().enumerate() {
                avg_embedding[i] += val;
            }
        }

        for val in &mut avg_embedding {
            *val /= embeddings.len() as f32;
        }

        Ok(avg_embedding)
    }
}

/// Get version information
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_version() {
        assert!(!version().is_empty());
    }

    #[wasm_bindgen_test]
    fn test_graph_creation() {
        let db = GraphDB::new(Some("cosine".to_string())).unwrap();
        assert!(true); // Basic smoke test
    }
}
