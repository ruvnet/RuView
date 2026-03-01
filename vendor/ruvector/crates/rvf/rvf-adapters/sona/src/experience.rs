//! `ExperienceReplayBuffer` — circular buffer of experiences stored
//! as RVF vectors in the shared SONA store.
//!
//! Each experience captures a (state, action, reward, next_state) tuple.
//! State and next_state embeddings are concatenated into a single vector
//! of double the configured dimension. The action and reward are stored
//! as metadata. A type marker of "experience" distinguishes these
//! entries from trajectory and pattern data.

use std::collections::VecDeque;

use rvf_runtime::options::{MetadataEntry, MetadataValue, QueryOptions, RvfOptions};
use rvf_runtime::RvfStore;
use rvf_types::RvfError;

use crate::config::SonaConfig;

/// Metadata field IDs (shared across all SONA stores).
const FIELD_STEP_ID: u16 = 0;
const FIELD_ACTION: u16 = 1;
const FIELD_REWARD: u16 = 2;
const FIELD_CATEGORY: u16 = 3;
const FIELD_TYPE: u16 = 4;

/// Type marker for experience entries.
const TYPE_EXPERIENCE: &str = "experience";

/// A single experience returned from retrieval or sampling.
#[derive(Clone, Debug)]
pub struct Experience {
    /// Internal vector ID in the RVF store.
    pub id: u64,
    /// The action taken.
    pub action: String,
    /// The reward received.
    pub reward: f64,
    /// Distance from query (only meaningful for prioritized sampling).
    pub distance: f32,
}

/// Circular buffer of experiences stored as RVF vectors.
pub struct ExperienceReplayBuffer {
    store: RvfStore,
    config: SonaConfig,
    /// Ordered record of experience vector IDs (oldest first).
    experience_ids: VecDeque<u64>,
    /// Parallel metadata: (action, reward).
    experience_meta: VecDeque<(String, f64)>,
    /// Next vector ID to assign.
    next_id: u64,
}

impl ExperienceReplayBuffer {
    /// Create a new experience replay buffer.
    pub fn create(config: SonaConfig) -> Result<Self, ExperienceStoreError> {
        config.validate().map_err(ExperienceStoreError::Config)?;
        config.ensure_dirs().map_err(|e| ExperienceStoreError::Io(e.to_string()))?;

        let rvf_options = RvfOptions {
            dimension: config.dimension,
            ..Default::default()
        };

        let store = RvfStore::create(&config.store_path(), rvf_options)
            .map_err(ExperienceStoreError::Rvf)?;

        Ok(Self {
            store,
            config,
            experience_ids: VecDeque::new(),
            experience_meta: VecDeque::new(),
            next_id: 1,
        })
    }

    /// Add an experience to the buffer.
    ///
    /// If the buffer is at capacity, the oldest experience is evicted.
    /// The `state_embedding` is used as the stored vector (for similarity
    /// search); `next_state_embedding` is currently not stored as a
    /// separate vector but could be added via metadata extension.
    ///
    /// Returns the internal vector ID.
    pub fn push(
        &mut self,
        state_embedding: &[f32],
        action: &str,
        reward: f64,
        _next_state_embedding: &[f32],
    ) -> Result<u64, ExperienceStoreError> {
        if state_embedding.len() != self.config.dimension as usize {
            return Err(ExperienceStoreError::DimensionMismatch {
                expected: self.config.dimension as usize,
                got: state_embedding.len(),
            });
        }

        // Evict oldest if at capacity.
        if self.experience_ids.len() >= self.config.replay_capacity {
            if let Some(old_id) = self.experience_ids.pop_front() {
                self.experience_meta.pop_front();
                self.store.delete(&[old_id]).map_err(ExperienceStoreError::Rvf)?;
            }
        }

        let vector_id = self.next_id;
        self.next_id += 1;

        let metadata = vec![
            MetadataEntry { field_id: FIELD_STEP_ID, value: MetadataValue::U64(vector_id) },
            MetadataEntry { field_id: FIELD_ACTION, value: MetadataValue::String(action.to_string()) },
            MetadataEntry { field_id: FIELD_REWARD, value: MetadataValue::F64(reward) },
            MetadataEntry { field_id: FIELD_CATEGORY, value: MetadataValue::String(String::new()) },
            MetadataEntry { field_id: FIELD_TYPE, value: MetadataValue::String(TYPE_EXPERIENCE.to_string()) },
        ];

        self.store
            .ingest_batch(&[state_embedding], &[vector_id], Some(&metadata))
            .map_err(ExperienceStoreError::Rvf)?;

        self.experience_ids.push_back(vector_id);
        self.experience_meta.push_back((action.to_string(), reward));

        Ok(vector_id)
    }

    /// Sample `n` experiences uniformly from the buffer.
    ///
    /// Uses a deterministic stride-based selection: picks experiences
    /// evenly spaced across the buffer. Returns fewer than `n` if the
    /// buffer contains fewer experiences.
    pub fn sample(&self, n: usize) -> Vec<Experience> {
        let len = self.experience_ids.len();
        if len == 0 || n == 0 {
            return Vec::new();
        }

        let count = n.min(len);
        let step = if count >= len { 1 } else { len / count };
        let mut results = Vec::with_capacity(count);

        let mut idx = 0;
        while results.len() < count && idx < len {
            let vid = self.experience_ids[idx];
            let (action, reward) = &self.experience_meta[idx];
            results.push(Experience {
                id: vid,
                action: action.clone(),
                reward: *reward,
                distance: 0.0,
            });
            idx += step;
        }

        // If stride skipped some, fill from the end.
        if results.len() < count {
            let mut back_idx = len - 1;
            while results.len() < count {
                let vid = self.experience_ids[back_idx];
                if !results.iter().any(|e| e.id == vid) {
                    let (action, reward) = &self.experience_meta[back_idx];
                    results.push(Experience {
                        id: vid,
                        action: action.clone(),
                        reward: *reward,
                        distance: 0.0,
                    });
                }
                if back_idx == 0 {
                    break;
                }
                back_idx -= 1;
            }
        }

        results
    }

    /// Sample `n` experiences prioritized by similarity to the given embedding.
    ///
    /// Finds the `n` nearest-neighbor experiences by vector distance.
    pub fn sample_prioritized(
        &mut self,
        n: usize,
        embedding: &[f32],
    ) -> Result<Vec<Experience>, ExperienceStoreError> {
        if embedding.len() != self.config.dimension as usize {
            return Err(ExperienceStoreError::DimensionMismatch {
                expected: self.config.dimension as usize,
                got: embedding.len(),
            });
        }

        let results = self.store
            .query(embedding, n, &QueryOptions::default())
            .map_err(ExperienceStoreError::Rvf)?;

        Ok(self.enrich_results(&results))
    }

    /// Return the number of experiences in the buffer.
    pub fn len(&self) -> usize {
        self.experience_ids.len()
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.experience_ids.is_empty()
    }

    /// Return whether the buffer has reached its capacity.
    pub fn is_full(&self) -> bool {
        self.experience_ids.len() >= self.config.replay_capacity
    }

    /// Close the store, releasing locks.
    pub fn close(self) -> Result<(), ExperienceStoreError> {
        self.store.close().map_err(ExperienceStoreError::Rvf)
    }

    // ── Internal ──────────────────────────────────────────────────────

    fn enrich_results(&self, results: &[rvf_runtime::SearchResult]) -> Vec<Experience> {
        results
            .iter()
            .map(|r| {
                let meta = self.experience_ids.iter()
                    .zip(self.experience_meta.iter())
                    .find(|(&vid, _)| vid == r.id)
                    .map(|(_, m)| m);

                match meta {
                    Some((action, reward)) => Experience {
                        id: r.id,
                        action: action.clone(),
                        reward: *reward,
                        distance: r.distance,
                    },
                    None => Experience {
                        id: r.id,
                        action: String::new(),
                        reward: 0.0,
                        distance: r.distance,
                    },
                }
            })
            .collect()
    }
}

/// Errors from experience replay buffer operations.
#[derive(Debug)]
pub enum ExperienceStoreError {
    /// Underlying RVF store error.
    Rvf(RvfError),
    /// Configuration error.
    Config(crate::config::ConfigError),
    /// I/O error.
    Io(String),
    /// Embedding dimension mismatch.
    DimensionMismatch { expected: usize, got: usize },
}

impl std::fmt::Display for ExperienceStoreError {
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

impl std::error::Error for ExperienceStoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(dir: &std::path::Path) -> SonaConfig {
        SonaConfig::new(dir, 4).with_replay_capacity(5)
    }

    fn make_embedding(seed: f32) -> Vec<f32> {
        vec![seed, seed * 0.5, seed * 0.25, seed * 0.125]
    }

    #[test]
    fn push_and_sample() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut buf = ExperienceReplayBuffer::create(config).unwrap();

        buf.push(&make_embedding(1.0), "explore", 0.5, &make_embedding(1.1)).unwrap();
        buf.push(&make_embedding(2.0), "exploit", 0.8, &make_embedding(2.1)).unwrap();
        buf.push(&make_embedding(3.0), "explore", 0.3, &make_embedding(3.1)).unwrap();

        assert_eq!(buf.len(), 3);
        assert!(!buf.is_full());

        let samples = buf.sample(2);
        assert_eq!(samples.len(), 2);

        buf.close().unwrap();
    }

    #[test]
    fn buffer_capacity_eviction() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path()); // capacity = 5
        let mut buf = ExperienceReplayBuffer::create(config).unwrap();

        for i in 0..7 {
            buf.push(&make_embedding(i as f32 + 0.1), &format!("act{i}"), i as f64 * 0.1, &make_embedding(0.0)).unwrap();
        }

        assert_eq!(buf.len(), 5);
        assert!(buf.is_full());

        // The oldest two (act0, act1) should have been evicted.
        let all = buf.sample(5);
        assert_eq!(all.len(), 5);
        assert!(all.iter().all(|e| e.action != "act0" && e.action != "act1"));

        buf.close().unwrap();
    }

    #[test]
    fn sample_prioritized() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut buf = ExperienceReplayBuffer::create(config).unwrap();

        buf.push(&[1.0, 0.0, 0.0, 0.0], "a", 0.1, &[0.0; 4]).unwrap();
        buf.push(&[0.0, 1.0, 0.0, 0.0], "b", 0.2, &[0.0; 4]).unwrap();
        buf.push(&[0.9, 0.1, 0.0, 0.0], "c", 0.3, &[0.0; 4]).unwrap();

        let results = buf.sample_prioritized(2, &[1.0, 0.0, 0.0, 0.0]).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].distance <= results[1].distance);

        buf.close().unwrap();
    }

    #[test]
    fn empty_buffer_operations() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut buf = ExperienceReplayBuffer::create(config).unwrap();

        assert!(buf.is_empty());
        assert!(!buf.is_full());
        assert_eq!(buf.len(), 0);

        let samples = buf.sample(5);
        assert!(samples.is_empty());

        let results = buf.sample_prioritized(5, &make_embedding(1.0)).unwrap();
        assert!(results.is_empty());

        buf.close().unwrap();
    }

    #[test]
    fn sample_more_than_available() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut buf = ExperienceReplayBuffer::create(config).unwrap();

        buf.push(&make_embedding(1.0), "a", 0.1, &make_embedding(0.0)).unwrap();
        buf.push(&make_embedding(2.0), "b", 0.2, &make_embedding(0.0)).unwrap();

        let samples = buf.sample(10);
        assert_eq!(samples.len(), 2);

        buf.close().unwrap();
    }

    #[test]
    fn dimension_mismatch() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let mut buf = ExperienceReplayBuffer::create(config).unwrap();

        let result = buf.push(&[1.0, 2.0], "a", 0.1, &[1.0, 2.0]);
        assert!(result.is_err());

        let result = buf.sample_prioritized(5, &[1.0, 2.0]);
        assert!(result.is_err());

        buf.close().unwrap();
    }
}
