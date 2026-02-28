//! Maps agentdb HNSW operations to RVF INDEX_SEG layers.
//!
//! Bridges agentdb's HNSW index lifecycle to the three-layer progressive
//! indexing model (Layer A / B / C) defined in `rvf-index`.

use std::collections::BTreeSet;

use rvf_index::builder::{build_full_index, build_layer_a, build_layer_b, build_layer_c};
use rvf_index::distance::{cosine_distance, l2_distance};
use rvf_index::hnsw::{HnswConfig, HnswGraph};

type DistanceFn = Box<dyn Fn(&[f32], &[f32]) -> f32>;
use rvf_index::layers::{IndexLayer, LayerA, LayerB, LayerC};
use rvf_index::progressive::ProgressiveIndex;
use rvf_index::traits::InMemoryVectorStore;

/// Configuration for the RVF index adapter.
#[derive(Clone, Debug)]
pub struct IndexAdapterConfig {
    /// HNSW M parameter.
    pub m: usize,
    /// HNSW M0 (layer-0 neighbors).
    pub m0: usize,
    /// ef_construction beam width.
    pub ef_construction: usize,
    /// ef_search beam width for queries.
    pub ef_search: usize,
    /// Use cosine distance (default true for agentdb text embeddings).
    pub use_cosine: bool,
    /// Hot node fraction for Layer B (0.0 - 1.0).
    pub hot_fraction: f32,
}

impl Default for IndexAdapterConfig {
    fn default() -> Self {
        Self {
            m: 16,
            m0: 32,
            ef_construction: 200,
            ef_search: 100,
            use_cosine: true,
            hot_fraction: 0.2,
        }
    }
}

/// Adapter that maps agentdb HNSW operations to RVF INDEX_SEG layers.
///
/// Manages the full HNSW graph and can extract progressive layers (A/B/C)
/// for serialization into INDEX_SEG segments.
pub struct RvfIndexAdapter {
    config: IndexAdapterConfig,
    graph: Option<HnswGraph>,
    vectors: Vec<Vec<f32>>,
    id_map: Vec<u64>,
    progressive: ProgressiveIndex,
    loaded_layers: Vec<IndexLayer>,
}

impl RvfIndexAdapter {
    /// Create a new index adapter with the given configuration.
    pub fn new(config: IndexAdapterConfig) -> Self {
        Self {
            config,
            graph: None,
            vectors: Vec::new(),
            id_map: Vec::new(),
            progressive: ProgressiveIndex::new(),
            loaded_layers: Vec::new(),
        }
    }

    /// Build the full HNSW index from a set of vectors and IDs.
    ///
    /// This replaces any existing index.
    pub fn build(&mut self, vectors: Vec<Vec<f32>>, ids: Vec<u64>) {
        let n = vectors.len();
        if n == 0 {
            return;
        }

        let hnsw_config = HnswConfig {
            m: self.config.m,
            m0: self.config.m0,
            ef_construction: self.config.ef_construction,
        };

        let store = InMemoryVectorStore::new(vectors.clone());
        let distance_fn = self.distance_fn();

        // Generate deterministic pseudo-random values for level selection.
        let rng_values: Vec<f64> = (0..n)
            .map(|i| {
                let seed = (i as u64)
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let val = (seed >> 33) as f64 / (1u64 << 31) as f64;
                val.clamp(0.001, 0.999)
            })
            .collect();

        let graph = build_full_index(&store, n, &hnsw_config, &rng_values, &distance_fn);

        self.vectors = vectors;
        self.id_map = ids;
        self.graph = Some(graph);
    }

    /// Extract Layer A (entry points + coarse routing) from the current graph.
    pub fn extract_layer_a(&self) -> Option<LayerA> {
        let graph = self.graph.as_ref()?;
        let n = self.vectors.len();

        // Simple centroid computation: split vectors into 2 partitions.
        let mid = n / 2;
        let dim = self.vectors.first().map_or(0, |v| v.len());

        let centroid_0 = compute_centroid(&self.vectors[..mid], dim);
        let centroid_1 = if mid < n {
            compute_centroid(&self.vectors[mid..], dim)
        } else {
            centroid_0.clone()
        };

        let centroids = vec![centroid_0, centroid_1];
        let assignments: Vec<u32> = (0..n).map(|i| if i < mid { 0 } else { 1 }).collect();

        Some(build_layer_a(graph, &centroids, &assignments, n as u64))
    }

    /// Extract Layer B (hot region partial adjacency) from the current graph.
    pub fn extract_layer_b(&self) -> Option<LayerB> {
        let graph = self.graph.as_ref()?;
        let n = self.vectors.len();
        let hot_count = ((n as f32) * self.config.hot_fraction).ceil() as usize;
        let hot_ids: BTreeSet<u64> = (0..hot_count as u64).collect();
        Some(build_layer_b(graph, &hot_ids))
    }

    /// Extract Layer C (full adjacency) from the current graph.
    pub fn extract_layer_c(&self) -> Option<LayerC> {
        let graph = self.graph.as_ref()?;
        Some(build_layer_c(graph))
    }

    /// Load progressive layers and configure the progressive index for search.
    pub fn load_progressive(&mut self, layers: &[IndexLayer]) {
        self.loaded_layers = layers.to_vec();

        let mut idx = ProgressiveIndex::new();
        for layer in layers {
            match layer {
                IndexLayer::A => {
                    idx.layer_a = self.extract_layer_a();
                }
                IndexLayer::B => {
                    idx.layer_b = self.extract_layer_b();
                }
                IndexLayer::C => {
                    idx.layer_c = self.extract_layer_c();
                }
            }
        }
        self.progressive = idx;
    }

    /// Search using the progressive index with whatever layers are loaded.
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(u64, f32)> {
        let store = InMemoryVectorStore::new(self.vectors.clone());
        let distance_fn = self.distance_fn();
        self.progressive
            .search_with_distance(query, k, self.config.ef_search, &store, &distance_fn)
    }

    /// Search using the full HNSW graph directly (bypasses progressive layers).
    pub fn search_full(&self, query: &[f32], k: usize) -> Vec<(u64, f32)> {
        let graph = match self.graph.as_ref() {
            Some(g) => g,
            None => return Vec::new(),
        };
        let store = InMemoryVectorStore::new(self.vectors.clone());
        let distance_fn = self.distance_fn();
        graph.search(query, k, self.config.ef_search, &store, &distance_fn)
    }

    /// Get the node count in the HNSW graph.
    pub fn node_count(&self) -> usize {
        self.graph.as_ref().map_or(0, |g| g.node_count())
    }

    /// Get the currently loaded layers.
    pub fn loaded_layers(&self) -> &[IndexLayer] {
        &self.loaded_layers
    }

    fn distance_fn(&self) -> DistanceFn {
        if self.config.use_cosine {
            Box::new(cosine_distance)
        } else {
            Box::new(l2_distance)
        }
    }
}

/// Compute the centroid of a set of vectors.
fn compute_centroid(vectors: &[Vec<f32>], dim: usize) -> Vec<f32> {
    if vectors.is_empty() || dim == 0 {
        return vec![0.0; dim];
    }
    let n = vectors.len() as f32;
    let mut centroid = vec![0.0f32; dim];
    for v in vectors {
        for (i, &val) in v.iter().enumerate().take(dim) {
            centroid[i] += val;
        }
    }
    for c in &mut centroid {
        *c /= n;
    }
    centroid
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vectors(n: usize, dim: usize) -> (Vec<Vec<f32>>, Vec<u64>) {
        let vecs: Vec<Vec<f32>> = (0..n)
            .map(|i| (0..dim).map(|d| (i * dim + d) as f32).collect())
            .collect();
        let ids: Vec<u64> = (0..n as u64).collect();
        (vecs, ids)
    }

    #[test]
    fn build_and_search_full() {
        let (vecs, ids) = make_vectors(100, 8);
        let mut adapter = RvfIndexAdapter::new(IndexAdapterConfig {
            use_cosine: false,
            ..Default::default()
        });
        adapter.build(vecs.clone(), ids);

        assert_eq!(adapter.node_count(), 100);

        let results = adapter.search_full(&vecs[50], 5);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, 50);
    }

    #[test]
    fn extract_layers() {
        let (vecs, ids) = make_vectors(50, 4);
        let mut adapter = RvfIndexAdapter::new(IndexAdapterConfig {
            use_cosine: false,
            ..Default::default()
        });
        adapter.build(vecs, ids);

        let layer_a = adapter.extract_layer_a();
        assert!(layer_a.is_some());
        let la = layer_a.unwrap();
        assert!(!la.entry_points.is_empty());
        assert_eq!(la.centroids.len(), 2);

        let layer_b = adapter.extract_layer_b();
        assert!(layer_b.is_some());

        let layer_c = adapter.extract_layer_c();
        assert!(layer_c.is_some());
    }

    #[test]
    fn progressive_search_with_layers() {
        let (vecs, ids) = make_vectors(100, 4);
        let mut adapter = RvfIndexAdapter::new(IndexAdapterConfig {
            use_cosine: false,
            ..Default::default()
        });
        adapter.build(vecs.clone(), ids);

        // Load all three layers.
        adapter.load_progressive(&[IndexLayer::A, IndexLayer::B, IndexLayer::C]);

        let results = adapter.search(&vecs[25], 5);
        assert!(!results.is_empty());
        // With full Layer C, we should find the exact match.
        assert_eq!(results[0].0, 25);
    }

    #[test]
    fn progressive_layer_a_only() {
        let (vecs, ids) = make_vectors(100, 4);
        let mut adapter = RvfIndexAdapter::new(IndexAdapterConfig {
            use_cosine: false,
            ..Default::default()
        });
        adapter.build(vecs.clone(), ids);

        adapter.load_progressive(&[IndexLayer::A]);
        let results = adapter.search(&vecs[10], 5);
        // Layer A alone provides coarse results; we just verify non-empty.
        assert!(!results.is_empty());
    }

    #[test]
    fn empty_adapter() {
        let adapter = RvfIndexAdapter::new(IndexAdapterConfig::default());
        assert_eq!(adapter.node_count(), 0);
        let results = adapter.search_full(&[0.0; 4], 5);
        assert!(results.is_empty());
    }

    #[test]
    fn compute_centroid_basic() {
        let vecs = vec![
            vec![1.0, 2.0, 3.0],
            vec![3.0, 4.0, 5.0],
        ];
        let centroid = compute_centroid(&vecs, 3);
        assert_eq!(centroid, vec![2.0, 3.0, 4.0]);
    }
}
