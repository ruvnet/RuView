//! JavaScript-friendly type conversions for graph database

use js_sys::{Array, Object, Reflect};
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::{from_value, to_value};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

/// Node ID type (alias for clarity)
pub type NodeId = String;

/// Edge ID type
pub type EdgeId = String;

/// Hyperedge ID type
pub type HyperedgeId = String;

/// JavaScript-compatible Node
#[wasm_bindgen]
#[derive(Clone)]
pub struct JsNode {
    pub(crate) id: NodeId,
    pub(crate) labels: Vec<String>,
    pub(crate) properties: HashMap<String, serde_json::Value>,
    pub(crate) embedding: Option<Vec<f32>>,
}

#[wasm_bindgen]
impl JsNode {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn labels(&self) -> Vec<String> {
        self.labels.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn properties(&self) -> JsValue {
        to_value(&self.properties).unwrap_or(JsValue::NULL)
    }

    #[wasm_bindgen(getter)]
    pub fn embedding(&self) -> Option<Vec<f32>> {
        self.embedding.clone()
    }

    /// Get a specific property value
    #[wasm_bindgen(js_name = getProperty)]
    pub fn get_property(&self, key: &str) -> JsValue {
        self.properties
            .get(key)
            .map(|v| to_value(v).unwrap())
            .unwrap_or(JsValue::NULL)
    }

    /// Check if node has a specific label
    #[wasm_bindgen(js_name = hasLabel)]
    pub fn has_label(&self, label: &str) -> bool {
        self.labels.contains(&label.to_string())
    }
}

/// JavaScript-compatible Edge (relationship)
#[wasm_bindgen]
#[derive(Clone)]
pub struct JsEdge {
    pub(crate) id: EdgeId,
    pub(crate) from: NodeId,
    pub(crate) to: NodeId,
    pub(crate) edge_type: String,
    pub(crate) properties: HashMap<String, serde_json::Value>,
}

#[wasm_bindgen]
impl JsEdge {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn from(&self) -> String {
        self.from.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn to(&self) -> String {
        self.to.clone()
    }

    #[wasm_bindgen(getter, js_name = "type")]
    pub fn edge_type(&self) -> String {
        self.edge_type.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn properties(&self) -> JsValue {
        to_value(&self.properties).unwrap_or(JsValue::NULL)
    }

    #[wasm_bindgen(js_name = getProperty)]
    pub fn get_property(&self, key: &str) -> JsValue {
        self.properties
            .get(key)
            .map(|v| to_value(v).unwrap())
            .unwrap_or(JsValue::NULL)
    }
}

/// JavaScript-compatible Hyperedge (n-ary relationship)
#[wasm_bindgen]
#[derive(Clone)]
pub struct JsHyperedge {
    pub(crate) id: HyperedgeId,
    pub(crate) nodes: Vec<NodeId>,
    pub(crate) description: String,
    pub(crate) embedding: Vec<f32>,
    pub(crate) confidence: f32,
    pub(crate) properties: HashMap<String, serde_json::Value>,
}

#[wasm_bindgen]
impl JsHyperedge {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn nodes(&self) -> Vec<String> {
        self.nodes.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn description(&self) -> String {
        self.description.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn embedding(&self) -> Vec<f32> {
        self.embedding.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn confidence(&self) -> f32 {
        self.confidence
    }

    #[wasm_bindgen(getter)]
    pub fn properties(&self) -> JsValue {
        to_value(&self.properties).unwrap_or(JsValue::NULL)
    }

    #[wasm_bindgen(getter)]
    pub fn order(&self) -> usize {
        self.nodes.len()
    }
}

/// Query result that can contain nodes, edges, or hyperedges
#[wasm_bindgen]
pub struct QueryResult {
    pub(crate) nodes: Vec<JsNode>,
    pub(crate) edges: Vec<JsEdge>,
    pub(crate) hyperedges: Vec<JsHyperedge>,
    pub(crate) data: Vec<HashMap<String, serde_json::Value>>,
}

#[wasm_bindgen]
impl QueryResult {
    #[wasm_bindgen(getter)]
    pub fn nodes(&self) -> Vec<JsNode> {
        self.nodes.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn edges(&self) -> Vec<JsEdge> {
        self.edges.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn hyperedges(&self) -> Vec<JsHyperedge> {
        self.hyperedges.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn data(&self) -> JsValue {
        to_value(&self.data).unwrap_or(JsValue::NULL)
    }

    #[wasm_bindgen(getter)]
    pub fn count(&self) -> usize {
        self.nodes.len() + self.edges.len() + self.hyperedges.len()
    }

    /// Check if result is empty
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }
}

/// WASM-specific error type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphError {
    pub message: String,
    pub kind: String,
}

impl GraphError {
    pub fn new(message: String, kind: String) -> Self {
        Self { message, kind }
    }
}

impl From<GraphError> for JsValue {
    fn from(err: GraphError) -> Self {
        let obj = Object::new();
        Reflect::set(&obj, &"message".into(), &err.message.into()).unwrap();
        Reflect::set(&obj, &"kind".into(), &err.kind.into()).unwrap();
        obj.into()
    }
}

impl From<String> for GraphError {
    fn from(msg: String) -> Self {
        GraphError::new(msg, "GraphError".to_string())
    }
}

/// Internal node representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Node {
    pub id: NodeId,
    pub labels: Vec<String>,
    pub properties: HashMap<String, serde_json::Value>,
    pub embedding: Option<Vec<f32>>,
}

impl Node {
    pub fn to_js(&self) -> JsNode {
        JsNode {
            id: self.id.clone(),
            labels: self.labels.clone(),
            properties: self.properties.clone(),
            embedding: self.embedding.clone(),
        }
    }
}

/// Internal edge representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Edge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub edge_type: String,
    pub properties: HashMap<String, serde_json::Value>,
}

impl Edge {
    pub fn to_js(&self) -> JsEdge {
        JsEdge {
            id: self.id.clone(),
            from: self.from.clone(),
            to: self.to.clone(),
            edge_type: self.edge_type.clone(),
            properties: self.properties.clone(),
        }
    }
}

/// Internal hyperedge representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Hyperedge {
    pub id: HyperedgeId,
    pub nodes: Vec<NodeId>,
    pub description: String,
    pub embedding: Vec<f32>,
    pub confidence: f32,
    pub properties: HashMap<String, serde_json::Value>,
}

impl Hyperedge {
    pub fn to_js(&self) -> JsHyperedge {
        JsHyperedge {
            id: self.id.clone(),
            nodes: self.nodes.clone(),
            description: self.description.clone(),
            embedding: self.embedding.clone(),
            confidence: self.confidence,
            properties: self.properties.clone(),
        }
    }
}

/// Convert JavaScript object to HashMap
pub(crate) fn js_object_to_hashmap(
    obj: JsValue,
) -> Result<HashMap<String, serde_json::Value>, String> {
    from_value(obj).map_err(|e| format!("Failed to convert JS object: {}", e))
}
