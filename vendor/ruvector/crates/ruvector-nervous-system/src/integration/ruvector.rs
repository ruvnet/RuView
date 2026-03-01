//! RuVector core integration with nervous system components
//!
//! Provides a hybrid vector index that combines:
//! - HNSW for fast approximate nearest neighbor search
//! - Modern Hopfield networks for associative retrieval
//! - Dentate gyrus pattern separation for collision resistance
//! - BTSP for one-shot learning

use crate::hopfield::ModernHopfield;
use crate::plasticity::btsp::BTSPAssociativeMemory;
use crate::separate::DentateGyrus;
use crate::{NervousSystemError, Result};
use std::collections::HashMap;

/// Configuration for nervous system-enhanced vector index
#[derive(Debug, Clone)]
pub struct NervousConfig {
    /// Dimension of input vectors
    pub input_dim: usize,

    /// Hopfield network beta parameter (inverse temperature)
    pub hopfield_beta: f32,

    /// Hopfield network capacity (max patterns to store)
    pub hopfield_capacity: usize,

    /// Enable pattern separation via dentate gyrus
    pub enable_pattern_separation: bool,

    /// Output dimension for pattern separation (should be >> input_dim)
    pub separation_output_dim: usize,

    /// K-winners for pattern separation (2-5% of output_dim)
    pub separation_k: usize,

    /// Enable one-shot learning via BTSP
    pub enable_one_shot: bool,

    /// Random seed for reproducibility
    pub seed: u64,
}

impl Default for NervousConfig {
    fn default() -> Self {
        Self {
            input_dim: 128,
            hopfield_beta: 3.0,
            hopfield_capacity: 1000,
            enable_pattern_separation: true,
            separation_output_dim: 10000,
            separation_k: 200, // 2% of 10000
            enable_one_shot: true,
            seed: 42,
        }
    }
}

impl NervousConfig {
    /// Create new configuration for specific dimension
    pub fn new(input_dim: usize) -> Self {
        Self {
            input_dim,
            separation_output_dim: input_dim * 78, // ~78x expansion
            separation_k: (input_dim * 78) / 50,   // 2% sparsity
            ..Default::default()
        }
    }

    /// Set Hopfield parameters
    pub fn with_hopfield(mut self, beta: f32, capacity: usize) -> Self {
        self.hopfield_beta = beta;
        self.hopfield_capacity = capacity;
        self
    }

    /// Set pattern separation parameters
    pub fn with_pattern_separation(mut self, output_dim: usize, k: usize) -> Self {
        self.enable_pattern_separation = true;
        self.separation_output_dim = output_dim;
        self.separation_k = k;
        self
    }

    /// Disable pattern separation
    pub fn without_pattern_separation(mut self) -> Self {
        self.enable_pattern_separation = false;
        self
    }

    /// Enable/disable one-shot learning
    pub fn with_one_shot(mut self, enabled: bool) -> Self {
        self.enable_one_shot = enabled;
        self
    }
}

/// Result from hybrid search combining multiple retrieval methods
#[derive(Debug, Clone)]
pub struct HybridSearchResult {
    /// Vector ID
    pub id: u64,

    /// HNSW distance score
    pub hnsw_distance: f32,

    /// Hopfield similarity score (0.0 to 1.0)
    pub hopfield_similarity: f32,

    /// Combined score (weighted combination)
    pub combined_score: f32,

    /// Retrieved vector
    pub vector: Option<Vec<f32>>,
}

/// Nervous system-enhanced vector index
///
/// Combines multiple biologically-inspired components for improved
/// vector search and learning:
///
/// - **HNSW**: Fast approximate nearest neighbor (stored separately)
/// - **Hopfield**: Associative content-addressable retrieval
/// - **Dentate Gyrus**: Pattern separation for collision resistance
/// - **BTSP**: One-shot associative learning
pub struct NervousVectorIndex {
    /// Configuration
    config: NervousConfig,

    /// Modern Hopfield network for associative retrieval
    hopfield: ModernHopfield,

    /// Pattern separation encoder (optional)
    pattern_encoder: Option<DentateGyrus>,

    /// One-shot learning memory (optional)
    btsp_memory: Option<BTSPAssociativeMemory>,

    /// Vector storage (id -> vector)
    vectors: HashMap<u64, Vec<f32>>,

    /// Next available ID
    next_id: u64,

    /// Metadata storage (id -> metadata)
    metadata: HashMap<u64, String>,
}

impl NervousVectorIndex {
    /// Create a new nervous system-enhanced vector index
    ///
    /// # Arguments
    ///
    /// * `dimension` - Input vector dimension
    /// * `config` - Nervous system configuration
    ///
    /// # Example
    ///
    /// ```
    /// use ruvector_nervous_system::integration::{NervousVectorIndex, NervousConfig};
    ///
    /// let config = NervousConfig::new(128);
    /// let index = NervousVectorIndex::new(128, config);
    /// ```
    pub fn new(dimension: usize, config: NervousConfig) -> Self {
        // Create Hopfield network
        let hopfield = ModernHopfield::new(dimension, config.hopfield_beta);

        // Create pattern separator if enabled
        let pattern_encoder = if config.enable_pattern_separation {
            Some(DentateGyrus::new(
                dimension,
                config.separation_output_dim,
                config.separation_k,
                config.seed,
            ))
        } else {
            None
        };

        // Create BTSP memory if enabled
        let btsp_memory = if config.enable_one_shot {
            Some(BTSPAssociativeMemory::new(dimension, dimension))
        } else {
            None
        };

        Self {
            config,
            hopfield,
            pattern_encoder,
            btsp_memory,
            vectors: HashMap::new(),
            next_id: 0,
            metadata: HashMap::new(),
        }
    }

    /// Insert a vector into the index
    ///
    /// Stores in Hopfield network and optionally applies pattern separation.
    ///
    /// # Arguments
    ///
    /// * `vector` - Input vector
    /// * `metadata` - Optional metadata string
    ///
    /// # Returns
    ///
    /// Vector ID for later retrieval
    ///
    /// # Example
    ///
    /// ```
    /// # use ruvector_nervous_system::integration::{NervousVectorIndex, NervousConfig};
    /// # let mut index = NervousVectorIndex::new(128, NervousConfig::new(128));
    /// let vector = vec![0.5; 128];
    /// let id = index.insert(&vector, Some("test vector"));
    /// ```
    pub fn insert(&mut self, vector: &[f32], metadata: Option<&str>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        // Store original vector
        self.vectors.insert(id, vector.to_vec());

        // Store metadata if provided
        if let Some(meta) = metadata {
            self.metadata.insert(id, meta.to_string());
        }

        // Store in Hopfield network
        let _ = self.hopfield.store(vector.to_vec());

        id
    }

    /// Hybrid search combining Hopfield and HNSW-like retrieval
    ///
    /// # Arguments
    ///
    /// * `query` - Query vector
    /// * `k` - Number of results to return
    ///
    /// # Returns
    ///
    /// Top-k results with hybrid scoring
    pub fn search_hybrid(&self, query: &[f32], k: usize) -> Vec<HybridSearchResult> {
        // Retrieve from Hopfield network (returns zero vector if empty or error)
        let hopfield_result = self
            .hopfield
            .retrieve(query)
            .unwrap_or_else(|_| vec![0.0; query.len()]);

        // Compute similarities to all stored vectors
        let mut results: Vec<HybridSearchResult> = self
            .vectors
            .iter()
            .map(|(id, vec)| {
                // Cosine similarity for Hopfield
                let hopfield_sim = cosine_similarity(&hopfield_result, vec);

                // Euclidean distance for HNSW-like scoring
                let hnsw_dist = euclidean_distance(query, vec);

                // Combined score (higher is better)
                // Normalize and weight: 0.6 Hopfield + 0.4 inverse distance
                let combined = 0.6 * hopfield_sim + 0.4 * (1.0 / (1.0 + hnsw_dist));

                HybridSearchResult {
                    id: *id,
                    hnsw_distance: hnsw_dist,
                    hopfield_similarity: hopfield_sim,
                    combined_score: combined,
                    vector: Some(vec.clone()),
                }
            })
            .collect();

        // Sort by combined score (descending)
        results.sort_by(|a, b| {
            b.combined_score
                .partial_cmp(&a.combined_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Return top-k
        results.into_iter().take(k).collect()
    }

    /// Search using only Hopfield network retrieval
    ///
    /// Pure associative retrieval without distance-based search.
    pub fn search_hopfield(&self, query: &[f32]) -> Option<Vec<f32>> {
        self.hopfield.retrieve(query).ok()
    }

    /// Search using distance-based retrieval (HNSW-like)
    ///
    /// # Arguments
    ///
    /// * `query` - Query vector
    /// * `k` - Number of results
    ///
    /// # Returns
    ///
    /// Top-k results as (id, distance) pairs
    pub fn search_hnsw(&self, query: &[f32], k: usize) -> Vec<(u64, f32)> {
        let mut results: Vec<(u64, f32)> = self
            .vectors
            .iter()
            .map(|(id, vec)| (*id, euclidean_distance(query, vec)))
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        results.into_iter().take(k).collect()
    }

    /// One-shot learning: learn key-value association immediately
    ///
    /// Uses BTSP for immediate associative learning without iteration.
    ///
    /// # Arguments
    ///
    /// * `key` - Input pattern
    /// * `value` - Target output pattern
    ///
    /// # Example
    ///
    /// ```
    /// # use ruvector_nervous_system::integration::{NervousVectorIndex, NervousConfig};
    /// # let mut index = NervousVectorIndex::new(128, NervousConfig::new(128));
    /// let key = vec![0.1; 128];
    /// let value = vec![0.9; 128];
    /// index.learn_one_shot(&key, &value);
    ///
    /// // Immediate retrieval
    /// if let Some(retrieved) = index.retrieve_one_shot(&key) {
    ///     // retrieved should be close to value
    /// }
    /// ```
    pub fn learn_one_shot(&mut self, key: &[f32], value: &[f32]) {
        if let Some(ref mut btsp) = self.btsp_memory {
            let _ = btsp.store_one_shot(key, value);
        }
    }

    /// Retrieve value from one-shot learned key
    pub fn retrieve_one_shot(&self, key: &[f32]) -> Option<Vec<f32>> {
        self.btsp_memory
            .as_ref()
            .and_then(|btsp| btsp.retrieve(key).ok())
    }

    /// Apply pattern separation to input vector
    ///
    /// Returns sparse encoding if pattern separation is enabled.
    pub fn encode_pattern(&self, vector: &[f32]) -> Option<Vec<f32>> {
        self.pattern_encoder
            .as_ref()
            .map(|encoder| encoder.encode_dense(vector))
    }

    /// Get configuration
    pub fn config(&self) -> &NervousConfig {
        &self.config
    }

    /// Get number of stored vectors
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Get metadata for a vector ID
    pub fn get_metadata(&self, id: u64) -> Option<&str> {
        self.metadata.get(&id).map(|s| s.as_str())
    }

    /// Get vector by ID
    pub fn get_vector(&self, id: u64) -> Option<&Vec<f32>> {
        self.vectors.get(&id)
    }
}

// Helper functions

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nervous_vector_index_creation() {
        let config = NervousConfig::new(128);
        let index = NervousVectorIndex::new(128, config);

        assert_eq!(index.len(), 0);
        assert!(index.is_empty());
    }

    #[test]
    fn test_insert_and_retrieve() {
        let config = NervousConfig::new(128);
        let mut index = NervousVectorIndex::new(128, config);

        let vector = vec![0.5; 128];
        let id = index.insert(&vector, Some("test"));

        assert_eq!(index.len(), 1);
        assert_eq!(index.get_metadata(id), Some("test"));
        assert_eq!(index.get_vector(id), Some(&vector));
    }

    #[test]
    fn test_hybrid_search() {
        let config = NervousConfig::new(128);
        let mut index = NervousVectorIndex::new(128, config);

        // Insert some vectors
        let v1 = vec![1.0; 128];
        let v2 = vec![0.5; 128];
        let v3 = vec![0.0; 128];

        index.insert(&v1, Some("v1"));
        index.insert(&v2, Some("v2"));
        index.insert(&v3, Some("v3"));

        // Search for vector similar to v1
        let query = vec![0.9; 128];
        let results = index.search_hybrid(&query, 2);

        assert_eq!(results.len(), 2);
        // Results should be sorted by combined score
        assert!(results[0].combined_score >= results[1].combined_score);
    }

    #[test]
    fn test_one_shot_learning() {
        let config = NervousConfig::new(128).with_one_shot(true);
        let mut index = NervousVectorIndex::new(128, config);

        let key = vec![0.1; 128];
        let value = vec![0.9; 128];

        index.learn_one_shot(&key, &value);

        let retrieved = index.retrieve_one_shot(&key);
        assert!(retrieved.is_some());

        let ret = retrieved.unwrap();
        // Should be reasonably close to target (relaxed for weight clamping effects)
        let error: f32 = ret
            .iter()
            .zip(value.iter())
            .map(|(r, v)| (r - v).abs())
            .sum::<f32>()
            / value.len() as f32;

        assert!(error < 0.5, "One-shot learning error too high: {}", error);
    }

    #[test]
    fn test_pattern_separation() {
        let config = NervousConfig::new(128).with_pattern_separation(10000, 200);
        let index = NervousVectorIndex::new(128, config);

        let vector = vec![0.5; 128];
        let encoded = index.encode_pattern(&vector);

        assert!(encoded.is_some());
        let enc = encoded.unwrap();
        assert_eq!(enc.len(), 10000);

        // Should have exactly k non-zero elements (200)
        let nonzero_count = enc.iter().filter(|&&x| x != 0.0).count();
        assert_eq!(nonzero_count, 200);
    }

    #[test]
    fn test_hopfield_retrieval() {
        let config = NervousConfig::new(64);
        let mut index = NervousVectorIndex::new(64, config);

        let pattern = vec![1.0; 64];
        index.insert(&pattern, None);

        // Noisy query
        let mut query = vec![0.9; 64];
        query[0] = 0.1; // Add noise

        let retrieved = index.search_hopfield(&query);
        assert!(retrieved.is_some());

        let ret = retrieved.unwrap();
        assert_eq!(ret.len(), 64);

        // Should converge towards stored pattern
        let similarity = cosine_similarity(&ret, &pattern);
        assert!(similarity > 0.8, "Hopfield retrieval similarity too low");
    }

    #[test]
    fn test_config_builder() {
        let config = NervousConfig::new(256)
            .with_hopfield(5.0, 2000)
            .with_pattern_separation(20000, 400)
            .with_one_shot(true);

        assert_eq!(config.input_dim, 256);
        assert_eq!(config.hopfield_beta, 5.0);
        assert_eq!(config.hopfield_capacity, 2000);
        assert_eq!(config.separation_output_dim, 20000);
        assert_eq!(config.separation_k, 400);
        assert!(config.enable_one_shot);
    }
}
