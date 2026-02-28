//! HNSW Semantic Router for Browser-Compatible Pattern Routing
//!
//! Pure Rust implementation of HNSW (Hierarchical Navigable Small World) graph
//! for semantic pattern routing in WASM environments. Uses cosine similarity
//! for embedding comparison.
//!
//! ## Features
//!
//! - **Browser-Compatible**: Pure Rust with no external WASM-incompatible deps
//! - **Pattern Storage**: Store embeddings with metadata for routing decisions
//! - **Semantic Search**: Find similar patterns using approximate nearest neighbor search
//! - **Memory-Efficient**: Configurable max patterns to limit memory usage
//! - **Serializable**: JSON serialization for IndexedDB persistence
//!
//! ## Example (JavaScript)
//!
//! ```javascript
//! import { HnswRouterWasm, PatternWasm } from 'ruvllm-wasm';
//!
//! // Create router for 384-dimensional embeddings
//! const router = HnswRouterWasm.new(384, 1000);
//!
//! // Add patterns with embeddings
//! const embedding = new Float32Array([0.1, 0.2, ...]); // 384 dims
//! router.addPattern(embedding, "rust-expert", JSON.stringify({
//!   domain: "rust",
//!   expertise: "high"
//! }));
//!
//! // Route a query
//! const queryEmbedding = new Float32Array([0.15, 0.18, ...]);
//! const results = router.route(queryEmbedding, 5); // top 5 matches
//!
//! results.forEach(result => {
//!   console.log(`Match: ${result.name}, Score: ${result.score}`);
//! });
//!
//! // Serialize to JSON for persistence
//! const json = router.toJson();
//! localStorage.setItem('router', json);
//!
//! // Restore from JSON
//! const restored = HnswRouterWasm.fromJson(json);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

/// Maximum connections per node in the HNSW graph (M parameter)
const DEFAULT_M: usize = 16;

/// Maximum connections in layer 0 (M0 = M * 2)
const DEFAULT_M0: usize = 32;

/// Number of nearest neighbors to explore during construction (efConstruction)
const DEFAULT_EF_CONSTRUCTION: usize = 100;

/// Number of nearest neighbors to explore during search (efSearch)
const DEFAULT_EF_SEARCH: usize = 50;

/// A stored pattern with embedding and metadata
///
/// Represents a routing pattern that can be matched against queries.
/// Each pattern has a name, embedding vector, and optional metadata.
#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct PatternWasm {
    #[wasm_bindgen(skip)]
    pub name: String,
    #[wasm_bindgen(skip)]
    pub embedding: Vec<f32>,
    #[wasm_bindgen(skip)]
    pub metadata: String,
}

#[wasm_bindgen]
impl PatternWasm {
    /// Create a new pattern
    ///
    /// # Parameters
    ///
    /// - `embedding`: Float32Array of embedding values
    /// - `name`: Pattern name/identifier
    /// - `metadata`: JSON string with additional metadata
    #[wasm_bindgen(constructor)]
    pub fn new(embedding: &[f32], name: &str, metadata: &str) -> Self {
        Self {
            name: name.to_string(),
            embedding: embedding.to_vec(),
            metadata: metadata.to_string(),
        }
    }

    /// Get pattern name
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Get pattern embedding as Float32Array
    #[wasm_bindgen(getter)]
    pub fn embedding(&self) -> Vec<f32> {
        self.embedding.clone()
    }

    /// Get pattern metadata JSON string
    #[wasm_bindgen(getter)]
    pub fn metadata(&self) -> String {
        self.metadata.clone()
    }

    /// Set pattern name
    #[wasm_bindgen(setter)]
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Set pattern metadata
    #[wasm_bindgen(setter)]
    pub fn set_metadata(&mut self, metadata: String) {
        self.metadata = metadata;
    }
}

/// A routing search result with similarity score
///
/// Represents a matched pattern from a semantic search query.
#[wasm_bindgen]
#[derive(Clone, Serialize, Deserialize)]
pub struct RouteResultWasm {
    #[wasm_bindgen(skip)]
    pub name: String,
    #[wasm_bindgen(skip)]
    pub score: f32,
    #[wasm_bindgen(skip)]
    pub metadata: String,
    #[wasm_bindgen(skip)]
    pub embedding: Vec<f32>,
}

#[wasm_bindgen]
impl RouteResultWasm {
    /// Get result pattern name
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Get similarity score (higher is better, 0.0-1.0 for cosine)
    #[wasm_bindgen(getter)]
    pub fn score(&self) -> f32 {
        self.score
    }

    /// Get result metadata JSON string
    #[wasm_bindgen(getter)]
    pub fn metadata(&self) -> String {
        self.metadata.clone()
    }

    /// Get result embedding as Float32Array
    #[wasm_bindgen(getter)]
    pub fn embedding(&self) -> Vec<f32> {
        self.embedding.clone()
    }
}

/// HNSW node representing a pattern in the graph
#[derive(Clone, Serialize, Deserialize)]
struct HnswNode {
    /// Node ID (index in patterns vector)
    id: usize,
    /// Graph layer (0 = base layer, higher = upper layers)
    layer: usize,
    /// Connections to other nodes at this layer
    neighbors: Vec<usize>,
}

/// Internal HNSW graph state
#[derive(Clone, Serialize, Deserialize)]
struct HnswGraph {
    /// All stored patterns
    patterns: Vec<PatternWasm>,
    /// HNSW nodes per layer (layer -> node_id -> node)
    layers: Vec<HashMap<usize, HnswNode>>,
    /// Entry point node ID
    entry_point: Option<usize>,
    /// Maximum layer
    max_layer: usize,
    /// Configuration parameters
    m: usize,
    m0: usize,
    ef_construction: usize,
    ef_search: usize,
}

impl HnswGraph {
    fn new(m: usize, ef_construction: usize, ef_search: usize) -> Self {
        Self {
            patterns: Vec::new(),
            layers: vec![HashMap::new()],
            entry_point: None,
            max_layer: 0,
            m,
            m0: m * 2,
            ef_construction,
            ef_search,
        }
    }

    /// Select layer for new node using exponential decay
    fn select_layer(&self) -> usize {
        let ml = 1.0 / (self.m as f64).ln();
        let level = (-js_sys::Math::random().ln() * ml).floor() as usize;
        level.min(self.max_layer + 1)
    }

    /// Calculate cosine similarity between two embeddings
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a < 1e-8 || norm_b < 1e-8 {
            return 0.0;
        }

        (dot / (norm_a * norm_b)).max(-1.0).min(1.0)
    }

    /// Add a pattern to the HNSW graph
    fn add_pattern(&mut self, pattern: PatternWasm) {
        let node_id = self.patterns.len();
        let layer = self.select_layer();

        // Ensure we have enough layers
        while self.layers.len() <= layer {
            self.layers.push(HashMap::new());
        }

        // Update max layer and entry point if needed
        if layer > self.max_layer {
            self.max_layer = layer;
            self.entry_point = Some(node_id);
        }

        // Insert node at all layers from 0 to selected layer
        for l in 0..=layer {
            let node = HnswNode {
                id: node_id,
                layer: l,
                neighbors: Vec::new(),
            };
            self.layers[l].insert(node_id, node);
        }

        // Connect the new node to the graph
        if self.patterns.is_empty() {
            self.entry_point = Some(node_id);
        } else {
            self.connect_node(node_id, &pattern.embedding, layer);
        }

        self.patterns.push(pattern);
    }

    /// Connect a new node to existing nodes in the graph
    fn connect_node(&mut self, node_id: usize, embedding: &[f32], node_layer: usize) {
        let entry_point = self.entry_point.unwrap();

        // Search for nearest neighbors from top to node layer
        let mut curr = entry_point;
        for l in (node_layer + 1..=self.max_layer).rev() {
            curr = self.search_layer(embedding, curr, 1, l)[0].0;
        }

        // Insert connections from node_layer down to 0
        for l in (0..=node_layer).rev() {
            let m = if l == 0 { self.m0 } else { self.m };
            let candidates = self.search_layer(embedding, curr, self.ef_construction, l);

            // Select M nearest neighbors
            let neighbors: Vec<usize> = candidates.iter().take(m).map(|(id, _)| *id).collect();

            // Add bidirectional connections
            if let Some(node) = self.layers[l].get_mut(&node_id) {
                node.neighbors = neighbors.clone();
            }

            // Collect neighbors that need pruning
            let mut to_prune = Vec::new();

            for &neighbor_id in &neighbors {
                if let Some(neighbor) = self.layers[l].get_mut(&neighbor_id) {
                    if !neighbor.neighbors.contains(&node_id) {
                        neighbor.neighbors.push(node_id);

                        // Check if pruning needed
                        if neighbor.neighbors.len() > m {
                            to_prune.push(neighbor_id);
                        }
                    }
                }
            }

            // Prune connections after iteration
            for neighbor_id in to_prune {
                let neighbor_emb = self.patterns[neighbor_id].embedding.clone();
                self.prune_connections(neighbor_id, &neighbor_emb, m, l);
            }

            curr = candidates[0].0;
        }
    }

    /// Prune connections to maintain M maximum
    fn prune_connections(&mut self, node_id: usize, embedding: &[f32], m: usize, layer: usize) {
        if let Some(node) = self.layers[layer].get(&node_id) {
            let mut scored_neighbors: Vec<(usize, f32)> = node
                .neighbors
                .iter()
                .map(|&id| {
                    let sim = Self::cosine_similarity(embedding, &self.patterns[id].embedding);
                    (id, sim)
                })
                .collect();

            scored_neighbors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let pruned: Vec<usize> = scored_neighbors
                .into_iter()
                .take(m)
                .map(|(id, _)| id)
                .collect();

            if let Some(node) = self.layers[layer].get_mut(&node_id) {
                node.neighbors = pruned;
            }
        }
    }

    /// Search a single layer for nearest neighbors
    fn search_layer(
        &self,
        query: &[f32],
        entry_point: usize,
        ef: usize,
        layer: usize,
    ) -> Vec<(usize, f32)> {
        let mut visited = vec![false; self.patterns.len()];
        let mut candidates = Vec::new();
        let mut best = Vec::new();

        let entry_sim = Self::cosine_similarity(query, &self.patterns[entry_point].embedding);
        candidates.push((entry_point, entry_sim));
        best.push((entry_point, entry_sim));
        visited[entry_point] = true;

        while !candidates.is_empty() {
            // Get candidate with highest similarity
            candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let (curr_id, curr_sim) = candidates.pop().unwrap();

            // If worse than worst in best set, stop
            if !best.is_empty() {
                let worst_best = best
                    .iter()
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                    .unwrap();
                if curr_sim < worst_best.1 {
                    break;
                }
            }

            // Explore neighbors
            if let Some(node) = self.layers[layer].get(&curr_id) {
                for &neighbor_id in &node.neighbors {
                    if !visited[neighbor_id] {
                        visited[neighbor_id] = true;
                        let sim =
                            Self::cosine_similarity(query, &self.patterns[neighbor_id].embedding);

                        if best.len() < ef
                            || sim
                                > best
                                    .iter()
                                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                                    .unwrap()
                                    .1
                        {
                            candidates.push((neighbor_id, sim));
                            best.push((neighbor_id, sim));

                            if best.len() > ef {
                                best.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                                best.truncate(ef);
                            }
                        }
                    }
                }
            }
        }

        best.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        best
    }

    /// Search the graph for k nearest neighbors
    fn search(&self, query: &[f32], k: usize) -> Vec<RouteResultWasm> {
        if self.patterns.is_empty() {
            return Vec::new();
        }

        let entry_point = self.entry_point.unwrap();
        let mut curr = entry_point;

        // Search from top layer down to layer 1
        for l in (1..=self.max_layer).rev() {
            curr = self.search_layer(query, curr, 1, l)[0].0;
        }

        // Search layer 0 with ef_search
        let results = self.search_layer(query, curr, self.ef_search.max(k), 0);

        // Convert to RouteResultWasm
        results
            .into_iter()
            .take(k)
            .map(|(id, score)| {
                let pattern = &self.patterns[id];
                RouteResultWasm {
                    name: pattern.name.clone(),
                    score,
                    metadata: pattern.metadata.clone(),
                    embedding: pattern.embedding.clone(),
                }
            })
            .collect()
    }
}

/// HNSW Semantic Router for browser-compatible pattern routing
///
/// Provides approximate nearest neighbor search over pattern embeddings
/// using the HNSW (Hierarchical Navigable Small World) algorithm.
///
/// ## Memory Efficiency
///
/// The router enforces a maximum number of patterns to prevent unbounded
/// memory growth in browser environments. When the limit is reached, adding
/// new patterns will fail.
///
/// ## Thread Safety
///
/// This implementation is single-threaded and designed for use in browser
/// main thread or Web Workers.
#[wasm_bindgen]
pub struct HnswRouterWasm {
    dimensions: usize,
    max_patterns: usize,
    graph: HnswGraph,
}

#[wasm_bindgen]
impl HnswRouterWasm {
    /// Create a new HNSW router
    ///
    /// # Parameters
    ///
    /// - `dimensions`: Size of embedding vectors (e.g., 384 for all-MiniLM-L6-v2)
    /// - `max_patterns`: Maximum number of patterns to store (memory limit)
    ///
    /// # Example
    ///
    /// ```javascript
    /// const router = HnswRouterWasm.new(384, 1000);
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(dimensions: usize, max_patterns: usize) -> Self {
        crate::utils::set_panic_hook();

        Self {
            dimensions,
            max_patterns,
            graph: HnswGraph::new(DEFAULT_M, DEFAULT_EF_CONSTRUCTION, DEFAULT_EF_SEARCH),
        }
    }

    /// Get embedding dimensions
    #[wasm_bindgen(getter)]
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Get maximum patterns limit
    #[wasm_bindgen(getter, js_name = maxPatterns)]
    pub fn max_patterns(&self) -> usize {
        self.max_patterns
    }

    /// Get current number of patterns
    #[wasm_bindgen(getter, js_name = patternCount)]
    pub fn pattern_count(&self) -> usize {
        self.graph.patterns.len()
    }

    /// Add a pattern to the router
    ///
    /// # Parameters
    ///
    /// - `embedding`: Float32Array of embedding values (must match dimensions)
    /// - `name`: Pattern name/identifier
    /// - `metadata`: JSON string with additional metadata
    ///
    /// # Returns
    ///
    /// `true` if pattern was added, `false` if max_patterns limit reached
    ///
    /// # Example
    ///
    /// ```javascript
    /// const embedding = new Float32Array([0.1, 0.2, 0.3, ...]); // 384 dims
    /// const success = router.addPattern(
    ///   embedding,
    ///   "rust-expert",
    ///   JSON.stringify({ domain: "rust", expertise: "high" })
    /// );
    /// ```
    #[wasm_bindgen(js_name = addPattern)]
    pub fn add_pattern(&mut self, embedding: &[f32], name: &str, metadata: &str) -> bool {
        if self.graph.patterns.len() >= self.max_patterns {
            return false;
        }

        if embedding.len() != self.dimensions {
            crate::utils::warn(&format!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                embedding.len()
            ));
            return false;
        }

        let pattern = PatternWasm::new(embedding, name, metadata);
        self.graph.add_pattern(pattern);
        true
    }

    /// Route a query to find similar patterns
    ///
    /// # Parameters
    ///
    /// - `query`: Float32Array of query embedding (must match dimensions)
    /// - `top_k`: Number of top results to return
    ///
    /// # Returns
    ///
    /// Array of RouteResultWasm ordered by similarity (highest first)
    ///
    /// # Example
    ///
    /// ```javascript
    /// const query = new Float32Array([0.15, 0.18, ...]); // 384 dims
    /// const results = router.route(query, 5);
    /// results.forEach(result => {
    ///   console.log(`${result.name}: ${result.score}`);
    /// });
    /// ```
    #[wasm_bindgen]
    pub fn route(&self, query: &[f32], top_k: usize) -> Vec<RouteResultWasm> {
        if query.len() != self.dimensions {
            crate::utils::warn(&format!(
                "Query dimension mismatch: expected {}, got {}",
                self.dimensions,
                query.len()
            ));
            return Vec::new();
        }

        self.graph.search(query, top_k)
    }

    /// Serialize the router to JSON string
    ///
    /// Useful for persisting to IndexedDB or localStorage.
    ///
    /// # Example
    ///
    /// ```javascript
    /// const json = router.toJson();
    /// localStorage.setItem('router', json);
    /// ```
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(&SerializableRouter {
            dimensions: self.dimensions,
            max_patterns: self.max_patterns,
            graph: self.graph.clone(),
        })
        .map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }

    /// Deserialize a router from JSON string
    ///
    /// # Example
    ///
    /// ```javascript
    /// const json = localStorage.getItem('router');
    /// const router = HnswRouterWasm.fromJson(json);
    /// ```
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<HnswRouterWasm, JsValue> {
        let data: SerializableRouter = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("Deserialization failed: {}", e)))?;

        Ok(Self {
            dimensions: data.dimensions,
            max_patterns: data.max_patterns,
            graph: data.graph,
        })
    }

    /// Clear all patterns from the router
    ///
    /// Resets the router to empty state.
    #[wasm_bindgen]
    pub fn clear(&mut self) {
        self.graph = HnswGraph::new(DEFAULT_M, DEFAULT_EF_CONSTRUCTION, DEFAULT_EF_SEARCH);
    }

    /// Get pattern by index
    ///
    /// # Parameters
    ///
    /// - `index`: Pattern index (0 to patternCount - 1)
    ///
    /// # Returns
    ///
    /// PatternWasm or null if index out of bounds
    #[wasm_bindgen(js_name = getPattern)]
    pub fn get_pattern(&self, index: usize) -> Option<PatternWasm> {
        self.graph.patterns.get(index).cloned()
    }

    /// Set efSearch parameter for query-time accuracy tuning
    ///
    /// Higher values = more accurate but slower search.
    /// Recommended range: 10-200.
    ///
    /// # Parameters
    ///
    /// - `ef_search`: Number of neighbors to explore during search
    #[wasm_bindgen(js_name = setEfSearch)]
    pub fn set_ef_search(&mut self, ef_search: usize) {
        self.graph.ef_search = ef_search;
    }

    /// Get current efSearch parameter
    #[wasm_bindgen(getter, js_name = efSearch)]
    pub fn ef_search(&self) -> usize {
        self.graph.ef_search
    }
}

/// Serializable router format
#[derive(Serialize, Deserialize)]
struct SerializableRouter {
    dimensions: usize,
    max_patterns: usize,
    graph: HnswGraph,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_embedding(dim: usize, seed: f32) -> Vec<f32> {
        (0..dim).map(|i| (i as f32 * seed).sin()).collect()
    }

    #[test]
    fn test_router_creation() {
        let router = HnswRouterWasm::new(128, 100);
        assert_eq!(router.dimensions(), 128);
        assert_eq!(router.max_patterns(), 100);
        assert_eq!(router.pattern_count(), 0);
    }

    #[test]
    fn test_add_pattern() {
        let mut router = HnswRouterWasm::new(128, 100);
        let embedding = create_test_embedding(128, 1.0);

        let success = router.add_pattern(&embedding, "test-pattern", "{}");
        assert!(success);
        assert_eq!(router.pattern_count(), 1);
    }

    #[test]
    fn test_max_patterns_limit() {
        let mut router = HnswRouterWasm::new(128, 2);

        let emb1 = create_test_embedding(128, 1.0);
        let emb2 = create_test_embedding(128, 2.0);
        let emb3 = create_test_embedding(128, 3.0);

        assert!(router.add_pattern(&emb1, "pattern1", "{}"));
        assert!(router.add_pattern(&emb2, "pattern2", "{}"));
        assert!(!router.add_pattern(&emb3, "pattern3", "{}"));
        assert_eq!(router.pattern_count(), 2);
    }

    #[test]
    fn test_route() {
        let mut router = HnswRouterWasm::new(128, 100);

        // Add similar patterns
        let emb1 = create_test_embedding(128, 1.0);
        let emb2 = create_test_embedding(128, 1.1);
        let emb3 = create_test_embedding(128, 5.0);

        router.add_pattern(&emb1, "similar1", r#"{"type":"A"}"#);
        router.add_pattern(&emb2, "similar2", r#"{"type":"A"}"#);
        router.add_pattern(&emb3, "different", r#"{"type":"B"}"#);

        // Query similar to emb1
        let query = create_test_embedding(128, 1.05);
        let results = router.route(&query, 2);

        assert_eq!(results.len(), 2);
        // First result should be most similar
        assert!(results[0].score() > results[1].score());
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = HnswGraph::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-5);

        let c = vec![1.0, 0.0, 0.0];
        let d = vec![0.0, 1.0, 0.0];
        let sim2 = HnswGraph::cosine_similarity(&c, &d);
        assert!(sim2.abs() < 1e-5);
    }

    #[test]
    fn test_serialization() {
        let mut router = HnswRouterWasm::new(128, 100);
        let embedding = create_test_embedding(128, 1.0);
        router.add_pattern(&embedding, "test", r#"{"key":"value"}"#);

        let json = router.to_json().unwrap();
        let restored = HnswRouterWasm::from_json(&json).unwrap();

        assert_eq!(restored.dimensions(), 128);
        assert_eq!(restored.pattern_count(), 1);
    }

    #[test]
    fn test_clear() {
        let mut router = HnswRouterWasm::new(128, 100);
        let embedding = create_test_embedding(128, 1.0);
        router.add_pattern(&embedding, "test", "{}");

        assert_eq!(router.pattern_count(), 1);
        router.clear();
        assert_eq!(router.pattern_count(), 0);
    }

    #[test]
    fn test_get_pattern() {
        let mut router = HnswRouterWasm::new(128, 100);
        let embedding = create_test_embedding(128, 1.0);
        router.add_pattern(&embedding, "test-pattern", r#"{"foo":"bar"}"#);

        let pattern = router.get_pattern(0).unwrap();
        assert_eq!(pattern.name(), "test-pattern");
        assert_eq!(pattern.metadata(), r#"{"foo":"bar"}"#);

        assert!(router.get_pattern(1).is_none());
    }

    #[test]
    fn test_ef_search() {
        let mut router = HnswRouterWasm::new(128, 100);
        assert_eq!(router.ef_search(), DEFAULT_EF_SEARCH);

        router.set_ef_search(200);
        assert_eq!(router.ef_search(), 200);
    }
}
