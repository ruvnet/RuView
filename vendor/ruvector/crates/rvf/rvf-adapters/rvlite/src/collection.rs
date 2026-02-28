//! The main rvlite collection API.
//!
//! [`RvliteCollection`] provides a minimal, ergonomic interface for
//! embedded vector storage. No metadata, no filters, no namespaces --
//! just vectors with IDs.

use std::path::Path;

use rvf_runtime::options::{QueryOptions, RvfOptions};
use rvf_runtime::store::RvfStore;

use crate::config::RvliteConfig;
use crate::error::{Result, RvliteError};

/// A single search result: vector ID and distance from the query.
#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    /// The vector's unique identifier.
    pub id: u64,
    /// Distance from the query vector (lower = more similar).
    pub distance: f32,
}

/// Statistics returned by the [`RvliteCollection::compact`] operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactStats {
    /// Number of segments that were compacted.
    pub segments_compacted: u32,
    /// Total bytes of dead space reclaimed.
    pub bytes_reclaimed: u64,
}

/// A lightweight embedded vector collection wrapping [`RvfStore`].
pub struct RvliteCollection {
    store: RvfStore,
    dimension: u16,
}

impl RvliteCollection {
    /// Create a new collection at the configured path (file must not exist).
    pub fn create(config: RvliteConfig) -> Result<Self> {
        let options = RvfOptions {
            dimension: config.dimension,
            metric: config.metric.into(),
            profile: 1, // Core profile
            ..Default::default()
        };

        let store = RvfStore::create(&config.path, options)?;
        Ok(Self {
            store,
            dimension: config.dimension,
        })
    }

    /// Open an existing collection (file must exist with a valid RVF manifest).
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let store = RvfStore::open(path.as_ref())?;
        // The dimension is stored in the manifest and recovered on boot,
        // so we query it via a probe against the store.
        let dim = Self::probe_dimension(&store);
        Ok(Self {
            store,
            dimension: dim,
        })
    }

    /// Add a single vector with the given ID. Errors on dimension mismatch.
    pub fn add(&mut self, id: u64, vector: &[f32]) -> Result<()> {
        self.check_dimension(vector.len())?;
        self.store.ingest_batch(&[vector], &[id], None)?;
        Ok(())
    }

    /// Add multiple vectors in a single batch. Returns count added.
    pub fn add_batch(&mut self, ids: &[u64], vectors: &[&[f32]]) -> Result<usize> {
        if ids.len() != vectors.len() {
            return Err(RvliteError::Io(
                "ids and vectors must have the same length".into(),
            ));
        }
        let result = self.store.ingest_batch(vectors, ids, None)?;
        Ok(result.accepted as usize)
    }

    /// Find the `k` nearest neighbors, sorted by distance (closest first).
    pub fn search(&self, vector: &[f32], k: usize) -> Vec<Match> {
        if vector.len() != self.dimension as usize {
            return Vec::new();
        }
        let query_opts = QueryOptions::default();
        match self.store.query(vector, k, &query_opts) {
            Ok(results) => results
                .into_iter()
                .map(|r| Match {
                    id: r.id,
                    distance: r.distance,
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Remove a single vector by ID. Returns whether it existed.
    pub fn remove(&mut self, id: u64) -> Result<bool> {
        let result = self.store.delete(&[id])?;
        Ok(result.deleted > 0)
    }

    /// Remove multiple vectors by ID. Returns count actually removed.
    pub fn remove_batch(&mut self, ids: &[u64]) -> Result<usize> {
        let result = self.store.delete(ids)?;
        Ok(result.deleted as usize)
    }

    /// Check whether a vector with the given ID exists (soft-deleted = absent).
    pub fn contains(&self, id: u64) -> bool {
        let total = self.store.status().total_vectors as usize;
        if total == 0 {
            return false;
        }
        // Brute-force scan via query; acceptable for rvlite's small collections.
        let zero_vec = vec![0.0f32; self.dimension as usize];
        match self.store.query(&zero_vec, total, &QueryOptions::default()) {
            Ok(results) => results.iter().any(|r| r.id == id),
            Err(_) => false,
        }
    }

    /// Return the number of live (non-deleted) vectors in the collection.
    pub fn len(&self) -> usize {
        self.store.status().total_vectors as usize
    }

    /// Return `true` if the collection has no live vectors.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Compact the collection, reclaiming space from deleted vectors.
    pub fn compact(&mut self) -> Result<CompactStats> {
        let result = self.store.compact()?;
        Ok(CompactStats {
            segments_compacted: result.segments_compacted,
            bytes_reclaimed: result.bytes_reclaimed,
        })
    }

    /// Flush all pending writes and close the collection, consuming the handle.
    pub fn close(self) -> Result<()> {
        self.store.close()?;
        Ok(())
    }

    /// Return the configured vector dimension.
    pub fn dimension(&self) -> u16 {
        self.dimension
    }

    // ---- Internal helpers ------------------------------------------------

    /// Validate that a vector length matches the collection dimension.
    fn check_dimension(&self, len: usize) -> Result<()> {
        if len != self.dimension as usize {
            return Err(RvliteError::DimensionMismatch {
                expected: self.dimension,
                got: len,
            });
        }
        Ok(())
    }

    /// Probe the dimension of an opened store by trying queries with
    /// increasing dimensions until one succeeds.
    ///
    /// RvfStore stores the dimension internally but does not expose it
    /// directly. When there are vectors present, a query with the wrong
    /// dimension returns `DimensionMismatch`, so we try dimensions
    /// 1..=4096 until one succeeds. For empty stores we return 0 as a
    /// sentinel.
    fn probe_dimension(store: &RvfStore) -> u16 {
        if store.status().total_vectors == 0 {
            return 0;
        }
        let opts = QueryOptions::default();
        for dim in 1u16..=4096 {
            let probe = vec![0.0f32; dim as usize];
            if store.query(&probe, 1, &opts).is_ok() {
                return dim;
            }
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RvliteConfig, RvliteMetric};
    use tempfile::TempDir;

    fn temp_path(dir: &TempDir, name: &str) -> std::path::PathBuf {
        dir.path().join(name)
    }

    #[test]
    fn create_add_search() {
        let dir = TempDir::new().unwrap();
        let config =
            RvliteConfig::new(temp_path(&dir, "basic.rvf"), 4).with_metric(RvliteMetric::L2);

        let mut col = RvliteCollection::create(config).unwrap();
        assert!(col.is_empty());
        assert_eq!(col.len(), 0);

        col.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
        col.add(2, &[0.0, 1.0, 0.0, 0.0]).unwrap();
        col.add(3, &[0.0, 0.0, 1.0, 0.0]).unwrap();

        assert_eq!(col.len(), 3);
        assert!(!col.is_empty());

        let results = col.search(&[1.0, 0.0, 0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, 1);
        assert!(results[0].distance < f32::EPSILON);

        col.close().unwrap();
    }

    #[test]
    fn batch_add_and_search() {
        let dir = TempDir::new().unwrap();
        let config =
            RvliteConfig::new(temp_path(&dir, "batch.rvf"), 3).with_metric(RvliteMetric::L2);

        let mut col = RvliteCollection::create(config).unwrap();

        let ids = vec![10, 20, 30];
        let v1 = [1.0, 0.0, 0.0];
        let v2 = [0.0, 1.0, 0.0];
        let v3 = [0.0, 0.0, 1.0];
        let vecs: Vec<&[f32]> = vec![&v1, &v2, &v3];

        let count = col.add_batch(&ids, &vecs).unwrap();
        assert_eq!(count, 3);
        assert_eq!(col.len(), 3);

        let results = col.search(&[0.0, 1.0, 0.0], 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 20);

        col.close().unwrap();
    }

    #[test]
    fn remove_and_verify() {
        let dir = TempDir::new().unwrap();
        let config =
            RvliteConfig::new(temp_path(&dir, "remove.rvf"), 4).with_metric(RvliteMetric::L2);

        let mut col = RvliteCollection::create(config).unwrap();

        col.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
        col.add(2, &[0.0, 1.0, 0.0, 0.0]).unwrap();
        col.add(3, &[0.0, 0.0, 1.0, 0.0]).unwrap();

        assert_eq!(col.len(), 3);
        assert!(col.contains(2));

        let removed = col.remove(2).unwrap();
        assert!(removed);
        assert_eq!(col.len(), 2);
        assert!(!col.contains(2));

        // Removing again returns false
        let removed_again = col.remove(2).unwrap();
        assert!(!removed_again);

        col.close().unwrap();
    }

    #[test]
    fn remove_batch_and_verify() {
        let dir = TempDir::new().unwrap();
        let config =
            RvliteConfig::new(temp_path(&dir, "rm_batch.rvf"), 4).with_metric(RvliteMetric::L2);

        let mut col = RvliteCollection::create(config).unwrap();

        for i in 0..5u64 {
            col.add(i, &[i as f32, 0.0, 0.0, 0.0]).unwrap();
        }

        let count = col.remove_batch(&[1, 3, 99]).unwrap();
        // 99 never existed, so only 2 are removed
        assert_eq!(count, 2);
        assert_eq!(col.len(), 3);

        col.close().unwrap();
    }

    #[test]
    fn dimension_mismatch_error() {
        let dir = TempDir::new().unwrap();
        let config = RvliteConfig::new(temp_path(&dir, "dim.rvf"), 4).with_metric(RvliteMetric::L2);

        let mut col = RvliteCollection::create(config).unwrap();

        // Wrong dimension: 3 instead of 4
        let result = col.add(1, &[1.0, 0.0, 0.0]);
        assert!(result.is_err());
        match result.unwrap_err() {
            RvliteError::DimensionMismatch { expected, got } => {
                assert_eq!(expected, 4);
                assert_eq!(got, 3);
            }
            other => panic!("expected DimensionMismatch, got: {other}"),
        }

        col.close().unwrap();
    }

    #[test]
    fn empty_collection_edge_cases() {
        let dir = TempDir::new().unwrap();
        let config =
            RvliteConfig::new(temp_path(&dir, "empty.rvf"), 4).with_metric(RvliteMetric::L2);

        let col = RvliteCollection::create(config).unwrap();

        assert!(col.is_empty());
        assert_eq!(col.len(), 0);
        assert!(!col.contains(1));

        let results = col.search(&[1.0, 0.0, 0.0, 0.0], 10);
        assert!(results.is_empty());

        col.close().unwrap();
    }

    #[test]
    fn search_returns_empty_on_wrong_dimension() {
        let dir = TempDir::new().unwrap();
        let config =
            RvliteConfig::new(temp_path(&dir, "dim_search.rvf"), 4).with_metric(RvliteMetric::L2);

        let mut col = RvliteCollection::create(config).unwrap();
        col.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();

        // Search with wrong dimension returns empty (graceful degradation)
        let results = col.search(&[1.0, 0.0], 10);
        assert!(results.is_empty());

        col.close().unwrap();
    }

    #[test]
    fn open_existing_collection() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir, "reopen.rvf");
        let config = RvliteConfig::new(path.clone(), 4).with_metric(RvliteMetric::L2);

        {
            let mut col = RvliteCollection::create(config).unwrap();
            col.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
            col.add(2, &[0.0, 1.0, 0.0, 0.0]).unwrap();
            col.close().unwrap();
        }

        {
            let col = RvliteCollection::open(&path).unwrap();
            assert_eq!(col.len(), 2);
            assert_eq!(col.dimension(), 4);

            let results = col.search(&[1.0, 0.0, 0.0, 0.0], 2);
            assert_eq!(results.len(), 2);
            assert_eq!(results[0].id, 1);

            col.close().unwrap();
        }
    }

    #[test]
    fn compact_and_verify() {
        let dir = TempDir::new().unwrap();
        let config =
            RvliteConfig::new(temp_path(&dir, "compact.rvf"), 4).with_metric(RvliteMetric::L2);

        let mut col = RvliteCollection::create(config).unwrap();

        for i in 0..10u64 {
            col.add(i, &[i as f32, 0.0, 0.0, 0.0]).unwrap();
        }

        col.remove_batch(&[0, 2, 4, 6, 8]).unwrap();
        assert_eq!(col.len(), 5);

        let stats = col.compact().unwrap();
        assert_eq!(stats.segments_compacted, 5);
        assert!(stats.bytes_reclaimed > 0);

        // Verify remaining vectors are intact after compaction
        assert_eq!(col.len(), 5);
        assert!(col.contains(1));
        assert!(col.contains(3));
        assert!(!col.contains(0));

        col.close().unwrap();
    }

    #[test]
    fn len_is_empty_contains() {
        let dir = TempDir::new().unwrap();
        let config =
            RvliteConfig::new(temp_path(&dir, "accessors.rvf"), 2).with_metric(RvliteMetric::L2);

        let mut col = RvliteCollection::create(config).unwrap();

        assert_eq!(col.len(), 0);
        assert!(col.is_empty());
        assert!(!col.contains(42));

        col.add(42, &[1.0, 2.0]).unwrap();

        assert_eq!(col.len(), 1);
        assert!(!col.is_empty());
        assert!(col.contains(42));
        assert!(!col.contains(99));

        col.close().unwrap();
    }

    #[test]
    fn cosine_metric() {
        let dir = TempDir::new().unwrap();
        let config =
            RvliteConfig::new(temp_path(&dir, "cosine.rvf"), 3).with_metric(RvliteMetric::Cosine);

        let mut col = RvliteCollection::create(config).unwrap();

        col.add(1, &[1.0, 0.0, 0.0]).unwrap();
        col.add(2, &[0.0, 1.0, 0.0]).unwrap();
        col.add(3, &[1.0, 1.0, 0.0]).unwrap();

        // Query for [1, 0, 0] -- id=1 should be closest (exact match)
        let results = col.search(&[1.0, 0.0, 0.0], 3);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].id, 1);
        assert!(results[0].distance < f32::EPSILON);

        col.close().unwrap();
    }

    #[test]
    fn dimension_accessor() {
        let dir = TempDir::new().unwrap();
        let config =
            RvliteConfig::new(temp_path(&dir, "dim_acc.rvf"), 256).with_metric(RvliteMetric::L2);

        let col = RvliteCollection::create(config).unwrap();
        assert_eq!(col.dimension(), 256);
        col.close().unwrap();
    }

    #[test]
    fn batch_length_mismatch() {
        let dir = TempDir::new().unwrap();
        let config =
            RvliteConfig::new(temp_path(&dir, "mismatch.rvf"), 2).with_metric(RvliteMetric::L2);

        let mut col = RvliteCollection::create(config).unwrap();

        let ids = vec![1, 2, 3];
        let v1 = [1.0, 0.0];
        let v2 = [0.0, 1.0];
        let vecs: Vec<&[f32]> = vec![&v1, &v2]; // 2 vectors but 3 ids

        let result = col.add_batch(&ids, &vecs);
        assert!(result.is_err());

        col.close().unwrap();
    }
}
