//! WASM bindings for EXO-AI 2025 Cognitive Substrate
//!
//! This module provides browser bindings for the EXO substrate, enabling:
//! - Pattern storage and retrieval
//! - Similarity search with various distance metrics
//! - Temporal memory coordination
//! - Causal queries
//! - Browser-based cognitive operations

use js_sys::{Array, Float32Array, Object, Promise, Reflect};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::{from_value, to_value};
use std::collections::HashMap;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use web_sys::console;

mod types;
mod utils;

pub use types::*;
pub use utils::*;

/// Initialize panic hook and tracing for better error messages
#[wasm_bindgen(start)]
pub fn init() {
    utils::set_panic_hook();
    tracing_wasm::set_as_global_default();
}

/// WASM-specific error type that can cross the JS boundary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExoError {
    pub message: String,
    pub kind: String,
}

impl ExoError {
    pub fn new(message: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: kind.into(),
        }
    }
}

impl From<ExoError> for JsValue {
    fn from(err: ExoError) -> Self {
        let obj = Object::new();
        Reflect::set(&obj, &"message".into(), &err.message.into()).unwrap();
        Reflect::set(&obj, &"kind".into(), &err.kind.into()).unwrap();
        obj.into()
    }
}

impl From<String> for ExoError {
    fn from(s: String) -> Self {
        ExoError::new(s, "Error")
    }
}

#[allow(dead_code)]
type ExoResult<T> = Result<T, ExoError>;

/// Configuration for EXO substrate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubstrateConfig {
    /// Vector dimensions
    pub dimensions: usize,
    /// Distance metric (euclidean, cosine, dotproduct, manhattan)
    #[serde(default = "default_metric")]
    pub distance_metric: String,
    /// Enable HNSW index for faster search
    #[serde(default = "default_true")]
    pub use_hnsw: bool,
    /// Enable temporal memory coordination
    #[serde(default = "default_true")]
    pub enable_temporal: bool,
    /// Enable causal tracking
    #[serde(default = "default_true")]
    pub enable_causal: bool,
}

fn default_metric() -> String {
    "cosine".to_string()
}

fn default_true() -> bool {
    true
}

/// Pattern representation in the cognitive substrate
#[wasm_bindgen]
#[derive(Clone)]
pub struct Pattern {
    inner: PatternInner,
}

#[derive(Clone, Serialize, Deserialize)]
struct PatternInner {
    /// Vector embedding
    embedding: Vec<f32>,
    /// Metadata (stored as HashMap to match ruvector-core)
    metadata: Option<HashMap<String, serde_json::Value>>,
    /// Temporal timestamp (milliseconds since epoch)
    timestamp: f64,
    /// Pattern ID
    id: Option<String>,
    /// Causal antecedents (IDs of patterns that influenced this one)
    antecedents: Vec<String>,
}

#[wasm_bindgen]
impl Pattern {
    #[wasm_bindgen(constructor)]
    pub fn new(
        embedding: Float32Array,
        metadata: Option<JsValue>,
        antecedents: Option<Vec<String>>,
    ) -> Result<Pattern, JsValue> {
        let embedding_vec = embedding.to_vec();

        if embedding_vec.is_empty() {
            return Err(JsValue::from_str("Embedding cannot be empty"));
        }

        let metadata = if let Some(meta) = metadata {
            let json_val: serde_json::Value = from_value(meta)
                .map_err(|e| JsValue::from_str(&format!("Invalid metadata: {}", e)))?;
            // Convert to HashMap if it's an object, otherwise wrap it
            match json_val {
                serde_json::Value::Object(map) => Some(map.into_iter().collect()),
                other => {
                    let mut map = HashMap::new();
                    map.insert("value".to_string(), other);
                    Some(map)
                }
            }
        } else {
            None
        };

        Ok(Pattern {
            inner: PatternInner {
                embedding: embedding_vec,
                metadata,
                timestamp: js_sys::Date::now(),
                id: None,
                antecedents: antecedents.unwrap_or_default(),
            },
        })
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> Option<String> {
        self.inner.id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn embedding(&self) -> Float32Array {
        Float32Array::from(&self.inner.embedding[..])
    }

    #[wasm_bindgen(getter)]
    pub fn metadata(&self) -> Option<JsValue> {
        self.inner.metadata.as_ref().map(|m| {
            let json_val = serde_json::Value::Object(m.clone().into_iter().collect());
            to_value(&json_val).unwrap()
        })
    }

    #[wasm_bindgen(getter)]
    pub fn timestamp(&self) -> f64 {
        self.inner.timestamp
    }

    #[wasm_bindgen(getter)]
    pub fn antecedents(&self) -> Vec<String> {
        self.inner.antecedents.clone()
    }
}

/// Search result from substrate query
#[wasm_bindgen]
pub struct SearchResult {
    inner: SearchResultInner,
}

#[derive(Clone, Serialize, Deserialize)]
struct SearchResultInner {
    id: String,
    score: f32,
    pattern: Option<PatternInner>,
}

#[wasm_bindgen]
impl SearchResult {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.inner.id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn score(&self) -> f32 {
        self.inner.score
    }

    #[wasm_bindgen(getter)]
    pub fn pattern(&self) -> Option<Pattern> {
        self.inner.pattern.clone().map(|p| Pattern { inner: p })
    }
}

/// Main EXO substrate interface for browser deployment
#[wasm_bindgen]
pub struct ExoSubstrate {
    // Using ruvector-core as placeholder until exo-core is implemented
    db: Arc<Mutex<ruvector_core::vector_db::VectorDB>>,
    config: SubstrateConfig,
    dimensions: usize,
}

#[wasm_bindgen]
impl ExoSubstrate {
    /// Create a new EXO substrate instance
    ///
    /// # Arguments
    /// * `config` - Configuration object with dimensions, distance_metric, etc.
    ///
    /// # Example
    /// ```javascript
    /// const substrate = new ExoSubstrate({
    ///   dimensions: 384,
    ///   distance_metric: "cosine",
    ///   use_hnsw: true,
    ///   enable_temporal: true,
    ///   enable_causal: true
    /// });
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<ExoSubstrate, JsValue> {
        let config: SubstrateConfig =
            from_value(config).map_err(|e| JsValue::from_str(&format!("Invalid config: {}", e)))?;

        // Validate configuration
        if config.dimensions == 0 {
            return Err(JsValue::from_str("Dimensions must be greater than 0"));
        }

        // Create underlying vector database
        let distance_metric = match config.distance_metric.as_str() {
            "euclidean" => ruvector_core::types::DistanceMetric::Euclidean,
            "cosine" => ruvector_core::types::DistanceMetric::Cosine,
            "dotproduct" => ruvector_core::types::DistanceMetric::DotProduct,
            "manhattan" => ruvector_core::types::DistanceMetric::Manhattan,
            _ => {
                return Err(JsValue::from_str(&format!(
                    "Unknown distance metric: {}",
                    config.distance_metric
                )))
            }
        };

        let hnsw_config = if config.use_hnsw {
            Some(ruvector_core::types::HnswConfig::default())
        } else {
            None
        };

        let db_options = ruvector_core::types::DbOptions {
            dimensions: config.dimensions,
            distance_metric,
            storage_path: ":memory:".to_string(), // WASM uses in-memory storage
            hnsw_config,
            quantization: None,
        };

        let db = ruvector_core::vector_db::VectorDB::new(db_options)
            .map_err(|e| JsValue::from_str(&format!("Failed to create substrate: {}", e)))?;

        console::log_1(
            &format!(
                "EXO substrate initialized with {} dimensions",
                config.dimensions
            )
            .into(),
        );

        Ok(ExoSubstrate {
            db: Arc::new(Mutex::new(db)),
            dimensions: config.dimensions,
            config,
        })
    }

    /// Store a pattern in the substrate
    ///
    /// # Arguments
    /// * `pattern` - Pattern object with embedding, metadata, and optional antecedents
    ///
    /// # Returns
    /// Pattern ID as a string
    #[wasm_bindgen]
    pub fn store(&self, pattern: &Pattern) -> Result<String, JsValue> {
        if pattern.inner.embedding.len() != self.dimensions {
            return Err(JsValue::from_str(&format!(
                "Pattern embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                pattern.inner.embedding.len()
            )));
        }

        let entry = ruvector_core::types::VectorEntry {
            id: pattern.inner.id.clone(),
            vector: pattern.inner.embedding.clone(),
            metadata: pattern.inner.metadata.clone(),
        };

        let db = self.db.lock();
        let id = db
            .insert(entry)
            .map_err(|e| JsValue::from_str(&format!("Failed to store pattern: {}", e)))?;

        console::log_1(&format!("Pattern stored with ID: {}", id).into());
        Ok(id)
    }

    /// Query the substrate for similar patterns
    ///
    /// # Arguments
    /// * `embedding` - Query embedding as Float32Array
    /// * `k` - Number of results to return
    ///
    /// # Returns
    /// Promise that resolves to an array of SearchResult objects
    #[wasm_bindgen]
    pub fn query(&self, embedding: Float32Array, k: u32) -> Result<Promise, JsValue> {
        let query_vec = embedding.to_vec();

        if query_vec.len() != self.dimensions {
            return Err(JsValue::from_str(&format!(
                "Query embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                query_vec.len()
            )));
        }

        let db = self.db.clone();

        let promise = future_to_promise(async move {
            let search_query = ruvector_core::types::SearchQuery {
                vector: query_vec,
                k: k as usize,
                filter: None,
                ef_search: None,
            };

            let db_guard = db.lock();
            let results = db_guard
                .search(search_query)
                .map_err(|e| JsValue::from_str(&format!("Search failed: {}", e)))?;
            drop(db_guard);

            let js_results: Vec<JsValue> = results
                .into_iter()
                .map(|r| {
                    let result = SearchResult {
                        inner: SearchResultInner {
                            id: r.id,
                            score: r.score,
                            pattern: None, // Can be populated if needed
                        },
                    };
                    to_value(&result.inner).unwrap()
                })
                .collect();

            Ok(Array::from_iter(js_results).into())
        });

        Ok(promise)
    }

    /// Get substrate statistics
    ///
    /// # Returns
    /// Object with substrate statistics
    #[wasm_bindgen]
    pub fn stats(&self) -> Result<JsValue, JsValue> {
        let db = self.db.lock();
        let count = db
            .len()
            .map_err(|e| JsValue::from_str(&format!("Failed to get stats: {}", e)))?;

        let stats = serde_json::json!({
            "dimensions": self.dimensions,
            "pattern_count": count,
            "distance_metric": self.config.distance_metric,
            "temporal_enabled": self.config.enable_temporal,
            "causal_enabled": self.config.enable_causal,
        });

        to_value(&stats)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize stats: {}", e)))
    }

    /// Get a pattern by ID
    ///
    /// # Arguments
    /// * `id` - Pattern ID
    ///
    /// # Returns
    /// Pattern object or null if not found
    #[wasm_bindgen]
    pub fn get(&self, id: &str) -> Result<Option<Pattern>, JsValue> {
        let db = self.db.lock();
        let entry = db
            .get(id)
            .map_err(|e| JsValue::from_str(&format!("Failed to get pattern: {}", e)))?;

        Ok(entry.map(|e| Pattern {
            inner: PatternInner {
                embedding: e.vector,
                metadata: e.metadata,
                timestamp: js_sys::Date::now(),
                id: e.id,
                antecedents: vec![],
            },
        }))
    }

    /// Delete a pattern by ID
    ///
    /// # Arguments
    /// * `id` - Pattern ID to delete
    ///
    /// # Returns
    /// True if deleted, false if not found
    #[wasm_bindgen]
    pub fn delete(&self, id: &str) -> Result<bool, JsValue> {
        let db = self.db.lock();
        db.delete(id)
            .map_err(|e| JsValue::from_str(&format!("Failed to delete pattern: {}", e)))
    }

    /// Get the number of patterns in the substrate
    #[wasm_bindgen]
    pub fn len(&self) -> Result<usize, JsValue> {
        let db = self.db.lock();
        db.len()
            .map_err(|e| JsValue::from_str(&format!("Failed to get length: {}", e)))
    }

    /// Check if the substrate is empty
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> Result<bool, JsValue> {
        let db = self.db.lock();
        db.is_empty()
            .map_err(|e| JsValue::from_str(&format!("Failed to check if empty: {}", e)))
    }

    /// Get substrate dimensions
    #[wasm_bindgen(getter)]
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}

/// Get version information
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Detect SIMD support in the current environment
#[wasm_bindgen(js_name = detectSIMD)]
pub fn detect_simd() -> bool {
    #[cfg(target_feature = "simd128")]
    {
        true
    }
    #[cfg(not(target_feature = "simd128"))]
    {
        false
    }
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
    fn test_detect_simd() {
        let _ = detect_simd();
    }
}
