//! Storage layer with redb and memory-mapped files

use crate::error::{Result, VectorDbError};
use crate::types::VectorEntry;
use parking_lot::RwLock;
use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

// Table definitions
const VECTORS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("vectors");
const METADATA_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("metadata");
const INDEX_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("index");

/// Storage backend for vector database
pub struct Storage {
    db: Arc<Database>,
    vector_cache: Arc<RwLock<HashMap<String, Vec<f32>>>>,
}

impl Storage {
    /// Create a new storage instance
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        // SECURITY: Validate path to prevent directory traversal attacks
        let path_ref = path.as_ref();

        // Create parent directories if they don't exist
        if let Some(parent) = path_ref.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    VectorDbError::InvalidPath(format!("Failed to create directory: {}", e))
                })?;
            }
        }

        // Convert to absolute path
        let canonical_path = if path_ref.is_absolute() {
            path_ref.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| VectorDbError::InvalidPath(format!("Failed to get cwd: {}", e)))?
                .join(path_ref)
        };

        // SECURITY: Check for path traversal attempts
        let path_str = path_ref.to_string_lossy();
        if path_str.contains("..") && !path_ref.is_absolute() {
            if let Ok(cwd) = std::env::current_dir() {
                let mut normalized = cwd.clone();
                for component in path_ref.components() {
                    match component {
                        std::path::Component::ParentDir => {
                            if !normalized.pop() || !normalized.starts_with(&cwd) {
                                return Err(VectorDbError::InvalidPath(
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

        let db = Database::create(canonical_path)?;

        Ok(Self {
            db: Arc::new(db),
            vector_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Open an existing storage instance
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        // SECURITY: Validate path to prevent directory traversal attacks
        let path_ref = path.as_ref();

        // Convert to absolute path - file must exist for open
        let canonical_path = path_ref.canonicalize().map_err(|e| {
            VectorDbError::InvalidPath(format!("Path does not exist or cannot be resolved: {}", e))
        })?;

        // SECURITY: Check for path traversal attempts
        let path_str = path_ref.to_string_lossy();
        if path_str.contains("..") && !path_ref.is_absolute() {
            if let Ok(cwd) = std::env::current_dir() {
                if !canonical_path.starts_with(&cwd) {
                    return Err(VectorDbError::InvalidPath(
                        "Path traversal attempt detected".to_string(),
                    ));
                }
            }
        }

        let db = Database::open(canonical_path)?;

        Ok(Self {
            db: Arc::new(db),
            vector_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Insert a vector entry
    pub fn insert(&self, entry: &VectorEntry) -> Result<()> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table = write_txn.open_table(VECTORS_TABLE)?;

            // Serialize vector as bytes
            let vector_bytes = bincode::encode_to_vec(&entry.vector, bincode::config::standard())
                .map_err(|e| VectorDbError::Serialization(e.to_string()))?;

            table.insert(entry.id.as_str(), vector_bytes.as_slice())?;
        }

        {
            let mut table = write_txn.open_table(METADATA_TABLE)?;

            // Serialize metadata (use JSON for serde_json::Value compatibility)
            let metadata_bytes = serde_json::to_vec(&entry.metadata)
                .map_err(|e| VectorDbError::Serialization(e.to_string()))?;

            table.insert(entry.id.as_str(), metadata_bytes.as_slice())?;
        }

        write_txn.commit()?;

        // Update cache
        self.vector_cache
            .write()
            .insert(entry.id.clone(), entry.vector.clone());

        Ok(())
    }

    /// Insert multiple vector entries in a batch
    pub fn insert_batch(&self, entries: &[VectorEntry]) -> Result<()> {
        let write_txn = self.db.begin_write()?;

        {
            let mut vectors_table = write_txn.open_table(VECTORS_TABLE)?;
            let mut metadata_table = write_txn.open_table(METADATA_TABLE)?;

            for entry in entries {
                // Serialize vector
                let vector_bytes =
                    bincode::encode_to_vec(&entry.vector, bincode::config::standard())
                        .map_err(|e| VectorDbError::Serialization(e.to_string()))?;

                vectors_table.insert(entry.id.as_str(), vector_bytes.as_slice())?;

                // Serialize metadata (use JSON for serde_json::Value compatibility)
                let metadata_bytes = serde_json::to_vec(&entry.metadata)
                    .map_err(|e| VectorDbError::Serialization(e.to_string()))?;

                metadata_table.insert(entry.id.as_str(), metadata_bytes.as_slice())?;
            }
        }

        write_txn.commit()?;

        // Update cache
        let mut cache = self.vector_cache.write();
        for entry in entries {
            cache.insert(entry.id.clone(), entry.vector.clone());
        }

        Ok(())
    }

    /// Get a vector by ID
    pub fn get(&self, id: &str) -> Result<Option<Vec<f32>>> {
        // Check cache first
        if let Some(vector) = self.vector_cache.read().get(id) {
            return Ok(Some(vector.clone()));
        }

        // Read from database
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VECTORS_TABLE)?;

        if let Some(bytes) = table.get(id)? {
            let (vector, _): (Vec<f32>, usize) =
                bincode::decode_from_slice(bytes.value(), bincode::config::standard())
                    .map_err(|e| VectorDbError::Serialization(e.to_string()))?;

            // Update cache
            self.vector_cache
                .write()
                .insert(id.to_string(), vector.clone());

            Ok(Some(vector))
        } else {
            Ok(None)
        }
    }

    /// Get metadata for a vector
    pub fn get_metadata(&self, id: &str) -> Result<Option<HashMap<String, serde_json::Value>>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(METADATA_TABLE)?;

        if let Some(bytes) = table.get(id)? {
            let metadata: HashMap<String, serde_json::Value> =
                serde_json::from_slice(bytes.value())
                    .map_err(|e| VectorDbError::Serialization(e.to_string()))?;

            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }

    /// Delete a vector by ID
    pub fn delete(&self, id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;

        let deleted;

        {
            let mut table = write_txn.open_table(VECTORS_TABLE)?;
            deleted = table.remove(id)?.is_some();
        }

        {
            let mut table = write_txn.open_table(METADATA_TABLE)?;
            table.remove(id)?;
        }

        write_txn.commit()?;

        // Remove from cache
        self.vector_cache.write().remove(id);

        Ok(deleted)
    }

    /// Get all vector IDs
    pub fn get_all_ids(&self) -> Result<Vec<String>> {
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

    /// Count total vectors
    pub fn count(&self) -> Result<usize> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(VECTORS_TABLE)?;
        Ok(table.len()? as usize)
    }

    /// Store index data
    pub fn store_index(&self, key: &str, data: &[u8]) -> Result<()> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table = write_txn.open_table(INDEX_TABLE)?;
            table.insert(key, data)?;
        }

        write_txn.commit()?;
        Ok(())
    }

    /// Load index data
    pub fn load_index(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(INDEX_TABLE)?;

        if let Some(bytes) = table.get(key)? {
            Ok(Some(bytes.value().to_vec()))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_storage_insert_and_get() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let storage = Storage::new(&path).unwrap();

        let entry = VectorEntry {
            id: "test1".to_string(),
            vector: vec![1.0, 2.0, 3.0],
            metadata: HashMap::new(),
            timestamp: 0,
        };

        storage.insert(&entry).unwrap();

        let retrieved = storage.get("test1").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_storage_delete() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let storage = Storage::new(&path).unwrap();

        let entry = VectorEntry {
            id: "test1".to_string(),
            vector: vec![1.0, 2.0, 3.0],
            metadata: HashMap::new(),
            timestamp: 0,
        };

        storage.insert(&entry).unwrap();
        assert!(storage.delete("test1").unwrap());
        assert!(storage.get("test1").unwrap().is_none());
    }
}
