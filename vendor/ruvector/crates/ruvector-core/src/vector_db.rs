//! Main VectorDB interface

use crate::error::Result;
use crate::index::flat::FlatIndex;

#[cfg(feature = "hnsw")]
use crate::index::hnsw::HnswIndex;

use crate::index::VectorIndex;
use crate::types::*;
use parking_lot::RwLock;
use std::sync::Arc;

// Import appropriate storage backend based on features
#[cfg(feature = "storage")]
use crate::storage::VectorStorage;

#[cfg(not(feature = "storage"))]
use crate::storage_memory::MemoryStorage as VectorStorage;

/// Main vector database
pub struct VectorDB {
    storage: Arc<VectorStorage>,
    index: Arc<RwLock<Box<dyn VectorIndex>>>,
    options: DbOptions,
}

impl VectorDB {
    /// Create a new vector database with the given options
    ///
    /// If a storage path is provided and contains persisted vectors,
    /// the HNSW index will be automatically rebuilt from storage.
    /// If opening an existing database, the stored configuration (dimensions,
    /// distance metric, etc.) will be used instead of the provided options.
    #[allow(unused_mut)] // `options` is mutated only when feature = "storage"
    pub fn new(mut options: DbOptions) -> Result<Self> {
        #[cfg(feature = "storage")]
        let storage = {
            // First, try to load existing configuration from the database
            // We create a temporary storage to check for config
            let temp_storage = VectorStorage::new(&options.storage_path, options.dimensions)?;

            let stored_config = temp_storage.load_config()?;

            if let Some(config) = stored_config {
                // Existing database - use stored configuration
                tracing::info!(
                    "Loading existing database with {} dimensions",
                    config.dimensions
                );
                options = DbOptions {
                    // Keep the provided storage path (may have changed)
                    storage_path: options.storage_path.clone(),
                    // Use stored configuration for everything else
                    dimensions: config.dimensions,
                    distance_metric: config.distance_metric,
                    hnsw_config: config.hnsw_config,
                    quantization: config.quantization,
                };
                // Recreate storage with correct dimensions
                Arc::new(VectorStorage::new(
                    &options.storage_path,
                    options.dimensions,
                )?)
            } else {
                // New database - save the configuration
                tracing::info!(
                    "Creating new database with {} dimensions",
                    options.dimensions
                );
                temp_storage.save_config(&options)?;
                Arc::new(temp_storage)
            }
        };

        #[cfg(not(feature = "storage"))]
        let storage = Arc::new(VectorStorage::new(options.dimensions)?);

        // Choose index based on configuration and available features
        #[allow(unused_mut)] // `index` is mutated only when feature = "storage"
        let mut index: Box<dyn VectorIndex> = if let Some(hnsw_config) = &options.hnsw_config {
            #[cfg(feature = "hnsw")]
            {
                Box::new(HnswIndex::new(
                    options.dimensions,
                    options.distance_metric,
                    hnsw_config.clone(),
                )?)
            }
            #[cfg(not(feature = "hnsw"))]
            {
                // Fall back to flat index if HNSW is not available
                tracing::warn!("HNSW requested but not available (WASM build), using flat index");
                Box::new(FlatIndex::new(options.dimensions, options.distance_metric))
            }
        } else {
            Box::new(FlatIndex::new(options.dimensions, options.distance_metric))
        };

        // Rebuild index from persisted vectors if storage is not empty
        // This fixes the bug where search() returns empty results after restart
        #[cfg(feature = "storage")]
        {
            let stored_ids = storage.all_ids()?;
            if !stored_ids.is_empty() {
                tracing::info!(
                    "Rebuilding index from {} persisted vectors",
                    stored_ids.len()
                );

                // Batch load all vectors for efficient index rebuilding
                let mut entries = Vec::with_capacity(stored_ids.len());
                for id in stored_ids {
                    if let Some(entry) = storage.get(&id)? {
                        entries.push((id, entry.vector));
                    }
                }

                // Add all vectors to index in batch for better performance
                index.add_batch(entries)?;

                tracing::info!("Index rebuilt successfully");
            }
        }

        Ok(Self {
            storage,
            index: Arc::new(RwLock::new(index)),
            options,
        })
    }

    /// Create with default options
    pub fn with_dimensions(dimensions: usize) -> Result<Self> {
        let options = DbOptions {
            dimensions,
            ..DbOptions::default()
        };
        Self::new(options)
    }

    /// Insert a vector entry
    pub fn insert(&self, entry: VectorEntry) -> Result<VectorId> {
        let id = self.storage.insert(&entry)?;

        // Add to index
        let mut index = self.index.write();
        index.add(id.clone(), entry.vector)?;

        Ok(id)
    }

    /// Insert multiple vectors in a batch
    pub fn insert_batch(&self, entries: Vec<VectorEntry>) -> Result<Vec<VectorId>> {
        let ids = self.storage.insert_batch(&entries)?;

        // Add to index
        let mut index = self.index.write();
        let index_entries: Vec<_> = ids
            .iter()
            .zip(entries.iter())
            .map(|(id, entry)| (id.clone(), entry.vector.clone()))
            .collect();

        index.add_batch(index_entries)?;

        Ok(ids)
    }

    /// Search for similar vectors
    pub fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>> {
        let index = self.index.read();
        let mut results = index.search(&query.vector, query.k)?;

        // Enrich results with full data if needed
        for result in &mut results {
            if let Ok(Some(entry)) = self.storage.get(&result.id) {
                result.vector = Some(entry.vector);
                result.metadata = entry.metadata;
            }
        }

        // Apply metadata filters if specified
        if let Some(filter) = &query.filter {
            results.retain(|r| {
                if let Some(metadata) = &r.metadata {
                    filter
                        .iter()
                        .all(|(key, value)| metadata.get(key).is_some_and(|v| v == value))
                } else {
                    false
                }
            });
        }

        Ok(results)
    }

    /// Delete a vector by ID
    pub fn delete(&self, id: &str) -> Result<bool> {
        let deleted_storage = self.storage.delete(id)?;

        if deleted_storage {
            let mut index = self.index.write();
            let _ = index.remove(&id.to_string())?;
        }

        Ok(deleted_storage)
    }

    /// Get a vector by ID
    pub fn get(&self, id: &str) -> Result<Option<VectorEntry>> {
        self.storage.get(id)
    }

    /// Get the number of vectors
    pub fn len(&self) -> Result<usize> {
        self.storage.len()
    }

    /// Check if database is empty
    pub fn is_empty(&self) -> Result<bool> {
        self.storage.is_empty()
    }

    /// Get database options
    pub fn options(&self) -> &DbOptions {
        &self.options
    }

    /// Get all vector IDs (for iteration/serialization)
    pub fn keys(&self) -> Result<Vec<String>> {
        self.storage.all_ids()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn test_vector_db_creation() -> Result<()> {
        let dir = tempdir().unwrap();
        let mut options = DbOptions::default();
        options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
        options.dimensions = 3;

        let db = VectorDB::new(options)?;
        assert!(db.is_empty()?);

        Ok(())
    }

    #[test]
    fn test_insert_and_search() -> Result<()> {
        let dir = tempdir().unwrap();
        let mut options = DbOptions::default();
        options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
        options.dimensions = 3;
        options.distance_metric = DistanceMetric::Euclidean; // Use Euclidean for clearer test
        options.hnsw_config = None; // Use flat index for testing

        let db = VectorDB::new(options)?;

        // Insert vectors
        db.insert(VectorEntry {
            id: Some("v1".to_string()),
            vector: vec![1.0, 0.0, 0.0],
            metadata: None,
        })?;

        db.insert(VectorEntry {
            id: Some("v2".to_string()),
            vector: vec![0.0, 1.0, 0.0],
            metadata: None,
        })?;

        db.insert(VectorEntry {
            id: Some("v3".to_string()),
            vector: vec![0.0, 0.0, 1.0],
            metadata: None,
        })?;

        // Search for exact match
        let results = db.search(SearchQuery {
            vector: vec![1.0, 0.0, 0.0],
            k: 2,
            filter: None,
            ef_search: None,
        })?;

        assert!(results.len() >= 1);
        assert_eq!(results[0].id, "v1", "First result should be exact match");
        assert!(
            results[0].score < 0.01,
            "Exact match should have ~0 distance"
        );

        Ok(())
    }

    /// Test that search works after simulated restart (new VectorDB instance)
    /// This verifies the fix for issue #30: HNSW index not rebuilt from storage
    #[test]
    #[cfg(feature = "storage")]
    fn test_search_after_restart() -> Result<()> {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("persist.db").to_string_lossy().to_string();

        // Phase 1: Create database and insert vectors
        {
            let mut options = DbOptions::default();
            options.storage_path = db_path.clone();
            options.dimensions = 3;
            options.distance_metric = DistanceMetric::Euclidean;
            options.hnsw_config = None;

            let db = VectorDB::new(options)?;

            db.insert(VectorEntry {
                id: Some("v1".to_string()),
                vector: vec![1.0, 0.0, 0.0],
                metadata: None,
            })?;

            db.insert(VectorEntry {
                id: Some("v2".to_string()),
                vector: vec![0.0, 1.0, 0.0],
                metadata: None,
            })?;

            db.insert(VectorEntry {
                id: Some("v3".to_string()),
                vector: vec![0.7, 0.7, 0.0],
                metadata: None,
            })?;

            // Verify search works before "restart"
            let results = db.search(SearchQuery {
                vector: vec![0.8, 0.6, 0.0],
                k: 3,
                filter: None,
                ef_search: None,
            })?;
            assert_eq!(results.len(), 3, "Should find all 3 vectors before restart");
        }
        // db is dropped here, simulating application shutdown

        // Phase 2: Create new database instance (simulates restart)
        {
            let mut options = DbOptions::default();
            options.storage_path = db_path.clone();
            options.dimensions = 3;
            options.distance_metric = DistanceMetric::Euclidean;
            options.hnsw_config = None;

            let db = VectorDB::new(options)?;

            // Verify vectors are still accessible
            assert_eq!(db.len()?, 3, "Should have 3 vectors after restart");

            // Verify get() works
            let v1 = db.get("v1")?;
            assert!(v1.is_some(), "get() should work after restart");

            // Verify search() works - THIS WAS THE BUG
            let results = db.search(SearchQuery {
                vector: vec![0.8, 0.6, 0.0],
                k: 3,
                filter: None,
                ef_search: None,
            })?;

            assert_eq!(
                results.len(),
                3,
                "search() should return results after restart (was returning 0 before fix)"
            );

            // v3 should be closest to query [0.8, 0.6, 0.0]
            assert_eq!(
                results[0].id, "v3",
                "v3 [0.7, 0.7, 0.0] should be closest to query [0.8, 0.6, 0.0]"
            );
        }

        Ok(())
    }
}
