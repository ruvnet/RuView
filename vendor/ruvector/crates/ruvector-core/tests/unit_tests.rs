//! Unit tests with mocking using mockall (London School TDD)
//!
//! These tests use mocks to isolate components and test behavior in isolation.

use mockall::mock;
use mockall::predicate::*;
use ruvector_core::error::{Result, RuvectorError};
use ruvector_core::types::*;
use std::collections::HashMap;

// ============================================================================
// Mock Definitions
// ============================================================================

// Mock for storage operations
mock! {
    pub Storage {
        fn insert(&self, entry: &VectorEntry) -> Result<VectorId>;
        fn insert_batch(&self, entries: &[VectorEntry]) -> Result<Vec<VectorId>>;
        fn get(&self, id: &str) -> Result<Option<VectorEntry>>;
        fn delete(&self, id: &str) -> Result<bool>;
        fn len(&self) -> Result<usize>;
        fn is_empty(&self) -> Result<bool>;
        fn all_ids(&self) -> Result<Vec<VectorId>>;
    }
}

// Mock for index operations
mock! {
    pub Index {
        fn add(&mut self, id: VectorId, vector: Vec<f32>) -> Result<()>;
        fn add_batch(&mut self, entries: Vec<(VectorId, Vec<f32>)>) -> Result<()>;
        fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>>;
        fn remove(&mut self, id: &VectorId) -> Result<bool>;
        fn len(&self) -> usize;
        fn is_empty(&self) -> bool;
    }
}

// ============================================================================
// Distance Metric Tests
// ============================================================================

#[cfg(test)]
mod distance_tests {
    use super::*;
    use ruvector_core::distance::*;

    #[test]
    fn test_euclidean_same_vector() {
        let v = vec![1.0, 2.0, 3.0];
        let dist = euclidean_distance(&v, &v);
        assert!(dist < 0.001, "Distance to self should be ~0");
    }

    #[test]
    fn test_euclidean_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let dist = euclidean_distance(&a, &b);
        assert!(
            (dist - 1.414).abs() < 0.01,
            "Expected sqrt(2), got {}",
            dist
        );
    }

    #[test]
    fn test_cosine_parallel_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![2.0, 4.0, 6.0]; // Parallel to a
        let dist = cosine_distance(&a, &b);
        assert!(
            dist < 0.01,
            "Parallel vectors should have ~0 cosine distance, got {}",
            dist
        );
    }

    #[test]
    fn test_cosine_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let dist = cosine_distance(&a, &b);
        assert!(
            dist > 0.9 && dist < 1.1,
            "Orthogonal vectors should have distance ~1, got {}",
            dist
        );
    }

    #[test]
    fn test_dot_product_positive() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let dist = dot_product_distance(&a, &b);
        assert!(
            dist < 0.0,
            "Dot product distance should be negative for similar vectors"
        );
    }

    #[test]
    fn test_manhattan_distance() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 1.0, 1.0];
        let dist = manhattan_distance(&a, &b);
        assert!(
            (dist - 3.0).abs() < 0.001,
            "Manhattan distance should be 3.0, got {}",
            dist
        );
    }

    #[test]
    fn test_dimension_mismatch() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let result = distance(&a, &b, DistanceMetric::Euclidean);
        assert!(result.is_err(), "Should error on dimension mismatch");

        match result {
            Err(RuvectorError::DimensionMismatch { expected, actual }) => {
                assert_eq!(expected, 2);
                assert_eq!(actual, 3);
            }
            _ => panic!("Expected DimensionMismatch error"),
        }
    }
}

// ============================================================================
// Quantization Tests
// ============================================================================

#[cfg(test)]
mod quantization_tests {
    use super::*;
    use ruvector_core::quantization::*;

    #[test]
    fn test_scalar_quantization_reconstruction() {
        let original = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let quantized = ScalarQuantized::quantize(&original);
        let reconstructed = quantized.reconstruct();

        assert_eq!(original.len(), reconstructed.len());

        for (orig, recon) in original.iter().zip(reconstructed.iter()) {
            let error = (orig - recon).abs();
            assert!(error < 0.1, "Reconstruction error {} too large", error);
        }
    }

    #[test]
    fn test_scalar_quantization_distance() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.1, 2.1, 3.1];

        let qa = ScalarQuantized::quantize(&a);
        let qb = ScalarQuantized::quantize(&b);

        let quantized_dist = qa.distance(&qb);
        assert!(quantized_dist >= 0.0, "Distance should be non-negative");
    }

    #[test]
    fn test_binary_quantization_sign_preservation() {
        let vector = vec![1.0, -1.0, 2.0, -2.0, 0.5, -0.5];
        let quantized = BinaryQuantized::quantize(&vector);
        let reconstructed = quantized.reconstruct();

        for (orig, recon) in vector.iter().zip(reconstructed.iter()) {
            assert_eq!(orig.signum(), *recon, "Sign should be preserved");
        }
    }

    #[test]
    fn test_binary_quantization_hamming() {
        let a = vec![1.0, 1.0, 1.0, 1.0];
        let b = vec![1.0, 1.0, -1.0, -1.0];

        let qa = BinaryQuantized::quantize(&a);
        let qb = BinaryQuantized::quantize(&b);

        let dist = qa.distance(&qb);
        assert_eq!(dist, 2.0, "Hamming distance should be 2.0");
    }

    #[test]
    fn test_binary_quantization_zero_distance() {
        let vector = vec![1.0, 2.0, 3.0, 4.0];
        let quantized = BinaryQuantized::quantize(&vector);

        let dist = quantized.distance(&quantized);
        assert_eq!(dist, 0.0, "Distance to self should be 0");
    }
}

// ============================================================================
// Storage Layer Tests
// ============================================================================

#[cfg(test)]
mod storage_tests {
    use super::*;
    use ruvector_core::storage::VectorStorage;
    use tempfile::tempdir;

    #[test]
    fn test_insert_with_explicit_id() -> Result<()> {
        let dir = tempdir().unwrap();
        let storage = VectorStorage::new(dir.path().join("test.db"), 3)?;

        let entry = VectorEntry {
            id: Some("explicit_id".to_string()),
            vector: vec![1.0, 2.0, 3.0],
            metadata: None,
        };

        let id = storage.insert(&entry)?;
        assert_eq!(id, "explicit_id");

        Ok(())
    }

    #[test]
    fn test_insert_auto_generates_id() -> Result<()> {
        let dir = tempdir().unwrap();
        let storage = VectorStorage::new(dir.path().join("test.db"), 3)?;

        let entry = VectorEntry {
            id: None,
            vector: vec![1.0, 2.0, 3.0],
            metadata: None,
        };

        let id = storage.insert(&entry)?;
        assert!(!id.is_empty(), "Should generate a UUID");
        assert!(id.contains('-'), "Should be a valid UUID format");

        Ok(())
    }

    #[test]
    fn test_insert_with_metadata() -> Result<()> {
        let dir = tempdir().unwrap();
        let storage = VectorStorage::new(dir.path().join("test.db"), 3)?;

        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), serde_json::json!("value"));

        let entry = VectorEntry {
            id: Some("meta_test".to_string()),
            vector: vec![1.0, 2.0, 3.0],
            metadata: Some(metadata.clone()),
        };

        storage.insert(&entry)?;
        let retrieved = storage.get("meta_test")?.unwrap();

        assert_eq!(retrieved.metadata, Some(metadata));

        Ok(())
    }

    #[test]
    fn test_dimension_mismatch_error() {
        let dir = tempdir().unwrap();
        let storage = VectorStorage::new(dir.path().join("test.db"), 3).unwrap();

        let entry = VectorEntry {
            id: None,
            vector: vec![1.0, 2.0], // Wrong dimension
            metadata: None,
        };

        let result = storage.insert(&entry);
        assert!(result.is_err());

        match result {
            Err(RuvectorError::DimensionMismatch { expected, actual }) => {
                assert_eq!(expected, 3);
                assert_eq!(actual, 2);
            }
            _ => panic!("Expected DimensionMismatch error"),
        }
    }

    #[test]
    fn test_get_nonexistent() -> Result<()> {
        let dir = tempdir().unwrap();
        let storage = VectorStorage::new(dir.path().join("test.db"), 3)?;

        let result = storage.get("nonexistent")?;
        assert!(result.is_none());

        Ok(())
    }

    #[test]
    fn test_delete_nonexistent() -> Result<()> {
        let dir = tempdir().unwrap();
        let storage = VectorStorage::new(dir.path().join("test.db"), 3)?;

        let deleted = storage.delete("nonexistent")?;
        assert!(!deleted);

        Ok(())
    }

    #[test]
    fn test_batch_insert_empty() -> Result<()> {
        let dir = tempdir().unwrap();
        let storage = VectorStorage::new(dir.path().join("test.db"), 3)?;

        let ids = storage.insert_batch(&[])?;
        assert_eq!(ids.len(), 0);

        Ok(())
    }

    #[test]
    fn test_batch_insert_dimension_mismatch() {
        let dir = tempdir().unwrap();
        let storage = VectorStorage::new(dir.path().join("test.db"), 3).unwrap();

        let entries = vec![
            VectorEntry {
                id: None,
                vector: vec![1.0, 2.0, 3.0],
                metadata: None,
            },
            VectorEntry {
                id: None,
                vector: vec![1.0, 2.0], // Wrong dimension
                metadata: None,
            },
        ];

        let result = storage.insert_batch(&entries);
        assert!(
            result.is_err(),
            "Should error on dimension mismatch in batch"
        );
    }

    #[test]
    fn test_all_ids_empty() -> Result<()> {
        let dir = tempdir().unwrap();
        let storage = VectorStorage::new(dir.path().join("test.db"), 3)?;

        let ids = storage.all_ids()?;
        assert_eq!(ids.len(), 0);

        Ok(())
    }

    #[test]
    fn test_all_ids_after_insert() -> Result<()> {
        let dir = tempdir().unwrap();
        let storage = VectorStorage::new(dir.path().join("test.db"), 3)?;

        storage.insert(&VectorEntry {
            id: Some("id1".to_string()),
            vector: vec![1.0, 2.0, 3.0],
            metadata: None,
        })?;

        storage.insert(&VectorEntry {
            id: Some("id2".to_string()),
            vector: vec![4.0, 5.0, 6.0],
            metadata: None,
        })?;

        let ids = storage.all_ids()?;
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"id1".to_string()));
        assert!(ids.contains(&"id2".to_string()));

        Ok(())
    }
}

// ============================================================================
// VectorDB Tests (High-level API)
// ============================================================================

#[cfg(test)]
mod vector_db_tests {
    use super::*;
    use ruvector_core::VectorDB;
    use tempfile::tempdir;

    #[test]
    fn test_empty_database() -> Result<()> {
        let dir = tempdir().unwrap();
        let mut options = DbOptions::default();
        options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
        options.dimensions = 3;

        let db = VectorDB::new(options)?;
        assert!(db.is_empty()?);
        assert_eq!(db.len()?, 0);

        Ok(())
    }

    #[test]
    fn test_insert_updates_len() -> Result<()> {
        let dir = tempdir().unwrap();
        let mut options = DbOptions::default();
        options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
        options.dimensions = 3;

        let db = VectorDB::new(options)?;

        db.insert(VectorEntry {
            id: None,
            vector: vec![1.0, 2.0, 3.0],
            metadata: None,
        })?;

        assert_eq!(db.len()?, 1);
        assert!(!db.is_empty()?);

        Ok(())
    }

    #[test]
    fn test_delete_updates_len() -> Result<()> {
        let dir = tempdir().unwrap();
        let mut options = DbOptions::default();
        options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
        options.dimensions = 3;

        let db = VectorDB::new(options)?;

        let id = db.insert(VectorEntry {
            id: Some("test_id".to_string()),
            vector: vec![1.0, 2.0, 3.0],
            metadata: None,
        })?;

        assert_eq!(db.len()?, 1);

        let deleted = db.delete(&id)?;
        assert!(deleted);
        assert_eq!(db.len()?, 0);

        Ok(())
    }

    #[test]
    fn test_search_empty_database() -> Result<()> {
        let dir = tempdir().unwrap();
        let mut options = DbOptions::default();
        options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
        options.dimensions = 3;
        options.hnsw_config = None; // Use flat index

        let db = VectorDB::new(options)?;

        let results = db.search(SearchQuery {
            vector: vec![1.0, 2.0, 3.0],
            k: 10,
            filter: None,
            ef_search: None,
        })?;

        assert_eq!(results.len(), 0);

        Ok(())
    }

    #[test]
    fn test_search_with_filter() -> Result<()> {
        let dir = tempdir().unwrap();
        let mut options = DbOptions::default();
        options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
        options.dimensions = 3;
        options.hnsw_config = None;

        let db = VectorDB::new(options)?;

        // Insert vectors with metadata
        let mut meta1 = HashMap::new();
        meta1.insert("category".to_string(), serde_json::json!("A"));

        let mut meta2 = HashMap::new();
        meta2.insert("category".to_string(), serde_json::json!("B"));

        db.insert(VectorEntry {
            id: Some("v1".to_string()),
            vector: vec![1.0, 0.0, 0.0],
            metadata: Some(meta1),
        })?;

        db.insert(VectorEntry {
            id: Some("v2".to_string()),
            vector: vec![0.9, 0.1, 0.0],
            metadata: Some(meta2),
        })?;

        // Search with filter
        let mut filter = HashMap::new();
        filter.insert("category".to_string(), serde_json::json!("A"));

        let results = db.search(SearchQuery {
            vector: vec![1.0, 0.0, 0.0],
            k: 10,
            filter: Some(filter),
            ef_search: None,
        })?;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "v1");

        Ok(())
    }

    #[test]
    fn test_batch_insert() -> Result<()> {
        let dir = tempdir().unwrap();
        let mut options = DbOptions::default();
        options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
        options.dimensions = 3;

        let db = VectorDB::new(options)?;

        let entries = vec![
            VectorEntry {
                id: None,
                vector: vec![1.0, 0.0, 0.0],
                metadata: None,
            },
            VectorEntry {
                id: None,
                vector: vec![0.0, 1.0, 0.0],
                metadata: None,
            },
            VectorEntry {
                id: None,
                vector: vec![0.0, 0.0, 1.0],
                metadata: None,
            },
        ];

        let ids = db.insert_batch(entries)?;
        assert_eq!(ids.len(), 3);
        assert_eq!(db.len()?, 3);

        Ok(())
    }
}
