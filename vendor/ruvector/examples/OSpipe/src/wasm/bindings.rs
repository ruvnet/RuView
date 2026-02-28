//! WASM-bindgen exports for OSpipe browser usage.
//!
//! This module exposes a self-contained vector store that runs entirely in the
//! browser via WebAssembly. It supports embedding insertion, semantic search
//! with optional time-range filtering, deduplication checks, simple text
//! embedding (hash-based, suitable for demos), content safety checks, and
//! query routing heuristics.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use super::helpers;

/// Initialize WASM module: installs `console_error_panic_hook` so that Rust
/// panics produce readable error messages in the browser developer console
/// instead of the default `unreachable` with no context.
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

// ---------------------------------------------------------------------------
// Internal data structures
// ---------------------------------------------------------------------------

/// A single stored embedding with metadata.
struct WasmEmbedding {
    id: String,
    vector: Vec<f32>,
    metadata: String, // JSON string
    timestamp: f64,   // Unix milliseconds
}

/// A search result returned to JavaScript.
#[derive(Serialize, Deserialize)]
struct SearchHit {
    id: String,
    score: f64,
    metadata: String,
    timestamp: f64,
}

// ---------------------------------------------------------------------------
// Public WASM API
// ---------------------------------------------------------------------------

/// OSpipe WASM -- browser-based personal AI memory search.
#[wasm_bindgen]
pub struct OsPipeWasm {
    dimension: usize,
    embeddings: Vec<WasmEmbedding>,
}

#[wasm_bindgen]
impl OsPipeWasm {
    // -- lifecycle ---------------------------------------------------------

    /// Create a new OsPipeWasm instance with the given embedding dimension.
    #[wasm_bindgen(constructor)]
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            embeddings: Vec::new(),
        }
    }

    // -- insertion ---------------------------------------------------------

    /// Insert a frame embedding into the store.
    ///
    /// * `id`        - Unique identifier for this frame.
    /// * `embedding` - Float32 vector whose length must match `dimension`.
    /// * `metadata`  - Arbitrary JSON string attached to this frame.
    /// * `timestamp` - Unix timestamp in milliseconds.
    pub fn insert(
        &mut self,
        id: &str,
        embedding: &[f32],
        metadata: &str,
        timestamp: f64,
    ) -> Result<(), JsValue> {
        if embedding.len() != self.dimension {
            return Err(JsValue::from_str(&format!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.dimension,
                embedding.len()
            )));
        }
        self.embeddings.push(WasmEmbedding {
            id: id.to_string(),
            vector: embedding.to_vec(),
            metadata: metadata.to_string(),
            timestamp,
        });
        Ok(())
    }

    // -- search ------------------------------------------------------------

    /// Semantic search by embedding vector.  Returns the top-k results as a
    /// JSON-serialized `JsValue` array of `{ id, score, metadata, timestamp }`.
    pub fn search(&self, query_embedding: &[f32], k: usize) -> Result<JsValue, JsValue> {
        if query_embedding.len() != self.dimension {
            return Err(JsValue::from_str(&format!(
                "Query dimension mismatch: expected {}, got {}",
                self.dimension,
                query_embedding.len()
            )));
        }

        let mut scored: Vec<(usize, f32)> = self
            .embeddings
            .iter()
            .enumerate()
            .map(|(i, e)| (i, helpers::cosine_similarity(query_embedding, &e.vector)))
            .collect();

        // Sort descending by similarity.
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let hits: Vec<SearchHit> = scored
            .into_iter()
            .take(k)
            .map(|(i, score)| {
                let e = &self.embeddings[i];
                SearchHit {
                    id: e.id.clone(),
                    score: score as f64,
                    metadata: e.metadata.clone(),
                    timestamp: e.timestamp,
                }
            })
            .collect();

        serde_wasm_bindgen::to_value(&hits).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Search with a time-range filter.  Only embeddings whose timestamp falls
    /// within `[start_time, end_time]` (inclusive) are considered.
    pub fn search_filtered(
        &self,
        query_embedding: &[f32],
        k: usize,
        start_time: f64,
        end_time: f64,
    ) -> Result<JsValue, JsValue> {
        if query_embedding.len() != self.dimension {
            return Err(JsValue::from_str(&format!(
                "Query dimension mismatch: expected {}, got {}",
                self.dimension,
                query_embedding.len()
            )));
        }

        let mut scored: Vec<(usize, f32)> = self
            .embeddings
            .iter()
            .enumerate()
            .filter(|(_, e)| e.timestamp >= start_time && e.timestamp <= end_time)
            .map(|(i, e)| (i, helpers::cosine_similarity(query_embedding, &e.vector)))
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let hits: Vec<SearchHit> = scored
            .into_iter()
            .take(k)
            .map(|(i, score)| {
                let e = &self.embeddings[i];
                SearchHit {
                    id: e.id.clone(),
                    score: score as f64,
                    metadata: e.metadata.clone(),
                    timestamp: e.timestamp,
                }
            })
            .collect();

        serde_wasm_bindgen::to_value(&hits).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // -- deduplication -----------------------------------------------------

    /// Check whether `embedding` is a near-duplicate of any stored embedding.
    ///
    /// Returns `true` when the cosine similarity to any existing embedding is
    /// greater than or equal to `threshold`.
    pub fn is_duplicate(&self, embedding: &[f32], threshold: f32) -> bool {
        self.embeddings
            .iter()
            .any(|e| helpers::cosine_similarity(embedding, &e.vector) >= threshold)
    }

    // -- stats / accessors -------------------------------------------------

    /// Number of stored embeddings.
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// Returns true if no embeddings are stored.
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// Return pipeline statistics as a JSON string.
    pub fn stats(&self) -> String {
        serde_json::json!({
            "dimension": self.dimension,
            "total_embeddings": self.embeddings.len(),
            "memory_estimate_bytes": self.embeddings.len() * (self.dimension * 4 + 128),
        })
        .to_string()
    }

    // -- text embedding (demo / hash-based) --------------------------------

    /// Generate a simple deterministic embedding from text.
    ///
    /// This uses a hash-based approach and is **not** a real neural embedding.
    /// Suitable for demos and testing only.
    pub fn embed_text(&self, text: &str) -> Vec<f32> {
        helpers::hash_embed(text, self.dimension)
    }

    /// Batch-embed multiple texts.
    ///
    /// `texts` must be a JS `Array<string>`.  Returns a JS `Array<Float32Array>`.
    pub fn batch_embed(&self, texts: JsValue) -> Result<JsValue, JsValue> {
        let text_list: Vec<String> = serde_wasm_bindgen::from_value(texts)
            .map_err(|e| JsValue::from_str(&format!("Failed to deserialize texts: {e}")))?;

        let results: Vec<Vec<f32>> = text_list
            .iter()
            .map(|t| helpers::hash_embed(t, self.dimension))
            .collect();

        serde_wasm_bindgen::to_value(&results).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // -- safety ------------------------------------------------------------

    /// Run a lightweight safety check on `content`.
    ///
    /// Returns one of:
    /// - `"deny"`   -- content contains patterns that should not be stored
    ///                 (e.g. credit card numbers, SSNs).
    /// - `"redact"` -- content contains potentially sensitive information
    ///                 that could be redacted.
    /// - `"allow"`  -- content appears safe.
    pub fn safety_check(&self, content: &str) -> String {
        helpers::safety_classify(content).to_string()
    }

    // -- query routing -----------------------------------------------------

    /// Route a query string to the optimal search backend based on simple
    /// keyword heuristics.
    ///
    /// Returns one of: `"Graph"`, `"Temporal"`, `"Keyword"`, `"Semantic"`.
    pub fn route_query(&self, query: &str) -> String {
        helpers::route_query(query).to_string()
    }
}
