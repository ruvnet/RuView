//! Stress tests for scalability, concurrency, and resilience
//!
//! These tests push the system to its limits to verify robustness.

use ruvector_core::types::{DbOptions, HnswConfig, SearchQuery};
use ruvector_core::{VectorDB, VectorEntry};
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::tempdir;

// ============================================================================
// Large-Scale Insertion Tests
// ============================================================================

#[test]
#[ignore] // Run with: cargo test --test stress_tests -- --ignored --test-threads=1
fn test_million_vector_insertion() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir.path().join("million.db").to_string_lossy().to_string();
    options.dimensions = 128;
    options.hnsw_config = Some(HnswConfig {
        m: 16,
        ef_construction: 100,
        ef_search: 50,
        max_elements: 2_000_000,
    });

    let db = VectorDB::new(options).unwrap();

    println!("Starting million-vector insertion test...");
    let batch_size = 10_000;
    let num_batches = 100; // Total: 1M vectors

    for batch_idx in 0..num_batches {
        println!("Inserting batch {}/{}...", batch_idx + 1, num_batches);

        let vectors: Vec<VectorEntry> = (0..batch_size)
            .map(|i| {
                let global_idx = batch_idx * batch_size + i;
                VectorEntry {
                    id: Some(format!("vec_{}", global_idx)),
                    vector: (0..128)
                        .map(|j| ((global_idx + j) as f32) * 0.0001)
                        .collect(),
                    metadata: None,
                }
            })
            .collect();

        let start = std::time::Instant::now();
        db.insert_batch(vectors).unwrap();
        let duration = start.elapsed();
        println!("Batch {} took: {:?}", batch_idx + 1, duration);
    }

    println!("Final database size: {}", db.len().unwrap());
    assert_eq!(db.len().unwrap(), 1_000_000);

    // Perform some searches to verify functionality
    println!("Testing search on 1M vectors...");
    for i in 0..10 {
        let query: Vec<f32> = (0..128)
            .map(|j| ((i * 10000 + j) as f32) * 0.0001)
            .collect();
        let start = std::time::Instant::now();
        let results = db
            .search(SearchQuery {
                vector: query,
                k: 10,
                filter: None,
                ef_search: Some(50),
            })
            .unwrap();
        let duration = start.elapsed();

        println!(
            "Search {} took: {:?}, found {} results",
            i + 1,
            duration,
            results.len()
        );
        assert_eq!(results.len(), 10);
    }
}

// ============================================================================
// Concurrent Query Tests
// ============================================================================

#[test]
fn test_concurrent_queries() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir
        .path()
        .join("concurrent.db")
        .to_string_lossy()
        .to_string();
    options.dimensions = 64;
    options.hnsw_config = Some(HnswConfig::default());

    let db = Arc::new(VectorDB::new(options).unwrap());

    // Insert test data
    println!("Inserting test data...");
    let vectors: Vec<VectorEntry> = (0..1000)
        .map(|i| VectorEntry {
            id: Some(format!("vec_{}", i)),
            vector: (0..64).map(|j| ((i + j) as f32) * 0.01).collect(),
            metadata: None,
        })
        .collect();

    db.insert_batch(vectors).unwrap();

    // Spawn multiple threads doing concurrent searches
    println!("Starting 10 concurrent query threads...");
    let num_threads = 10;
    let queries_per_thread = 100;

    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let db_clone = Arc::clone(&db);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            // Wait for all threads to be ready
            barrier_clone.wait();

            let start = std::time::Instant::now();

            for i in 0..queries_per_thread {
                let query: Vec<f32> = (0..64)
                    .map(|j| ((thread_id * 1000 + i + j) as f32) * 0.01)
                    .collect();

                let results = db_clone
                    .search(SearchQuery {
                        vector: query,
                        k: 10,
                        filter: None,
                        ef_search: None,
                    })
                    .unwrap();

                assert_eq!(results.len(), 10);
            }

            let duration = start.elapsed();
            println!(
                "Thread {} completed {} queries in {:?}",
                thread_id, queries_per_thread, duration
            );
            duration
        });

        handles.push(handle);
    }

    // Wait for all threads and collect results
    let mut total_duration = std::time::Duration::ZERO;
    for handle in handles {
        let duration = handle.join().unwrap();
        total_duration += duration;
    }

    let total_queries = num_threads * queries_per_thread;
    println!("Total queries: {}", total_queries);
    println!(
        "Average duration per thread: {:?}",
        total_duration / num_threads as u32
    );
}

#[test]
fn test_concurrent_inserts_and_queries() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir
        .path()
        .join("mixed_concurrent.db")
        .to_string_lossy()
        .to_string();
    options.dimensions = 32;
    options.hnsw_config = Some(HnswConfig::default());

    let db = Arc::new(VectorDB::new(options).unwrap());

    // Initial data
    let initial: Vec<VectorEntry> = (0..100)
        .map(|i| VectorEntry {
            id: Some(format!("initial_{}", i)),
            vector: (0..32).map(|j| ((i + j) as f32) * 0.1).collect(),
            metadata: None,
        })
        .collect();

    db.insert_batch(initial).unwrap();

    // Spawn reader threads
    let num_readers = 5;
    let num_writers = 2;
    let barrier = Arc::new(Barrier::new(num_readers + num_writers));
    let mut handles = vec![];

    // Reader threads
    for reader_id in 0..num_readers {
        let db_clone = Arc::clone(&db);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            for i in 0..50 {
                let query: Vec<f32> = (0..32)
                    .map(|j| ((reader_id * 100 + i + j) as f32) * 0.1)
                    .collect();
                let results = db_clone
                    .search(SearchQuery {
                        vector: query,
                        k: 5,
                        filter: None,
                        ef_search: None,
                    })
                    .unwrap();

                assert!(results.len() > 0 && results.len() <= 5);
            }

            println!("Reader {} completed", reader_id);
        });

        handles.push(handle);
    }

    // Writer threads
    for writer_id in 0..num_writers {
        let db_clone = Arc::clone(&db);
        let barrier_clone = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            barrier_clone.wait();

            for i in 0..20 {
                let entry = VectorEntry {
                    id: Some(format!("writer_{}_{}", writer_id, i)),
                    vector: (0..32)
                        .map(|j| ((writer_id * 1000 + i + j) as f32) * 0.1)
                        .collect(),
                    metadata: None,
                };

                db_clone.insert(entry).unwrap();
            }

            println!("Writer {} completed", writer_id);
        });

        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state
    let final_len = db.len().unwrap();
    println!("Final database size: {}", final_len);
    assert!(final_len >= 100); // At least initial data should remain
}

// ============================================================================
// Memory Pressure Tests
// ============================================================================

#[test]
#[ignore] // Run with: cargo test --test stress_tests -- --ignored
fn test_memory_pressure_large_vectors() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir
        .path()
        .join("large_vectors.db")
        .to_string_lossy()
        .to_string();
    options.dimensions = 2048; // Very large vectors
    options.hnsw_config = Some(HnswConfig {
        m: 8,
        ef_construction: 50,
        ef_search: 50,
        max_elements: 100_000,
    });

    let db = VectorDB::new(options).unwrap();

    println!("Testing with large 2048-dimensional vectors...");
    let num_vectors = 10_000;
    let batch_size = 1000;

    for batch_idx in 0..(num_vectors / batch_size) {
        let vectors: Vec<VectorEntry> = (0..batch_size)
            .map(|i| {
                let global_idx = batch_idx * batch_size + i;
                VectorEntry {
                    id: Some(format!("vec_{}", global_idx)),
                    vector: (0..2048)
                        .map(|j| ((global_idx + j) as f32) * 0.0001)
                        .collect(),
                    metadata: None,
                }
            })
            .collect();

        db.insert_batch(vectors).unwrap();
        println!(
            "Inserted batch {}/{}",
            batch_idx + 1,
            num_vectors / batch_size
        );
    }

    println!("Database size: {}", db.len().unwrap());
    assert_eq!(db.len().unwrap(), num_vectors);

    // Perform searches
    for i in 0..5 {
        let query: Vec<f32> = (0..2048)
            .map(|j| ((i * 1000 + j) as f32) * 0.0001)
            .collect();
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

// ============================================================================
// Error Recovery Tests
// ============================================================================

#[test]
fn test_invalid_operations_dont_crash() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
    options.dimensions = 32;

    let db = VectorDB::new(options).unwrap();

    // Try various invalid operations

    // 1. Delete non-existent vector
    let _ = db.delete("nonexistent");

    // 2. Get non-existent vector
    let _ = db.get("nonexistent");

    // 3. Search with k=0
    let result = db.search(SearchQuery {
        vector: vec![0.0; 32],
        k: 0,
        filter: None,
        ef_search: None,
    });
    // Should either return empty or error gracefully
    let _ = result;

    // 4. Insert and immediately delete in rapid succession
    for i in 0..100 {
        let id = db
            .insert(VectorEntry {
                id: Some(format!("temp_{}", i)),
                vector: vec![1.0; 32],
                metadata: None,
            })
            .unwrap();

        db.delete(&id).unwrap();
    }

    // Database should still be functional
    db.insert(VectorEntry {
        id: Some("final".to_string()),
        vector: vec![1.0; 32],
        metadata: None,
    })
    .unwrap();

    assert!(db.get("final").unwrap().is_some());
}

#[test]
fn test_repeated_operations() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
    options.dimensions = 16;
    options.hnsw_config = None;

    let db = VectorDB::new(options).unwrap();

    // Insert the same ID multiple times (should replace or error)
    for _ in 0..10 {
        let _ = db.insert(VectorEntry {
            id: Some("same_id".to_string()),
            vector: vec![1.0; 16],
            metadata: None,
        });
    }

    // Delete the same ID multiple times
    for _ in 0..5 {
        let _ = db.delete("same_id");
    }

    // Search repeatedly with the same query
    let query = vec![1.0; 16];
    for _ in 0..100 {
        let _ = db.search(SearchQuery {
            vector: query.clone(),
            k: 10,
            filter: None,
            ef_search: None,
        });
    }
}

// ============================================================================
// Extreme Parameter Tests
// ============================================================================

#[test]
fn test_extreme_k_values() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
    options.dimensions = 16;
    options.hnsw_config = None;

    let db = VectorDB::new(options).unwrap();

    // Insert some vectors
    for i in 0..10 {
        db.insert(VectorEntry {
            id: Some(format!("vec_{}", i)),
            vector: vec![i as f32; 16],
            metadata: None,
        })
        .unwrap();
    }

    // Search with k larger than database size
    let results = db
        .search(SearchQuery {
            vector: vec![1.0; 16],
            k: 1000,
            filter: None,
            ef_search: None,
        })
        .unwrap();

    // Should return at most 10 results
    assert!(results.len() <= 10);

    // Search with k=1
    let results = db
        .search(SearchQuery {
            vector: vec![1.0; 16],
            k: 1,
            filter: None,
            ef_search: None,
        })
        .unwrap();

    assert_eq!(results.len(), 1);
}
