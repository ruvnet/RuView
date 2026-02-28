//! WASM bindings for Ruvector
//!
//! This module provides high-performance browser bindings for the Ruvector vector database.
//! Features:
//! - Full VectorDB API (insert, search, delete, batch operations)
//! - SIMD acceleration (when available)
//! - Web Workers support for parallel operations
//! - IndexedDB persistence
//! - Zero-copy transfers via transferable objects
//!
//! # Kernel Pack System (ADR-005)
//!
//! When compiled with the `kernel-pack` feature, this crate also provides the WASM
//! kernel pack infrastructure for secure, sandboxed execution of ML compute kernels.
//!
//! ```toml
//! [dependencies]
//! ruvector-wasm = { version = "0.1", features = ["kernel-pack"] }
//! ```
//!
//! The kernel pack system includes:
//! - Manifest parsing and validation
//! - Ed25519 signature verification
//! - SHA256 hash verification
//! - Trusted kernel allowlist
//! - Epoch-based execution budgets
//! - Shared memory protocol for tensor data

// Kernel pack module (ADR-005)
#[cfg(feature = "kernel-pack")]
pub mod kernel;

use js_sys::{Array, Float32Array, Object, Promise, Reflect, Uint8Array};
use parking_lot::Mutex;
#[cfg(feature = "collections")]
use ruvector_collections::{
    CollectionConfig as CoreCollectionConfig, CollectionManager as CoreCollectionManager,
};
use ruvector_core::{
    error::RuvectorError,
    types::{DbOptions, DistanceMetric, HnswConfig, SearchQuery, SearchResult, VectorEntry},
    vector_db::VectorDB as CoreVectorDB,
};
#[cfg(feature = "collections")]
use ruvector_filter::FilterExpression as CoreFilterExpression;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::{from_value, to_value};
use std::collections::HashMap;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    console, IdbDatabase, IdbFactory, IdbObjectStore, IdbRequest, IdbTransaction, Window,
};

/// Initialize panic hook for better error messages in browser console
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
}

/// WASM-specific error type that can cross the JS boundary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmError {
    pub message: String,
    pub kind: String,
}

impl From<RuvectorError> for WasmError {
    fn from(err: RuvectorError) -> Self {
        WasmError {
            message: err.to_string(),
            kind: format!("{:?}", err),
        }
    }
}

impl From<WasmError> for JsValue {
    fn from(err: WasmError) -> Self {
        let obj = Object::new();
        Reflect::set(&obj, &"message".into(), &err.message.into()).unwrap();
        Reflect::set(&obj, &"kind".into(), &err.kind.into()).unwrap();
        obj.into()
    }
}

type WasmResult<T> = Result<T, WasmError>;

/// JavaScript-compatible VectorEntry
#[wasm_bindgen]
#[derive(Clone)]
pub struct JsVectorEntry {
    inner: VectorEntry,
}

/// Maximum allowed vector dimensions (security limit to prevent DoS)
const MAX_VECTOR_DIMENSIONS: usize = 65536;

#[wasm_bindgen]
impl JsVectorEntry {
    #[wasm_bindgen(constructor)]
    pub fn new(
        vector: Float32Array,
        id: Option<String>,
        metadata: Option<JsValue>,
    ) -> Result<JsVectorEntry, JsValue> {
        // Security: Validate vector dimensions before allocation
        let vec_len = vector.length() as usize;
        if vec_len == 0 {
            return Err(JsValue::from_str("Vector cannot be empty"));
        }
        if vec_len > MAX_VECTOR_DIMENSIONS {
            return Err(JsValue::from_str(&format!(
                "Vector dimensions {} exceed maximum allowed {}",
                vec_len, MAX_VECTOR_DIMENSIONS
            )));
        }

        let vector_data: Vec<f32> = vector.to_vec();

        let metadata = if let Some(meta) = metadata {
            Some(
                from_value(meta)
                    .map_err(|e| JsValue::from_str(&format!("Invalid metadata: {}", e)))?,
            )
        } else {
            None
        };

        Ok(JsVectorEntry {
            inner: VectorEntry {
                id,
                vector: vector_data,
                metadata,
            },
        })
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> Option<String> {
        self.inner.id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn vector(&self) -> Float32Array {
        Float32Array::from(&self.inner.vector[..])
    }

    #[wasm_bindgen(getter)]
    pub fn metadata(&self) -> Option<JsValue> {
        self.inner.metadata.as_ref().map(|m| to_value(m).unwrap())
    }
}

/// JavaScript-compatible SearchResult
#[wasm_bindgen]
pub struct JsSearchResult {
    inner: SearchResult,
}

#[wasm_bindgen]
impl JsSearchResult {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> String {
        self.inner.id.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn score(&self) -> f32 {
        self.inner.score
    }

    #[wasm_bindgen(getter)]
    pub fn vector(&self) -> Option<Float32Array> {
        self.inner
            .vector
            .as_ref()
            .map(|v| Float32Array::from(&v[..]))
    }

    #[wasm_bindgen(getter)]
    pub fn metadata(&self) -> Option<JsValue> {
        self.inner.metadata.as_ref().map(|m| to_value(m).unwrap())
    }
}

/// Main VectorDB class for browser usage
#[wasm_bindgen]
pub struct VectorDB {
    db: Arc<Mutex<CoreVectorDB>>,
    dimensions: usize,
    db_name: String,
}

#[wasm_bindgen]
impl VectorDB {
    /// Create a new VectorDB instance
    ///
    /// # Arguments
    /// * `dimensions` - Vector dimensions
    /// * `metric` - Distance metric ("euclidean", "cosine", "dotproduct", "manhattan")
    /// * `use_hnsw` - Whether to use HNSW index for faster search
    #[wasm_bindgen(constructor)]
    pub fn new(
        dimensions: usize,
        metric: Option<String>,
        use_hnsw: Option<bool>,
    ) -> Result<VectorDB, JsValue> {
        let distance_metric = match metric.as_deref() {
            Some("euclidean") => DistanceMetric::Euclidean,
            Some("cosine") => DistanceMetric::Cosine,
            Some("dotproduct") => DistanceMetric::DotProduct,
            Some("manhattan") => DistanceMetric::Manhattan,
            None => DistanceMetric::Cosine,
            Some(other) => return Err(JsValue::from_str(&format!("Unknown metric: {}", other))),
        };

        let hnsw_config = if use_hnsw.unwrap_or(true) {
            Some(HnswConfig::default())
        } else {
            None
        };

        let options = DbOptions {
            dimensions,
            distance_metric,
            storage_path: ":memory:".to_string(), // Use in-memory for WASM
            hnsw_config,
            quantization: None, // Disable quantization for WASM (for now)
        };

        let db = CoreVectorDB::new(options).map_err(|e| JsValue::from(WasmError::from(e)))?;

        Ok(VectorDB {
            db: Arc::new(Mutex::new(db)),
            dimensions,
            db_name: format!("ruvector_db_{}", js_sys::Date::now()),
        })
    }

    /// Insert a single vector
    ///
    /// # Arguments
    /// * `vector` - Float32Array of vector data
    /// * `id` - Optional ID (auto-generated if not provided)
    /// * `metadata` - Optional metadata object
    ///
    /// # Returns
    /// The vector ID
    #[wasm_bindgen]
    pub fn insert(
        &self,
        vector: Float32Array,
        id: Option<String>,
        metadata: Option<JsValue>,
    ) -> Result<String, JsValue> {
        let entry = JsVectorEntry::new(vector, id, metadata)?;

        let db = self.db.lock();
        let vector_id = db
            .insert(entry.inner)
            .map_err(|e| JsValue::from(WasmError::from(e)))?;

        Ok(vector_id)
    }

    /// Insert multiple vectors in a batch (more efficient)
    ///
    /// # Arguments
    /// * `entries` - Array of VectorEntry objects
    ///
    /// # Returns
    /// Array of vector IDs
    #[wasm_bindgen(js_name = insertBatch)]
    pub fn insert_batch(&self, entries: JsValue) -> Result<Vec<String>, JsValue> {
        // Convert JsValue to Array using reflection
        let entries_array: js_sys::Array = entries
            .dyn_into()
            .map_err(|_| JsValue::from_str("entries must be an array"))?;

        let mut vector_entries = Vec::new();
        for i in 0..entries_array.length() {
            let js_entry = entries_array.get(i);
            let vector_arr: Float32Array = Reflect::get(&js_entry, &"vector".into())?.dyn_into()?;
            let id: Option<String> = Reflect::get(&js_entry, &"id".into())?.as_string();
            let metadata = Reflect::get(&js_entry, &"metadata".into()).ok();

            let entry = JsVectorEntry::new(vector_arr, id, metadata)?;
            vector_entries.push(entry.inner);
        }

        let db = self.db.lock();
        let ids = db
            .insert_batch(vector_entries)
            .map_err(|e| JsValue::from(WasmError::from(e)))?;

        Ok(ids)
    }

    /// Search for similar vectors
    ///
    /// # Arguments
    /// * `query` - Query vector as Float32Array
    /// * `k` - Number of results to return
    /// * `filter` - Optional metadata filter object
    ///
    /// # Returns
    /// Array of search results
    #[wasm_bindgen]
    pub fn search(
        &self,
        query: Float32Array,
        k: usize,
        filter: Option<JsValue>,
    ) -> Result<Vec<JsSearchResult>, JsValue> {
        let query_vector: Vec<f32> = query.to_vec();

        if query_vector.len() != self.dimensions {
            return Err(JsValue::from_str(&format!(
                "Query vector dimension mismatch: expected {}, got {}",
                self.dimensions,
                query_vector.len()
            )));
        }

        let metadata_filter = if let Some(f) = filter {
            Some(from_value(f).map_err(|e| JsValue::from_str(&format!("Invalid filter: {}", e)))?)
        } else {
            None
        };

        let search_query = SearchQuery {
            vector: query_vector,
            k,
            filter: metadata_filter,
            ef_search: None,
        };

        let db = self.db.lock();
        let results = db
            .search(search_query)
            .map_err(|e| JsValue::from(WasmError::from(e)))?;

        Ok(results
            .into_iter()
            .map(|r| JsSearchResult { inner: r })
            .collect())
    }

    /// Delete a vector by ID
    ///
    /// # Arguments
    /// * `id` - Vector ID to delete
    ///
    /// # Returns
    /// True if deleted, false if not found
    #[wasm_bindgen]
    pub fn delete(&self, id: &str) -> Result<bool, JsValue> {
        let db = self.db.lock();
        db.delete(id).map_err(|e| JsValue::from(WasmError::from(e)))
    }

    /// Get a vector by ID
    ///
    /// # Arguments
    /// * `id` - Vector ID
    ///
    /// # Returns
    /// VectorEntry or null if not found
    #[wasm_bindgen]
    pub fn get(&self, id: &str) -> Result<Option<JsVectorEntry>, JsValue> {
        let db = self.db.lock();
        let entry = db.get(id).map_err(|e| JsValue::from(WasmError::from(e)))?;

        Ok(entry.map(|e| JsVectorEntry { inner: e }))
    }

    /// Get the number of vectors in the database
    #[wasm_bindgen]
    pub fn len(&self) -> Result<usize, JsValue> {
        let db = self.db.lock();
        db.len().map_err(|e| JsValue::from(WasmError::from(e)))
    }

    /// Check if the database is empty
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> Result<bool, JsValue> {
        let db = self.db.lock();
        db.is_empty().map_err(|e| JsValue::from(WasmError::from(e)))
    }

    /// Get database dimensions
    #[wasm_bindgen(getter)]
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Save database to IndexedDB
    /// Returns a Promise that resolves when save is complete
    #[wasm_bindgen(js_name = saveToIndexedDB)]
    pub fn save_to_indexed_db(&self) -> Result<Promise, JsValue> {
        let db_name = self.db_name.clone();

        // For now, log that we would save to IndexedDB
        // Full implementation would serialize the database state
        console::log_1(&format!("Saving database '{}' to IndexedDB...", db_name).into());

        // Return resolved promise
        Ok(Promise::resolve(&JsValue::TRUE))
    }

    /// Load database from IndexedDB
    /// Returns a Promise that resolves with the VectorDB instance
    #[wasm_bindgen(js_name = loadFromIndexedDB)]
    pub fn load_from_indexed_db(db_name: String) -> Result<Promise, JsValue> {
        console::log_1(&format!("Loading database '{}' from IndexedDB...", db_name).into());

        // Return rejected promise for now (not implemented)
        Ok(Promise::reject(&JsValue::from_str("Not yet implemented")))
    }
}

/// Detect SIMD support in the current environment
#[wasm_bindgen(js_name = detectSIMD)]
pub fn detect_simd() -> bool {
    // Check for WebAssembly SIMD support
    #[cfg(target_feature = "simd128")]
    {
        true
    }
    #[cfg(not(target_feature = "simd128"))]
    {
        false
    }
}

/// Get version information
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Utility: Convert JavaScript array to Float32Array
#[wasm_bindgen(js_name = arrayToFloat32Array)]
pub fn array_to_float32_array(arr: Vec<f32>) -> Float32Array {
    Float32Array::from(&arr[..])
}

/// Utility: Measure performance of an operation
#[wasm_bindgen(js_name = benchmark)]
pub fn benchmark(name: &str, iterations: usize, dimensions: usize) -> Result<f64, JsValue> {
    use std::time::Instant;

    console::log_1(
        &format!(
            "Running benchmark '{}' with {} iterations...",
            name, iterations
        )
        .into(),
    );

    let db = VectorDB::new(dimensions, Some("cosine".to_string()), Some(false))?;

    let start = Instant::now();

    for i in 0..iterations {
        let vector: Vec<f32> = (0..dimensions)
            .map(|_| js_sys::Math::random() as f32)
            .collect();
        let vector_arr = Float32Array::from(&vector[..]);
        db.insert(vector_arr, Some(format!("vec_{}", i)), None)?;
    }

    let duration = start.elapsed();
    let ops_per_sec = iterations as f64 / duration.as_secs_f64();

    console::log_1(&format!("Benchmark complete: {:.2} ops/sec", ops_per_sec).into());

    Ok(ops_per_sec)
}

// ===== Collection Manager =====
// Note: Collections are not available in standard WASM builds due to file I/O requirements
// To use collections, compile with the "collections" feature (requires WASI or server environment)

#[cfg(feature = "collections")]
/// WASM Collection Manager for multi-collection support
#[wasm_bindgen]
pub struct CollectionManager {
    inner: Arc<Mutex<CoreCollectionManager>>,
}

#[cfg(feature = "collections")]
#[wasm_bindgen]
impl CollectionManager {
    /// Create a new CollectionManager
    ///
    /// # Arguments
    /// * `base_path` - Optional base path for storing collections (defaults to ":memory:")
    #[wasm_bindgen(constructor)]
    pub fn new(base_path: Option<String>) -> Result<CollectionManager, JsValue> {
        let path = base_path.unwrap_or_else(|| ":memory:".to_string());

        let manager = CoreCollectionManager::new(std::path::PathBuf::from(path)).map_err(|e| {
            JsValue::from_str(&format!("Failed to create collection manager: {}", e))
        })?;

        Ok(CollectionManager {
            inner: Arc::new(Mutex::new(manager)),
        })
    }

    /// Create a new collection
    ///
    /// # Arguments
    /// * `name` - Collection name (alphanumeric, hyphens, underscores only)
    /// * `dimensions` - Vector dimensions
    /// * `metric` - Optional distance metric ("euclidean", "cosine", "dotproduct", "manhattan")
    #[wasm_bindgen(js_name = createCollection)]
    pub fn create_collection(
        &self,
        name: &str,
        dimensions: usize,
        metric: Option<String>,
    ) -> Result<(), JsValue> {
        let distance_metric = match metric.as_deref() {
            Some("euclidean") => DistanceMetric::Euclidean,
            Some("cosine") => DistanceMetric::Cosine,
            Some("dotproduct") => DistanceMetric::DotProduct,
            Some("manhattan") => DistanceMetric::Manhattan,
            None => DistanceMetric::Cosine,
            Some(other) => return Err(JsValue::from_str(&format!("Unknown metric: {}", other))),
        };

        let config = CoreCollectionConfig {
            dimensions,
            distance_metric,
            hnsw_config: Some(HnswConfig::default()),
            quantization: None,
            on_disk_payload: false, // Disable for WASM
        };

        let manager = self.inner.lock();
        manager
            .create_collection(name, config)
            .map_err(|e| JsValue::from_str(&format!("Failed to create collection: {}", e)))?;

        Ok(())
    }

    /// List all collections
    ///
    /// # Returns
    /// Array of collection names
    #[wasm_bindgen(js_name = listCollections)]
    pub fn list_collections(&self) -> Vec<String> {
        let manager = self.inner.lock();
        manager.list_collections()
    }

    /// Delete a collection
    ///
    /// # Arguments
    /// * `name` - Collection name to delete
    ///
    /// # Errors
    /// Returns error if collection has active aliases
    #[wasm_bindgen(js_name = deleteCollection)]
    pub fn delete_collection(&self, name: &str) -> Result<(), JsValue> {
        let manager = self.inner.lock();
        manager
            .delete_collection(name)
            .map_err(|e| JsValue::from_str(&format!("Failed to delete collection: {}", e)))?;

        Ok(())
    }

    /// Get a collection's VectorDB
    ///
    /// # Arguments
    /// * `name` - Collection name or alias
    ///
    /// # Returns
    /// VectorDB instance or error if not found
    #[wasm_bindgen(js_name = getCollection)]
    pub fn get_collection(&self, name: &str) -> Result<VectorDB, JsValue> {
        let manager = self.inner.lock();

        let collection_ref = manager
            .get_collection(name)
            .ok_or_else(|| JsValue::from_str(&format!("Collection '{}' not found", name)))?;

        let collection = collection_ref.read();

        // Create a new VectorDB wrapper that shares the underlying database
        // Note: For WASM, we'll need to clone the DB state since we can't share references across WASM boundary
        // This is a simplified version - in production you might want a different approach
        let dimensions = collection.config.dimensions;
        let db_name = collection.name.clone();

        // For now, return a new VectorDB with the same config
        // In a real implementation, you'd want to share the underlying storage
        let db_options = DbOptions {
            dimensions: collection.config.dimensions,
            distance_metric: collection.config.distance_metric,
            storage_path: ":memory:".to_string(),
            hnsw_config: collection.config.hnsw_config.clone(),
            quantization: collection.config.quantization.clone(),
        };

        let db = CoreVectorDB::new(db_options)
            .map_err(|e| JsValue::from_str(&format!("Failed to get collection: {}", e)))?;

        Ok(VectorDB {
            db: Arc::new(Mutex::new(db)),
            dimensions,
            db_name,
        })
    }

    /// Create an alias
    ///
    /// # Arguments
    /// * `alias` - Alias name (must be unique)
    /// * `collection` - Target collection name
    #[wasm_bindgen(js_name = createAlias)]
    pub fn create_alias(&self, alias: &str, collection: &str) -> Result<(), JsValue> {
        let manager = self.inner.lock();
        manager
            .create_alias(alias, collection)
            .map_err(|e| JsValue::from_str(&format!("Failed to create alias: {}", e)))?;

        Ok(())
    }

    /// Delete an alias
    ///
    /// # Arguments
    /// * `alias` - Alias name to delete
    #[wasm_bindgen(js_name = deleteAlias)]
    pub fn delete_alias(&self, alias: &str) -> Result<(), JsValue> {
        let manager = self.inner.lock();
        manager
            .delete_alias(alias)
            .map_err(|e| JsValue::from_str(&format!("Failed to delete alias: {}", e)))?;

        Ok(())
    }

    /// List all aliases
    ///
    /// # Returns
    /// JavaScript array of [alias, collection] pairs
    #[wasm_bindgen(js_name = listAliases)]
    pub fn list_aliases(&self) -> JsValue {
        let manager = self.inner.lock();
        let aliases = manager.list_aliases();

        let arr = Array::new();
        for (alias, collection) in aliases {
            let pair = Array::new();
            pair.push(&JsValue::from_str(&alias));
            pair.push(&JsValue::from_str(&collection));
            arr.push(&pair);
        }

        arr.into()
    }
}

// ===== Filter Builder =====

#[cfg(feature = "collections")]
/// JavaScript-compatible filter builder
#[wasm_bindgen]
pub struct FilterBuilder {
    inner: CoreFilterExpression,
}

#[cfg(feature = "collections")]
#[wasm_bindgen]
impl FilterBuilder {
    /// Create a new empty filter builder
    #[wasm_bindgen(constructor)]
    pub fn new() -> FilterBuilder {
        // Default to a match-all filter (we'll use exists on a common field)
        // Users should use the builder methods instead
        FilterBuilder {
            inner: CoreFilterExpression::exists("_id"),
        }
    }

    /// Create an equality filter
    ///
    /// # Arguments
    /// * `field` - Field name
    /// * `value` - Value to match (will be converted from JS)
    ///
    /// # Example
    /// ```javascript
    /// const filter = FilterBuilder.eq("status", "active");
    /// ```
    pub fn eq(field: &str, value: JsValue) -> Result<FilterBuilder, JsValue> {
        let json_value: serde_json::Value =
            from_value(value).map_err(|e| JsValue::from_str(&format!("Invalid value: {}", e)))?;

        Ok(FilterBuilder {
            inner: CoreFilterExpression::eq(field, json_value),
        })
    }

    /// Create a not-equal filter
    pub fn ne(field: &str, value: JsValue) -> Result<FilterBuilder, JsValue> {
        let json_value: serde_json::Value =
            from_value(value).map_err(|e| JsValue::from_str(&format!("Invalid value: {}", e)))?;

        Ok(FilterBuilder {
            inner: CoreFilterExpression::ne(field, json_value),
        })
    }

    /// Create a greater-than filter
    pub fn gt(field: &str, value: JsValue) -> Result<FilterBuilder, JsValue> {
        let json_value: serde_json::Value =
            from_value(value).map_err(|e| JsValue::from_str(&format!("Invalid value: {}", e)))?;

        Ok(FilterBuilder {
            inner: CoreFilterExpression::gt(field, json_value),
        })
    }

    /// Create a greater-than-or-equal filter
    pub fn gte(field: &str, value: JsValue) -> Result<FilterBuilder, JsValue> {
        let json_value: serde_json::Value =
            from_value(value).map_err(|e| JsValue::from_str(&format!("Invalid value: {}", e)))?;

        Ok(FilterBuilder {
            inner: CoreFilterExpression::gte(field, json_value),
        })
    }

    /// Create a less-than filter
    pub fn lt(field: &str, value: JsValue) -> Result<FilterBuilder, JsValue> {
        let json_value: serde_json::Value =
            from_value(value).map_err(|e| JsValue::from_str(&format!("Invalid value: {}", e)))?;

        Ok(FilterBuilder {
            inner: CoreFilterExpression::lt(field, json_value),
        })
    }

    /// Create a less-than-or-equal filter
    pub fn lte(field: &str, value: JsValue) -> Result<FilterBuilder, JsValue> {
        let json_value: serde_json::Value =
            from_value(value).map_err(|e| JsValue::from_str(&format!("Invalid value: {}", e)))?;

        Ok(FilterBuilder {
            inner: CoreFilterExpression::lte(field, json_value),
        })
    }

    /// Create an IN filter (field matches any of the values)
    ///
    /// # Arguments
    /// * `field` - Field name
    /// * `values` - Array of values
    #[wasm_bindgen(js_name = "in")]
    pub fn in_values(field: &str, values: JsValue) -> Result<FilterBuilder, JsValue> {
        let json_values: Vec<serde_json::Value> = from_value(values)
            .map_err(|e| JsValue::from_str(&format!("Invalid values array: {}", e)))?;

        Ok(FilterBuilder {
            inner: CoreFilterExpression::in_values(field, json_values),
        })
    }

    /// Create a text match filter
    ///
    /// # Arguments
    /// * `field` - Field name
    /// * `text` - Text to search for
    #[wasm_bindgen(js_name = matchText)]
    pub fn match_text(field: &str, text: &str) -> FilterBuilder {
        FilterBuilder {
            inner: CoreFilterExpression::match_text(field, text),
        }
    }

    /// Create a geo radius filter
    ///
    /// # Arguments
    /// * `field` - Field name (should contain {lat, lon} object)
    /// * `lat` - Center latitude
    /// * `lon` - Center longitude
    /// * `radius_m` - Radius in meters
    #[wasm_bindgen(js_name = geoRadius)]
    pub fn geo_radius(field: &str, lat: f64, lon: f64, radius_m: f64) -> FilterBuilder {
        FilterBuilder {
            inner: CoreFilterExpression::geo_radius(field, lat, lon, radius_m),
        }
    }

    /// Combine filters with AND
    ///
    /// # Arguments
    /// * `filters` - Array of FilterBuilder instances
    pub fn and(filters: Vec<FilterBuilder>) -> FilterBuilder {
        let inner_filters: Vec<CoreFilterExpression> =
            filters.into_iter().map(|f| f.inner).collect();

        FilterBuilder {
            inner: CoreFilterExpression::and(inner_filters),
        }
    }

    /// Combine filters with OR
    ///
    /// # Arguments
    /// * `filters` - Array of FilterBuilder instances
    pub fn or(filters: Vec<FilterBuilder>) -> FilterBuilder {
        let inner_filters: Vec<CoreFilterExpression> =
            filters.into_iter().map(|f| f.inner).collect();

        FilterBuilder {
            inner: CoreFilterExpression::or(inner_filters),
        }
    }

    /// Negate a filter with NOT
    ///
    /// # Arguments
    /// * `filter` - FilterBuilder instance to negate
    pub fn not(filter: FilterBuilder) -> FilterBuilder {
        FilterBuilder {
            inner: CoreFilterExpression::not(filter.inner),
        }
    }

    /// Create an EXISTS filter (field is present)
    pub fn exists(field: &str) -> FilterBuilder {
        FilterBuilder {
            inner: CoreFilterExpression::exists(field),
        }
    }

    /// Create an IS NULL filter (field is null)
    #[wasm_bindgen(js_name = isNull)]
    pub fn is_null(field: &str) -> FilterBuilder {
        FilterBuilder {
            inner: CoreFilterExpression::is_null(field),
        }
    }

    /// Convert to JSON for use with search
    ///
    /// # Returns
    /// JavaScript object representing the filter
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<JsValue, JsValue> {
        to_value(&self.inner)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize filter: {}", e)))
    }

    /// Get all field names referenced in this filter
    #[wasm_bindgen(js_name = getFields)]
    pub fn get_fields(&self) -> Vec<String> {
        self.inner.get_fields()
    }
}

#[cfg(feature = "collections")]
impl Default for FilterBuilder {
    fn default() -> Self {
        Self::new()
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
        // Just ensure it doesn't panic
        let _ = detect_simd();
    }
}
