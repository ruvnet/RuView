//! RVF-backed vector store for agentdb.
//!
//! Wraps [`RvfStore`] to provide the vector CRUD operations that agentdb
//! expects: add, search, delete, get, save, and load.

use std::path::{Path, PathBuf};

use rvf_runtime::options::{
    DistanceMetric, MetadataEntry, QueryOptions, RvfOptions, SearchResult,
};
use rvf_runtime::RvfStore;
use rvf_types::{ErrorCode, RvfError};

/// Distance metric selection matching agentdb's API.
#[derive(Clone, Copy, Debug, Default)]
pub enum AgentDbMetric {
    #[default]
    Cosine,
    L2,
    InnerProduct,
}

impl From<AgentDbMetric> for DistanceMetric {
    fn from(m: AgentDbMetric) -> Self {
        match m {
            AgentDbMetric::Cosine => DistanceMetric::Cosine,
            AgentDbMetric::L2 => DistanceMetric::L2,
            AgentDbMetric::InnerProduct => DistanceMetric::InnerProduct,
        }
    }
}

/// Configuration for the RVF vector store.
#[derive(Clone, Debug)]
pub struct VectorStoreConfig {
    /// Vector dimensionality.
    pub dimension: u16,
    /// Distance metric for similarity search.
    pub metric: AgentDbMetric,
    /// HNSW ef_search beam width for queries.
    pub ef_search: u16,
}

impl Default for VectorStoreConfig {
    fn default() -> Self {
        Self {
            dimension: 128,
            metric: AgentDbMetric::Cosine,
            ef_search: 100,
        }
    }
}

/// RVF-backed vector store that provides the agentdb vector storage interface.
///
/// Maps agentdb operations to RvfStore calls:
/// - `add_vectors` -> `ingest_batch`
/// - `search` -> `query`
/// - `delete_vectors` -> `delete`
/// - `get_vector` -> single-vector query
/// - `save` / `load` -> close / open
pub struct RvfVectorStore {
    store: Option<RvfStore>,
    path: PathBuf,
    config: VectorStoreConfig,
}

impl RvfVectorStore {
    /// Create a new RVF vector store at the given path.
    pub fn create(path: &Path, config: VectorStoreConfig) -> Result<Self, RvfError> {
        let rvf_opts = RvfOptions {
            dimension: config.dimension,
            metric: config.metric.into(),
            profile: 1, // RVText profile
            ..Default::default()
        };

        let store = RvfStore::create(path, rvf_opts)?;

        Ok(Self {
            store: Some(store),
            path: path.to_path_buf(),
            config,
        })
    }

    /// Open an existing RVF vector store.
    pub fn open(path: &Path, config: VectorStoreConfig) -> Result<Self, RvfError> {
        let store = RvfStore::open(path)?;
        Ok(Self {
            store: Some(store),
            path: path.to_path_buf(),
            config,
        })
    }

    /// Add vectors with their IDs and optional metadata.
    ///
    /// `vectors`: slice of float slices, one per vector.
    /// `ids`: one ID per vector.
    /// `metadata`: optional metadata entries (flat list, one entry per vector).
    pub fn add_vectors(
        &mut self,
        vectors: &[&[f32]],
        ids: &[u64],
        metadata: Option<&[MetadataEntry]>,
    ) -> Result<u64, RvfError> {
        let store = self.store.as_mut().ok_or(RvfError::Code(ErrorCode::InvalidManifest))?;
        let result = store.ingest_batch(vectors, ids, metadata)?;
        Ok(result.accepted)
    }

    /// Search for the k nearest neighbors of a query vector.
    ///
    /// Returns results sorted by distance (ascending).
    pub fn search(
        &self,
        query: &[f32],
        k: usize,
        ef_search: Option<u16>,
    ) -> Result<Vec<SearchResult>, RvfError> {
        let store = self.store.as_ref().ok_or(RvfError::Code(ErrorCode::InvalidManifest))?;
        let opts = QueryOptions {
            ef_search: ef_search.unwrap_or(self.config.ef_search),
            ..Default::default()
        };
        store.query(query, k, &opts)
    }

    /// Delete vectors by their IDs.
    pub fn delete_vectors(&mut self, ids: &[u64]) -> Result<u64, RvfError> {
        let store = self.store.as_mut().ok_or(RvfError::Code(ErrorCode::InvalidManifest))?;
        let result = store.delete(ids)?;
        Ok(result.deleted)
    }

    /// Retrieve a single vector by ID.
    ///
    /// Uses a zero-distance search trick: queries with each candidate until
    /// the exact ID is found. For small stores this is acceptable; for large
    /// stores the caller should maintain an ID index.
    ///
    /// Returns `None` if the vector is not found or has been deleted.
    pub fn get_vector(&self, id: u64) -> Option<SearchResult> {
        let store = self.store.as_ref()?;
        let status = store.status();
        if status.total_vectors == 0 {
            return None;
        }
        // Query a large k and find the matching ID in results.
        // This is O(n) but correct. Production agentdb should cache vectors.
        let dim = self.config.dimension as usize;
        let zero_query = vec![0.0f32; dim];
        let opts = QueryOptions {
            ef_search: self.config.ef_search,
            ..Default::default()
        };
        let results = store.query(&zero_query, status.total_vectors as usize, &opts).ok()?;
        results.into_iter().find(|r| r.id == id)
    }

    /// Save the store (flushes and closes the underlying RVF file).
    pub fn save(&mut self) -> Result<(), RvfError> {
        if let Some(store) = self.store.take() {
            store.close()?;
        }
        Ok(())
    }

    /// Reload the store from disk.
    pub fn load(&mut self) -> Result<(), RvfError> {
        if self.store.is_some() {
            return Ok(());
        }
        let store = RvfStore::open(&self.path)?;
        self.store = Some(store);
        Ok(())
    }

    /// Get the current vector count.
    pub fn len(&self) -> u64 {
        self.store.as_ref().map_or(0, |s| s.status().total_vectors)
    }

    /// Returns true if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Run compaction to reclaim space from deleted vectors.
    pub fn compact(&mut self) -> Result<u64, RvfError> {
        let store = self.store.as_mut().ok_or(RvfError::Code(ErrorCode::InvalidManifest))?;
        let result = store.compact()?;
        Ok(result.bytes_reclaimed)
    }

    /// Get the file path of the underlying RVF store.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the store configuration.
    pub fn config(&self) -> &VectorStoreConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rvf_runtime::options::MetadataValue;
    use tempfile::TempDir;

    fn make_config(dim: u16) -> VectorStoreConfig {
        VectorStoreConfig {
            dimension: dim,
            metric: AgentDbMetric::L2,
            ef_search: 100,
        }
    }

    #[test]
    fn create_add_search() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agentdb.rvf");

        let mut store = RvfVectorStore::create(&path, make_config(4)).unwrap();

        let v1 = [1.0f32, 0.0, 0.0, 0.0];
        let v2 = [0.0f32, 1.0, 0.0, 0.0];
        let v3 = [0.0f32, 0.0, 1.0, 0.0];

        let accepted = store
            .add_vectors(&[&v1, &v2, &v3], &[10, 20, 30], None)
            .unwrap();
        assert_eq!(accepted, 3);

        let results = store.search(&[1.0, 0.0, 0.0, 0.0], 2, None).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, 10);
        assert!(results[0].distance < f32::EPSILON);
    }

    #[test]
    fn delete_and_compact() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agentdb_del.rvf");

        let mut store = RvfVectorStore::create(&path, make_config(4)).unwrap();

        let vecs: Vec<[f32; 4]> = (0..10).map(|i| [i as f32, 0.0, 0.0, 0.0]).collect();
        let refs: Vec<&[f32]> = vecs.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (0..10).collect();

        store.add_vectors(&refs, &ids, None).unwrap();

        let deleted = store.delete_vectors(&[0, 2, 4]).unwrap();
        assert_eq!(deleted, 3);
        assert_eq!(store.len(), 7);

        let reclaimed = store.compact().unwrap();
        assert!(reclaimed > 0);
        assert_eq!(store.len(), 7);
    }

    #[test]
    fn save_and_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agentdb_persist.rvf");

        let config = make_config(4);
        {
            let mut store = RvfVectorStore::create(&path, config.clone()).unwrap();
            let v1 = [1.0f32, 2.0, 3.0, 4.0];
            store.add_vectors(&[&v1], &[42], None).unwrap();
            store.save().unwrap();
        }

        {
            let store = RvfVectorStore::open(&path, config).unwrap();
            assert_eq!(store.len(), 1);
            let results = store.search(&[1.0, 2.0, 3.0, 4.0], 1, None).unwrap();
            assert_eq!(results[0].id, 42);
        }
    }

    #[test]
    fn add_with_metadata() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agentdb_meta.rvf");

        let mut store = RvfVectorStore::create(&path, make_config(4)).unwrap();

        let v1 = [1.0f32, 0.0, 0.0, 0.0];
        let v2 = [0.0f32, 1.0, 0.0, 0.0];

        let metadata = vec![
            MetadataEntry {
                field_id: 0,
                value: MetadataValue::String("episode_a".into()),
            },
            MetadataEntry {
                field_id: 0,
                value: MetadataValue::String("episode_b".into()),
            },
        ];

        let accepted = store
            .add_vectors(&[&v1, &v2], &[1, 2], Some(&metadata))
            .unwrap();
        assert_eq!(accepted, 2);
    }

    #[test]
    fn empty_store() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agentdb_empty.rvf");

        let store = RvfVectorStore::create(&path, make_config(4)).unwrap();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);

        let results = store.search(&[0.0, 0.0, 0.0, 0.0], 5, None).unwrap();
        assert!(results.is_empty());
    }
}
