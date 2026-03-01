//! RvLite - Standalone vector database with SQL, SPARQL, and Cypher
//!
//! A WASM-compatible vector database powered by RuVector.
//!
//! # Features
//! - Vector storage and similarity search
//! - SQL queries with pgvector-compatible syntax
//! - SPARQL queries for RDF data
//! - Cypher queries for property graphs
//! - IndexedDB persistence for browsers
//!
//! # Example (JavaScript)
//! ```javascript
//! import init, { RvLite, RvLiteConfig } from './rvlite.js';
//!
//! await init();
//! const config = new RvLiteConfig(384);
//! const db = new RvLite(config);
//!
//! // Insert vectors
//! db.insert([0.1, 0.2, 0.3, ...], { label: "test" });
//!
//! // Search
//! const results = db.search([0.1, 0.2, 0.3, ...], 10);
//!
//! // Cypher queries
//! db.cypher("CREATE (n:Person {name: 'Alice'})");
//!
//! // SPARQL queries
//! db.add_triple("<http://example.org/a>", "<http://example.org/knows>", "<http://example.org/b>");
//! db.sparql("SELECT ?s WHERE { ?s <http://example.org/knows> ?o }");
//!
//! // Persistence
//! await db.save();  // Save to IndexedDB
//! const db2 = await RvLite.load(config);  // Load from IndexedDB
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

// Import ruvector-core
use ruvector_core::types::DbOptions;
use ruvector_core::{DistanceMetric, SearchQuery, VectorDB, VectorEntry};

// Query language modules
pub mod cypher;
pub mod sparql;
pub mod sql;
pub mod storage;

// Re-export storage types
pub use storage::{GraphState, RvLiteState, TripleStoreState, VectorState};

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"RvLite v0.2.0 - SQL, SPARQL, Cypher + Persistence".into());
}

/// Error type for RvLite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RvLiteError {
    pub message: String,
    pub kind: ErrorKind,
}

impl std::fmt::Display for RvLiteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for RvLiteError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorKind {
    VectorError,
    SqlError,
    CypherError,
    SparqlError,
    StorageError,
    WasmError,
    NotImplemented,
}

impl From<ruvector_core::RuvectorError> for RvLiteError {
    fn from(e: ruvector_core::RuvectorError) -> Self {
        RvLiteError {
            message: e.to_string(),
            kind: ErrorKind::VectorError,
        }
    }
}

impl From<RvLiteError> for JsValue {
    fn from(e: RvLiteError) -> Self {
        serde_wasm_bindgen::to_value(&e).unwrap_or_else(|_| JsValue::from_str(&e.message))
    }
}

impl From<sparql::SparqlError> for RvLiteError {
    fn from(e: sparql::SparqlError) -> Self {
        RvLiteError {
            message: e.to_string(),
            kind: ErrorKind::SparqlError,
        }
    }
}

impl From<sql::ParseError> for RvLiteError {
    fn from(e: sql::ParseError) -> Self {
        RvLiteError {
            message: e.to_string(),
            kind: ErrorKind::SqlError,
        }
    }
}

/// Configuration for RvLite database
#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct RvLiteConfig {
    /// Vector dimensions
    dimensions: usize,
    /// Distance metric (euclidean, cosine, dotproduct, manhattan)
    distance_metric: String,
}

#[wasm_bindgen]
impl RvLiteConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(dimensions: usize) -> Self {
        RvLiteConfig {
            dimensions,
            distance_metric: "cosine".to_string(),
        }
    }

    /// Set distance metric (euclidean, cosine, dotproduct, manhattan)
    pub fn with_distance_metric(mut self, metric: String) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Get dimensions
    pub fn get_dimensions(&self) -> usize {
        self.dimensions
    }

    /// Get distance metric name
    pub fn get_distance_metric(&self) -> String {
        self.distance_metric.clone()
    }
}

impl RvLiteConfig {
    fn to_db_options(&self) -> DbOptions {
        let metric = match self.distance_metric.to_lowercase().as_str() {
            "euclidean" => DistanceMetric::Euclidean,
            "cosine" => DistanceMetric::Cosine,
            "dotproduct" => DistanceMetric::DotProduct,
            "manhattan" => DistanceMetric::Manhattan,
            _ => DistanceMetric::Cosine,
        };

        DbOptions {
            dimensions: self.dimensions,
            distance_metric: metric,
            storage_path: "memory://".to_string(),
            hnsw_config: None,
            quantization: None,
        }
    }
}

/// Main RvLite database
#[wasm_bindgen]
pub struct RvLite {
    db: VectorDB,
    config: RvLiteConfig,
    cypher_engine: cypher::CypherEngine,
    sql_engine: sql::SqlEngine,
    triple_store: sparql::TripleStore,
    storage: Option<storage::IndexedDBStorage>,
}

#[wasm_bindgen]
impl RvLite {
    /// Create a new RvLite database
    #[wasm_bindgen(constructor)]
    pub fn new(config: RvLiteConfig) -> Result<RvLite, JsValue> {
        let db = VectorDB::new(config.to_db_options()).map_err(|e| RvLiteError::from(e))?;

        Ok(RvLite {
            db,
            config,
            cypher_engine: cypher::CypherEngine::new(),
            sql_engine: sql::SqlEngine::new(),
            triple_store: sparql::TripleStore::new(),
            storage: None,
        })
    }

    /// Create with default configuration (384 dimensions, cosine similarity)
    pub fn default() -> Result<RvLite, JsValue> {
        Self::new(RvLiteConfig::new(384))
    }

    /// Check if database is ready
    pub fn is_ready(&self) -> bool {
        true
    }

    /// Get version string
    pub fn get_version(&self) -> String {
        "0.2.0".to_string()
    }

    /// Get enabled features
    pub fn get_features(&self) -> Result<JsValue, JsValue> {
        let features = vec![
            "core",
            "vectors",
            "search",
            "sql",
            "sparql",
            "cypher",
            "memory-storage",
            "indexeddb-persistence",
        ];
        serde_wasm_bindgen::to_value(&features).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // ===== Persistence Methods =====

    /// Initialize IndexedDB storage for persistence
    /// Must be called before save() or load()
    pub fn init_storage(&mut self) -> js_sys::Promise {
        let mut storage = storage::IndexedDBStorage::new();

        future_to_promise(async move {
            storage.init().await?;
            Ok(JsValue::TRUE)
        })
    }

    /// Check if IndexedDB is available in the browser
    pub fn is_storage_available() -> bool {
        storage::IndexedDBStorage::is_available()
    }

    /// Save database state to IndexedDB
    /// Returns a Promise that resolves when save is complete
    pub fn save(&self) -> js_sys::Promise {
        let state = self.export_state();
        let mut storage = storage::IndexedDBStorage::new();

        future_to_promise(async move {
            storage.init().await?;
            storage.save(&state).await?;
            Ok(JsValue::TRUE)
        })
    }

    /// Load database from IndexedDB
    /// Returns a Promise<RvLite> with the restored database
    pub fn load(config: RvLiteConfig) -> js_sys::Promise {
        future_to_promise(async move {
            let mut storage = storage::IndexedDBStorage::new();
            storage.init().await?;

            let state = storage.load().await?;

            if let Some(state) = state {
                // Create new database with restored state
                let mut rvlite = RvLite::new(config)?;
                rvlite.import_state(&state)?;
                Ok(serde_wasm_bindgen::to_value(&"loaded").unwrap())
            } else {
                Ok(JsValue::NULL)
            }
        })
    }

    /// Check if saved state exists in IndexedDB
    pub fn has_saved_state() -> js_sys::Promise {
        future_to_promise(async move {
            let mut storage = storage::IndexedDBStorage::new();
            storage.init().await?;
            let exists = storage.exists().await?;
            Ok(JsValue::from_bool(exists))
        })
    }

    /// Clear saved state from IndexedDB
    pub fn clear_storage() -> js_sys::Promise {
        future_to_promise(async move {
            let mut storage = storage::IndexedDBStorage::new();
            storage.init().await?;
            storage.clear().await?;
            Ok(JsValue::TRUE)
        })
    }

    /// Export database state as JSON (for manual backup)
    pub fn export_json(&self) -> Result<JsValue, JsValue> {
        let state = self.export_state();
        serde_wasm_bindgen::to_value(&state)
            .map_err(|e| JsValue::from_str(&format!("Export failed: {}", e)))
    }

    /// Import database state from JSON
    pub fn import_json(&mut self, json: JsValue) -> Result<(), JsValue> {
        let state: RvLiteState = serde_wasm_bindgen::from_value(json)
            .map_err(|e| JsValue::from_str(&format!("Import failed: {}", e)))?;
        self.import_state(&state)
    }

    // ===== Vector Operations =====

    /// Insert a vector with optional metadata
    /// Returns the vector ID
    pub fn insert(&self, vector: Vec<f32>, metadata: JsValue) -> Result<String, JsValue> {
        let metadata_map = if metadata.is_null() || metadata.is_undefined() {
            None
        } else {
            Some(
                serde_wasm_bindgen::from_value::<HashMap<String, serde_json::Value>>(metadata)
                    .map_err(|e| RvLiteError {
                        message: format!("Invalid metadata: {}", e),
                        kind: ErrorKind::WasmError,
                    })?,
            )
        };

        let entry = VectorEntry {
            id: None,
            vector,
            metadata: metadata_map,
        };

        self.db
            .insert(entry)
            .map_err(|e| RvLiteError::from(e).into())
    }

    /// Insert a vector with a specific ID
    pub fn insert_with_id(
        &self,
        id: String,
        vector: Vec<f32>,
        metadata: JsValue,
    ) -> Result<(), JsValue> {
        let metadata_map = if metadata.is_null() || metadata.is_undefined() {
            None
        } else {
            Some(
                serde_wasm_bindgen::from_value::<HashMap<String, serde_json::Value>>(metadata)
                    .map_err(|e| RvLiteError {
                        message: format!("Invalid metadata: {}", e),
                        kind: ErrorKind::WasmError,
                    })?,
            )
        };

        let entry = VectorEntry {
            id: Some(id),
            vector,
            metadata: metadata_map,
        };

        self.db.insert(entry).map_err(|e| RvLiteError::from(e))?;

        Ok(())
    }

    /// Search for similar vectors
    pub fn search(&self, query_vector: Vec<f32>, k: usize) -> Result<JsValue, JsValue> {
        let query = SearchQuery {
            vector: query_vector,
            k,
            filter: None,
            ef_search: None,
        };

        let results = self.db.search(query).map_err(|e| RvLiteError::from(e))?;

        serde_wasm_bindgen::to_value(&results).map_err(|e| {
            RvLiteError {
                message: format!("Failed to serialize results: {}", e),
                kind: ErrorKind::WasmError,
            }
            .into()
        })
    }

    /// Search with metadata filter
    pub fn search_with_filter(
        &self,
        query_vector: Vec<f32>,
        k: usize,
        filter: JsValue,
    ) -> Result<JsValue, JsValue> {
        let filter_map = serde_wasm_bindgen::from_value::<HashMap<String, serde_json::Value>>(
            filter,
        )
        .map_err(|e| RvLiteError {
            message: format!("Invalid filter: {}", e),
            kind: ErrorKind::WasmError,
        })?;

        let query = SearchQuery {
            vector: query_vector,
            k,
            filter: Some(filter_map),
            ef_search: None,
        };

        let results = self.db.search(query).map_err(|e| RvLiteError::from(e))?;

        serde_wasm_bindgen::to_value(&results).map_err(|e| {
            RvLiteError {
                message: format!("Failed to serialize results: {}", e),
                kind: ErrorKind::WasmError,
            }
            .into()
        })
    }

    /// Get a vector by ID
    pub fn get(&self, id: String) -> Result<JsValue, JsValue> {
        let entry = self.db.get(&id).map_err(|e| RvLiteError::from(e))?;

        serde_wasm_bindgen::to_value(&entry).map_err(|e| {
            RvLiteError {
                message: format!("Failed to serialize entry: {}", e),
                kind: ErrorKind::WasmError,
            }
            .into()
        })
    }

    /// Delete a vector by ID
    pub fn delete(&self, id: String) -> Result<bool, JsValue> {
        self.db.delete(&id).map_err(|e| RvLiteError::from(e).into())
    }

    /// Get the number of vectors in the database
    pub fn len(&self) -> Result<usize, JsValue> {
        self.db.len().map_err(|e| RvLiteError::from(e).into())
    }

    /// Check if database is empty
    pub fn is_empty(&self) -> Result<bool, JsValue> {
        self.db.is_empty().map_err(|e| RvLiteError::from(e).into())
    }

    /// Get configuration
    pub fn get_config(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.config).map_err(|e| {
            RvLiteError {
                message: format!("Failed to serialize config: {}", e),
                kind: ErrorKind::WasmError,
            }
            .into()
        })
    }

    // ===== SQL Query Methods =====

    /// Execute SQL query
    ///
    /// Supported syntax:
    /// - CREATE TABLE vectors (id TEXT PRIMARY KEY, vector VECTOR(384))
    /// - SELECT * FROM vectors WHERE id = 'x'
    /// - SELECT id, vector <-> '[...]' AS distance FROM vectors ORDER BY distance LIMIT 10
    /// - INSERT INTO vectors (id, vector) VALUES ('x', '[...]')
    /// - DELETE FROM vectors WHERE id = 'x'
    pub fn sql(&self, query: String) -> Result<JsValue, JsValue> {
        // Parse SQL
        let mut parser = sql::SqlParser::new(&query).map_err(|e| RvLiteError {
            message: e.to_string(),
            kind: ErrorKind::SqlError,
        })?;
        let statement = parser.parse().map_err(|e| RvLiteError {
            message: e.to_string(),
            kind: ErrorKind::SqlError,
        })?;

        // Execute
        let result = self
            .sql_engine
            .execute(statement)
            .map_err(|e| RvLiteError {
                message: e.to_string(),
                kind: ErrorKind::SqlError,
            })?;

        // Use serde_json + js_sys::JSON::parse for proper serialization
        // (serde_wasm_bindgen can fail silently on complex enum types)
        let json_str = serde_json::to_string(&result).map_err(|e| RvLiteError {
            message: format!("Failed to serialize result: {}", e),
            kind: ErrorKind::WasmError,
        })?;

        js_sys::JSON::parse(&json_str).map_err(|e| {
            RvLiteError {
                message: format!("Failed to parse JSON: {:?}", e),
                kind: ErrorKind::WasmError,
            }
            .into()
        })
    }

    // ===== Cypher Query Methods =====

    /// Execute Cypher query
    ///
    /// Supported operations:
    /// - CREATE (n:Label {prop: value})
    /// - MATCH (n:Label) WHERE n.prop = value RETURN n
    /// - CREATE (a)-[r:REL]->(b)
    /// - DELETE n
    pub fn cypher(&mut self, query: String) -> Result<JsValue, JsValue> {
        self.cypher_engine.execute(&query)
    }

    /// Get Cypher graph statistics
    pub fn cypher_stats(&self) -> Result<JsValue, JsValue> {
        self.cypher_engine.stats()
    }

    /// Clear the Cypher graph
    pub fn cypher_clear(&mut self) {
        self.cypher_engine.clear();
    }

    // ===== SPARQL Query Methods =====

    /// Execute SPARQL query
    ///
    /// Supported operations:
    /// - SELECT ?s ?p ?o WHERE { ?s ?p ?o }
    /// - SELECT ?s WHERE { ?s <predicate> ?o }
    /// - ASK { ?s ?p ?o }
    pub fn sparql(&self, query: String) -> Result<JsValue, JsValue> {
        let parsed = sparql::parse_sparql(&query).map_err(|e| RvLiteError {
            message: format!("SPARQL parse error: {}", e),
            kind: ErrorKind::SparqlError,
        })?;

        let result = sparql::execute_sparql(&self.triple_store, &parsed)
            .map_err(|e| RvLiteError::from(e))?;

        // Convert result to serializable format
        let serializable = convert_sparql_result(&result);

        // Convert JSON to string and then parse in JS for proper object conversion
        let json_string = serializable.to_string();
        let js_obj = js_sys::JSON::parse(&json_string).map_err(|e| RvLiteError {
            message: format!("Failed to parse JSON: {:?}", e),
            kind: ErrorKind::WasmError,
        })?;

        Ok(js_obj)
    }

    /// Add an RDF triple
    ///
    /// # Arguments
    /// * `subject` - Subject IRI or blank node (e.g., "<http://example.org/s>" or "_:b1")
    /// * `predicate` - Predicate IRI (e.g., "<http://example.org/p>")
    /// * `object` - Object IRI, blank node, or literal (e.g., "<http://example.org/o>" or '"value"')
    pub fn add_triple(
        &self,
        subject: String,
        predicate: String,
        object: String,
    ) -> Result<(), JsValue> {
        let subj = parse_rdf_term(&subject)?;
        let pred = parse_iri(&predicate)?;
        let obj = parse_rdf_term(&object)?;

        let triple = sparql::Triple::new(subj, pred, obj);
        self.triple_store.insert(triple);
        Ok(())
    }

    /// Get the number of triples in the store
    pub fn triple_count(&self) -> usize {
        self.triple_store.count()
    }

    /// Clear all triples
    pub fn clear_triples(&self) {
        self.triple_store.clear();
    }
}

// Private impl block for state export/import
impl RvLite {
    /// Export the complete database state
    fn export_state(&self) -> RvLiteState {
        use storage::state::*;

        // Get current timestamp
        let saved_at = js_sys::Date::now() as u64;

        // Export vector state
        let vector_entries = self
            .db
            .keys()
            .unwrap_or_default()
            .iter()
            .filter_map(|id| {
                self.db
                    .get(id)
                    .ok()
                    .flatten()
                    .map(|entry| storage::state::VectorEntry {
                        id: entry.id.unwrap_or_default(),
                        vector: entry.vector,
                        metadata: entry.metadata,
                    })
            })
            .collect();

        let vectors = VectorState {
            entries: vector_entries,
            dimensions: self.config.dimensions,
            distance_metric: self.config.distance_metric.clone(),
            next_id: 0, // Will be recalculated on load
        };

        // Export graph state
        let graph = self.cypher_engine.export_state();

        // Export triple store state
        let triples = self.export_triple_state();

        // Export SQL schemas (not fully implemented yet)
        let sql_schemas = Vec::new();

        RvLiteState {
            version: 1,
            saved_at,
            vectors,
            graph,
            triples,
            sql_schemas,
        }
    }

    /// Import state into the database
    fn import_state(&mut self, state: &RvLiteState) -> Result<(), JsValue> {
        // Import vectors
        for entry in &state.vectors.entries {
            let vector_entry = VectorEntry {
                id: Some(entry.id.clone()),
                vector: entry.vector.clone(),
                metadata: entry.metadata.clone(),
            };
            self.db
                .insert(vector_entry)
                .map_err(|e| RvLiteError::from(e))?;
        }

        // Import graph
        self.cypher_engine.import_state(&state.graph)?;

        // Import triples
        self.import_triple_state(&state.triples)?;

        Ok(())
    }

    /// Export triple store state
    fn export_triple_state(&self) -> storage::state::TripleStoreState {
        use storage::state::*;

        let triples: Vec<TripleState> = self
            .triple_store
            .all_triples()
            .into_iter()
            .enumerate()
            .map(|(id, t)| TripleState {
                id: id as u64,
                subject: rdf_term_to_state(&t.subject),
                predicate: t.predicate.0.clone(),
                object: rdf_term_to_state(&t.object),
            })
            .collect();

        TripleStoreState {
            triples,
            named_graphs: HashMap::new(),
            default_graph: Vec::new(),
            next_id: 0,
        }
    }

    /// Import triple store state
    fn import_triple_state(&self, state: &storage::state::TripleStoreState) -> Result<(), JsValue> {
        self.triple_store.clear();

        for triple_state in &state.triples {
            let subject = state_to_rdf_term(&triple_state.subject)?;
            let predicate = sparql::Iri::new(&triple_state.predicate);
            let object = state_to_rdf_term(&triple_state.object)?;

            let triple = sparql::Triple::new(subject, predicate, object);
            self.triple_store.insert(triple);
        }

        Ok(())
    }
}

// Helper function to convert RdfTerm to clean JSON value
fn term_to_json(term: &sparql::ast::RdfTerm) -> serde_json::Value {
    use sparql::ast::RdfTerm;
    match term {
        RdfTerm::Iri(iri) => serde_json::json!({
            "type": "iri",
            "value": iri.as_str()
        }),
        RdfTerm::Literal(lit) => {
            let mut obj = serde_json::Map::new();
            obj.insert("type".to_string(), serde_json::json!("literal"));
            obj.insert("value".to_string(), serde_json::json!(lit.value.clone()));
            if let Some(lang) = &lit.language {
                obj.insert("language".to_string(), serde_json::json!(lang));
            }
            obj.insert(
                "datatype".to_string(),
                serde_json::json!(lit.datatype.as_str()),
            );
            serde_json::Value::Object(obj)
        }
        RdfTerm::BlankNode(id) => serde_json::json!({
            "type": "bnode",
            "value": id
        }),
    }
}

// Helper function to convert SPARQL result to serializable format
fn convert_sparql_result(result: &sparql::executor::QueryResult) -> serde_json::Value {
    use sparql::executor::QueryResult;

    match result {
        QueryResult::Select(select_result) => {
            let bindings: Vec<serde_json::Value> = select_result
                .bindings
                .iter()
                .map(|binding| {
                    let mut obj = serde_json::Map::new();
                    for (var, term) in binding {
                        obj.insert(var.clone(), term_to_json(term));
                    }
                    serde_json::Value::Object(obj)
                })
                .collect();

            serde_json::json!({
                "type": "select",
                "variables": select_result.variables,
                "bindings": bindings
            })
        }
        QueryResult::Ask(result) => {
            serde_json::json!({
                "type": "ask",
                "result": result
            })
        }
        QueryResult::Construct(triples) => {
            let triple_json: Vec<serde_json::Value> = triples
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "subject": term_to_json(&t.subject),
                        "predicate": t.predicate.0.clone(),
                        "object": term_to_json(&t.object)
                    })
                })
                .collect();

            serde_json::json!({
                "type": "construct",
                "triples": triple_json
            })
        }
        QueryResult::Describe(triples) => {
            let triple_json: Vec<serde_json::Value> = triples
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "subject": term_to_json(&t.subject),
                        "predicate": t.predicate.0.clone(),
                        "object": term_to_json(&t.object)
                    })
                })
                .collect();

            serde_json::json!({
                "type": "describe",
                "triples": triple_json
            })
        }
        QueryResult::Update => {
            serde_json::json!({
                "type": "update",
                "success": true
            })
        }
    }
}

// Helper functions for parsing RDF terms
fn parse_rdf_term(s: &str) -> Result<sparql::RdfTerm, JsValue> {
    let s = s.trim();
    if s.starts_with('<') && s.ends_with('>') {
        Ok(sparql::RdfTerm::iri(&s[1..s.len() - 1]))
    } else if s.starts_with("_:") {
        Ok(sparql::RdfTerm::blank(&s[2..]))
    } else if s.starts_with('"') {
        let end = s.rfind('"').unwrap_or(s.len() - 1);
        let value = &s[1..end];
        Ok(sparql::RdfTerm::literal(value))
    } else {
        Ok(sparql::RdfTerm::literal(s))
    }
}

fn parse_iri(s: &str) -> Result<sparql::Iri, JsValue> {
    let s = s.trim();
    if s.starts_with('<') && s.ends_with('>') {
        Ok(sparql::Iri::new(&s[1..s.len() - 1]))
    } else {
        Ok(sparql::Iri::new(s))
    }
}

// Helper functions for RDF term state conversion
fn rdf_term_to_state(term: &sparql::RdfTerm) -> storage::state::RdfTermState {
    use storage::state::RdfTermState;

    match term {
        sparql::RdfTerm::Iri(iri) => RdfTermState::Iri {
            value: iri.0.clone(),
        },
        sparql::RdfTerm::Literal(lit) => RdfTermState::Literal {
            value: lit.value.clone(),
            datatype: lit.datatype.0.clone(),
            language: lit.language.clone(),
        },
        sparql::RdfTerm::BlankNode(id) => RdfTermState::BlankNode { id: id.clone() },
    }
}

fn state_to_rdf_term(state: &storage::state::RdfTermState) -> Result<sparql::RdfTerm, JsValue> {
    use storage::state::RdfTermState;

    match state {
        RdfTermState::Iri { value } => Ok(sparql::RdfTerm::iri(value)),
        RdfTermState::Literal {
            value,
            datatype: _,
            language: _,
        } => Ok(sparql::RdfTerm::literal(value)),
        RdfTermState::BlankNode { id } => Ok(sparql::RdfTerm::blank(id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = RvLiteConfig::new(384);
        assert_eq!(config.dimensions, 384);
        assert_eq!(config.distance_metric, "cosine");
    }
}
