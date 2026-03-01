//! WebAssembly Bindings for Hyperbolic HNSW
//!
//! This module provides JavaScript/TypeScript bindings for hyperbolic embeddings
//! and HNSW search in the browser and Node.js environments.
//!
//! # Usage in JavaScript
//!
//! ```javascript
//! import init, {
//!   HyperbolicIndex,
//!   poincareDistance,
//!   mobiusAdd,
//!   expMap,
//!   logMap
//! } from 'ruvector-hyperbolic-hnsw-wasm';
//!
//! // Initialize WASM module
//! await init();
//!
//! // Create index
//! const index = new HyperbolicIndex(16, 1.0); // ef_search=16, curvature=1.0
//!
//! // Insert vectors
//! index.insert(new Float32Array([0.1, 0.2, 0.3]));
//! index.insert(new Float32Array([-0.1, 0.15, 0.25]));
//!
//! // Search
//! const results = index.search(new Float32Array([0.15, 0.1, 0.2]), 2);
//! console.log(results); // [{id: 0, distance: 0.123}, ...]
//!
//! // Use low-level math operations
//! const d = poincareDistance(
//!   new Float32Array([0.3, 0.2]),
//!   new Float32Array([-0.1, 0.4]),
//!   1.0
//! );
//! ```

use ruvector_hyperbolic_hnsw::{
    exp_map, frechet_mean, log_map, mobius_add, mobius_scalar_mult, poincare_distance,
    project_to_ball, HyperbolicHnsw, HyperbolicHnswConfig, PoincareConfig, ShardedHyperbolicHnsw,
    TangentCache, DEFAULT_CURVATURE, EPS,
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[cfg(feature = "console_error_panic_hook")]
fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

/// Initialize the WASM module
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    set_panic_hook();
}

// ============================================================================
// Low-Level Math Operations
// ============================================================================

/// Compute Poincaré distance between two points
///
/// @param u - First point (Float32Array)
/// @param v - Second point (Float32Array)
/// @param curvature - Curvature parameter (positive)
/// @returns Geodesic distance in hyperbolic space
#[wasm_bindgen(js_name = poincareDistance)]
pub fn wasm_poincare_distance(u: &[f32], v: &[f32], curvature: f32) -> f32 {
    poincare_distance(u, v, curvature)
}

/// Möbius addition in Poincaré ball
///
/// Computes the hyperbolic analog of vector addition: x ⊕_c y
///
/// @param x - First point (Float32Array)
/// @param y - Second point (Float32Array)
/// @param curvature - Curvature parameter
/// @returns Result of Möbius addition (Float32Array)
#[wasm_bindgen(js_name = mobiusAdd)]
pub fn wasm_mobius_add(x: &[f32], y: &[f32], curvature: f32) -> Vec<f32> {
    mobius_add(x, y, curvature)
}

/// Möbius scalar multiplication
///
/// Computes r ⊗_c x for scalar r and point x
///
/// @param r - Scalar value
/// @param x - Point in Poincaré ball (Float32Array)
/// @param curvature - Curvature parameter
/// @returns Scaled point (Float32Array)
#[wasm_bindgen(js_name = mobiusScalarMult)]
pub fn wasm_mobius_scalar_mult(r: f32, x: &[f32], curvature: f32) -> Vec<f32> {
    mobius_scalar_mult(r, x, curvature)
}

/// Exponential map at point p
///
/// Maps a tangent vector v at point p to the Poincaré ball
///
/// @param v - Tangent vector (Float32Array)
/// @param p - Base point (Float32Array)
/// @param curvature - Curvature parameter
/// @returns Point on the manifold (Float32Array)
#[wasm_bindgen(js_name = expMap)]
pub fn wasm_exp_map(v: &[f32], p: &[f32], curvature: f32) -> Vec<f32> {
    exp_map(v, p, curvature)
}

/// Logarithmic map at point p
///
/// Maps a point y to the tangent space at point p
///
/// @param y - Target point (Float32Array)
/// @param p - Base point (Float32Array)
/// @param curvature - Curvature parameter
/// @returns Tangent vector at p (Float32Array)
#[wasm_bindgen(js_name = logMap)]
pub fn wasm_log_map(y: &[f32], p: &[f32], curvature: f32) -> Vec<f32> {
    log_map(y, p, curvature)
}

/// Project point to Poincaré ball
///
/// Ensures ||x|| < 1/√c - eps for numerical stability
///
/// @param x - Point to project (Float32Array)
/// @param curvature - Curvature parameter
/// @returns Projected point (Float32Array)
#[wasm_bindgen(js_name = projectToBall)]
pub fn wasm_project_to_ball(x: &[f32], curvature: f32) -> Vec<f32> {
    project_to_ball(x, curvature, EPS)
}

/// Compute Fréchet mean (hyperbolic centroid)
///
/// @param points - Array of points as flat Float32Array
/// @param dim - Dimension of each point
/// @param curvature - Curvature parameter
/// @returns Centroid point (Float32Array)
#[wasm_bindgen(js_name = frechetMean)]
pub fn wasm_frechet_mean(points: &[f32], dim: usize, curvature: f32) -> Result<Vec<f32>, JsValue> {
    if points.is_empty() || dim == 0 {
        return Err(JsValue::from_str("Empty points or invalid dimension"));
    }

    let point_vecs: Vec<Vec<f32>> = points.chunks(dim).map(|c| c.to_vec()).collect();

    let point_refs: Vec<&[f32]> = point_vecs.iter().map(|v| v.as_slice()).collect();

    let config = PoincareConfig::with_curvature(curvature)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    frechet_mean(&point_refs, None, &config).map_err(|e| JsValue::from_str(&e.to_string()))
}

// ============================================================================
// Search Result Type
// ============================================================================

/// Search result from hyperbolic HNSW
#[derive(Debug, Clone, Serialize, Deserialize)]
#[wasm_bindgen]
pub struct WasmSearchResult {
    /// Vector ID
    pub id: usize,
    /// Hyperbolic distance to query
    pub distance: f32,
}

#[wasm_bindgen]
impl WasmSearchResult {
    #[wasm_bindgen(constructor)]
    pub fn new(id: usize, distance: f32) -> Self {
        Self { id, distance }
    }
}

// ============================================================================
// Hyperbolic HNSW Index
// ============================================================================

/// Hyperbolic HNSW Index for hierarchy-aware vector search
///
/// @example
/// ```javascript
/// const index = new HyperbolicIndex(16, 1.0);
/// index.insert(new Float32Array([0.1, 0.2]));
/// index.insert(new Float32Array([-0.1, 0.3]));
/// const results = index.search(new Float32Array([0.05, 0.25]), 2);
/// ```
#[wasm_bindgen]
pub struct HyperbolicIndex {
    inner: HyperbolicHnsw,
}

#[wasm_bindgen]
impl HyperbolicIndex {
    /// Create a new hyperbolic HNSW index
    ///
    /// @param ef_search - Size of dynamic candidate list during search (default: 50)
    /// @param curvature - Curvature parameter for Poincaré ball (default: 1.0)
    #[wasm_bindgen(constructor)]
    pub fn new(ef_search: Option<usize>, curvature: Option<f32>) -> Self {
        let mut config = HyperbolicHnswConfig::default();
        config.ef_search = ef_search.unwrap_or(50);
        config.curvature = curvature.unwrap_or(DEFAULT_CURVATURE);

        Self {
            inner: HyperbolicHnsw::new(config),
        }
    }

    /// Create with custom configuration
    ///
    /// @param config - JSON configuration object
    #[wasm_bindgen(js_name = fromConfig)]
    pub fn from_config(config: JsValue) -> Result<HyperbolicIndex, JsValue> {
        let config: HyperbolicHnswConfig =
            serde_wasm_bindgen::from_value(config).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self {
            inner: HyperbolicHnsw::new(config),
        })
    }

    /// Insert a vector into the index
    ///
    /// @param vector - Vector to insert (Float32Array)
    /// @returns ID of inserted vector
    #[wasm_bindgen]
    pub fn insert(&mut self, vector: &[f32]) -> Result<usize, JsValue> {
        self.inner
            .insert(vector.to_vec())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Insert batch of vectors
    ///
    /// @param vectors - Flat array of vectors
    /// @param dim - Dimension of each vector
    /// @returns Array of inserted IDs
    #[wasm_bindgen(js_name = insertBatch)]
    pub fn insert_batch(&mut self, vectors: &[f32], dim: usize) -> Result<Vec<usize>, JsValue> {
        let vecs: Vec<Vec<f32>> = vectors.chunks(dim).map(|c| c.to_vec()).collect();
        self.inner
            .insert_batch(vecs)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Search for k nearest neighbors
    ///
    /// @param query - Query vector (Float32Array)
    /// @param k - Number of neighbors to return
    /// @returns Array of search results as JSON
    #[wasm_bindgen]
    pub fn search(&self, query: &[f32], k: usize) -> Result<JsValue, JsValue> {
        let results = self
            .inner
            .search(query, k)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let wasm_results: Vec<WasmSearchResult> = results
            .into_iter()
            .map(|r| WasmSearchResult::new(r.id, r.distance))
            .collect();

        serde_wasm_bindgen::to_value(&wasm_results).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Search with tangent space pruning (optimized)
    ///
    /// @param query - Query vector (Float32Array)
    /// @param k - Number of neighbors to return
    /// @returns Array of search results as JSON
    #[wasm_bindgen(js_name = searchWithPruning)]
    pub fn search_with_pruning(&self, query: &[f32], k: usize) -> Result<JsValue, JsValue> {
        let results = self
            .inner
            .search_with_pruning(query, k)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let wasm_results: Vec<WasmSearchResult> = results
            .into_iter()
            .map(|r| WasmSearchResult::new(r.id, r.distance))
            .collect();

        serde_wasm_bindgen::to_value(&wasm_results).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Build tangent cache for optimized search
    #[wasm_bindgen(js_name = buildTangentCache)]
    pub fn build_tangent_cache(&mut self) -> Result<(), JsValue> {
        self.inner
            .build_tangent_cache()
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get number of vectors in index
    #[wasm_bindgen]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if index is empty
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get vector dimension
    #[wasm_bindgen]
    pub fn dim(&self) -> Option<usize> {
        self.inner.dim()
    }

    /// Update curvature parameter
    ///
    /// @param curvature - New curvature value (must be positive)
    #[wasm_bindgen(js_name = setCurvature)]
    pub fn set_curvature(&mut self, curvature: f32) -> Result<(), JsValue> {
        self.inner
            .set_curvature(curvature)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a vector by ID
    ///
    /// @param id - Vector ID
    /// @returns Vector data or null if not found
    #[wasm_bindgen(js_name = getVector)]
    pub fn get_vector(&self, id: usize) -> Option<Vec<f32>> {
        self.inner.get_vector(id).map(|v| v.to_vec())
    }

    /// Export index configuration as JSON
    #[wasm_bindgen(js_name = exportConfig)]
    pub fn export_config(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.inner.config)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

// ============================================================================
// Sharded Index
// ============================================================================

/// Sharded Hyperbolic HNSW with per-shard curvature
///
/// @example
/// ```javascript
/// const manager = new ShardedIndex(1.0);
/// manager.insertToShard("taxonomy", new Float32Array([0.1, 0.2]), 0);
/// manager.insertToShard("taxonomy", new Float32Array([0.3, 0.1]), 3);
/// manager.updateCurvature("taxonomy", 0.5);
/// const results = manager.search(new Float32Array([0.2, 0.15]), 5);
/// ```
#[wasm_bindgen]
pub struct ShardedIndex {
    inner: ShardedHyperbolicHnsw,
}

#[wasm_bindgen]
impl ShardedIndex {
    /// Create a new sharded index
    ///
    /// @param default_curvature - Default curvature for new shards
    #[wasm_bindgen(constructor)]
    pub fn new(default_curvature: f32) -> Self {
        Self {
            inner: ShardedHyperbolicHnsw::new(default_curvature),
        }
    }

    /// Insert vector with automatic shard assignment
    ///
    /// @param vector - Vector to insert (Float32Array)
    /// @param depth - Optional hierarchy depth for shard assignment
    /// @returns Global vector ID
    #[wasm_bindgen]
    pub fn insert(&mut self, vector: &[f32], depth: Option<usize>) -> Result<usize, JsValue> {
        self.inner
            .insert(vector.to_vec(), depth)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Insert vector into specific shard
    ///
    /// @param shard_id - Target shard ID
    /// @param vector - Vector to insert (Float32Array)
    /// @returns Global vector ID
    #[wasm_bindgen(js_name = insertToShard)]
    pub fn insert_to_shard(&mut self, shard_id: &str, vector: &[f32]) -> Result<usize, JsValue> {
        self.inner
            .insert_to_shard(shard_id, vector.to_vec())
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Search across all shards
    ///
    /// @param query - Query vector (Float32Array)
    /// @param k - Number of neighbors to return
    /// @returns Array of search results as JSON
    #[wasm_bindgen]
    pub fn search(&self, query: &[f32], k: usize) -> Result<JsValue, JsValue> {
        let results = self
            .inner
            .search(query, k)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let wasm_results: Vec<WasmSearchResult> = results
            .into_iter()
            .map(|(id, r)| WasmSearchResult::new(id, r.distance))
            .collect();

        serde_wasm_bindgen::to_value(&wasm_results).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Update curvature for a shard
    ///
    /// @param shard_id - Shard ID
    /// @param curvature - New curvature value
    #[wasm_bindgen(js_name = updateCurvature)]
    pub fn update_curvature(&mut self, shard_id: &str, curvature: f32) -> Result<(), JsValue> {
        self.inner
            .update_curvature(shard_id, curvature)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Set canary curvature for A/B testing
    ///
    /// @param shard_id - Shard ID
    /// @param curvature - Canary curvature value
    /// @param traffic - Percentage of traffic for canary (0-100)
    #[wasm_bindgen(js_name = setCanaryCurvature)]
    pub fn set_canary_curvature(&mut self, shard_id: &str, curvature: f32, traffic: u8) {
        self.inner.registry.set_canary(shard_id, curvature, traffic);
    }

    /// Promote canary to production
    ///
    /// @param shard_id - Shard ID
    #[wasm_bindgen(js_name = promoteCanary)]
    pub fn promote_canary(&mut self, shard_id: &str) -> Result<(), JsValue> {
        if let Some(shard_curv) = self.inner.registry.shards.get_mut(shard_id) {
            shard_curv.promote_canary();
        }
        self.inner
            .reload_curvatures()
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Rollback canary
    ///
    /// @param shard_id - Shard ID
    #[wasm_bindgen(js_name = rollbackCanary)]
    pub fn rollback_canary(&mut self, shard_id: &str) {
        if let Some(shard_curv) = self.inner.registry.shards.get_mut(shard_id) {
            shard_curv.rollback_canary();
        }
    }

    /// Build tangent caches for all shards
    #[wasm_bindgen(js_name = buildCaches)]
    pub fn build_caches(&mut self) -> Result<(), JsValue> {
        self.inner
            .build_caches()
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get total vector count
    #[wasm_bindgen]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if empty
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get number of shards
    #[wasm_bindgen(js_name = numShards)]
    pub fn num_shards(&self) -> usize {
        self.inner.num_shards()
    }

    /// Get curvature registry as JSON
    #[wasm_bindgen(js_name = getRegistry)]
    pub fn get_registry(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.inner.registry)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

// ============================================================================
// Tangent Cache Operations
// ============================================================================

/// Tangent space cache for fast pruning
#[wasm_bindgen]
pub struct WasmTangentCache {
    inner: TangentCache,
}

#[wasm_bindgen]
impl WasmTangentCache {
    /// Create tangent cache from points
    ///
    /// @param points - Flat array of points
    /// @param dim - Dimension of each point
    /// @param curvature - Curvature parameter
    #[wasm_bindgen(constructor)]
    pub fn new(points: &[f32], dim: usize, curvature: f32) -> Result<WasmTangentCache, JsValue> {
        let point_vecs: Vec<Vec<f32>> = points.chunks(dim).map(|c| c.to_vec()).collect();
        let indices: Vec<usize> = (0..point_vecs.len()).collect();

        let cache = TangentCache::new(&point_vecs, &indices, curvature)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(Self { inner: cache })
    }

    /// Get centroid of the cache
    #[wasm_bindgen]
    pub fn centroid(&self) -> Vec<f32> {
        self.inner.centroid.clone()
    }

    /// Get tangent coordinates for a query
    ///
    /// @param query - Query point (Float32Array)
    /// @returns Tangent coordinates (Float32Array)
    #[wasm_bindgen(js_name = queryTangent)]
    pub fn query_tangent(&self, query: &[f32]) -> Vec<f32> {
        self.inner.query_tangent(query)
    }

    /// Compute tangent distance squared (for fast pruning)
    ///
    /// @param query_tangent - Query in tangent space (Float32Array)
    /// @param idx - Index of cached point
    /// @returns Squared distance in tangent space
    #[wasm_bindgen(js_name = tangentDistanceSquared)]
    pub fn tangent_distance_squared(&self, query_tangent: &[f32], idx: usize) -> f32 {
        self.inner.tangent_distance_squared(query_tangent, idx)
    }

    /// Get number of cached points
    #[wasm_bindgen]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Get dimension
    #[wasm_bindgen]
    pub fn dim(&self) -> usize {
        self.inner.dim()
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Get library version
#[wasm_bindgen(js_name = getVersion)]
pub fn get_version() -> String {
    ruvector_hyperbolic_hnsw::VERSION.to_string()
}

/// Get default curvature value
#[wasm_bindgen(js_name = getDefaultCurvature)]
pub fn get_default_curvature() -> f32 {
    DEFAULT_CURVATURE
}

/// Get numerical stability epsilon
#[wasm_bindgen(js_name = getEps)]
pub fn get_eps() -> f32 {
    EPS
}

/// Compute vector norm
#[wasm_bindgen(js_name = vectorNorm)]
pub fn vector_norm(x: &[f32]) -> f32 {
    ruvector_hyperbolic_hnsw::norm(x)
}

/// Compute squared vector norm
#[wasm_bindgen(js_name = vectorNormSquared)]
pub fn vector_norm_squared(x: &[f32]) -> f32 {
    ruvector_hyperbolic_hnsw::norm_squared(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_poincare_distance() {
        let u = vec![0.3, 0.2];
        let v = vec![-0.1, 0.4];
        let d = wasm_poincare_distance(&u, &v, 1.0);
        assert!(d > 0.0);
    }

    #[wasm_bindgen_test]
    fn test_mobius_add() {
        let x = vec![0.2, 0.1];
        let y = vec![0.1, -0.1];
        let z = wasm_mobius_add(&x, &y, 1.0);
        assert_eq!(z.len(), 2);
    }

    #[wasm_bindgen_test]
    fn test_hyperbolic_index() {
        let mut index = HyperbolicIndex::new(Some(16), Some(1.0));

        index.insert(&[0.1, 0.2, 0.3]).unwrap();
        index.insert(&[-0.1, 0.15, 0.25]).unwrap();
        index.insert(&[0.2, -0.1, 0.1]).unwrap();

        assert_eq!(index.len(), 3);
        assert!(!index.is_empty());
        assert_eq!(index.dim(), Some(3));
    }
}
