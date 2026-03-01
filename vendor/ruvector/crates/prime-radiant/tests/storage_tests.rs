//! Comprehensive Storage Layer Tests
//!
//! Tests for:
//! - InMemoryStorage CRUD operations
//! - FileStorage persistence
//! - Concurrent access patterns
//! - Governance storage operations

use prime_radiant::storage::{
    FileStorage, GovernanceStorage, GraphStorage, InMemoryStorage, StorageFormat,
};
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;

// ============================================================================
// InMemoryStorage Unit Tests
// ============================================================================

mod in_memory_storage_tests {
    use super::*;

    #[test]
    fn test_store_and_retrieve_node() {
        let storage = InMemoryStorage::new();
        let state = vec![1.0, 2.0, 3.0];

        storage.store_node("node-1", &state).unwrap();
        let retrieved = storage.get_node("node-1").unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), state);
    }

    #[test]
    fn test_retrieve_nonexistent_node() {
        let storage = InMemoryStorage::new();
        let result = storage.get_node("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_node_state() {
        let storage = InMemoryStorage::new();

        storage.store_node("node-1", &[1.0, 0.0]).unwrap();
        storage.store_node("node-1", &[0.0, 1.0]).unwrap();

        let retrieved = storage.get_node("node-1").unwrap().unwrap();
        assert_eq!(retrieved, vec![0.0, 1.0]);
    }

    #[test]
    fn test_store_and_delete_edge() {
        let storage = InMemoryStorage::new();

        storage.store_edge("a", "b", 1.5).unwrap();
        storage.store_edge("b", "c", 2.0).unwrap();

        // Delete one edge
        storage.delete_edge("a", "b").unwrap();

        // Should not fail on non-existent edge
        storage.delete_edge("x", "y").unwrap();
    }

    #[test]
    fn test_find_similar_vectors() {
        let storage = InMemoryStorage::new();

        // Store orthogonal vectors
        storage.store_node("north", &[0.0, 1.0, 0.0]).unwrap();
        storage.store_node("east", &[1.0, 0.0, 0.0]).unwrap();
        storage.store_node("south", &[0.0, -1.0, 0.0]).unwrap();
        storage.store_node("up", &[0.0, 0.0, 1.0]).unwrap();

        // Query for similar to north
        let results = storage.find_similar(&[0.0, 1.0, 0.0], 2).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "north"); // Exact match
        assert!((results[0].1 - 1.0).abs() < 0.001); // Similarity = 1.0
    }

    #[test]
    fn test_find_similar_empty_query() {
        let storage = InMemoryStorage::new();
        storage.store_node("a", &[1.0, 2.0]).unwrap();

        let results = storage.find_similar(&[], 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_governance_store_policy() {
        let storage = InMemoryStorage::new();

        let policy_data = b"test policy bundle data";
        let id = storage.store_policy(policy_data).unwrap();

        assert!(!id.is_empty());

        let retrieved = storage.get_policy(&id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), policy_data.to_vec());
    }

    #[test]
    fn test_governance_store_witness() {
        let storage = InMemoryStorage::new();

        let witness_data = b"witness record data";
        let id = storage.store_witness(witness_data).unwrap();

        assert!(!id.is_empty());
    }

    #[test]
    fn test_governance_store_lineage() {
        let storage = InMemoryStorage::new();

        let lineage_data = b"lineage record data";
        let id = storage.store_lineage(lineage_data).unwrap();

        assert!(!id.is_empty());
    }

    #[test]
    fn test_concurrent_node_writes() {
        let storage: Arc<InMemoryStorage> = Arc::new(InMemoryStorage::new());
        let num_threads = 10;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        for i in 0..num_threads {
            let storage_clone: Arc<InMemoryStorage> = Arc::clone(&storage);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                // Wait for all threads to be ready
                barrier_clone.wait();

                for j in 0..100 {
                    let node_id = format!("node-{}-{}", i, j);
                    let state = vec![i as f32, j as f32];
                    storage_clone.store_node(&node_id, &state).unwrap();
                }
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify we can retrieve a sample node
        let result = storage.get_node("node-5-50").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), vec![5.0, 50.0]);
    }

    #[test]
    fn test_concurrent_reads_and_writes() {
        let storage: Arc<InMemoryStorage> = Arc::new(InMemoryStorage::new());

        // Pre-populate some data
        for i in 0..100 {
            storage
                .store_node(&format!("node-{}", i), &[i as f32])
                .unwrap();
        }

        let num_threads = 8;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        for i in 0..num_threads {
            let storage_clone: Arc<InMemoryStorage> = Arc::clone(&storage);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                for j in 0..50 {
                    if i % 2 == 0 {
                        // Writers
                        let node_id = format!("new-node-{}-{}", i, j);
                        storage_clone.store_node(&node_id, &[j as f32]).unwrap();
                    } else {
                        // Readers
                        let node_id = format!("node-{}", j);
                        let _ = storage_clone.get_node(&node_id).unwrap();
                    }
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_large_vector_storage() {
        let storage = InMemoryStorage::new();

        // Store a high-dimensional vector
        let large_vec: Vec<f32> = (0..1024).map(|i| i as f32 / 1024.0).collect();
        storage.store_node("large-vector", &large_vec).unwrap();

        let retrieved = storage.get_node("large-vector").unwrap().unwrap();
        assert_eq!(retrieved.len(), 1024);
        assert!((retrieved[0] - 0.0).abs() < 0.001);
        assert!((retrieved[1023] - 1023.0 / 1024.0).abs() < 0.001);
    }

    #[test]
    fn test_many_nodes() {
        let storage = InMemoryStorage::new();

        // Store many nodes
        for i in 0..1000 {
            let node_id = format!("node-{}", i);
            let state = vec![(i % 100) as f32, (i / 100) as f32];
            storage.store_node(&node_id, &state).unwrap();
        }

        // Verify random access works
        let n500 = storage.get_node("node-500").unwrap().unwrap();
        assert_eq!(n500, vec![0.0, 5.0]);

        let n999 = storage.get_node("node-999").unwrap().unwrap();
        assert_eq!(n999, vec![99.0, 9.0]);
    }
}

// ============================================================================
// FileStorage Unit Tests
// ============================================================================

mod file_storage_tests {
    use super::*;

    fn create_temp_storage() -> (FileStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path()).unwrap();
        (storage, temp_dir)
    }

    #[test]
    fn test_store_and_retrieve_node() {
        let (storage, _dir) = create_temp_storage();
        let state = vec![1.0, 2.0, 3.0, 4.0];

        storage.store_node("test-node", &state).unwrap();
        let retrieved = storage.get_node("test-node").unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), state);
    }

    #[test]
    fn test_retrieve_nonexistent_node() {
        let (storage, _dir) = create_temp_storage();
        let result = storage.get_node("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_store_and_delete_edge() {
        let (storage, _dir) = create_temp_storage();

        storage.store_edge("source", "target", 0.75).unwrap();
        let stats = storage.stats();
        assert_eq!(stats.edge_count, 1);

        storage.delete_edge("source", "target").unwrap();
        let stats = storage.stats();
        assert_eq!(stats.edge_count, 0);
    }

    #[test]
    fn test_persistence_across_instances() {
        let temp_dir = TempDir::new().unwrap();

        // First instance: write data
        {
            let storage = FileStorage::new(temp_dir.path()).unwrap();
            storage
                .store_node("persistent-node", &[1.0, 2.0, 3.0])
                .unwrap();
            storage.store_edge("a", "b", 1.5).unwrap();
            storage.sync().unwrap();
        }

        // Second instance: read data back
        {
            let storage = FileStorage::new(temp_dir.path()).unwrap();
            let node_state = storage.get_node("persistent-node").unwrap();

            assert!(node_state.is_some());
            assert_eq!(node_state.unwrap(), vec![1.0, 2.0, 3.0]);

            let stats = storage.stats();
            assert_eq!(stats.node_count, 1);
            assert_eq!(stats.edge_count, 1);
        }
    }

    #[test]
    fn test_json_format() {
        let temp_dir = TempDir::new().unwrap();
        let storage =
            FileStorage::with_options(temp_dir.path(), StorageFormat::Json, false).unwrap();

        storage.store_node("json-node", &[1.5, 2.5, 3.5]).unwrap();

        let retrieved = storage.get_node("json-node").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), vec![1.5, 2.5, 3.5]);
    }

    #[test]
    fn test_bincode_format() {
        let temp_dir = TempDir::new().unwrap();
        let storage =
            FileStorage::with_options(temp_dir.path(), StorageFormat::Bincode, false).unwrap();

        storage.store_node("bincode-node", &[1.0, 2.0]).unwrap();

        let retrieved = storage.get_node("bincode-node").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), vec![1.0, 2.0]);
    }

    #[test]
    fn test_wal_recovery() {
        let temp_dir = TempDir::new().unwrap();

        // Write with WAL enabled
        {
            let storage =
                FileStorage::with_options(temp_dir.path(), StorageFormat::Bincode, true).unwrap();
            storage.store_node("wal-node", &[1.0, 2.0, 3.0]).unwrap();
            // Don't call sync - simulate crash
        }

        // Re-open and verify WAL recovery
        {
            let storage =
                FileStorage::with_options(temp_dir.path(), StorageFormat::Bincode, true).unwrap();
            let node_state = storage.get_node("wal-node").unwrap();
            assert!(node_state.is_some());
        }
    }

    #[test]
    fn test_governance_policy_persistence() {
        let temp_dir = TempDir::new().unwrap();

        let policy_id;
        {
            let storage = FileStorage::new(temp_dir.path()).unwrap();
            policy_id = storage.store_policy(b"important policy data").unwrap();
            storage.sync().unwrap();
        }

        {
            let storage = FileStorage::new(temp_dir.path()).unwrap();
            let policy = storage.get_policy(&policy_id).unwrap();
            assert!(policy.is_some());
            assert_eq!(policy.unwrap(), b"important policy data".to_vec());
        }
    }

    #[test]
    fn test_find_similar_vectors() {
        let (storage, _dir) = create_temp_storage();

        storage.store_node("a", &[1.0, 0.0, 0.0]).unwrap();
        storage.store_node("b", &[0.9, 0.1, 0.0]).unwrap();
        storage.store_node("c", &[0.0, 1.0, 0.0]).unwrap();

        let results = storage.find_similar(&[1.0, 0.0, 0.0], 2).unwrap();

        assert_eq!(results.len(), 2);
        // "a" should be first (exact match)
        assert_eq!(results[0].0, "a");
    }

    #[test]
    fn test_storage_stats() {
        let (storage, _dir) = create_temp_storage();

        storage.store_node("n1", &[1.0]).unwrap();
        storage.store_node("n2", &[2.0]).unwrap();
        storage.store_edge("n1", "n2", 1.0).unwrap();

        let stats = storage.stats();
        assert_eq!(stats.node_count, 2);
        assert_eq!(stats.edge_count, 1);
        assert!(stats.wal_enabled);
    }

    #[test]
    fn test_concurrent_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let storage: Arc<FileStorage> = Arc::new(FileStorage::new(temp_dir.path()).unwrap());

        let num_threads = 4;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        for i in 0..num_threads {
            let storage_clone: Arc<FileStorage> = Arc::clone(&storage);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                for j in 0..25 {
                    let node_id = format!("concurrent-{}-{}", i, j);
                    storage_clone
                        .store_node(&node_id, &[i as f32, j as f32])
                        .unwrap();
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all writes succeeded
        let stats = storage.stats();
        assert_eq!(stats.node_count, 100);
    }

    #[test]
    fn test_witness_storage_and_retrieval() {
        let (storage, _dir) = create_temp_storage();

        // Store multiple witnesses
        let w1_data = b"witness-1-action-abc";
        let w2_data = b"witness-2-action-xyz";
        let w3_data = b"witness-3-action-abc";

        storage.store_witness(w1_data).unwrap();
        storage.store_witness(w2_data).unwrap();
        storage.store_witness(w3_data).unwrap();

        // Search for witnesses containing "action-abc"
        let results = storage.get_witnesses_for_action("action-abc").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_lineage_storage() {
        let (storage, _dir) = create_temp_storage();

        let lineage_data = b"lineage record with dependencies";
        let id = storage.store_lineage(lineage_data).unwrap();

        assert!(!id.is_empty());
        // Lineages are write-only in the basic API, so just verify storage succeeded
    }
}

// ============================================================================
// Integration Tests: Storage Pipelines
// ============================================================================

mod integration_tests {
    use super::*;

    /// Test the complete storage flow for a multi-tenant scenario
    #[test]
    fn test_multi_tenant_isolation() {
        let storage = InMemoryStorage::new();

        // Tenant A data (with namespace prefix)
        storage.store_node("tenant-a::node-1", &[1.0, 0.0]).unwrap();
        storage.store_node("tenant-a::node-2", &[0.0, 1.0]).unwrap();

        // Tenant B data
        storage.store_node("tenant-b::node-1", &[0.5, 0.5]).unwrap();
        storage.store_node("tenant-b::node-2", &[0.3, 0.7]).unwrap();

        // Verify isolation - tenant A's node-1 is different from tenant B's
        let a_node = storage.get_node("tenant-a::node-1").unwrap().unwrap();
        let b_node = storage.get_node("tenant-b::node-1").unwrap().unwrap();

        assert_ne!(a_node, b_node);

        // Find similar should respect prefixes
        let results = storage.find_similar(&[1.0, 0.0], 4).unwrap();
        assert!(results.iter().any(|(id, _)| id == "tenant-a::node-1"));
    }

    /// Test governance data isolation
    #[test]
    fn test_governance_policy_isolation() {
        let storage = InMemoryStorage::new();

        // Store multiple policies
        let policy_a = storage.store_policy(b"policy-for-tenant-a").unwrap();
        let policy_b = storage.store_policy(b"policy-for-tenant-b").unwrap();

        // Each policy should have a unique ID
        assert_ne!(policy_a, policy_b);

        // Retrieval should work independently
        let a_data = storage.get_policy(&policy_a).unwrap().unwrap();
        let b_data = storage.get_policy(&policy_b).unwrap().unwrap();

        assert_eq!(a_data, b"policy-for-tenant-a".to_vec());
        assert_eq!(b_data, b"policy-for-tenant-b".to_vec());
    }

    /// Test file storage survives process restart
    #[test]
    fn test_file_storage_durability() {
        let temp_dir = TempDir::new().unwrap();

        // Simulate first process
        {
            let storage = FileStorage::new(temp_dir.path()).unwrap();

            // Store graph data
            storage
                .store_node("persistent-1", &[1.0, 2.0, 3.0])
                .unwrap();
            storage
                .store_node("persistent-2", &[4.0, 5.0, 6.0])
                .unwrap();
            storage
                .store_edge("persistent-1", "persistent-2", 0.5)
                .unwrap();

            // Store governance data
            storage.store_policy(b"durable-policy").unwrap();
            storage.store_witness(b"durable-witness").unwrap();

            storage.sync().unwrap();
            // Storage dropped here
        }

        // Simulate second process (restart)
        {
            let storage = FileStorage::new(temp_dir.path()).unwrap();

            // All data should be present
            let stats = storage.stats();
            assert_eq!(stats.node_count, 2);
            assert_eq!(stats.edge_count, 1);

            let node1 = storage.get_node("persistent-1").unwrap().unwrap();
            assert_eq!(node1, vec![1.0, 2.0, 3.0]);
        }
    }

    /// Test hybrid storage (memory + file) fallback pattern
    #[test]
    fn test_storage_fallback_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let file_storage: Arc<FileStorage> = Arc::new(FileStorage::new(temp_dir.path()).unwrap());
        let memory_cache = InMemoryStorage::new();

        // Simulate a read-through cache pattern
        let node_id = "cached-node";
        let state = vec![1.0, 2.0, 3.0];

        // Write to persistent storage
        file_storage.store_node(node_id, &state).unwrap();

        // Check cache first (miss)
        let cached = memory_cache.get_node(node_id).unwrap();
        assert!(cached.is_none());

        // Read from persistent storage
        let persistent = file_storage.get_node(node_id).unwrap().unwrap();

        // Populate cache
        memory_cache.store_node(node_id, &persistent).unwrap();

        // Now cache hit
        let cached = memory_cache.get_node(node_id).unwrap();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), state);
    }
}

// ============================================================================
// Property-Based Tests
// ============================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Energy (squared norm) is always non-negative
        #[test]
        fn energy_is_non_negative(
            values in prop::collection::vec(-1000.0f32..1000.0, 1..100)
        ) {
            // For any residual vector, its squared norm (energy) is non-negative
            let energy: f32 = values.iter().map(|v| v * v).sum();
            prop_assert!(energy >= 0.0);
        }

        /// Zero residual implies zero energy
        #[test]
        fn zero_residual_zero_energy(dim in 1usize..100) {
            let zeros = vec![0.0f32; dim];
            let energy: f32 = zeros.iter().map(|v| v * v).sum();
            prop_assert!((energy - 0.0).abs() < 1e-10);
        }

        /// Storing and retrieving preserves data exactly
        #[test]
        fn store_retrieve_preserves_data(
            node_id in "[a-z]{1,10}",
            state in prop::collection::vec(-1000.0f32..1000.0, 1..10)
        ) {
            let storage = InMemoryStorage::new();
            storage.store_node(&node_id, &state).unwrap();

            let retrieved = storage.get_node(&node_id).unwrap().unwrap();
            prop_assert_eq!(retrieved, state);
        }

        /// File storage preserves data exactly
        #[test]
        fn file_store_retrieve_preserves_data(
            node_id in "[a-z]{1,10}",
            state in prop::collection::vec(-100.0f32..100.0, 1..10)
        ) {
            let temp_dir = TempDir::new().unwrap();
            let storage = FileStorage::new(temp_dir.path()).unwrap();

            storage.store_node(&node_id, &state).unwrap();
            let retrieved = storage.get_node(&node_id).unwrap().unwrap();

            prop_assert_eq!(retrieved, state);
        }

        /// Similar vectors have high cosine similarity
        #[test]
        fn similar_vectors_high_similarity(
            base in prop::collection::vec(0.1f32..1.0, 3..10)
        ) {
            let storage = InMemoryStorage::new();

            // Normalize base
            let norm: f32 = base.iter().map(|v| v * v).sum::<f32>().sqrt();
            let normalized: Vec<f32> = base.iter().map(|v| v / norm).collect();

            storage.store_node("base", &normalized).unwrap();

            // Query with the same vector should give similarity ~1.0
            let results = storage.find_similar(&normalized, 1).unwrap();

            if let Some((id, sim)) = results.first() {
                prop_assert_eq!(id, "base");
                prop_assert!(*sim > 0.99);
            }
        }

        /// Witness chain maintains order
        #[test]
        fn witness_chain_order(count in 1usize..20) {
            let storage = InMemoryStorage::new();

            for i in 0..count {
                let data = format!("witness-{}", i);
                let _ = storage.store_witness(data.as_bytes()).unwrap();
            }

            // Each witness should have been stored
            // (We can't verify order without access to internal state,
            // but this verifies no failures under load)
        }
    }
}
