//! Storage compatibility layer
//!
//! This module provides a unified interface that works with both
//! file-based (redb) and in-memory storage backends.

use crate::error::Result;
use crate::types::{VectorEntry, VectorId};

#[cfg(feature = "storage")]
pub use crate::storage::VectorStorage;

#[cfg(not(feature = "storage"))]
pub use crate::storage_memory::MemoryStorage as VectorStorage;

/// Unified storage trait
pub trait StorageBackend {
    fn insert(&self, entry: &VectorEntry) -> Result<VectorId>;
    fn insert_batch(&self, entries: &[VectorEntry]) -> Result<Vec<VectorId>>;
    fn get(&self, id: &str) -> Result<Option<VectorEntry>>;
    fn delete(&self, id: &str) -> Result<bool>;
    fn len(&self) -> Result<usize>;
    fn is_empty(&self) -> Result<bool>;
}

// Implement trait for redb-based storage
#[cfg(feature = "storage")]
impl StorageBackend for crate::storage::VectorStorage {
    fn insert(&self, entry: &VectorEntry) -> Result<VectorId> {
        self.insert(entry)
    }

    fn insert_batch(&self, entries: &[VectorEntry]) -> Result<Vec<VectorId>> {
        self.insert_batch(entries)
    }

    fn get(&self, id: &str) -> Result<Option<VectorEntry>> {
        self.get(id)
    }

    fn delete(&self, id: &str) -> Result<bool> {
        self.delete(id)
    }

    fn len(&self) -> Result<usize> {
        self.len()
    }

    fn is_empty(&self) -> Result<bool> {
        self.is_empty()
    }
}

// Implement trait for memory storage
#[cfg(not(feature = "storage"))]
impl StorageBackend for crate::storage_memory::MemoryStorage {
    fn insert(&self, entry: &VectorEntry) -> Result<VectorId> {
        self.insert(entry)
    }

    fn insert_batch(&self, entries: &[VectorEntry]) -> Result<Vec<VectorId>> {
        self.insert_batch(entries)
    }

    fn get(&self, id: &str) -> Result<Option<VectorEntry>> {
        self.get(id)
    }

    fn delete(&self, id: &str) -> Result<bool> {
        self.delete(id)
    }

    fn len(&self) -> Result<usize> {
        self.len()
    }

    fn is_empty(&self) -> Result<bool> {
        self.is_empty()
    }
}
