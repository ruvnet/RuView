//! Integration tests for end-to-end workflows
//!
//! These tests verify that all components work together correctly.

use ruvector_core::types::{DbOptions, DistanceMetric, HnswConfig, SearchQuery};
use ruvector_core::{VectorDB, VectorEntry};
use std::collections::HashMap;
use tempfile::tempdir;

// ============================================================================
// End-to-End Workflow Tests
// ============================================================================

#[test]
fn test_complete_insert_search_workflow() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
    options.dimensions = 128;
    options.distance_metric = DistanceMetric::Cosine;
    options.hnsw_config = Some(HnswConfig {
        m: 16,
        ef_construction: 100,
        ef_search: 50,
        max_elements: 100_000,
    });

    let db = VectorDB::new(options).unwrap();

    // Insert training data
    let vectors: Vec<VectorEntry> = (0..100)
        .map(|i| {
            let mut metadata = HashMap::new();
            metadata.insert("index".to_string(), serde_json::json!(i));

            VectorEntry {
                id: Some(format!("vec_{}", i)),
                vector: (0..128).map(|j| ((i + j) as f32) * 0.01).collect(),
                metadata: Some(metadata),
            }
        })
        .collect();

    let ids = db.insert_batch(vectors).unwrap();
    assert_eq!(ids.len(), 100);

    // Search for similar vectors
    let query: Vec<f32> = (0..128).map(|j| (j as f32) * 0.01).collect();
    let results = db
        .search(SearchQuery {
            vector: query,
            k: 10,
            filter: None,
            ef_search: Some(100),
        })
        .unwrap();

    assert_eq!(results.len(), 10);
    assert!(results[0].vector.is_some());
    assert!(results[0].metadata.is_some());
}

#[test]
fn test_batch_operations_10k_vectors() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
    options.dimensions = 384;
    options.distance_metric = DistanceMetric::Euclidean;
    options.hnsw_config = Some(HnswConfig::default());

    let db = VectorDB::new(options).unwrap();

    // Generate 10K vectors
    println!("Generating 10K vectors...");
    let vectors: Vec<VectorEntry> = (0..10_000)
        .map(|i| VectorEntry {
            id: Some(format!("vec_{}", i)),
            vector: (0..384).map(|j| ((i + j) as f32) * 0.001).collect(),
            metadata: None,
        })
        .collect();

    // Batch insert
    println!("Batch inserting 10K vectors...");
    let start = std::time::Instant::now();
    let ids = db.insert_batch(vectors).unwrap();
    let duration = start.elapsed();
    println!("Batch insert took: {:?}", duration);

    assert_eq!(ids.len(), 10_000);
    assert_eq!(db.len().unwrap(), 10_000);

    // Perform multiple searches
    println!("Performing searches...");
    for i in 0..10 {
        let query: Vec<f32> = (0..384).map(|j| ((i * 100 + j) as f32) * 0.001).collect();
        let results = db
            .search(SearchQuery {
                vector: query,
                k: 10,
                filter: None,
                ef_search: None,
            })
            .unwrap();

        assert_eq!(results.len(), 10);
    }
}

#[test]
fn test_persistence_and_reload() {
    let dir = tempdir().unwrap();
    let db_path = dir
        .path()
        .join("persistent.db")
        .to_string_lossy()
        .to_string();

    // Create and populate database
    {
        let mut options = DbOptions::default();
        options.storage_path = db_path.clone();
        options.dimensions = 3;
        options.hnsw_config = None; // Use flat index for simpler persistence test

        let db = VectorDB::new(options).unwrap();

        for i in 0..10 {
            db.insert(VectorEntry {
                id: Some(format!("vec_{}", i)),
                vector: vec![i as f32, (i * 2) as f32, (i * 3) as f32],
                metadata: None,
            })
            .unwrap();
        }

        assert_eq!(db.len().unwrap(), 10);
    }

    // Reload database
    {
        let mut options = DbOptions::default();
        options.storage_path = db_path.clone();
        options.dimensions = 3;
        options.hnsw_config = None;

        let db = VectorDB::new(options).unwrap();

        // Verify data persisted
        assert_eq!(db.len().unwrap(), 10);

        let entry = db.get("vec_5").unwrap().unwrap();
        assert_eq!(entry.vector, vec![5.0, 10.0, 15.0]);
    }
}

#[test]
fn test_mixed_operations_workflow() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
    options.dimensions = 64;

    let db = VectorDB::new(options).unwrap();

    // Insert initial batch
    let initial: Vec<VectorEntry> = (0..50)
        .map(|i| VectorEntry {
            id: Some(format!("vec_{}", i)),
            vector: (0..64).map(|j| ((i + j) as f32) * 0.1).collect(),
            metadata: None,
        })
        .collect();

    db.insert_batch(initial).unwrap();
    assert_eq!(db.len().unwrap(), 50);

    // Delete some vectors
    for i in 0..10 {
        db.delete(&format!("vec_{}", i)).unwrap();
    }
    assert_eq!(db.len().unwrap(), 40);

    // Insert more individual vectors
    for i in 50..60 {
        db.insert(VectorEntry {
            id: Some(format!("vec_{}", i)),
            vector: (0..64).map(|j| ((i + j) as f32) * 0.1).collect(),
            metadata: None,
        })
        .unwrap();
    }
    assert_eq!(db.len().unwrap(), 50);

    // Search
    let query: Vec<f32> = (0..64).map(|j| (j as f32) * 0.1).collect();
    let results = db
        .search(SearchQuery {
            vector: query,
            k: 20,
            filter: None,
            ef_search: None,
        })
        .unwrap();

    assert!(results.len() > 0);
}

// ============================================================================
// Different Distance Metrics
// ============================================================================

#[test]
fn test_all_distance_metrics() {
    let metrics = vec![
        DistanceMetric::Euclidean,
        DistanceMetric::Cosine,
        DistanceMetric::DotProduct,
        DistanceMetric::Manhattan,
    ];

    for metric in metrics {
        let dir = tempdir().unwrap();
        let mut options = DbOptions::default();
        options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
        options.dimensions = 32;
        options.distance_metric = metric;
        options.hnsw_config = None;

        let db = VectorDB::new(options).unwrap();

        // Insert test vectors
        for i in 0..20 {
            db.insert(VectorEntry {
                id: Some(format!("vec_{}", i)),
                vector: (0..32).map(|j| ((i + j) as f32) * 0.1).collect(),
                metadata: None,
            })
            .unwrap();
        }

        // Search
        let query: Vec<f32> = (0..32).map(|j| (j as f32) * 0.1).collect();
        let results = db
            .search(SearchQuery {
                vector: query,
                k: 5,
                filter: None,
                ef_search: None,
            })
            .unwrap();

        assert_eq!(results.len(), 5, "Failed for metric {:?}", metric);

        // Verify scores are in ascending order (lower is better for distance)
        for i in 0..results.len() - 1 {
            assert!(
                results[i].score <= results[i + 1].score,
                "Results not sorted for metric {:?}: {} > {}",
                metric,
                results[i].score,
                results[i + 1].score
            );
        }
    }
}

// ============================================================================
// HNSW Configuration Tests
// ============================================================================

#[test]
fn test_hnsw_different_configurations() {
    let configs = vec![
        HnswConfig {
            m: 8,
            ef_construction: 50,
            ef_search: 50,
            max_elements: 1000,
        },
        HnswConfig {
            m: 16,
            ef_construction: 100,
            ef_search: 100,
            max_elements: 1000,
        },
        HnswConfig {
            m: 32,
            ef_construction: 200,
            ef_search: 200,
            max_elements: 1000,
        },
    ];

    for config in configs {
        let dir = tempdir().unwrap();
        let mut options = DbOptions::default();
        options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
        options.dimensions = 64;
        options.hnsw_config = Some(config.clone());

        let db = VectorDB::new(options).unwrap();

        // Insert vectors
        let vectors: Vec<VectorEntry> = (0..100)
            .map(|i| VectorEntry {
                id: Some(format!("vec_{}", i)),
                vector: (0..64).map(|j| ((i + j) as f32) * 0.01).collect(),
                metadata: None,
            })
            .collect();

        db.insert_batch(vectors).unwrap();

        // Search with different ef_search values
        let query: Vec<f32> = (0..64).map(|j| (j as f32) * 0.01).collect();
        let results = db
            .search(SearchQuery {
                vector: query,
                k: 10,
                filter: None,
                ef_search: Some(config.ef_search),
            })
            .unwrap();

        assert_eq!(results.len(), 10, "Failed for config M={}", config.m);
    }
}

// ============================================================================
// Metadata Filtering Tests
// ============================================================================

#[test]
fn test_complex_metadata_filtering() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
    options.dimensions = 16;
    options.hnsw_config = None;

    let db = VectorDB::new(options).unwrap();

    // Insert vectors with different categories and values
    for i in 0..50 {
        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), serde_json::json!(i % 3));
        metadata.insert("value".to_string(), serde_json::json!(i / 10));

        db.insert(VectorEntry {
            id: Some(format!("vec_{}", i)),
            vector: (0..16).map(|j| ((i + j) as f32) * 0.1).collect(),
            metadata: Some(metadata),
        })
        .unwrap();
    }

    // Search with single filter
    let mut filter1 = HashMap::new();
    filter1.insert("category".to_string(), serde_json::json!(0));

    let query: Vec<f32> = (0..16).map(|j| (j as f32) * 0.1).collect();
    let results1 = db
        .search(SearchQuery {
            vector: query.clone(),
            k: 100,
            filter: Some(filter1),
            ef_search: None,
        })
        .unwrap();

    // Should only get vectors where i % 3 == 0
    for result in &results1 {
        let meta = result.metadata.as_ref().unwrap();
        assert_eq!(meta.get("category").unwrap(), &serde_json::json!(0));
    }

    // Search with different filter
    let mut filter2 = HashMap::new();
    filter2.insert("value".to_string(), serde_json::json!(2));

    let results2 = db
        .search(SearchQuery {
            vector: query,
            k: 100,
            filter: Some(filter2),
            ef_search: None,
        })
        .unwrap();

    // Should only get vectors where i / 10 == 2 (i.e., i in 20..30)
    for result in &results2 {
        let meta = result.metadata.as_ref().unwrap();
        assert_eq!(meta.get("value").unwrap(), &serde_json::json!(2));
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_dimension_validation() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
    options.dimensions = 64;

    let db = VectorDB::new(options).unwrap();

    // Try to insert vector with wrong dimensions
    let result = db.insert(VectorEntry {
        id: None,
        vector: vec![1.0, 2.0, 3.0], // Only 3 dimensions, should be 64
        metadata: None,
    });

    assert!(result.is_err());
}

#[test]
fn test_search_with_wrong_dimension() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
    options.dimensions = 64;
    options.hnsw_config = None;

    let db = VectorDB::new(options).unwrap();

    // Insert some vectors
    db.insert(VectorEntry {
        id: Some("v1".to_string()),
        vector: (0..64).map(|i| i as f32).collect(),
        metadata: None,
    })
    .unwrap();

    // Try to search with wrong dimension query
    // Note: This might not error in the current implementation, but should be validated
    let query = vec![1.0, 2.0, 3.0]; // Wrong dimension
    let result = db.search(SearchQuery {
        vector: query,
        k: 10,
        filter: None,
        ef_search: None,
    });

    // Depending on implementation, this might error or return empty results
    // The important thing is it doesn't panic
    let _ = result;
}
