//! Memory pattern storage using RVF META_SEG.
//!
//! Stores agentdb memory patterns (task descriptions, rewards, critiques,
//! success flags) as metadata alongside their state-embedding vectors.
//! Patterns can be searched by similarity and filtered by reward threshold.

use std::collections::HashMap;
use std::path::Path;

use rvf_runtime::options::{MetadataEntry, MetadataValue};
use rvf_types::RvfError;

use crate::vector_store::{AgentDbMetric, RvfVectorStore, VectorStoreConfig};

/// A memory pattern stored in the agentdb reasoning bank.
#[derive(Clone, Debug)]
pub struct MemoryPattern {
    /// Unique pattern identifier.
    pub id: u64,
    /// Task description that produced this pattern.
    pub task: String,
    /// Reward score (0.0 - 1.0) indicating quality.
    pub reward: f32,
    /// Whether the pattern was successful.
    pub success: bool,
    /// Self-critique / notes about the pattern.
    pub critique: String,
    /// State embedding vector for similarity search.
    pub embedding: Vec<f32>,
}

/// Well-known metadata field IDs for pattern attributes.
mod field_ids {
    pub const TASK: u16 = 0;
    pub const REWARD: u16 = 1;
    pub const SUCCESS: u16 = 2;
    pub const CRITIQUE: u16 = 3;
}

/// RVF-backed memory pattern store for agentdb.
///
/// Stores patterns as vectors (embeddings) with metadata (task, reward,
/// critique, success flag). Supports similarity search with reward filtering.
pub struct RvfPatternStore {
    vector_store: RvfVectorStore,
    patterns: HashMap<u64, PatternMetadata>,
    next_id: u64,
}

/// In-memory metadata for a pattern (kept alongside the RVF store).
#[derive(Clone, Debug)]
struct PatternMetadata {
    task: String,
    reward: f32,
    success: bool,
    critique: String,
}

impl RvfPatternStore {
    /// Create a new pattern store at the given path.
    pub fn create(path: &Path, dimension: u16) -> Result<Self, RvfError> {
        let config = VectorStoreConfig {
            dimension,
            metric: AgentDbMetric::Cosine,
            ef_search: 100,
        };
        let vector_store = RvfVectorStore::create(path, config)?;
        Ok(Self {
            vector_store,
            patterns: HashMap::new(),
            next_id: 1,
        })
    }

    /// Open an existing pattern store.
    pub fn open(path: &Path, dimension: u16) -> Result<Self, RvfError> {
        let config = VectorStoreConfig {
            dimension,
            metric: AgentDbMetric::Cosine,
            ef_search: 100,
        };
        let vector_store = RvfVectorStore::open(path, config)?;
        Ok(Self {
            vector_store,
            patterns: HashMap::new(),
            next_id: 1,
        })
    }

    /// Store a memory pattern.
    ///
    /// Returns the assigned pattern ID.
    pub fn store_pattern(&mut self, pattern: MemoryPattern) -> Result<u64, RvfError> {
        let id = if pattern.id > 0 {
            pattern.id
        } else {
            let id = self.next_id;
            self.next_id += 1;
            id
        };

        // Ensure next_id stays ahead of manually assigned IDs.
        if id >= self.next_id {
            self.next_id = id + 1;
        }

        let metadata = vec![
            MetadataEntry {
                field_id: field_ids::TASK,
                value: MetadataValue::String(pattern.task.clone()),
            },
            MetadataEntry {
                field_id: field_ids::REWARD,
                value: MetadataValue::F64(pattern.reward as f64),
            },
            MetadataEntry {
                field_id: field_ids::SUCCESS,
                value: MetadataValue::U64(if pattern.success { 1 } else { 0 }),
            },
            MetadataEntry {
                field_id: field_ids::CRITIQUE,
                value: MetadataValue::String(pattern.critique.clone()),
            },
        ];

        self.vector_store
            .add_vectors(&[pattern.embedding.as_slice()], &[id], Some(&metadata))?;

        self.patterns.insert(
            id,
            PatternMetadata {
                task: pattern.task,
                reward: pattern.reward,
                success: pattern.success,
                critique: pattern.critique,
            },
        );

        Ok(id)
    }

    /// Search for patterns similar to the given embedding.
    ///
    /// Returns `(pattern_id, distance)` pairs sorted by distance.
    /// Optionally filter by minimum reward score.
    pub fn search_patterns(
        &self,
        query_embedding: &[f32],
        k: usize,
        min_reward: Option<f32>,
    ) -> Result<Vec<PatternSearchResult>, RvfError> {
        let search_k = if min_reward.is_some() { k * 3 } else { k };
        let results = self.vector_store.search(query_embedding, search_k, None)?;

        let mut filtered: Vec<PatternSearchResult> = results
            .into_iter()
            .filter_map(|r| {
                let meta = self.patterns.get(&r.id)?;
                if let Some(threshold) = min_reward {
                    if meta.reward < threshold {
                        return None;
                    }
                }
                Some(PatternSearchResult {
                    id: r.id,
                    distance: r.distance,
                    task: meta.task.clone(),
                    reward: meta.reward,
                    success: meta.success,
                    critique: meta.critique.clone(),
                })
            })
            .collect();

        filtered.truncate(k);
        Ok(filtered)
    }

    /// Search for patterns that failed (success == false).
    pub fn search_failures(
        &self,
        query_embedding: &[f32],
        k: usize,
    ) -> Result<Vec<PatternSearchResult>, RvfError> {
        let results = self.vector_store.search(query_embedding, k * 5, None)?;

        let mut filtered: Vec<PatternSearchResult> = results
            .into_iter()
            .filter_map(|r| {
                let meta = self.patterns.get(&r.id)?;
                if meta.success {
                    return None;
                }
                Some(PatternSearchResult {
                    id: r.id,
                    distance: r.distance,
                    task: meta.task.clone(),
                    reward: meta.reward,
                    success: false,
                    critique: meta.critique.clone(),
                })
            })
            .collect();

        filtered.truncate(k);
        Ok(filtered)
    }

    /// Delete a pattern by ID.
    pub fn delete_pattern(&mut self, id: u64) -> Result<bool, RvfError> {
        let deleted = self.vector_store.delete_vectors(&[id])?;
        self.patterns.remove(&id);
        Ok(deleted > 0)
    }

    /// Get pattern metadata by ID.
    pub fn get_pattern(&self, id: u64) -> Option<PatternSearchResult> {
        let meta = self.patterns.get(&id)?;
        Some(PatternSearchResult {
            id,
            distance: 0.0,
            task: meta.task.clone(),
            reward: meta.reward,
            success: meta.success,
            critique: meta.critique.clone(),
        })
    }

    /// Get aggregate statistics about stored patterns.
    pub fn stats(&self) -> PatternStoreStats {
        let total = self.patterns.len();
        let successful = self.patterns.values().filter(|p| p.success).count();
        let avg_reward = if total > 0 {
            self.patterns.values().map(|p| p.reward as f64).sum::<f64>() / total as f64
        } else {
            0.0
        };

        PatternStoreStats {
            total_patterns: total,
            successful_patterns: successful,
            failed_patterns: total - successful,
            avg_reward,
            vector_count: self.vector_store.len(),
        }
    }

    /// Save the store to disk.
    pub fn save(&mut self) -> Result<(), RvfError> {
        self.vector_store.save()
    }

    /// Get the total number of patterns.
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Returns true if no patterns are stored.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }
}

/// A pattern search result with full metadata.
#[derive(Clone, Debug)]
pub struct PatternSearchResult {
    pub id: u64,
    pub distance: f32,
    pub task: String,
    pub reward: f32,
    pub success: bool,
    pub critique: String,
}

/// Aggregate statistics for the pattern store.
#[derive(Clone, Debug)]
pub struct PatternStoreStats {
    pub total_patterns: usize,
    pub successful_patterns: usize,
    pub failed_patterns: usize,
    pub avg_reward: f64,
    pub vector_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn dummy_embedding(dim: usize, seed: u64) -> Vec<f32> {
        let mut v = Vec::with_capacity(dim);
        let mut x = seed;
        for _ in 0..dim {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
            v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
        }
        v
    }

    #[test]
    fn store_and_search_patterns() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("patterns.rvf");

        let dim = 8;
        let mut store = RvfPatternStore::create(&path, dim as u16).unwrap();

        for i in 0..10u64 {
            let pattern = MemoryPattern {
                id: 0,
                task: format!("task_{}", i),
                reward: (i as f32) / 10.0,
                success: i >= 5,
                critique: format!("critique_{}", i),
                embedding: dummy_embedding(dim, i),
            };
            store.store_pattern(pattern).unwrap();
        }

        assert_eq!(store.len(), 10);

        let query = dummy_embedding(dim, 7);
        let results = store.search_patterns(&query, 3, None).unwrap();
        assert!(!results.is_empty());
        assert!(results.len() <= 3);
    }

    #[test]
    fn search_with_min_reward() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("patterns_reward.rvf");

        let dim = 8;
        let mut store = RvfPatternStore::create(&path, dim as u16).unwrap();

        for i in 0..10u64 {
            let pattern = MemoryPattern {
                id: 0,
                task: format!("task_{}", i),
                reward: (i as f32) / 10.0,
                success: true,
                critique: String::new(),
                embedding: dummy_embedding(dim, i),
            };
            store.store_pattern(pattern).unwrap();
        }

        let query = dummy_embedding(dim, 5);
        let results = store.search_patterns(&query, 10, Some(0.5)).unwrap();
        assert!(results.iter().all(|r| r.reward >= 0.5));
    }

    #[test]
    fn search_failures() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("patterns_fail.rvf");

        let dim = 8;
        let mut store = RvfPatternStore::create(&path, dim as u16).unwrap();

        for i in 0..10u64 {
            let pattern = MemoryPattern {
                id: 0,
                task: format!("task_{}", i),
                reward: 0.5,
                success: i % 2 == 0,
                critique: String::new(),
                embedding: dummy_embedding(dim, i),
            };
            store.store_pattern(pattern).unwrap();
        }

        let query = dummy_embedding(dim, 3);
        let results = store.search_failures(&query, 5).unwrap();
        assert!(results.iter().all(|r| !r.success));
    }

    #[test]
    fn delete_pattern() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("patterns_del.rvf");

        let dim = 4;
        let mut store = RvfPatternStore::create(&path, dim as u16).unwrap();

        let pattern = MemoryPattern {
            id: 42,
            task: "test".into(),
            reward: 0.9,
            success: true,
            critique: "good".into(),
            embedding: vec![1.0, 2.0, 3.0, 4.0],
        };
        store.store_pattern(pattern).unwrap();
        assert_eq!(store.len(), 1);

        let deleted = store.delete_pattern(42).unwrap();
        assert!(deleted);
        assert_eq!(store.len(), 0);
        assert!(store.get_pattern(42).is_none());
    }

    #[test]
    fn stats() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("patterns_stats.rvf");

        let dim = 4;
        let mut store = RvfPatternStore::create(&path, dim as u16).unwrap();

        for i in 0..5u64 {
            let pattern = MemoryPattern {
                id: 0,
                task: format!("task_{}", i),
                reward: (i as f32) * 0.2,
                success: i >= 3,
                critique: String::new(),
                embedding: vec![i as f32; dim],
            };
            store.store_pattern(pattern).unwrap();
        }

        let stats = store.stats();
        assert_eq!(stats.total_patterns, 5);
        assert_eq!(stats.successful_patterns, 2);
        assert_eq!(stats.failed_patterns, 3);
        assert!(stats.avg_reward > 0.0);
    }

    #[test]
    fn get_pattern_by_id() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("patterns_get.rvf");

        let dim = 4;
        let mut store = RvfPatternStore::create(&path, dim as u16).unwrap();

        let pattern = MemoryPattern {
            id: 100,
            task: "find_bugs".into(),
            reward: 0.85,
            success: true,
            critique: "good coverage".into(),
            embedding: vec![1.0, 0.0, 0.0, 0.0],
        };
        store.store_pattern(pattern).unwrap();

        let result = store.get_pattern(100).unwrap();
        assert_eq!(result.task, "find_bugs");
        assert_eq!(result.reward, 0.85);
        assert!(result.success);
        assert_eq!(result.critique, "good coverage");

        assert!(store.get_pattern(999).is_none());
    }
}
