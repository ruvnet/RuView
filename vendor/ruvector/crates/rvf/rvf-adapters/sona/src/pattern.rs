//! `NeuralPatternStore` — stores recognized neural patterns as RVF
//! vectors with confidence scores and categories.
//!
//! Patterns can be searched by embedding similarity, filtered by
//! category, or ranked by confidence. A type marker of "pattern"
//! distinguishes these entries from trajectory and experience data.

use std::collections::HashMap;

use rvf_runtime::options::{MetadataEntry, MetadataValue, QueryOptions, RvfOptions};
use rvf_runtime::RvfStore;
use rvf_types::RvfError;

use crate::config::SonaConfig;

/// Metadata field IDs (shared across all SONA stores).
const FIELD_STEP_ID: u16 = 0;
const FIELD_NAME: u16 = 1;
const FIELD_CONFIDENCE: u16 = 2;
const FIELD_CATEGORY: u16 = 3;
const FIELD_TYPE: u16 = 4;

/// Type marker for pattern entries.
const TYPE_PATTERN: &str = "pattern";

/// A recognized neural pattern returned from retrieval or search.
#[derive(Clone, Debug)]
pub struct NeuralPattern {
    /// Internal vector ID in the RVF store.
    pub id: u64,
    /// Human-readable pattern name.
    pub name: String,
    /// Category this pattern belongs to.
    pub category: String,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f64,
    /// Distance from query (only meaningful for search results).
    pub distance: f32,
}

/// Stores recognized neural patterns as RVF vectors.
pub struct NeuralPatternStore {
    store: RvfStore,
    config: SonaConfig,
    /// In-memory index of pattern metadata keyed by vector ID.
    patterns: HashMap<u64, PatternMeta>,
    /// In-memory index of category -> vector IDs.
    category_index: HashMap<String, Vec<u64>>,
    /// Next vector ID to assign.
    next_id: u64,
}

/// In-memory metadata for a pattern.
#[derive(Clone, Debug)]
struct PatternMeta {
    name: String,
    category: String,
    confidence: f64,
}

impl NeuralPatternStore {
    /// Create a new neural pattern store.
    pub fn create(config: SonaConfig) -> Result<Self, PatternStoreError> {
        config.validate().map_err(PatternStoreError::Config)?;
        config.ensure_dirs().map_err(|e| PatternStoreError::Io(e.to_string()))?;

        let rvf_options = RvfOptions {
            dimension: config.dimension,
            ..Default::default()
        };

        let store = RvfStore::create(&config.store_path(), rvf_options)
            .map_err(PatternStoreError::Rvf)?;

        Ok(Self {
            store,
            config,
            patterns: HashMap::new(),
            category_index: HashMap::new(),
            next_id: 1,
        })
    }

    /// Store a new neural pattern.
    ///
    /// Returns the internal vector ID assigned to this pattern.
    pub fn store_pattern(
        &mut self,
        name: &str,
        category: &str,
        embedding: &[f32],
        confidence: f64,
    ) -> Result<u64, PatternStoreError> {
        if embedding.len() != self.config.dimension as usize {
            return Err(PatternStoreError::DimensionMismatch {
                expected: self.config.dimension as usize,
                got: embedding.len(),
            });
        }

        let vector_id = self.next_id;
        self.next_id += 1;

        let metadata = vec![
            MetadataEntry { field_id: FIELD_STEP_ID, value: MetadataValue::U64(vector_id) },
            MetadataEntry { field_id: FIELD_NAME, value: MetadataValue::String(name.to_string()) },
            MetadataEntry { field_id: FIELD_CONFIDENCE, value: MetadataValue::F64(confidence) },
            MetadataEntry { field_id: FIELD_CATEGORY, value: MetadataValue::String(category.to_string()) },
            MetadataEntry { field_id: FIELD_TYPE, value: MetadataValue::String(TYPE_PATTERN.to_string()) },
        ];

        self.store
            .ingest_batch(&[embedding], &[vector_id], Some(&metadata))
            .map_err(PatternStoreError::Rvf)?;

        let meta = PatternMeta {
            name: name.to_string(),
            category: category.to_string(),
            confidence,
        };
        self.patterns.insert(vector_id, meta);
        self.category_index
            .entry(category.to_string())
            .or_default()
            .push(vector_id);

        Ok(vector_id)
    }

    /// Search for patterns whose embeddings are most similar to the given embedding.
    pub fn search_patterns(
        &mut self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<NeuralPattern>, PatternStoreError> {
        if embedding.len() != self.config.dimension as usize {
            return Err(PatternStoreError::DimensionMismatch {
                expected: self.config.dimension as usize,
                got: embedding.len(),
            });
        }

        let results = self.store
            .query(embedding, k, &QueryOptions::default())
            .map_err(PatternStoreError::Rvf)?;

        Ok(self.enrich_results(&results))
    }

    /// Get all patterns in a given category.
    pub fn get_by_category(&self, category: &str) -> Vec<NeuralPattern> {
        let ids = match self.category_index.get(category) {
            Some(ids) => ids,
            None => return Vec::new(),
        };

        ids.iter()
            .filter_map(|&vid| {
                self.patterns.get(&vid).map(|meta| NeuralPattern {
                    id: vid,
                    name: meta.name.clone(),
                    category: meta.category.clone(),
                    confidence: meta.confidence,
                    distance: 0.0,
                })
            })
            .collect()
    }

    /// Update the confidence score for a pattern by its vector ID.
    pub fn update_confidence(&mut self, id: u64, confidence: f64) -> Result<(), PatternStoreError> {
        match self.patterns.get_mut(&id) {
            Some(meta) => {
                meta.confidence = confidence;
                Ok(())
            }
            None => Err(PatternStoreError::PatternNotFound(id)),
        }
    }

    /// Get the top `k` patterns ranked by confidence (highest first).
    pub fn get_top_patterns(&self, k: usize) -> Vec<NeuralPattern> {
        let mut all: Vec<_> = self.patterns.iter()
            .map(|(&vid, meta)| NeuralPattern {
                id: vid,
                name: meta.name.clone(),
                category: meta.category.clone(),
                confidence: meta.confidence,
                distance: 0.0,
            })
            .collect();

        all.sort_by(|a, b| {
            b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal)
        });
        all.truncate(k);
        all
    }

    /// Return the total number of stored patterns.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Return whether the store has no patterns.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Close the store, releasing locks.
    pub fn close(self) -> Result<(), PatternStoreError> {
        self.store.close().map_err(PatternStoreError::Rvf)
    }

    // ── Internal ──────────────────────────────────────────────────────

    fn enrich_results(&self, results: &[rvf_runtime::SearchResult]) -> Vec<NeuralPattern> {
        results
            .iter()
            .map(|r| {
                match self.patterns.get(&r.id) {
                    Some(meta) => NeuralPattern {
                        id: r.id,
                        name: meta.name.clone(),
                        category: meta.category.clone(),
                        confidence: meta.confidence,
                        distance: r.distance,
                    },
                    None => NeuralPattern {
                        id: r.id,
                        name: String::new(),
                        category: String::new(),
                        confidence: 0.0,
                        distance: r.distance,
                    },
                }
            })
            .collect()
    }
}

/// Errors from neural pattern store operations.
#[derive(Debug)]
pub enum PatternStoreError {
    /// Underlying RVF store error.
    Rvf(RvfError),
    /// Configuration error.
    Config(crate::config::ConfigError),
    /// I/O error.
    Io(String),
    /// Embedding dimension mismatch.
    DimensionMismatch { expected: usize, got: usize },
    /// Pattern not found for the given ID.
    PatternNotFound(u64),
}

impl std::fmt::Display for PatternStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rvf(e) => write!(f, "RVF store error: {e}"),
            Self::Config(e) => write!(f, "config error: {e}"),
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
            Self::DimensionMismatch { expected, got } => {
                write!(f, "dimension mismatch: expected {expected}, got {got}")
            }
            Self::PatternNotFound(id) => write!(f, "pattern not found: {id}"),
        }
    }
}

impl std::error::Error for PatternStoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(dir: &std::path::Path) -> SonaConfig {
        SonaConfig::new(dir, 4)
    }

    fn make_embedding(seed: f32) -> Vec<f32> {
        vec![seed, seed * 0.5, seed * 0.25, seed * 0.125]
    }

    #[test]
    fn store_and_search_patterns() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = NeuralPatternStore::create(config).unwrap();

        store.store_pattern("convergent", "thinking", &[1.0, 0.0, 0.0, 0.0], 0.9).unwrap();
        store.store_pattern("divergent", "thinking", &[0.0, 1.0, 0.0, 0.0], 0.7).unwrap();
        store.store_pattern("lateral", "creative", &[0.0, 0.0, 1.0, 0.0], 0.8).unwrap();

        let results = store.search_patterns(&[1.0, 0.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].distance <= results[1].distance);

        store.close().unwrap();
    }

    #[test]
    fn get_by_category() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = NeuralPatternStore::create(config).unwrap();

        store.store_pattern("p1", "alpha", &make_embedding(1.0), 0.9).unwrap();
        store.store_pattern("p2", "beta", &make_embedding(2.0), 0.7).unwrap();
        store.store_pattern("p3", "alpha", &make_embedding(3.0), 0.8).unwrap();

        let alpha = store.get_by_category("alpha");
        assert_eq!(alpha.len(), 2);
        assert!(alpha.iter().all(|p| p.category == "alpha"));

        let beta = store.get_by_category("beta");
        assert_eq!(beta.len(), 1);
        assert_eq!(beta[0].name, "p2");

        let empty = store.get_by_category("nonexistent");
        assert!(empty.is_empty());

        store.close().unwrap();
    }

    #[test]
    fn update_confidence() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = NeuralPatternStore::create(config).unwrap();

        let id = store.store_pattern("p1", "cat", &make_embedding(1.0), 0.5).unwrap();

        store.update_confidence(id, 0.95).unwrap();

        let top = store.get_top_patterns(1);
        assert_eq!(top.len(), 1);
        assert!((top[0].confidence - 0.95).abs() < f64::EPSILON);

        store.close().unwrap();
    }

    #[test]
    fn update_confidence_not_found() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = NeuralPatternStore::create(config).unwrap();

        let result = store.update_confidence(999, 0.5);
        assert!(result.is_err());

        store.close().unwrap();
    }

    #[test]
    fn get_top_patterns() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = NeuralPatternStore::create(config).unwrap();

        store.store_pattern("low", "cat", &make_embedding(1.0), 0.3).unwrap();
        store.store_pattern("high", "cat", &make_embedding(2.0), 0.9).unwrap();
        store.store_pattern("mid", "cat", &make_embedding(3.0), 0.6).unwrap();

        let top = store.get_top_patterns(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].name, "high");
        assert_eq!(top[1].name, "mid");

        store.close().unwrap();
    }

    #[test]
    fn get_top_more_than_available() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = NeuralPatternStore::create(config).unwrap();

        store.store_pattern("only", "cat", &make_embedding(1.0), 0.5).unwrap();

        let top = store.get_top_patterns(10);
        assert_eq!(top.len(), 1);

        store.close().unwrap();
    }

    #[test]
    fn empty_store_operations() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = NeuralPatternStore::create(config).unwrap();

        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        let results = store.search_patterns(&make_embedding(1.0), 5).unwrap();
        assert!(results.is_empty());

        let by_cat = store.get_by_category("anything");
        assert!(by_cat.is_empty());

        let top = store.get_top_patterns(5);
        assert!(top.is_empty());

        store.close().unwrap();
    }

    #[test]
    fn dimension_mismatch() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = NeuralPatternStore::create(config).unwrap();

        let result = store.store_pattern("p", "c", &[1.0, 2.0], 0.5);
        assert!(result.is_err());

        let result = store.search_patterns(&[1.0, 2.0], 5);
        assert!(result.is_err());

        store.close().unwrap();
    }
}
