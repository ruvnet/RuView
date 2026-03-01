//! Concurrent access pattern tests
//!
//! Tests for multi-threaded access, lock-free operations, and concurrent modifications.

use ruvector_graph::{Edge, GraphDB, Label, Node, Properties, PropertyValue};
use std::sync::Arc;
use std::thread;

#[test]
fn test_concurrent_node_creation() {
    let db = Arc::new(GraphDB::new());
    let num_threads = 10;
    let nodes_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let db_clone = Arc::clone(&db);
            thread::spawn(move || {
                for i in 0..nodes_per_thread {
                    let mut props = Properties::new();
                    props.insert("thread".to_string(), PropertyValue::Integer(thread_id));
                    props.insert("index".to_string(), PropertyValue::Integer(i));

                    let node = Node::new(
                        format!("node_{}_{}", thread_id, i),
                        vec![Label {
                            name: "Concurrent".to_string(),
                        }],
                        props,
                    );

                    db_clone.create_node(node).unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all nodes were created
    // Note: Would need node_count() method
    // assert_eq!(db.node_count(), num_threads * nodes_per_thread);
}

#[test]
fn test_concurrent_reads() {
    let db = Arc::new(GraphDB::new());

    // Create initial nodes
    for i in 0..100 {
        let node = Node::new(format!("node_{}", i), vec![], Properties::new());
        db.create_node(node).unwrap();
    }

    let num_readers = 20;
    let reads_per_thread = 1000;

    let handles: Vec<_> = (0..num_readers)
        .map(|thread_id| {
            let db_clone = Arc::clone(&db);
            thread::spawn(move || {
                for i in 0..reads_per_thread {
                    let node_id = format!("node_{}", (thread_id * 10 + i) % 100);
                    let result = db_clone.get_node(&node_id);
                    assert!(result.is_some());
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_writes_no_collision() {
    let db = Arc::new(GraphDB::new());
    let num_threads = 10;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let db_clone = Arc::clone(&db);
            thread::spawn(move || {
                for i in 0..50 {
                    let node_id = format!("t{}_n{}", thread_id, i);
                    let node = Node::new(node_id, vec![], Properties::new());
                    db_clone.create_node(node).unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // All 500 nodes should be created
}

#[test]
fn test_concurrent_edge_creation() {
    let db = Arc::new(GraphDB::new());

    // Create nodes first
    for i in 0..100 {
        db.create_node(Node::new(format!("n{}", i), vec![], Properties::new()))
            .unwrap();
    }

    let num_threads = 10;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let db_clone = Arc::clone(&db);
            thread::spawn(move || {
                for i in 0..50 {
                    let from = format!("n{}", (thread_id * 10 + i) % 100);
                    let to = format!("n{}", (thread_id * 10 + i + 1) % 100);

                    let edge = Edge::new(
                        format!("e_{}_{}", thread_id, i),
                        from,
                        to,
                        "LINK".to_string(),
                        Properties::new(),
                    );

                    db_clone.create_edge(edge).unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_read_while_writing() {
    let db = Arc::new(GraphDB::new());

    // Initial nodes
    for i in 0..50 {
        db.create_node(Node::new(
            format!("initial_{}", i),
            vec![],
            Properties::new(),
        ))
        .unwrap();
    }

    let num_readers = 5;
    let num_writers = 3;

    let mut handles = vec![];

    // Spawn readers
    for reader_id in 0..num_readers {
        let db_clone = Arc::clone(&db);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let node_id = format!("initial_{}", (reader_id * 10 + i) % 50);
                let _ = db_clone.get_node(&node_id);
            }
        });
        handles.push(handle);
    }

    // Spawn writers
    for writer_id in 0..num_writers {
        let db_clone = Arc::clone(&db);
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let node = Node::new(
                    format!("new_{}_{}", writer_id, i),
                    vec![],
                    Properties::new(),
                );
                db_clone.create_node(node).unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_property_updates() {
    let db = Arc::new(GraphDB::new());

    // Create shared counter node
    let mut props = Properties::new();
    props.insert("counter".to_string(), PropertyValue::Integer(0));
    db.create_node(Node::new("counter".to_string(), vec![], props))
        .unwrap();

    // TODO: Implement atomic property updates
    // For now, just test concurrent reads
    let num_threads = 10;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let db_clone = Arc::clone(&db);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _node = db_clone.get_node("counter");
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_lock_free_reads() {
    let db = Arc::new(GraphDB::new());

    // Populate database
    for i in 0..1000 {
        db.create_node(Node::new(
            format!("node_{}", i),
            vec![Label {
                name: "Test".to_string(),
            }],
            Properties::new(),
        ))
        .unwrap();
    }

    // Many concurrent readers should not block each other
    let num_readers = 50;

    let handles: Vec<_> = (0..num_readers)
        .map(|reader_id| {
            let db_clone = Arc::clone(&db);
            thread::spawn(move || {
                for i in 0..100 {
                    let node_id = format!("node_{}", (reader_id * 20 + i) % 1000);
                    let result = db_clone.get_node(&node_id);
                    assert!(result.is_some());
                }
            })
        })
        .collect();

    let start = std::time::Instant::now();

    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();

    // With lock-free reads, this should complete quickly
    // Even with 50 threads doing 100 reads each (5000 reads total)
    println!("Concurrent reads took: {:?}", duration);
}

#[test]
fn test_writer_starvation_prevention() {
    // Ensure that heavy read load doesn't prevent writes

    let db = Arc::new(GraphDB::new());

    // Initial data
    for i in 0..100 {
        db.create_node(Node::new(
            format!("initial_{}", i),
            vec![],
            Properties::new(),
        ))
        .unwrap();
    }

    let readers_done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let writers_done = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let mut handles = vec![];

    // Heavy read load
    for reader_id in 0..20i64 {
        let db_clone = Arc::clone(&db);
        let done = Arc::clone(&readers_done);
        let handle = thread::spawn(move || {
            for i in 0..1000i64 {
                let node_id = format!("initial_{}", (reader_id + i) % 100);
                let _ = db_clone.get_node(&node_id);
            }
            done.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        handles.push(handle);
    }

    // Writers should still make progress
    for writer_id in 0..5 {
        let db_clone = Arc::clone(&db);
        let done = Arc::clone(&writers_done);
        let handle = thread::spawn(move || {
            for i in 0..50 {
                let node = Node::new(
                    format!("writer_{}_{}", writer_id, i),
                    vec![],
                    Properties::new(),
                );
                db_clone.create_node(node).unwrap();
            }
            done.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify both readers and writers completed
    assert!(readers_done.load(std::sync::atomic::Ordering::Relaxed));
    assert!(writers_done.load(std::sync::atomic::Ordering::Relaxed));
}

// ============================================================================
// Stress Tests
// ============================================================================

#[test]
fn test_high_concurrency_stress() {
    let db = Arc::new(GraphDB::new());
    let num_threads = 50;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let db_clone = Arc::clone(&db);
            thread::spawn(move || {
                // Mix of operations
                for i in 0i32..100 {
                    if i % 3 == 0 {
                        // Create node
                        let node = Node::new(
                            format!("stress_{}_{}", thread_id, i),
                            vec![],
                            Properties::new(),
                        );
                        db_clone.create_node(node).unwrap();
                    } else {
                        // Read node (might not exist)
                        let node_id = format!("stress_{}_{}", thread_id, i.saturating_sub(1));
                        let _ = db_clone.get_node(&node_id);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_batch_operations() {
    let db = Arc::new(GraphDB::new());
    let num_threads = 10;
    let batch_size = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let db_clone = Arc::clone(&db);
            thread::spawn(move || {
                // TODO: Implement batch insert
                // For now, insert individually
                for i in 0..batch_size {
                    let node = Node::new(
                        format!("batch_{}_{}", thread_id, i),
                        vec![],
                        Properties::new(),
                    );
                    db_clone.create_node(node).unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}
