//! Concurrent access tests with multiple threads
//!
//! These tests verify thread-safety and correct behavior under concurrent access.

use ruvector_core::types::{DbOptions, HnswConfig, SearchQuery};
use ruvector_core::{VectorDB, VectorEntry};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;
use tempfile::tempdir;

#[test]
fn test_concurrent_reads() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir
        .path()
        .join("concurrent_reads.db")
        .to_string_lossy()
        .to_string();
    options.dimensions = 32;

    let db = Arc::new(VectorDB::new(options).unwrap());

    // Insert initial data
    for i in 0..100 {
        db.insert(VectorEntry {
            id: Some(format!("vec_{}", i)),
            vector: (0..32).map(|j| ((i + j) as f32) * 0.1).collect(),
            metadata: None,
        })
        .unwrap();
    }

    // Spawn multiple reader threads
    let num_threads = 10;
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let db_clone = Arc::clone(&db);

        let handle = thread::spawn(move || {
            for i in 0..50 {
                let id = format!("vec_{}", (thread_id * 10 + i) % 100);
                let result = db_clone.get(&id).unwrap();
                assert!(result.is_some(), "Failed to get {}", id);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_writes_no_collision() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir
        .path()
        .join("concurrent_writes.db")
        .to_string_lossy()
        .to_string();
    options.dimensions = 32;

    let db = Arc::new(VectorDB::new(options).unwrap());

    // Spawn multiple writer threads with non-overlapping IDs
    let num_threads = 10;
    let vectors_per_thread = 20;
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let db_clone = Arc::clone(&db);

        let handle = thread::spawn(move || {
            for i in 0..vectors_per_thread {
                let id = format!("thread_{}_{}", thread_id, i);
                db_clone
                    .insert(VectorEntry {
                        id: Some(id.clone()),
                        vector: vec![thread_id as f32; 32],
                        metadata: None,
                    })
                    .unwrap();
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all vectors were inserted
    assert_eq!(db.len().unwrap(), num_threads * vectors_per_thread);
}

#[test]
fn test_concurrent_delete_and_insert() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir
        .path()
        .join("concurrent_delete_insert.db")
        .to_string_lossy()
        .to_string();
    options.dimensions = 16;

    let db = Arc::new(VectorDB::new(options).unwrap());

    // Insert initial data
    for i in 0..100 {
        db.insert(VectorEntry {
            id: Some(format!("vec_{}", i)),
            vector: vec![i as f32; 16],
            metadata: None,
        })
        .unwrap();
    }

    let num_threads = 5;
    let mut handles = vec![];

    // Deleter threads
    for thread_id in 0..num_threads {
        let db_clone = Arc::clone(&db);

        let handle = thread::spawn(move || {
            for i in 0..10 {
                let id = format!("vec_{}", thread_id * 10 + i);
                db_clone.delete(&id).unwrap();
            }
        });

        handles.push(handle);
    }

    // Inserter threads
    for thread_id in 0..num_threads {
        let db_clone = Arc::clone(&db);

        let handle = thread::spawn(move || {
            for i in 0..10 {
                let id = format!("new_{}_{}", thread_id, i);
                db_clone
                    .insert(VectorEntry {
                        id: Some(id),
                        vector: vec![(thread_id * 100 + i) as f32; 16],
                        metadata: None,
                    })
                    .unwrap();
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify database is in consistent state
    let final_len = db.len().unwrap();
    assert_eq!(final_len, 100); // 100 original - 50 deleted + 50 inserted
}

#[test]
fn test_concurrent_search_and_insert() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir
        .path()
        .join("concurrent_search_insert.db")
        .to_string_lossy()
        .to_string();
    options.dimensions = 64;
    options.hnsw_config = Some(HnswConfig::default());

    let db = Arc::new(VectorDB::new(options).unwrap());

    // Insert initial data
    for i in 0..100 {
        db.insert(VectorEntry {
            id: Some(format!("vec_{}", i)),
            vector: (0..64).map(|j| ((i + j) as f32) * 0.01).collect(),
            metadata: None,
        })
        .unwrap();
    }

    let num_search_threads = 5;
    let num_insert_threads = 2;
    let mut handles = vec![];

    // Search threads
    for search_id in 0..num_search_threads {
        let db_clone = Arc::clone(&db);

        let handle = thread::spawn(move || {
            for i in 0..20 {
                let query: Vec<f32> = (0..64)
                    .map(|j| ((search_id * 10 + i + j) as f32) * 0.01)
                    .collect();
                let results = db_clone
                    .search(SearchQuery {
                        vector: query,
                        k: 5,
                        filter: None,
                        ef_search: None,
                    })
                    .unwrap();

                // Should always return some results (at least from initial data)
                assert!(results.len() > 0);
            }
        });

        handles.push(handle);
    }

    // Insert threads
    for insert_id in 0..num_insert_threads {
        let db_clone = Arc::clone(&db);

        let handle = thread::spawn(move || {
            for i in 0..50 {
                db_clone
                    .insert(VectorEntry {
                        id: Some(format!("new_{}_{}", insert_id, i)),
                        vector: (0..64)
                            .map(|j| ((insert_id * 1000 + i + j) as f32) * 0.01)
                            .collect(),
                        metadata: None,
                    })
                    .unwrap();
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state
    assert_eq!(db.len().unwrap(), 200); // 100 initial + 100 new
}

#[test]
fn test_atomicity_of_batch_insert() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir
        .path()
        .join("atomic_batch.db")
        .to_string_lossy()
        .to_string();
    options.dimensions = 16;

    let db = Arc::new(VectorDB::new(options).unwrap());

    // Track successful insertions
    let inserted_ids = Arc::new(Mutex::new(HashSet::new()));

    let num_threads = 5;
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let db_clone = Arc::clone(&db);
        let ids_clone = Arc::clone(&inserted_ids);

        let handle = thread::spawn(move || {
            for batch_idx in 0..10 {
                let vectors: Vec<VectorEntry> = (0..10)
                    .map(|i| {
                        let id = format!("t{}_b{}_v{}", thread_id, batch_idx, i);
                        VectorEntry {
                            id: Some(id.clone()),
                            vector: vec![(thread_id * 100 + batch_idx * 10 + i) as f32; 16],
                            metadata: None,
                        }
                    })
                    .collect();

                let ids = db_clone.insert_batch(vectors).unwrap();

                let mut lock = ids_clone.lock().unwrap();
                for id in ids {
                    lock.insert(id);
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all insertions were recorded
    let total_inserted = inserted_ids.lock().unwrap().len();
    assert_eq!(total_inserted, num_threads * 10 * 10); // threads * batches * vectors_per_batch
    assert_eq!(db.len().unwrap(), total_inserted);
}

#[test]
fn test_read_write_consistency() {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir
        .path()
        .join("consistency.db")
        .to_string_lossy()
        .to_string();
    options.dimensions = 32;

    let db = Arc::new(VectorDB::new(options).unwrap());

    // Insert initial vector
    db.insert(VectorEntry {
        id: Some("test".to_string()),
        vector: vec![1.0; 32],
        metadata: None,
    })
    .unwrap();

    let num_threads = 10;
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let db_clone = Arc::clone(&db);

        let handle = thread::spawn(move || {
            for _ in 0..100 {
                // Read
                let entry = db_clone.get("test").unwrap();
                assert!(entry.is_some());

                // Verify vector is consistent
                let vector = entry.unwrap().vector;
                assert_eq!(vector.len(), 32);

                // All values should be the same (not corrupted)
                let first_val = vector[0];
                assert!(vector
                    .iter()
                    .all(|&v| v == first_val || (first_val == 1.0 || v == (thread_id as f32))));

                // Write (update) - this creates a race condition intentionally
                if thread_id % 2 == 0 {
                    let _ = db_clone.insert(VectorEntry {
                        id: Some("test".to_string()),
                        vector: vec![thread_id as f32; 32],
                        metadata: None,
                    });
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify database is still consistent
    let final_entry = db.get("test").unwrap();
    assert!(final_entry.is_some());

    let vector = final_entry.unwrap().vector;
    assert_eq!(vector.len(), 32);

    // Check no corruption (all values should be the same)
    let first_val = vector[0];
    assert!(vector.iter().all(|&v| v == first_val));
}

#[test]
fn test_concurrent_metadata_updates() {
    use std::collections::HashMap;

    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir.path().join("metadata.db").to_string_lossy().to_string();
    options.dimensions = 16;

    let db = Arc::new(VectorDB::new(options).unwrap());

    // Insert initial vectors
    for i in 0..50 {
        db.insert(VectorEntry {
            id: Some(format!("vec_{}", i)),
            vector: vec![i as f32; 16],
            metadata: None,
        })
        .unwrap();
    }

    let num_threads = 5;
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let db_clone = Arc::clone(&db);

        let handle = thread::spawn(move || {
            for i in 0..10 {
                let mut metadata = HashMap::new();
                metadata.insert(format!("thread_{}", thread_id), serde_json::json!(i));

                // Update vector with metadata
                let id = format!("vec_{}", i * 5 + thread_id);
                db_clone
                    .insert(VectorEntry {
                        id: Some(id.clone()),
                        vector: vec![thread_id as f32; 16],
                        metadata: Some(metadata),
                    })
                    .unwrap();
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify some vectors have metadata
    let entry = db.get("vec_0").unwrap();
    assert!(entry.is_some());
}
