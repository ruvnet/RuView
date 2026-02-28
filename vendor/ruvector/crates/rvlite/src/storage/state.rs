//! Serializable state structures for RvLite persistence
//!
//! These structures represent the complete state of the RvLite database
//! in a format that can be serialized to/from IndexedDB.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete serializable state for RvLite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RvLiteState {
    /// Version for schema migration
    pub version: u32,
    /// Timestamp of last save
    pub saved_at: u64,
    /// Vector database state
    pub vectors: VectorState,
    /// Cypher graph state
    pub graph: GraphState,
    /// SPARQL triple store state
    pub triples: TripleStoreState,
    /// SQL engine schemas
    pub sql_schemas: Vec<SqlTableState>,
}

impl Default for RvLiteState {
    fn default() -> Self {
        Self {
            version: 1,
            saved_at: 0,
            vectors: VectorState::default(),
            graph: GraphState::default(),
            triples: TripleStoreState::default(),
            sql_schemas: Vec::new(),
        }
    }
}

/// Serializable vector database state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VectorState {
    /// Vector entries: id -> (vector, metadata)
    pub entries: Vec<VectorEntry>,
    /// Database dimensions
    pub dimensions: usize,
    /// Distance metric name
    pub distance_metric: String,
    /// Next auto-generated ID counter
    pub next_id: u64,
}

/// Single vector entry for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    pub id: String,
    pub vector: Vec<f32>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Serializable Cypher graph state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphState {
    /// All nodes
    pub nodes: Vec<NodeState>,
    /// All edges
    pub edges: Vec<EdgeState>,
    /// Next node ID counter
    pub next_node_id: usize,
    /// Next edge ID counter
    pub next_edge_id: usize,
}

/// Serializable node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub id: String,
    pub labels: Vec<String>,
    pub properties: HashMap<String, PropertyValue>,
}

/// Serializable edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeState {
    pub id: String,
    pub from: String,
    pub to: String,
    pub edge_type: String,
    pub properties: HashMap<String, PropertyValue>,
}

/// Property value for serialization (mirrors cypher::Value)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum PropertyValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<PropertyValue>),
    Map(HashMap<String, PropertyValue>),
}

/// Serializable SPARQL triple store state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TripleStoreState {
    /// All triples
    pub triples: Vec<TripleState>,
    /// Named graphs
    pub named_graphs: HashMap<String, Vec<u64>>,
    /// Default graph triple IDs
    pub default_graph: Vec<u64>,
    /// Next triple ID counter
    pub next_id: u64,
}

/// Serializable RDF triple
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripleState {
    pub id: u64,
    pub subject: RdfTermState,
    pub predicate: String,
    pub object: RdfTermState,
}

/// Serializable RDF term
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RdfTermState {
    Iri {
        value: String,
    },
    Literal {
        value: String,
        datatype: String,
        language: Option<String>,
    },
    BlankNode {
        id: String,
    },
}

/// Serializable SQL table schema state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlTableState {
    pub name: String,
    pub columns: Vec<SqlColumnState>,
    pub vector_column: Option<String>,
    pub vector_dimensions: Option<usize>,
}

/// Serializable SQL column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlColumnState {
    pub name: String,
    pub data_type: String,
    pub dimensions: Option<usize>,
}
