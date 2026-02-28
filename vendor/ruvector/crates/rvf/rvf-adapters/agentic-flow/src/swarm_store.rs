//! `RvfSwarmStore` -- main API wrapping `RvfStore` for swarm operations.
//!
//! Maps agentic-flow's inter-agent memory sharing model onto the RVF
//! segment model:
//! - Embeddings are stored as vectors via `ingest_batch`
//! - Agent ID, key, value, and namespace are encoded as metadata fields
//! - Searches use `query` with optional namespace filtering
//! - Coordination state and learning patterns are managed by sub-stores

use std::collections::HashMap;

use rvf_runtime::options::{
    DistanceMetric, MetadataEntry, MetadataValue, QueryOptions, RvfOptions,
};
use rvf_runtime::RvfStore;
use rvf_types::RvfError;

use crate::config::{AgenticFlowConfig, ConfigError};
use crate::coordination::SwarmCoordination;
use crate::learning::LearningPatternStore;

/// Metadata field IDs for shared memory entries.
const FIELD_AGENT_ID: u16 = 0;
const FIELD_KEY: u16 = 1;
const FIELD_VALUE: u16 = 2;
const FIELD_NAMESPACE: u16 = 3;

/// A search result from shared memory, enriched with agent metadata.
#[derive(Clone, Debug)]
pub struct SharedMemoryResult {
    /// Vector ID in the underlying store.
    pub id: u64,
    /// Distance from the query embedding (lower = more similar).
    pub distance: f32,
    /// The agent that shared this memory.
    pub agent_id: String,
    /// The memory key.
    pub key: String,
}

/// A full shared memory entry retrieved by ID.
#[derive(Clone, Debug)]
pub struct SharedMemoryEntry {
    /// Vector ID in the underlying store.
    pub id: u64,
    /// The agent that shared this memory.
    pub agent_id: String,
    /// The memory key.
    pub key: String,
    /// The memory value.
    pub value: String,
    /// The namespace this entry belongs to.
    pub namespace: String,
}

/// The RVF-backed swarm store for agentic-flow.
pub struct RvfSwarmStore {
    store: RvfStore,
    config: AgenticFlowConfig,
    coordination: SwarmCoordination,
    learning: LearningPatternStore,
    /// Maps "agent_id/namespace/key" -> vector_id for fast lookup.
    key_index: HashMap<String, u64>,
    /// Maps vector_id -> SharedMemoryEntry for retrieval by ID.
    entry_index: HashMap<u64, SharedMemoryEntry>,
    /// Next vector ID to assign.
    next_id: u64,
}

impl RvfSwarmStore {
    /// Create a new swarm store, initializing the data directory and RVF file.
    pub fn create(config: AgenticFlowConfig) -> Result<Self, SwarmStoreError> {
        config.validate().map_err(SwarmStoreError::Config)?;
        config
            .ensure_dirs()
            .map_err(|e| SwarmStoreError::Io(e.to_string()))?;

        let rvf_options = RvfOptions {
            dimension: config.dimension,
            metric: DistanceMetric::Cosine,
            ..Default::default()
        };

        let store = RvfStore::create(&config.store_path(), rvf_options)
            .map_err(SwarmStoreError::Rvf)?;

        Ok(Self {
            store,
            config,
            coordination: SwarmCoordination::new(),
            learning: LearningPatternStore::new(),
            key_index: HashMap::new(),
            entry_index: HashMap::new(),
            next_id: 1,
        })
    }

    /// Open an existing swarm store.
    pub fn open(config: AgenticFlowConfig) -> Result<Self, SwarmStoreError> {
        config.validate().map_err(SwarmStoreError::Config)?;

        let store =
            RvfStore::open(&config.store_path()).map_err(SwarmStoreError::Rvf)?;

        // Rebuild next_id from the store status so new IDs don't collide.
        let status = store.status();
        let next_id = status.total_vectors + status.current_epoch as u64 + 1;

        Ok(Self {
            store,
            config,
            coordination: SwarmCoordination::new(),
            learning: LearningPatternStore::new(),
            key_index: HashMap::new(),
            entry_index: HashMap::new(),
            next_id,
        })
    }

    /// Share a memory entry with other agents.
    ///
    /// Stores the embedding vector with agent_id/key/value/namespace as
    /// metadata fields. If an entry with the same agent_id/namespace/key
    /// already exists, the old one is soft-deleted and replaced.
    ///
    /// Returns the assigned vector ID.
    pub fn share_memory(
        &mut self,
        key: &str,
        value: &str,
        namespace: &str,
        embedding: &[f32],
    ) -> Result<u64, SwarmStoreError> {
        if embedding.len() != self.config.dimension as usize {
            return Err(SwarmStoreError::DimensionMismatch {
                expected: self.config.dimension as usize,
                got: embedding.len(),
            });
        }

        let compound_key = format!(
            "{}/{}/{}",
            self.config.agent_id, namespace, key
        );

        // Soft-delete existing entry with the same compound key.
        if let Some(&old_id) = self.key_index.get(&compound_key) {
            self.store.delete(&[old_id]).map_err(SwarmStoreError::Rvf)?;
            self.entry_index.remove(&old_id);
        }

        let vector_id = self.next_id;
        self.next_id += 1;

        let metadata = vec![
            MetadataEntry {
                field_id: FIELD_AGENT_ID,
                value: MetadataValue::String(self.config.agent_id.clone()),
            },
            MetadataEntry {
                field_id: FIELD_KEY,
                value: MetadataValue::String(key.to_string()),
            },
            MetadataEntry {
                field_id: FIELD_VALUE,
                value: MetadataValue::String(value.to_string()),
            },
            MetadataEntry {
                field_id: FIELD_NAMESPACE,
                value: MetadataValue::String(namespace.to_string()),
            },
        ];

        self.store
            .ingest_batch(&[embedding], &[vector_id], Some(&metadata))
            .map_err(SwarmStoreError::Rvf)?;

        self.key_index.insert(compound_key, vector_id);
        self.entry_index.insert(
            vector_id,
            SharedMemoryEntry {
                id: vector_id,
                agent_id: self.config.agent_id.clone(),
                key: key.to_string(),
                value: value.to_string(),
                namespace: namespace.to_string(),
            },
        );

        Ok(vector_id)
    }

    /// Search for shared memories similar to the given embedding.
    ///
    /// Returns up to `k` results sorted by distance (closest first),
    /// enriched with agent metadata from the in-memory index.
    pub fn search_shared(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Vec<SharedMemoryResult> {
        let options = QueryOptions::default();
        let results = match self.store.query(embedding, k, &options) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        results
            .into_iter()
            .filter_map(|r| {
                let entry = self.entry_index.get(&r.id)?;
                Some(SharedMemoryResult {
                    id: r.id,
                    distance: r.distance,
                    agent_id: entry.agent_id.clone(),
                    key: entry.key.clone(),
                })
            })
            .collect()
    }

    /// Retrieve a shared memory entry by its vector ID.
    pub fn get_shared(&self, id: u64) -> Option<SharedMemoryEntry> {
        self.entry_index.get(&id).cloned()
    }

    /// Delete shared memory entries by their vector IDs.
    ///
    /// Returns the number of entries actually deleted.
    pub fn delete_shared(&mut self, ids: &[u64]) -> Result<usize, SwarmStoreError> {
        let existing: Vec<u64> = ids
            .iter()
            .filter(|id| self.entry_index.contains_key(id))
            .copied()
            .collect();

        if existing.is_empty() {
            return Ok(0);
        }

        self.store
            .delete(&existing)
            .map_err(SwarmStoreError::Rvf)?;

        let mut removed = 0;
        for &id in &existing {
            if let Some(entry) = self.entry_index.remove(&id) {
                let compound_key = format!(
                    "{}/{}/{}",
                    entry.agent_id, entry.namespace, entry.key
                );
                self.key_index.remove(&compound_key);
                removed += 1;
            }
        }

        Ok(removed)
    }

    /// Get a mutable reference to the coordination state tracker.
    pub fn coordination(&mut self) -> &mut SwarmCoordination {
        &mut self.coordination
    }

    /// Get an immutable reference to the coordination state tracker.
    pub fn coordination_ref(&self) -> &SwarmCoordination {
        &self.coordination
    }

    /// Get a mutable reference to the learning pattern store.
    pub fn learning(&mut self) -> &mut LearningPatternStore {
        &mut self.learning
    }

    /// Get an immutable reference to the learning pattern store.
    pub fn learning_ref(&self) -> &LearningPatternStore {
        &self.learning
    }

    /// Get the current store status.
    pub fn status(&self) -> rvf_runtime::StoreStatus {
        self.store.status()
    }

    /// Get the agent ID for this store.
    pub fn agent_id(&self) -> &str {
        &self.config.agent_id
    }

    /// Close the swarm store, releasing locks.
    pub fn close(self) -> Result<(), SwarmStoreError> {
        self.store.close().map_err(SwarmStoreError::Rvf)
    }
}

/// Errors from swarm store operations.
#[derive(Debug)]
pub enum SwarmStoreError {
    /// Underlying RVF store error.
    Rvf(RvfError),
    /// Configuration error.
    Config(ConfigError),
    /// I/O error.
    Io(String),
    /// Embedding dimension mismatch.
    DimensionMismatch { expected: usize, got: usize },
}

impl std::fmt::Display for SwarmStoreError {
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

impl std::error::Error for SwarmStoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(dir: &std::path::Path) -> AgenticFlowConfig {
        AgenticFlowConfig::new(dir, "test-agent").with_dimension(4)
    }

    fn make_embedding(seed: f32) -> Vec<f32> {
        vec![seed, seed * 0.5, seed * 0.25, seed * 0.125]
    }

    #[test]
    fn create_and_share() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfSwarmStore::create(config).unwrap();
        let id = store
            .share_memory("key1", "value1", "default", &make_embedding(1.0))
            .unwrap();
        assert!(id > 0);

        let status = store.status();
        assert_eq!(status.total_vectors, 1);

        store.close().unwrap();
    }

    #[test]
    fn share_and_search() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfSwarmStore::create(config).unwrap();

        store
            .share_memory("a", "val_a", "ns1", &[1.0, 0.0, 0.0, 0.0])
            .unwrap();
        store
            .share_memory("b", "val_b", "ns1", &[0.0, 1.0, 0.0, 0.0])
            .unwrap();
        store
            .share_memory("c", "val_c", "ns2", &[0.0, 0.0, 1.0, 0.0])
            .unwrap();

        let results = store.search_shared(&[1.0, 0.0, 0.0, 0.0], 3);
        assert_eq!(results.len(), 3);
        // Closest should be "a"
        assert_eq!(results[0].key, "a");

        store.close().unwrap();
    }

    #[test]
    fn get_shared_by_id() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfSwarmStore::create(config).unwrap();
        let id = store
            .share_memory("mykey", "myval", "ns", &make_embedding(2.0))
            .unwrap();

        let entry = store.get_shared(id).unwrap();
        assert_eq!(entry.key, "mykey");
        assert_eq!(entry.value, "myval");
        assert_eq!(entry.namespace, "ns");
        assert_eq!(entry.agent_id, "test-agent");

        assert!(store.get_shared(9999).is_none());

        store.close().unwrap();
    }

    #[test]
    fn delete_shared_entries() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfSwarmStore::create(config).unwrap();
        let id1 = store
            .share_memory("k1", "v1", "ns", &make_embedding(1.0))
            .unwrap();
        let id2 = store
            .share_memory("k2", "v2", "ns", &make_embedding(2.0))
            .unwrap();

        let removed = store.delete_shared(&[id1]).unwrap();
        assert_eq!(removed, 1);
        assert!(store.get_shared(id1).is_none());
        assert!(store.get_shared(id2).is_some());

        store.close().unwrap();
    }

    #[test]
    fn delete_nonexistent_ids() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfSwarmStore::create(config).unwrap();
        let removed = store.delete_shared(&[999, 1000]).unwrap();
        assert_eq!(removed, 0);

        store.close().unwrap();
    }

    #[test]
    fn replace_existing_key() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfSwarmStore::create(config).unwrap();
        let id1 = store
            .share_memory("k", "v1", "ns", &make_embedding(1.0))
            .unwrap();
        let id2 = store
            .share_memory("k", "v2", "ns", &make_embedding(2.0))
            .unwrap();

        assert_ne!(id1, id2);
        assert!(store.get_shared(id1).is_none());
        let entry = store.get_shared(id2).unwrap();
        assert_eq!(entry.value, "v2");

        let status = store.status();
        assert_eq!(status.total_vectors, 1);

        store.close().unwrap();
    }

    #[test]
    fn dimension_mismatch() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfSwarmStore::create(config).unwrap();
        let result = store.share_memory("k", "v", "ns", &[1.0, 2.0]);
        assert!(result.is_err());
    }

    #[test]
    fn coordination_state() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfSwarmStore::create(config).unwrap();
        store
            .coordination()
            .record_state("agent-1", "status", "active")
            .unwrap();
        store
            .coordination()
            .record_state("agent-2", "status", "idle")
            .unwrap();

        let states = store.coordination_ref().get_all_states();
        assert_eq!(states.len(), 2);

        store.close().unwrap();
    }

    #[test]
    fn learning_patterns() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfSwarmStore::create(config).unwrap();

        let id = store
            .learning()
            .store_pattern("convergent", "Use batched writes", 0.85)
            .unwrap();

        let pattern = store.learning_ref().get_pattern(id).unwrap();
        assert_eq!(pattern.pattern_type, "convergent");
        assert!((pattern.score - 0.85).abs() < f32::EPSILON);

        store.close().unwrap();
    }

    #[test]
    fn open_existing_store() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        {
            let mut store = RvfSwarmStore::create(config.clone()).unwrap();
            store
                .share_memory("k", "v", "ns", &make_embedding(1.0))
                .unwrap();
            store.close().unwrap();
        }

        {
            let store = RvfSwarmStore::open(config).unwrap();
            let status = store.status();
            assert_eq!(status.total_vectors, 1);
            store.close().unwrap();
        }
    }

    #[test]
    fn agent_id_accessor() {
        let dir = TempDir::new().unwrap();
        let config = AgenticFlowConfig::new(dir.path(), "special-agent")
            .with_dimension(4);

        let store = RvfSwarmStore::create(config).unwrap();
        assert_eq!(store.agent_id(), "special-agent");

        store.close().unwrap();
    }

    #[test]
    fn empty_store_search() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let store = RvfSwarmStore::create(config).unwrap();
        let results = store.search_shared(&[1.0, 0.0, 0.0, 0.0], 5);
        assert!(results.is_empty());

        store.close().unwrap();
    }

    #[test]
    fn consensus_votes() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfSwarmStore::create(config).unwrap();
        store
            .coordination()
            .record_consensus_vote("leader-election", "a1", true)
            .unwrap();
        store
            .coordination()
            .record_consensus_vote("leader-election", "a2", false)
            .unwrap();

        let votes = store.coordination_ref().get_votes("leader-election");
        assert_eq!(votes.len(), 2);
        assert!(votes[0].vote);
        assert!(!votes[1].vote);

        store.close().unwrap();
    }

    #[test]
    fn invalid_config_rejected() {
        let dir = TempDir::new().unwrap();

        // Zero dimension
        let config = AgenticFlowConfig::new(dir.path(), "a1").with_dimension(0);
        assert!(RvfSwarmStore::create(config).is_err());

        // Empty agent_id
        let config = AgenticFlowConfig::new(dir.path(), "").with_dimension(4);
        assert!(RvfSwarmStore::create(config).is_err());
    }
}
