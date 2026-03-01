//! REFRAG Store - Unified storage layer with hybrid search
//!
//! This module integrates the Compress, Sense, and Expand layers
//! into a cohesive REFRAG-enabled vector store.

use crate::compress::{BatchCompressor, CompressionStrategy, TensorCompressor};
use crate::expand::{ExpandLayer, ProjectorRegistry};
use crate::sense::{PolicyDecision, PolicyNetwork, RefragAction};
use crate::types::{RefragConfig, RefragEntry, RefragSearchResult, RefragStats};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ruvector_core::{SearchQuery, SearchResult, VectorEntry};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("Entry not found: {0}")]
    NotFound(String),

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Compression error: {0}")]
    CompressionError(String),

    #[error("Policy error: {0}")]
    PolicyError(String),

    #[error("Projection error: {0}")]
    ProjectionError(String),

    #[error("Core error: {0}")]
    CoreError(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// REFRAG-enabled vector store
///
/// Wraps ruvector-core with REFRAG capabilities:
/// - Stores both search vectors and representation tensors
/// - Uses policy network to decide COMPRESS vs EXPAND
/// - Projects tensors to target LLM dimensions
pub struct RefragStore {
    /// Configuration
    config: RefragConfig,
    /// Stored entries (in-memory for this example)
    entries: RwLock<HashMap<String, RefragEntry>>,
    /// Tensor compressor
    compressor: TensorCompressor,
    /// Policy network
    policy: PolicyNetwork,
    /// Expand layer
    expand: ExpandLayer,
    /// Statistics
    stats: RefragStoreStats,
}

/// Thread-safe statistics
struct RefragStoreStats {
    total_searches: AtomicU64,
    expand_count: AtomicU64,
    compress_count: AtomicU64,
    total_policy_time_us: AtomicU64,
    total_projection_time_us: AtomicU64,
}

impl RefragStoreStats {
    fn new() -> Self {
        Self {
            total_searches: AtomicU64::new(0),
            expand_count: AtomicU64::new(0),
            compress_count: AtomicU64::new(0),
            total_policy_time_us: AtomicU64::new(0),
            total_projection_time_us: AtomicU64::new(0),
        }
    }

    fn to_stats(&self) -> RefragStats {
        let total = self.total_searches.load(Ordering::Relaxed);
        RefragStats {
            total_searches: total,
            expand_count: self.expand_count.load(Ordering::Relaxed),
            compress_count: self.compress_count.load(Ordering::Relaxed),
            avg_policy_time_us: if total > 0 {
                self.total_policy_time_us.load(Ordering::Relaxed) as f64 / total as f64
            } else {
                0.0
            },
            avg_projection_time_us: if total > 0 {
                self.total_projection_time_us.load(Ordering::Relaxed) as f64 / total as f64
            } else {
                0.0
            },
            bytes_saved: 0, // Would need per-entry tracking
        }
    }
}

impl RefragStore {
    /// Create a new REFRAG store with default configuration
    pub fn new(search_dim: usize, tensor_dim: usize) -> Result<Self> {
        let config = RefragConfig {
            search_dimensions: search_dim,
            tensor_dimensions: tensor_dim,
            ..Default::default()
        };

        Self::with_config(config)
    }

    /// Create with custom configuration
    pub fn with_config(config: RefragConfig) -> Result<Self> {
        let compressor = TensorCompressor::new(config.tensor_dimensions)
            .with_strategy(CompressionStrategy::None);

        let policy = PolicyNetwork::threshold(config.compress_threshold);

        let expand = ExpandLayer::new(
            ProjectorRegistry::with_defaults(config.tensor_dimensions),
            "llama3-8b",
        );

        Ok(Self {
            config,
            entries: RwLock::new(HashMap::new()),
            compressor,
            policy,
            expand,
            stats: RefragStoreStats::new(),
        })
    }

    /// Set custom policy network
    pub fn with_policy(mut self, policy: PolicyNetwork) -> Self {
        self.policy = policy;
        self
    }

    /// Set custom expand layer
    pub fn with_expand(mut self, expand: ExpandLayer) -> Self {
        self.expand = expand;
        self
    }

    /// Insert a REFRAG entry
    pub fn insert(&self, entry: RefragEntry) -> Result<String> {
        if entry.search_vector.len() != self.config.search_dimensions {
            return Err(StoreError::DimensionMismatch {
                expected: self.config.search_dimensions,
                actual: entry.search_vector.len(),
            });
        }

        let id = entry.id.clone();
        self.entries.write().unwrap().insert(id.clone(), entry);
        Ok(id)
    }

    /// Insert with automatic tensor compression
    pub fn insert_with_tensor(
        &self,
        id: impl Into<String>,
        search_vector: Vec<f32>,
        representation_vector: Vec<f32>,
        text: impl Into<String>,
        model_id: impl Into<String>,
    ) -> Result<String> {
        // Compress the representation tensor
        let tensor = self
            .compressor
            .compress(&representation_vector)
            .map_err(|e| StoreError::CompressionError(e.to_string()))?;

        let entry = RefragEntry::new(id, search_vector, text).with_tensor(tensor, model_id);

        self.insert(entry)
    }

    /// Batch insert
    pub fn insert_batch(&self, entries: Vec<RefragEntry>) -> Result<Vec<String>> {
        let mut ids = Vec::with_capacity(entries.len());
        for entry in entries {
            ids.push(self.insert(entry)?);
        }
        Ok(ids)
    }

    /// Get entry by ID
    pub fn get(&self, id: &str) -> Result<RefragEntry> {
        self.entries
            .read()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(id.to_string()))
    }

    /// Delete entry
    pub fn delete(&self, id: &str) -> Result<bool> {
        Ok(self.entries.write().unwrap().remove(id).is_some())
    }

    /// Standard vector search (returns text only)
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<RefragSearchResult>> {
        self.search_with_options(query, k, None, false)
    }

    /// Hybrid search with REFRAG policy decisions
    ///
    /// Returns mixed COMPRESS/EXPAND results based on policy network decisions.
    pub fn search_hybrid(
        &self,
        query: &[f32],
        k: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<RefragSearchResult>> {
        self.search_with_options(query, k, threshold, true)
    }

    /// Full-featured search
    fn search_with_options(
        &self,
        query: &[f32],
        k: usize,
        threshold: Option<f32>,
        use_policy: bool,
    ) -> Result<Vec<RefragSearchResult>> {
        if query.len() != self.config.search_dimensions {
            return Err(StoreError::DimensionMismatch {
                expected: self.config.search_dimensions,
                actual: query.len(),
            });
        }

        let entries = self.entries.read().unwrap();

        // Compute similarities (brute force for this example)
        let mut scored: Vec<(&RefragEntry, f32)> = entries
            .values()
            .map(|entry| {
                let similarity = cosine_similarity(query, &entry.search_vector);
                (entry, similarity)
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Apply threshold filter
        let threshold_val = threshold.unwrap_or(0.0);
        let filtered: Vec<_> = scored
            .into_iter()
            .filter(|(_, score)| *score >= threshold_val)
            .take(k)
            .collect();

        // Process results with policy
        let mut results = Vec::with_capacity(filtered.len());

        for (entry, score) in filtered {
            self.stats.total_searches.fetch_add(1, Ordering::Relaxed);

            let result = if use_policy && entry.has_tensor() {
                self.process_with_policy(entry, query, score)?
            } else {
                // Default to EXPAND (text)
                self.stats.expand_count.fetch_add(1, Ordering::Relaxed);
                RefragSearchResult::expand(entry.id.clone(), score, entry.text_content.clone(), 1.0)
            };

            results.push(result);
        }

        Ok(results)
    }

    /// Process a single result through the REFRAG policy
    fn process_with_policy(
        &self,
        entry: &RefragEntry,
        query: &[f32],
        score: f32,
    ) -> Result<RefragSearchResult> {
        let tensor_bytes = entry.representation_tensor.as_ref().unwrap();

        // Decompress tensor for policy evaluation
        let tensor = self
            .compressor
            .decompress(tensor_bytes)
            .map_err(|e| StoreError::CompressionError(e.to_string()))?;

        // Run policy
        let start = Instant::now();
        let decision = self
            .policy
            .decide(&tensor, query)
            .map_err(|e| StoreError::PolicyError(e.to_string()))?;
        let policy_time = start.elapsed().as_micros() as u64;
        self.stats
            .total_policy_time_us
            .fetch_add(policy_time, Ordering::Relaxed);

        match decision.action {
            RefragAction::Compress => {
                self.stats.compress_count.fetch_add(1, Ordering::Relaxed);

                // Optionally project to target LLM dimensions
                let (final_tensor, projection_time) = if self.config.auto_project {
                    let model_id = entry.alignment_model_id.as_deref();
                    let start = Instant::now();
                    let projected = self
                        .expand
                        .expand_auto(&tensor, model_id)
                        .map_err(|e| StoreError::ProjectionError(e.to_string()))?;
                    let time = start.elapsed().as_micros() as u64;
                    (projected, time)
                } else {
                    (tensor, 0)
                };

                self.stats
                    .total_projection_time_us
                    .fetch_add(projection_time, Ordering::Relaxed);

                // Encode tensor as base64
                let tensor_bytes: Vec<u8> =
                    final_tensor.iter().flat_map(|f| f.to_le_bytes()).collect();
                let tensor_b64 = BASE64.encode(&tensor_bytes);

                Ok(RefragSearchResult::compress(
                    entry.id.clone(),
                    score,
                    tensor_b64,
                    final_tensor.len(),
                    entry.alignment_model_id.clone(),
                    decision.confidence,
                ))
            }
            RefragAction::Expand => {
                self.stats.expand_count.fetch_add(1, Ordering::Relaxed);

                Ok(RefragSearchResult::expand(
                    entry.id.clone(),
                    score,
                    entry.text_content.clone(),
                    decision.confidence,
                ))
            }
        }
    }

    /// Get store statistics
    pub fn stats(&self) -> RefragStats {
        self.stats.to_stats()
    }

    /// Get entry count
    pub fn len(&self) -> usize {
        self.entries.read().unwrap().len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.read().unwrap().is_empty()
    }

    /// Get configuration
    pub fn config(&self) -> &RefragConfig {
        &self.config
    }
}

/// Cosine similarity helper
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a > f32::EPSILON && norm_b > f32::EPSILON {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

/// Builder for RefragStore
pub struct RefragStoreBuilder {
    config: RefragConfig,
    policy: Option<PolicyNetwork>,
    expand: Option<ExpandLayer>,
    compression: CompressionStrategy,
}

impl RefragStoreBuilder {
    pub fn new() -> Self {
        Self {
            config: RefragConfig::default(),
            policy: None,
            expand: None,
            compression: CompressionStrategy::None,
        }
    }

    pub fn search_dimensions(mut self, dim: usize) -> Self {
        self.config.search_dimensions = dim;
        self
    }

    pub fn tensor_dimensions(mut self, dim: usize) -> Self {
        self.config.tensor_dimensions = dim;
        self
    }

    pub fn target_dimensions(mut self, dim: usize) -> Self {
        self.config.target_dimensions = dim;
        self
    }

    pub fn compress_threshold(mut self, threshold: f32) -> Self {
        self.config.compress_threshold = threshold;
        self
    }

    pub fn auto_project(mut self, enabled: bool) -> Self {
        self.config.auto_project = enabled;
        self
    }

    pub fn policy(mut self, policy: PolicyNetwork) -> Self {
        self.policy = Some(policy);
        self
    }

    pub fn expand_layer(mut self, expand: ExpandLayer) -> Self {
        self.expand = Some(expand);
        self
    }

    pub fn compression(mut self, strategy: CompressionStrategy) -> Self {
        self.compression = strategy;
        self
    }

    pub fn build(self) -> Result<RefragStore> {
        let mut store = RefragStore::with_config(self.config)?;

        if let Some(policy) = self.policy {
            store = store.with_policy(policy);
        }

        if let Some(expand) = self.expand {
            store = store.with_expand(expand);
        }

        Ok(store)
    }
}

impl Default for RefragStoreBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RefragResponseType;

    fn create_test_entry(id: &str, dim: usize) -> RefragEntry {
        let search_vec: Vec<f32> = (0..dim).map(|i| (i as f32) / (dim as f32)).collect();
        let tensor_vec: Vec<f32> = (0..768).map(|i| (i as f32) / 768.0).collect();
        let tensor_bytes: Vec<u8> = tensor_vec.iter().flat_map(|f| f.to_le_bytes()).collect();

        RefragEntry::new(id, search_vec, format!("Text content for {}", id))
            .with_tensor(tensor_bytes, "llama3-8b")
    }

    #[test]
    fn test_store_creation() {
        let store = RefragStore::new(384, 768).unwrap();
        assert_eq!(store.config().search_dimensions, 384);
        assert_eq!(store.config().tensor_dimensions, 768);
        assert!(store.is_empty());
    }

    #[test]
    fn test_insert_and_get() {
        let store = RefragStore::new(4, 768).unwrap();
        let entry = create_test_entry("doc_1", 4);

        let id = store.insert(entry.clone()).unwrap();
        assert_eq!(id, "doc_1");
        assert_eq!(store.len(), 1);

        let retrieved = store.get("doc_1").unwrap();
        assert_eq!(retrieved.id, "doc_1");
        assert!(retrieved.has_tensor());
    }

    #[test]
    fn test_standard_search() {
        let store = RefragStore::new(4, 768).unwrap();

        // Insert test entries
        for i in 0..5 {
            store
                .insert(create_test_entry(&format!("doc_{}", i), 4))
                .unwrap();
        }

        let query: Vec<f32> = (0..4).map(|i| (i as f32) / 4.0).collect();
        let results = store.search(&query, 3).unwrap();

        assert_eq!(results.len(), 3);
        // All should be EXPAND since we used standard search
        for result in &results {
            assert_eq!(result.response_type, RefragResponseType::Expand);
            assert!(result.content.is_some());
        }
    }

    #[test]
    fn test_hybrid_search() {
        // Use lower threshold to get COMPRESS results
        let store = RefragStoreBuilder::new()
            .search_dimensions(4)
            .tensor_dimensions(768)
            .compress_threshold(0.5)
            .build()
            .unwrap();

        for i in 0..5 {
            store
                .insert(create_test_entry(&format!("doc_{}", i), 4))
                .unwrap();
        }

        let query: Vec<f32> = (0..4).map(|i| (i as f32) / 4.0).collect();
        let results = store.search_hybrid(&query, 3, None).unwrap();

        assert_eq!(results.len(), 3);

        // Check that we got some policy decisions
        let stats = store.stats();
        assert!(stats.total_searches > 0);
    }

    #[test]
    fn test_statistics() {
        let store = RefragStore::new(4, 768).unwrap();

        for i in 0..3 {
            store
                .insert(create_test_entry(&format!("doc_{}", i), 4))
                .unwrap();
        }

        let query: Vec<f32> = (0..4).map(|i| (i as f32) / 4.0).collect();
        let _ = store.search_hybrid(&query, 3, None).unwrap();

        let stats = store.stats();
        assert_eq!(stats.total_searches, 3);
        assert_eq!(stats.expand_count + stats.compress_count, 3);
    }

    #[test]
    fn test_dimension_mismatch() {
        let store = RefragStore::new(4, 768).unwrap();

        let bad_entry = RefragEntry::new("bad", vec![1.0, 2.0, 3.0], "text"); // Only 3 dims
        let result = store.insert(bad_entry);

        assert!(matches!(result, Err(StoreError::DimensionMismatch { .. })));
    }
}
