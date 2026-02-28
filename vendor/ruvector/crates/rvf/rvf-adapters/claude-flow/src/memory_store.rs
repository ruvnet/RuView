//! `RvfMemoryStore` — wraps `RvfStore` for claude-flow memory operations.
//!
//! Maps claude-flow's key/value/namespace/tags/embedding model onto the
//! RVF segment model:
//! - Embeddings are stored as vectors via `ingest_batch`
//! - Keys and namespaces are encoded as metadata (META_SEG fields)
//! - Searches use `query` with optional namespace filtering
//! - Deletes use soft-delete with witness recording

use std::collections::HashMap;

use rvf_runtime::filter::{FilterExpr, FilterValue};
use rvf_runtime::options::{MetadataEntry, MetadataValue, QueryOptions, RvfOptions};
use rvf_runtime::{RvfStore, SearchResult};
use rvf_types::RvfError;

use crate::config::ClaudeFlowConfig;
use crate::witness::WitnessChain;

/// Metadata field IDs for claude-flow memory entries.
const FIELD_KEY: u16 = 0;
const FIELD_NAMESPACE: u16 = 1;
const FIELD_TAGS: u16 = 2;

/// A memory entry returned from retrieval or search.
#[derive(Clone, Debug)]
pub struct MemoryEntry {
    /// The memory key.
    pub key: String,
    /// The namespace this entry belongs to.
    pub namespace: String,
    /// Tags associated with this entry.
    pub tags: Vec<String>,
    /// The vector ID in the underlying store.
    pub vector_id: u64,
    /// Distance from query (only meaningful for search results).
    pub distance: f32,
}

/// The RVF-backed memory store for claude-flow.
pub struct RvfMemoryStore {
    store: RvfStore,
    witness: Option<WitnessChain>,
    config: ClaudeFlowConfig,
    /// Maps "namespace/key" -> vector_id for fast lookup.
    key_index: HashMap<String, u64>,
    /// Next vector ID to assign.
    next_id: u64,
}

impl RvfMemoryStore {
    /// Create a new memory store, initializing the data directory and RVF file.
    pub fn create(config: ClaudeFlowConfig) -> Result<Self, MemoryStoreError> {
        config.validate().map_err(MemoryStoreError::Config)?;
        config.ensure_dirs().map_err(|e| MemoryStoreError::Io(e.to_string()))?;

        let rvf_options = RvfOptions {
            dimension: config.dimension,
            metric: config.metric,
            ..Default::default()
        };

        let store = RvfStore::create(&config.store_path(), rvf_options)
            .map_err(MemoryStoreError::Rvf)?;

        let witness = if config.enable_witness {
            Some(WitnessChain::create(&config.witness_path())
                .map_err(MemoryStoreError::Witness)?)
        } else {
            None
        };

        Ok(Self {
            store,
            witness,
            config,
            key_index: HashMap::new(),
            next_id: 1,
        })
    }

    /// Open an existing memory store.
    pub fn open(config: ClaudeFlowConfig) -> Result<Self, MemoryStoreError> {
        config.validate().map_err(MemoryStoreError::Config)?;

        let store = RvfStore::open(&config.store_path())
            .map_err(MemoryStoreError::Rvf)?;

        let witness = if config.enable_witness {
            Some(WitnessChain::open_or_create(&config.witness_path())
                .map_err(MemoryStoreError::Witness)?)
        } else {
            None
        };

        // Rebuild the key_index from the store status.
        // Since RvfStore doesn't expose metadata iteration, we start fresh.
        // Existing vectors remain searchable by embedding; key lookup is
        // rebuilt as entries are re-stored.
        let status = store.status();
        let next_id = status.total_vectors + status.current_epoch as u64 + 1;

        Ok(Self {
            store,
            witness,
            config,
            key_index: HashMap::new(),
            next_id,
        })
    }

    /// Store a memory entry with its embedding vector.
    ///
    /// If an entry with the same key and namespace already exists, the old
    /// one is soft-deleted and replaced.
    pub fn store_memory(
        &mut self,
        key: &str,
        _value: &str,
        namespace: &str,
        tags: &[String],
        embedding: &[f32],
    ) -> Result<u64, MemoryStoreError> {
        if embedding.len() != self.config.dimension as usize {
            return Err(MemoryStoreError::DimensionMismatch {
                expected: self.config.dimension as usize,
                got: embedding.len(),
            });
        }

        // If key already exists in this namespace, soft-delete the old entry.
        let compound_key = format!("{namespace}/{key}");
        if let Some(&old_id) = self.key_index.get(&compound_key) {
            self.store.delete(&[old_id]).map_err(MemoryStoreError::Rvf)?;
        }

        let vector_id = self.next_id;
        self.next_id += 1;

        // Encode tags as a comma-separated string for metadata storage.
        let tags_str = tags.join(",");

        let metadata = vec![
            MetadataEntry { field_id: FIELD_KEY, value: MetadataValue::String(key.to_string()) },
            MetadataEntry { field_id: FIELD_NAMESPACE, value: MetadataValue::String(namespace.to_string()) },
            MetadataEntry { field_id: FIELD_TAGS, value: MetadataValue::String(tags_str) },
        ];

        self.store
            .ingest_batch(&[embedding], &[vector_id], Some(&metadata))
            .map_err(MemoryStoreError::Rvf)?;

        self.key_index.insert(compound_key, vector_id);

        if let Some(ref mut w) = self.witness {
            let _ = w.record_store(key, namespace);
        }

        Ok(vector_id)
    }

    /// Search memory by embedding vector, optionally filtering by namespace.
    pub fn search_memory(
        &mut self,
        query_embedding: &[f32],
        k: usize,
        namespace: Option<&str>,
        _threshold: Option<f32>,
    ) -> Result<Vec<SearchResult>, MemoryStoreError> {
        if query_embedding.len() != self.config.dimension as usize {
            return Err(MemoryStoreError::DimensionMismatch {
                expected: self.config.dimension as usize,
                got: query_embedding.len(),
            });
        }

        let filter = namespace.map(|ns| {
            FilterExpr::Eq(FIELD_NAMESPACE, FilterValue::String(ns.to_string()))
        });

        let options = QueryOptions {
            filter,
            ..Default::default()
        };

        let results = self.store.query(query_embedding, k, &options)
            .map_err(MemoryStoreError::Rvf)?;

        if let Some(ref mut w) = self.witness {
            let ns = namespace.unwrap_or("*");
            let _ = w.record_search(ns, k);
        }

        Ok(results)
    }

    /// Retrieve a memory entry by key and namespace.
    ///
    /// Returns the vector ID if found (the entry can then be used with
    /// the underlying store for further operations).
    pub fn retrieve_memory(
        &self,
        key: &str,
        namespace: &str,
    ) -> Option<u64> {
        let compound_key = format!("{namespace}/{key}");
        self.key_index.get(&compound_key).copied()
    }

    /// Soft-delete a memory entry by key and namespace.
    pub fn delete_memory(
        &mut self,
        key: &str,
        namespace: &str,
    ) -> Result<bool, MemoryStoreError> {
        let compound_key = format!("{namespace}/{key}");
        if let Some(vector_id) = self.key_index.remove(&compound_key) {
            self.store.delete(&[vector_id]).map_err(MemoryStoreError::Rvf)?;

            if let Some(ref mut w) = self.witness {
                let _ = w.record_delete(key, namespace);
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Run compaction on the underlying store.
    pub fn compact(&mut self) -> Result<(), MemoryStoreError> {
        self.store.compact().map_err(MemoryStoreError::Rvf)?;

        if let Some(ref mut w) = self.witness {
            let _ = w.record_compact();
        }

        Ok(())
    }

    /// Get the current store status.
    pub fn status(&self) -> rvf_runtime::StoreStatus {
        self.store.status()
    }

    /// Return a reference to the witness chain (if enabled).
    pub fn witness(&self) -> Option<&WitnessChain> {
        self.witness.as_ref()
    }

    /// Close the memory store, releasing locks.
    pub fn close(self) -> Result<(), MemoryStoreError> {
        self.store.close().map_err(MemoryStoreError::Rvf)
    }
}

/// Errors from memory store operations.
#[derive(Debug)]
pub enum MemoryStoreError {
    /// Underlying RVF store error.
    Rvf(RvfError),
    /// Witness chain error.
    Witness(crate::witness::WitnessError),
    /// Configuration error.
    Config(crate::config::ConfigError),
    /// I/O error.
    Io(String),
    /// Embedding dimension mismatch.
    DimensionMismatch { expected: usize, got: usize },
}

impl std::fmt::Display for MemoryStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rvf(e) => write!(f, "RVF store error: {e}"),
            Self::Witness(e) => write!(f, "witness error: {e}"),
            Self::Config(e) => write!(f, "config error: {e}"),
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
            Self::DimensionMismatch { expected, got } => {
                write!(f, "dimension mismatch: expected {expected}, got {got}")
            }
        }
    }
}

impl std::error::Error for MemoryStoreError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn test_config(dir: &Path) -> ClaudeFlowConfig {
        ClaudeFlowConfig::new(dir, 4)
    }

    fn make_embedding(seed: f32) -> Vec<f32> {
        vec![seed, seed * 0.5, seed * 0.25, seed * 0.125]
    }

    #[test]
    fn create_and_store() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfMemoryStore::create(config).unwrap();
        let id = store.store_memory(
            "key1", "value1", "default", &["tag1".into(), "tag2".into()],
            &make_embedding(1.0),
        ).unwrap();
        assert!(id > 0);

        let status = store.status();
        assert_eq!(status.total_vectors, 1);

        store.close().unwrap();
    }

    #[test]
    fn store_and_search() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfMemoryStore::create(config).unwrap();

        store.store_memory("a", "val_a", "ns1", &[], &[1.0, 0.0, 0.0, 0.0]).unwrap();
        store.store_memory("b", "val_b", "ns1", &[], &[0.0, 1.0, 0.0, 0.0]).unwrap();
        store.store_memory("c", "val_c", "ns2", &[], &[0.0, 0.0, 1.0, 0.0]).unwrap();

        // Search all namespaces
        let results = store.search_memory(&[1.0, 0.0, 0.0, 0.0], 3, None, None).unwrap();
        assert_eq!(results.len(), 3);

        // Search filtered by namespace
        let results = store.search_memory(&[1.0, 0.0, 0.0, 0.0], 3, Some("ns1"), None).unwrap();
        assert_eq!(results.len(), 2);

        store.close().unwrap();
    }

    #[test]
    fn retrieve_by_key() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfMemoryStore::create(config).unwrap();
        let id = store.store_memory("mykey", "myval", "ns", &[], &make_embedding(2.0)).unwrap();

        assert_eq!(store.retrieve_memory("mykey", "ns"), Some(id));
        assert_eq!(store.retrieve_memory("missing", "ns"), None);
        assert_eq!(store.retrieve_memory("mykey", "other_ns"), None);

        store.close().unwrap();
    }

    #[test]
    fn delete_memory() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfMemoryStore::create(config).unwrap();
        store.store_memory("k", "v", "ns", &[], &make_embedding(3.0)).unwrap();

        assert!(store.delete_memory("k", "ns").unwrap());
        assert!(!store.delete_memory("k", "ns").unwrap()); // already deleted
        assert_eq!(store.retrieve_memory("k", "ns"), None);

        store.close().unwrap();
    }

    #[test]
    fn replace_existing_key() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfMemoryStore::create(config).unwrap();
        let id1 = store.store_memory("k", "v1", "ns", &[], &make_embedding(1.0)).unwrap();
        let id2 = store.store_memory("k", "v2", "ns", &[], &make_embedding(2.0)).unwrap();

        // New ID should be different (old was soft-deleted)
        assert_ne!(id1, id2);
        assert_eq!(store.retrieve_memory("k", "ns"), Some(id2));

        // Only one live vector
        let status = store.status();
        assert_eq!(status.total_vectors, 1);

        store.close().unwrap();
    }

    #[test]
    fn dimension_mismatch() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfMemoryStore::create(config).unwrap();
        let result = store.store_memory("k", "v", "ns", &[], &[1.0, 2.0]); // dim=2 vs config dim=4
        assert!(result.is_err());
    }

    #[test]
    fn witness_audit_trail() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfMemoryStore::create(config).unwrap();
        store.store_memory("a", "v", "ns", &[], &make_embedding(1.0)).unwrap();
        store.search_memory(&make_embedding(1.0), 1, None, None).unwrap();
        store.delete_memory("a", "ns").unwrap();

        let witness = store.witness().unwrap();
        assert_eq!(witness.len(), 3); // store + search + delete
        assert_eq!(witness.verify().unwrap(), 3);

        store.close().unwrap();
    }

    #[test]
    fn compact_works() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());

        let mut store = RvfMemoryStore::create(config).unwrap();
        store.store_memory("a", "v", "ns", &[], &make_embedding(1.0)).unwrap();
        store.store_memory("b", "v", "ns", &[], &make_embedding(2.0)).unwrap();
        store.delete_memory("a", "ns").unwrap();
        store.compact().unwrap();

        let status = store.status();
        assert_eq!(status.total_vectors, 1);

        store.close().unwrap();
    }

    #[test]
    fn no_witness_when_disabled() {
        let dir = TempDir::new().unwrap();
        let config = ClaudeFlowConfig::new(dir.path(), 4).with_witness(false);

        let store = RvfMemoryStore::create(config).unwrap();
        assert!(store.witness().is_none());
        store.close().unwrap();
    }
}
