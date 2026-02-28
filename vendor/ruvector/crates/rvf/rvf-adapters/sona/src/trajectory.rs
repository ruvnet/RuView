//! `TrajectoryStore` — stores learning trajectories as sequences of
//! state embeddings in the shared SONA RVF file.
//!
//! Each trajectory step records a state embedding, the action taken,
//! the reward received, and a monotonically increasing step ID. Steps
//! are stored as RVF vectors with metadata fields encoding the step
//! details and a type marker of "trajectory".

use std::collections::VecDeque;

use rvf_runtime::options::{MetadataEntry, MetadataValue, QueryOptions, RvfOptions};
use rvf_runtime::{RvfStore, SearchResult};
use rvf_types::RvfError;

use crate::config::SonaConfig;

/// Metadata field IDs (shared across all SONA stores).
const FIELD_STEP_ID: u16 = 0;
const FIELD_ACTION: u16 = 1;
const FIELD_REWARD: u16 = 2;
const FIELD_CATEGORY: u16 = 3;
const FIELD_TYPE: u16 = 4;

/// Type marker for trajectory entries.
const TYPE_TRAJECTORY: &str = "trajectory";

/// A single trajectory step returned from retrieval or search.
#[derive(Clone, Debug)]
pub struct TrajectoryStep {
    /// Internal vector ID in the RVF store.
    pub id: u64,
    /// The step identifier within the trajectory.
    pub step_id: u64,
    /// The action taken at this step.
    pub action: String,
    /// The reward received at this step.
    pub reward: f64,
    /// Distance from query (only meaningful for search results).
    pub distance: f32,
}

/// Stores learning trajectories as sequences of state embeddings.
pub struct TrajectoryStore {
    store: RvfStore,
    config: SonaConfig,
    /// In-memory ordered record of trajectory step vector IDs, newest last.
    step_ids: VecDeque<u64>,
    /// Parallel deque of step metadata for fast retrieval.
    step_meta: VecDeque<(u64, String, f64)>, // (step_id, action, reward)
    /// Next vector ID to assign.
    next_id: u64,
}

impl TrajectoryStore {
    /// Create a new trajectory store, initializing the data directory and RVF file.
    pub fn create(config: SonaConfig) -> Result<Self, SonaStoreError> {
        config.validate().map_err(SonaStoreError::Config)?;
        config.ensure_dirs().map_err(|e| SonaStoreError::Io(e.to_string()))?;

        let rvf_options = RvfOptions {
            dimension: config.dimension,
            ..Default::default()
        };

        let store = RvfStore::create(&config.store_path(), rvf_options)
            .map_err(SonaStoreError::Rvf)?;

        Ok(Self {
            store,
            config,
            step_ids: VecDeque::new(),
            step_meta: VecDeque::new(),
            next_id: 1,
        })
    }

    /// Record a single trajectory step.
    ///
    /// Returns the internal vector ID assigned to this step.
    pub fn record_step(
        &mut self,
        step_id: u64,
        state_embedding: &[f32],
        action: &str,
        reward: f64,
    ) -> Result<u64, SonaStoreError> {
        if state_embedding.len() != self.config.dimension as usize {
            return Err(SonaStoreError::DimensionMismatch {
                expected: self.config.dimension as usize,
                got: state_embedding.len(),
            });
        }

        let vector_id = self.next_id;
        self.next_id += 1;

        let metadata = vec![
            MetadataEntry { field_id: FIELD_STEP_ID, value: MetadataValue::U64(step_id) },
            MetadataEntry { field_id: FIELD_ACTION, value: MetadataValue::String(action.to_string()) },
            MetadataEntry { field_id: FIELD_REWARD, value: MetadataValue::F64(reward) },
            MetadataEntry { field_id: FIELD_CATEGORY, value: MetadataValue::String(String::new()) },
            MetadataEntry { field_id: FIELD_TYPE, value: MetadataValue::String(TYPE_TRAJECTORY.to_string()) },
        ];

        self.store
            .ingest_batch(&[state_embedding], &[vector_id], Some(&metadata))
            .map_err(SonaStoreError::Rvf)?;

        self.step_ids.push_back(vector_id);
        self.step_meta.push_back((step_id, action.to_string(), reward));

        // Trim to trajectory window size.
        while self.step_ids.len() > self.config.trajectory_window {
            self.step_ids.pop_front();
            self.step_meta.pop_front();
        }

        Ok(vector_id)
    }

    /// Get the `n` most recent trajectory steps.
    ///
    /// Returns fewer than `n` if fewer steps are available.
    pub fn get_recent(&self, n: usize) -> Vec<TrajectoryStep> {
        let len = self.step_ids.len();
        let start = len.saturating_sub(n);
        self.step_ids
            .iter()
            .zip(self.step_meta.iter())
            .skip(start)
            .map(|(&vid, (step_id, action, reward))| TrajectoryStep {
                id: vid,
                step_id: *step_id,
                action: action.clone(),
                reward: *reward,
                distance: 0.0,
            })
            .collect()
    }

    /// Search for trajectory steps whose state embeddings are most
    /// similar to the given embedding.
    pub fn search_similar_states(
        &mut self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<TrajectoryStep>, SonaStoreError> {
        if embedding.len() != self.config.dimension as usize {
            return Err(SonaStoreError::DimensionMismatch {
                expected: self.config.dimension as usize,
                got: embedding.len(),
            });
        }

        let results = self.store
            .query(embedding, k, &QueryOptions::default())
            .map_err(SonaStoreError::Rvf)?;

        Ok(self.enrich_results(&results))
    }

    /// Get all steps in the current trajectory window.
    pub fn get_trajectory_window(&self) -> Vec<TrajectoryStep> {
        self.get_recent(self.config.trajectory_window)
    }

    /// Prune old trajectory data, keeping only the most recent `keep_last_n` steps.
    ///
    /// Returns the number of steps deleted.
    pub fn clear_old(&mut self, keep_last_n: usize) -> Result<usize, SonaStoreError> {
        let len = self.step_ids.len();
        if len <= keep_last_n {
            return Ok(0);
        }

        let to_remove = len - keep_last_n;
        let mut ids_to_delete = Vec::with_capacity(to_remove);

        for _ in 0..to_remove {
            if let Some(vid) = self.step_ids.pop_front() {
                ids_to_delete.push(vid);
                self.step_meta.pop_front();
            }
        }

        if !ids_to_delete.is_empty() {
            self.store.delete(&ids_to_delete).map_err(SonaStoreError::Rvf)?;
        }

        Ok(ids_to_delete.len())
    }

    /// Return the number of steps in the current in-memory window.
    pub fn len(&self) -> usize {
        self.step_ids.len()
    }

    /// Return whether the store has no steps in the window.
    pub fn is_empty(&self) -> bool {
        self.step_ids.is_empty()
    }

    /// Close the store, releasing locks.
    pub fn close(self) -> Result<(), SonaStoreError> {
        self.store.close().map_err(SonaStoreError::Rvf)
    }

    // ── Internal ──────────────────────────────────────────────────────

    /// Enrich raw search results with step metadata from the in-memory index.
    fn enrich_results(&self, results: &[SearchResult]) -> Vec<TrajectoryStep> {
        results
            .iter()
            .map(|r| {
                let meta = self.step_ids.iter()
                    .zip(self.step_meta.iter())
                    .find(|(&vid, _)| vid == r.id)
                    .map(|(_, m)| m);

                match meta {
                    Some((step_id, action, reward)) => TrajectoryStep {
                        id: r.id,
                        step_id: *step_id,
                        action: action.clone(),
                        reward: *reward,
                        distance: r.distance,
                    },
                    None => TrajectoryStep {
                        id: r.id,
                        step_id: 0,
                        action: String::new(),
                        reward: 0.0,
                        distance: r.distance,
                    },
                }
            })
            .collect()
    }
}

/// Errors from SONA store operations.
#[derive(Debug)]
pub enum SonaStoreError {
    /// Underlying RVF store error.
    Rvf(RvfError),
    /// Configuration error.
    Config(crate::config::ConfigError),
    /// I/O error.
    Io(String),
    /// Embedding dimension mismatch.
    DimensionMismatch { expected: usize, got: usize },
}

impl std::fmt::Display for SonaStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rvf(e) => write!(f, "RVF store error: {e}"),
            Self::Config(e) => write!(f, "config error: {e}"),
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
            Self::DimensionMismatch { expected, got } => {
                write!(f, "dimension mismatch: expected {expected}, got {got}")
            }
        }
    }
}

impl std::error::Error for SonaStoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(dir: &std::path::Path) -> SonaConfig {
        SonaConfig::new(dir, 4).with_trajectory_window(5)
    }

    fn make_embedding(seed: f32) -> Vec<f32> {
        vec![seed, seed * 0.5, seed * 0.25, seed * 0.125]
    }

    #[test]
    fn record_and_get_recent() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = TrajectoryStore::create(config).unwrap();

        store.record_step(1, &make_embedding(1.0), "explore", 0.5).unwrap();
        store.record_step(2, &make_embedding(2.0), "exploit", 0.8).unwrap();
        store.record_step(3, &make_embedding(3.0), "explore", 0.3).unwrap();

        let recent = store.get_recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].step_id, 2);
        assert_eq!(recent[1].step_id, 3);
        assert_eq!(recent[1].action, "explore");

        store.close().unwrap();
    }

    #[test]
    fn get_recent_more_than_available() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = TrajectoryStore::create(config).unwrap();

        store.record_step(1, &make_embedding(1.0), "a", 0.1).unwrap();

        let recent = store.get_recent(10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].step_id, 1);

        store.close().unwrap();
    }

    #[test]
    fn trajectory_window_trimming() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path()); // window = 5
        let mut store = TrajectoryStore::create(config).unwrap();

        for i in 0..8 {
            store.record_step(i, &make_embedding(i as f32 + 0.1), "act", 0.1).unwrap();
        }

        assert_eq!(store.len(), 5);
        let window = store.get_trajectory_window();
        assert_eq!(window.len(), 5);
        // Should have steps 3..7
        assert_eq!(window[0].step_id, 3);
        assert_eq!(window[4].step_id, 7);

        store.close().unwrap();
    }

    #[test]
    fn search_similar_states() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = TrajectoryStore::create(config).unwrap();

        store.record_step(1, &[1.0, 0.0, 0.0, 0.0], "a", 0.1).unwrap();
        store.record_step(2, &[0.0, 1.0, 0.0, 0.0], "b", 0.2).unwrap();
        store.record_step(3, &[0.9, 0.1, 0.0, 0.0], "c", 0.3).unwrap();

        let results = store.search_similar_states(&[1.0, 0.0, 0.0, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
        // Closest to [1,0,0,0] should be step 1 or step 3
        assert!(results[0].distance <= results[1].distance);

        store.close().unwrap();
    }

    #[test]
    fn clear_old_steps() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = TrajectoryStore::create(config).unwrap();

        for i in 0..5 {
            store.record_step(i, &make_embedding(i as f32 + 0.1), "act", 0.1).unwrap();
        }

        let removed = store.clear_old(2).unwrap();
        assert_eq!(removed, 3);
        assert_eq!(store.len(), 2);

        let remaining = store.get_recent(10);
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].step_id, 3);
        assert_eq!(remaining[1].step_id, 4);

        store.close().unwrap();
    }

    #[test]
    fn clear_old_no_op_when_within_limit() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = TrajectoryStore::create(config).unwrap();

        store.record_step(1, &make_embedding(1.0), "a", 0.1).unwrap();

        let removed = store.clear_old(10).unwrap();
        assert_eq!(removed, 0);
        assert_eq!(store.len(), 1);

        store.close().unwrap();
    }

    #[test]
    fn empty_store_operations() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = TrajectoryStore::create(config).unwrap();

        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert!(store.get_recent(5).is_empty());
        assert!(store.get_trajectory_window().is_empty());

        let results = store.search_similar_states(&make_embedding(1.0), 5).unwrap();
        assert!(results.is_empty());

        store.close().unwrap();
    }

    #[test]
    fn dimension_mismatch() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut store = TrajectoryStore::create(config).unwrap();

        let result = store.record_step(1, &[1.0, 2.0], "a", 0.1);
        assert!(result.is_err());

        let result = store.search_similar_states(&[1.0, 2.0], 5);
        assert!(result.is_err());

        store.close().unwrap();
    }
}
