//! Storage layer with redb for metadata and memory-mapped vectors
//!
//! This module is only available when the "storage" feature is enabled.
//! For WASM builds, use the in-memory storage backend instead.

#[cfg(feature = "storage")]
use crate::error::{Result, RuvectorError};
#[cfg(feature = "storage")]
use crate::types::{DbOptions, VectorEntry, VectorId};
#[cfg(feature = "storage")]
use bincode::config;
#[cfg(feature = "storage")]
use once_cell::sync::Lazy;
#[cfg(feature = "storage")]
use parking_lot::Mutex;
#[cfg(feature = "storage")]
use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};
#[cfg(feature = "storage")]
use serde_json;
#[cfg(feature = "storage")]
use std::collections::HashMap;
#[cfg(feature = "storage")]
use std::path::{Path, PathBuf};
#[cfg(feature = "storage")]
use std::sync::Arc;

#[cfg(feature = "storage")]
const VECTORS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("vectors");
const METADATA_TABLE: TableDefinition<&str, &str> = TableDefinition::new("metadata");
const CONFIG_TABLE: TableDefinition<&str, &str> = TableDefinition::new("config");

/// Key used to store database configuration in CONFIG_TABLE
const DB_CONFIG_KEY: &str = "__ruvector_db_config__";

// Global database connection pool to allow multiple VectorDB instances
// to share the same underlying database file
static DB_POOL: Lazy<Mutex<HashMap<PathBuf, Arc<Database>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Storage backend for vector database
pub struct VectorStorage {
    db: Arc<Database>,
    dimensions: usize,
}

impl VectorStorage {
    /// Create or open a vector storage at the given path
    ///
    /// This method uses a global connection pool to allow multiple VectorDB
    /// instances to share the same underlying database file, fixing the
    /// "Database already open. Cannot acquire lock" error.
    pub fn new<P: AsRef<Path>>(path: P, dimensions: usize) -> Result<Self> {
        // SECURITY: Validate path to prevent directory traversal attacks
        let path_ref = path.as_ref();

        // Create parent directories if they don't exist (needed for canonicalize)
        if let Some(parent) = path_ref.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    RuvectorError::InvalidPath(format!("Failed to create directory: {}", e))
                })?;
            }
        }

        // Convert to absolute path first, then validate
        let path_buf = if path_ref.is_absolute() {
            path_ref.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| RuvectorError::InvalidPath(format!("Failed to get cwd: {}", e)))?
                .join(path_ref)
        };

        // SECURITY: Check for path traversal attempts (e.g., "../../../etc/passwd")
        // Only reject paths that contain ".." components trying to escape
        let path_str = path_ref.to_string_lossy();
        if path_str.contains("..") {
            // Verify the resolved path doesn't escape intended boundaries
            // For absolute paths, we allow them as-is (user explicitly specified)
            // For relative paths with "..", check they don't escape cwd
            if !path_ref.is_absolute() {
                if let Ok(cwd) = std::env::current_dir() {
                    // Normalize the path by resolving .. components
                    let mut normalized = cwd.clone();
                    for component in path_ref.components() {
                        match component {
                            std::path::Component::ParentDir => {
                                if !normalized.pop() || !normalized.starts_with(&cwd) {
                                    return Err(RuvectorError::InvalidPath(
                                        "Path traversal attempt detected".to_string(),
                                    ));
                                }
                            }
                            std::path::Component::Normal(c) => normalized.push(c),
                            _ => {}
                        }
                    }
                }
            }
        }

        // Check if we already have a Database instance for this path
        let db = {
            let mut pool = DB_POOL.lock();

            if let Some(existing_db) = pool.get(&path_buf) {
                // Reuse existing database connection
                Arc::clone(existing_db)
            } else {
                // Create new database and add to pool
                let new_db = Arc::new(Database::create(&path_buf)?);

                // Initialize tables
                let write_txn = new_db.begin_write()?;
                {
                    let _ = write_txn.open_table(VECTORS_TABLE)?;
                    let _ = write_txn.open_table(METADATA_TABLE)?;
                    let _ = write_txn.open_table(CONFIG_TABLE)?;
                }
                write_txn.commit()?;

                pool.insert(path_buf, Arc::clone(&new_db));
                new_db
            }
        };

        Ok(Self { db, dimensions })
    }

    /// Insert a vector entry
    pub fn insert(&self, entry: &VectorEntry) -> Result<VectorId> {
        if entry.vector.len() != self.dimensions {
            return Err(RuvectorError::DimensionMismatch {
                expected: self.dimensions,
                actual: entry.vector.len(),
            });
        }

        let id = entry
            .id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(VECTORS_TABLE)?;

            // Serialize vector data
            let vector_data = bincode::encode_to_vec(&entry.vector, config::standard())
                .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;

            table.insert(id.as_str(), vector_data.as_slice())?;

            // Store metadata if present
            if let Some(metadata) = &entry.metadata {
                let mut meta_table = write_txn.open_table(METADATA_TABLE)?;
                let metadata_json = serde_json::to_string(metadata)
                    .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;
                meta_table.insert(id.as_str(), metadata_json.as_str())?;
            }
        }
        write_txn.commit()?;

        Ok(id)
    }

    /// Insert multiple vectors in a batch
    pub fn insert_batch(&self, entries: &[VectorEntry]) -> Result<Vec<VectorId>> {
        let write_txn = self.db.begin_write()?;
        let mut ids = Vec::with_capacity(entries.len());

        {
            let mut table = write_txn.open_table(VECTORS_TABLE)?;
            let mut meta_table = write_txn.open_table(METADATA_TABLE)?;

            for entry in entries {
                if entry.vector.len() != self.dimensions {
                    return Err(RuvectorError::DimensionMismatch {
                        expected: self.dimensions,
                        actual: entry.vector.len(),
                    });
                }

                let id = entry
                    .id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                // Serialize and insert vector
                let vector_data = bincode::encode_to_vec(&entry.vector, config::standard())
                    .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;
                table.insert(id.as_str(), vector_data.as_slice())?;

                // Insert metadata if present
                if let Some(metadata) = &entry.metadata {
                    let metadata_json = serde_json::to_string(metadata)
                        .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;
                    meta_table.insert(id.as_str(), metadata_json.as_str())?;
                }

                ids.push(id);
            }
        }

        write_txn.commit()?;
        Ok(ids)
    }

    /// Get a vector by ID
    pub fn get(&self, id: &str) -> Result<Option<VectorEntry>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VECTORS_TABLE)?;

        let Some(vector_data) = table.get(id)? else {
            return Ok(None);
        };

        let (vector, _): (Vec<f32>, usize) =
            bincode::decode_from_slice(vector_data.value(), config::standard())
                .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;

        // Try to get metadata
        let meta_table = read_txn.open_table(METADATA_TABLE)?;
        let metadata = if let Some(meta_data) = meta_table.get(id)? {
            let meta_str = meta_data.value();
            Some(
                serde_json::from_str(meta_str)
                    .map_err(|e| RuvectorError::SerializationError(e.to_string()))?,
            )
        } else {
            None
        };

        Ok(Some(VectorEntry {
            id: Some(id.to_string()),
            vector,
            metadata,
        }))
    }

    /// Delete a vector by ID
    pub fn delete(&self, id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let deleted;

        {
            let mut table = write_txn.open_table(VECTORS_TABLE)?;
            deleted = table.remove(id)?.is_some();

            let mut meta_table = write_txn.open_table(METADATA_TABLE)?;
            let _ = meta_table.remove(id)?;
        }

        write_txn.commit()?;
        Ok(deleted)
    }

    /// Get the number of vectors stored
    pub fn len(&self) -> Result<usize> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VECTORS_TABLE)?;
        Ok(table.len()? as usize)
    }

    /// Check if storage is empty
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    /// Get all vector IDs
    pub fn all_ids(&self) -> Result<Vec<VectorId>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VECTORS_TABLE)?;

        let mut ids = Vec::new();
        let iter = table.iter()?;
        for item in iter {
            let (key, _) = item?;
            ids.push(key.value().to_string());
        }

        Ok(ids)
    }

    /// Save database configuration to persistent storage
    pub fn save_config(&self, options: &DbOptions) -> Result<()> {
        let config_json = serde_json::to_string(options)
            .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(CONFIG_TABLE)?;
            table.insert(DB_CONFIG_KEY, config_json.as_str())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    /// Load database configuration from persistent storage
    pub fn load_config(&self) -> Result<Option<DbOptions>> {
        let read_txn = self.db.begin_read()?;

        // Try to open config table - may not exist in older databases
        let table = match read_txn.open_table(CONFIG_TABLE) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };

        let Some(config_data) = table.get(DB_CONFIG_KEY)? else {
            return Ok(None);
        };

        let config: DbOptions = serde_json::from_str(config_data.value())
            .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;

        Ok(Some(config))
    }

    /// Get the stored dimensions
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}

// Add uuid dependency
use uuid;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_insert_and_get() -> Result<()> {
        let dir = tempdir().unwrap();
        let storage = VectorStorage::new(dir.path().join("test.db"), 3)?;

        let entry = VectorEntry {
            id: Some("test1".to_string()),
            vector: vec![1.0, 2.0, 3.0],
            metadata: None,
        };

        let id = storage.insert(&entry)?;
        assert_eq!(id, "test1");

        let retrieved = storage.get("test1")?;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.vector, vec![1.0, 2.0, 3.0]);

        Ok(())
    }

    #[test]
    fn test_batch_insert() -> Result<()> {
        let dir = tempdir().unwrap();
        let storage = VectorStorage::new(dir.path().join("test.db"), 3)?;

        let entries = vec![
            VectorEntry {
                id: None,
                vector: vec![1.0, 2.0, 3.0],
                metadata: None,
            },
            VectorEntry {
                id: None,
                vector: vec![4.0, 5.0, 6.0],
                metadata: None,
            },
        ];

        let ids = storage.insert_batch(&entries)?;
        assert_eq!(ids.len(), 2);
        assert_eq!(storage.len()?, 2);

        Ok(())
    }

    #[test]
    fn test_delete() -> Result<()> {
        let dir = tempdir().unwrap();
        let storage = VectorStorage::new(dir.path().join("test.db"), 3)?;

        let entry = VectorEntry {
            id: Some("test1".to_string()),
            vector: vec![1.0, 2.0, 3.0],
            metadata: None,
        };

        storage.insert(&entry)?;
        assert_eq!(storage.len()?, 1);

        let deleted = storage.delete("test1")?;
        assert!(deleted);
        assert_eq!(storage.len()?, 0);

        Ok(())
    }

    #[test]
    fn test_multiple_instances_same_path() -> Result<()> {
        // This test verifies the fix for the database locking bug
        // Multiple VectorStorage instances should be able to share the same database file
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("shared.db");

        // Create first instance
        let storage1 = VectorStorage::new(&db_path, 3)?;

        // Insert data with first instance
        storage1.insert(&VectorEntry {
            id: Some("test1".to_string()),
            vector: vec![1.0, 2.0, 3.0],
            metadata: None,
        })?;

        // Create second instance with SAME path - this should NOT fail
        let storage2 = VectorStorage::new(&db_path, 3)?;

        // Both instances should see the same data
        assert_eq!(storage1.len()?, 1);
        assert_eq!(storage2.len()?, 1);

        // Insert with second instance
        storage2.insert(&VectorEntry {
            id: Some("test2".to_string()),
            vector: vec![4.0, 5.0, 6.0],
            metadata: None,
        })?;

        // Both instances should see both records
        assert_eq!(storage1.len()?, 2);
        assert_eq!(storage2.len()?, 2);

        // Verify data integrity
        let retrieved1 = storage1.get("test1")?;
        assert!(retrieved1.is_some());

        let retrieved2 = storage2.get("test2")?;
        assert!(retrieved2.is_some());

        Ok(())
    }
}
