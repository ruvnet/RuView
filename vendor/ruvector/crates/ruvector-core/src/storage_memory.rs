//! In-memory storage backend for WASM and testing
//!
//! This storage implementation doesn't require file system access,
//! making it suitable for WebAssembly environments.

use crate::error::{Result, RuvectorError};
use crate::types::{VectorEntry, VectorId};
use dashmap::DashMap;
use serde_json::Value as JsonValue;
use std::sync::atomic::{AtomicU64, Ordering};

/// In-memory storage backend using DashMap for thread-safe concurrent access
pub struct MemoryStorage {
    vectors: DashMap<String, Vec<f32>>,
    metadata: DashMap<String, JsonValue>,
    dimensions: usize,
    counter: AtomicU64,
}

impl MemoryStorage {
    /// Create a new in-memory storage
    pub fn new(dimensions: usize) -> Result<Self> {
        Ok(Self {
            vectors: DashMap::new(),
            metadata: DashMap::new(),
            dimensions,
            counter: AtomicU64::new(0),
        })
    }

    /// Generate a new unique ID
    fn generate_id(&self) -> String {
        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        format!("vec_{}", id)
    }

    /// Insert a vector entry
    pub fn insert(&self, entry: &VectorEntry) -> Result<VectorId> {
        if entry.vector.len() != self.dimensions {
            return Err(RuvectorError::DimensionMismatch {
                expected: self.dimensions,
                actual: entry.vector.len(),
            });
        }

        let id = entry.id.clone().unwrap_or_else(|| self.generate_id());

        // Insert vector
        self.vectors.insert(id.clone(), entry.vector.clone());

        // Insert metadata if present
        if let Some(metadata) = &entry.metadata {
            self.metadata.insert(
                id.clone(),
                serde_json::Value::Object(
                    metadata
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                ),
            );
        }

        Ok(id)
    }

    /// Insert multiple vectors in a batch
    pub fn insert_batch(&self, entries: &[VectorEntry]) -> Result<Vec<VectorId>> {
        let mut ids = Vec::with_capacity(entries.len());

        for entry in entries {
            if entry.vector.len() != self.dimensions {
                return Err(RuvectorError::DimensionMismatch {
                    expected: self.dimensions,
                    actual: entry.vector.len(),
                });
            }

            let id = entry.id.clone().unwrap_or_else(|| self.generate_id());

            self.vectors.insert(id.clone(), entry.vector.clone());

            if let Some(metadata) = &entry.metadata {
                self.metadata.insert(
                    id.clone(),
                    serde_json::Value::Object(
                        metadata
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    ),
                );
            }

            ids.push(id);
        }

        Ok(ids)
    }

    /// Get a vector by ID
    pub fn get(&self, id: &str) -> Result<Option<VectorEntry>> {
        if let Some(vector_ref) = self.vectors.get(id) {
            let vector = vector_ref.clone();
            let metadata = self.metadata.get(id).and_then(|m| {
                if let serde_json::Value::Object(map) = m.value() {
                    Some(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                } else {
                    None
                }
            });

            Ok(Some(VectorEntry {
                id: Some(id.to_string()),
                vector,
                metadata,
            }))
        } else {
            Ok(None)
        }
    }

    /// Delete a vector by ID
    pub fn delete(&self, id: &str) -> Result<bool> {
        let vector_removed = self.vectors.remove(id).is_some();
        self.metadata.remove(id);
        Ok(vector_removed)
    }

    /// Get the number of vectors stored
    pub fn len(&self) -> Result<usize> {
        Ok(self.vectors.len())
    }

    /// Check if the storage is empty
    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.vectors.is_empty())
    }

    /// Get all vector IDs (for iteration)
    pub fn keys(&self) -> Vec<String> {
        self.vectors
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get all vector IDs (alias for keys, for API compatibility with VectorStorage)
    pub fn all_ids(&self) -> Result<Vec<String>> {
        Ok(self.keys())
    }

    /// Clear all data
    pub fn clear(&self) -> Result<()> {
        self.vectors.clear();
        self.metadata.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_insert_and_get() {
        let storage = MemoryStorage::new(128).unwrap();

        let entry = VectorEntry {
            id: Some("test_1".to_string()),
            vector: vec![0.1; 128],
            metadata: Some(json!({"key": "value"})),
        };

        let id = storage.insert(&entry).unwrap();
        assert_eq!(id, "test_1");

        let retrieved = storage.get("test_1").unwrap().unwrap();
        assert_eq!(retrieved.vector.len(), 128);
        assert!(retrieved.metadata.is_some());
    }

    #[test]
    fn test_batch_insert() {
        let storage = MemoryStorage::new(64).unwrap();

        let entries: Vec<_> = (0..10)
            .map(|i| VectorEntry {
                id: Some(format!("vec_{}", i)),
                vector: vec![i as f32; 64],
                metadata: None,
            })
            .collect();

        let ids = storage.insert_batch(&entries).unwrap();
        assert_eq!(ids.len(), 10);
        assert_eq!(storage.len().unwrap(), 10);
    }

    #[test]
    fn test_delete() {
        let storage = MemoryStorage::new(32).unwrap();

        let entry = VectorEntry {
            id: Some("delete_me".to_string()),
            vector: vec![1.0; 32],
            metadata: None,
        };

        storage.insert(&entry).unwrap();
        assert_eq!(storage.len().unwrap(), 1);

        let deleted = storage.delete("delete_me").unwrap();
        assert!(deleted);
        assert_eq!(storage.len().unwrap(), 0);
    }

    #[test]
    fn test_auto_id_generation() {
        let storage = MemoryStorage::new(16).unwrap();

        let entry = VectorEntry {
            id: None,
            vector: vec![0.5; 16],
            metadata: None,
        };

        let id1 = storage.insert(&entry).unwrap();
        let id2 = storage.insert(&entry).unwrap();

        assert_ne!(id1, id2);
        assert!(id1.starts_with("vec_"));
        assert!(id2.starts_with("vec_"));
    }

    #[test]
    fn test_dimension_mismatch() {
        let storage = MemoryStorage::new(128).unwrap();

        let entry = VectorEntry {
            id: Some("bad".to_string()),
            vector: vec![0.1; 64], // Wrong dimension
            metadata: None,
        };

        let result = storage.insert(&entry);
        assert!(result.is_err());

        if let Err(RuvectorError::DimensionMismatch { expected, actual }) = result {
            assert_eq!(expected, 128);
            assert_eq!(actual, 64);
        } else {
            panic!("Expected DimensionMismatch error");
        }
    }
}
