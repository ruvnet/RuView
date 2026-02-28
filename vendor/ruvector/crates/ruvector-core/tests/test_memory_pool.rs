//! Memory Pool and Allocation Tests
//!
//! This module tests the arena allocator and cache-optimized storage
//! for correct memory management, eviction, and performance characteristics.

use ruvector_core::arena::{Arena, ArenaVec};
use ruvector_core::cache_optimized::SoAVectorStorage;
use std::sync::{Arc, Barrier};
use std::thread;

// ============================================================================
// Arena Allocator Tests
// ============================================================================

mod arena_tests {
    use super::*;

    #[test]
    fn test_arena_basic_allocation() {
        let arena = Arena::new(1024);
        let mut vec: ArenaVec<f32> = arena.alloc_vec(10);

        assert_eq!(vec.capacity(), 10);
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());

        vec.push(1.0);
        vec.push(2.0);
        vec.push(3.0);

        assert_eq!(vec.len(), 3);
        assert!(!vec.is_empty());
        assert_eq!(vec[0], 1.0);
        assert_eq!(vec[1], 2.0);
        assert_eq!(vec[2], 3.0);
    }

    #[test]
    fn test_arena_multiple_allocations() {
        let arena = Arena::new(4096);

        let vec1: ArenaVec<f32> = arena.alloc_vec(100);
        let vec2: ArenaVec<f64> = arena.alloc_vec(50);
        let vec3: ArenaVec<u32> = arena.alloc_vec(200);
        let vec4: ArenaVec<i64> = arena.alloc_vec(75);

        assert_eq!(vec1.capacity(), 100);
        assert_eq!(vec2.capacity(), 50);
        assert_eq!(vec3.capacity(), 200);
        assert_eq!(vec4.capacity(), 75);
    }

    #[test]
    fn test_arena_different_types() {
        let arena = Arena::new(2048);

        // Allocate different types
        let mut floats: ArenaVec<f32> = arena.alloc_vec(10);
        let mut doubles: ArenaVec<f64> = arena.alloc_vec(10);
        let mut ints: ArenaVec<i32> = arena.alloc_vec(10);
        let mut bytes: ArenaVec<u8> = arena.alloc_vec(10);

        // Push values
        for i in 0..10 {
            floats.push(i as f32);
            doubles.push(i as f64);
            ints.push(i);
            bytes.push(i as u8);
        }

        // Verify
        for i in 0..10 {
            assert_eq!(floats[i], i as f32);
            assert_eq!(doubles[i], i as f64);
            assert_eq!(ints[i], i as i32);
            assert_eq!(bytes[i], i as u8);
        }
    }

    #[test]
    fn test_arena_reset() {
        let arena = Arena::new(4096);

        // First allocation cycle
        {
            let mut vec1: ArenaVec<f32> = arena.alloc_vec(100);
            let mut vec2: ArenaVec<f32> = arena.alloc_vec(100);

            for i in 0..50 {
                vec1.push(i as f32);
                vec2.push(i as f32 * 2.0);
            }
        }

        let used_before = arena.used_bytes();
        assert!(used_before > 0, "Should have used some bytes");

        arena.reset();

        let used_after = arena.used_bytes();
        assert_eq!(used_after, 0, "Reset should set used bytes to 0");

        // Allocated bytes should remain (memory is reused, not freed)
        let allocated = arena.allocated_bytes();
        assert!(allocated > 0, "Allocated bytes should remain after reset");

        // Second allocation cycle - should reuse memory
        let mut vec3: ArenaVec<f32> = arena.alloc_vec(50);
        for i in 0..50 {
            vec3.push(i as f32);
        }

        // Memory was reused
        assert!(
            arena.allocated_bytes() == allocated,
            "Should reuse existing allocation"
        );
    }

    #[test]
    fn test_arena_chunk_growth() {
        // Small initial chunk size to force growth
        let arena = Arena::new(64);

        // Allocate more than fits in one chunk
        let vec1: ArenaVec<f32> = arena.alloc_vec(100);
        let vec2: ArenaVec<f32> = arena.alloc_vec(100);
        let vec3: ArenaVec<f32> = arena.alloc_vec(100);

        assert_eq!(vec1.capacity(), 100);
        assert_eq!(vec2.capacity(), 100);
        assert_eq!(vec3.capacity(), 100);

        // Should have allocated multiple chunks
        let allocated = arena.allocated_bytes();
        assert!(allocated > 64 * 3, "Should have grown beyond initial chunk");
    }

    #[test]
    fn test_arena_as_slice() {
        let arena = Arena::new(1024);
        let mut vec: ArenaVec<f32> = arena.alloc_vec(10);

        for i in 0..5 {
            vec.push((i * 10) as f32);
        }

        let slice = vec.as_slice();
        assert_eq!(slice, &[0.0, 10.0, 20.0, 30.0, 40.0]);
    }

    #[test]
    fn test_arena_as_mut_slice() {
        let arena = Arena::new(1024);
        let mut vec: ArenaVec<f32> = arena.alloc_vec(10);

        for i in 0..5 {
            vec.push((i * 10) as f32);
        }

        {
            let slice = vec.as_mut_slice();
            slice[0] = 100.0;
            slice[4] = 500.0;
        }

        assert_eq!(vec[0], 100.0);
        assert_eq!(vec[4], 500.0);
    }

    #[test]
    fn test_arena_deref() {
        let arena = Arena::new(1024);
        let mut vec: ArenaVec<f32> = arena.alloc_vec(10);

        vec.push(1.0);
        vec.push(2.0);
        vec.push(3.0);

        // Test Deref trait (can use slice methods)
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.iter().sum::<f32>(), 6.0);
    }

    #[test]
    fn test_arena_large_allocation() {
        let arena = Arena::new(1024);

        // Allocate something larger than the chunk size
        let large_vec: ArenaVec<f32> = arena.alloc_vec(10000);
        assert_eq!(large_vec.capacity(), 10000);

        // Should have grown to accommodate
        assert!(arena.allocated_bytes() >= 10000 * std::mem::size_of::<f32>());
    }

    #[test]
    fn test_arena_statistics() {
        let arena = Arena::new(1024);

        let initial_allocated = arena.allocated_bytes();
        let initial_used = arena.used_bytes();

        assert_eq!(initial_allocated, 0);
        assert_eq!(initial_used, 0);

        let _vec: ArenaVec<f32> = arena.alloc_vec(100);

        assert!(arena.allocated_bytes() > 0);
        assert!(arena.used_bytes() > 0);
    }

    #[test]
    #[should_panic(expected = "ArenaVec capacity exceeded")]
    fn test_arena_capacity_exceeded() {
        let arena = Arena::new(1024);
        let mut vec: ArenaVec<f32> = arena.alloc_vec(5);

        // Push more than capacity
        for i in 0..10 {
            vec.push(i as f32);
        }
    }

    #[test]
    fn test_arena_with_default_chunk_size() {
        let arena = Arena::with_default_chunk_size();

        // Default is 1MB
        let _vec: ArenaVec<f32> = arena.alloc_vec(1000);
        assert!(arena.allocated_bytes() >= 1024 * 1024);
    }
}

// ============================================================================
// Cache-Optimized Storage (SoA) Tests
// ============================================================================

mod soa_tests {
    use super::*;

    #[test]
    fn test_soa_basic_operations() {
        let mut storage = SoAVectorStorage::new(3, 4);

        assert_eq!(storage.len(), 0);
        assert!(storage.is_empty());
        assert_eq!(storage.dimensions(), 3);

        storage.push(&[1.0, 2.0, 3.0]);
        storage.push(&[4.0, 5.0, 6.0]);

        assert_eq!(storage.len(), 2);
        assert!(!storage.is_empty());
    }

    #[test]
    fn test_soa_get_vector() {
        let mut storage = SoAVectorStorage::new(4, 8);

        storage.push(&[1.0, 2.0, 3.0, 4.0]);
        storage.push(&[5.0, 6.0, 7.0, 8.0]);
        storage.push(&[9.0, 10.0, 11.0, 12.0]);

        let mut output = vec![0.0; 4];

        storage.get(0, &mut output);
        assert_eq!(output, vec![1.0, 2.0, 3.0, 4.0]);

        storage.get(1, &mut output);
        assert_eq!(output, vec![5.0, 6.0, 7.0, 8.0]);

        storage.get(2, &mut output);
        assert_eq!(output, vec![9.0, 10.0, 11.0, 12.0]);
    }

    #[test]
    fn test_soa_dimension_slice() {
        let mut storage = SoAVectorStorage::new(3, 8);

        storage.push(&[1.0, 10.0, 100.0]);
        storage.push(&[2.0, 20.0, 200.0]);
        storage.push(&[3.0, 30.0, 300.0]);
        storage.push(&[4.0, 40.0, 400.0]);

        // Dimension 0: all first elements
        let dim0 = storage.dimension_slice(0);
        assert_eq!(dim0, &[1.0, 2.0, 3.0, 4.0]);

        // Dimension 1: all second elements
        let dim1 = storage.dimension_slice(1);
        assert_eq!(dim1, &[10.0, 20.0, 30.0, 40.0]);

        // Dimension 2: all third elements
        let dim2 = storage.dimension_slice(2);
        assert_eq!(dim2, &[100.0, 200.0, 300.0, 400.0]);
    }

    #[test]
    fn test_soa_dimension_slice_mut() {
        let mut storage = SoAVectorStorage::new(3, 8);

        storage.push(&[1.0, 2.0, 3.0]);
        storage.push(&[4.0, 5.0, 6.0]);

        // Modify dimension 0
        {
            let dim0 = storage.dimension_slice_mut(0);
            dim0[0] = 100.0;
            dim0[1] = 400.0;
        }

        let mut output = vec![0.0; 3];
        storage.get(0, &mut output);
        assert_eq!(output, vec![100.0, 2.0, 3.0]);

        storage.get(1, &mut output);
        assert_eq!(output, vec![400.0, 5.0, 6.0]);
    }

    #[test]
    fn test_soa_auto_growth() {
        // Start with small capacity
        let mut storage = SoAVectorStorage::new(4, 2);

        // Push more vectors than initial capacity
        for i in 0..100 {
            storage.push(&[i as f32, (i * 2) as f32, (i * 3) as f32, (i * 4) as f32]);
        }

        assert_eq!(storage.len(), 100);

        // Verify all values are correct
        let mut output = vec![0.0; 4];
        for i in 0..100 {
            storage.get(i, &mut output);
            assert_eq!(
                output,
                vec![i as f32, (i * 2) as f32, (i * 3) as f32, (i * 4) as f32]
            );
        }
    }

    #[test]
    fn test_soa_batch_euclidean_distances() {
        let mut storage = SoAVectorStorage::new(3, 4);

        // Add orthogonal unit vectors
        storage.push(&[1.0, 0.0, 0.0]);
        storage.push(&[0.0, 1.0, 0.0]);
        storage.push(&[0.0, 0.0, 1.0]);

        let query = vec![1.0, 0.0, 0.0];
        let mut distances = vec![0.0; 3];

        storage.batch_euclidean_distances(&query, &mut distances);

        // Distance to itself should be 0
        assert!(distances[0] < 0.001, "Distance to self should be ~0");

        // Distance to orthogonal vectors should be sqrt(2)
        let sqrt2 = (2.0_f32).sqrt();
        assert!(
            (distances[1] - sqrt2).abs() < 0.01,
            "Expected sqrt(2), got {}",
            distances[1]
        );
        assert!(
            (distances[2] - sqrt2).abs() < 0.01,
            "Expected sqrt(2), got {}",
            distances[2]
        );
    }

    #[test]
    fn test_soa_batch_distances_large() {
        let dim = 128;
        let num_vectors = 1000;

        let mut storage = SoAVectorStorage::new(dim, 16);

        // Add random-ish vectors
        for i in 0..num_vectors {
            let vec: Vec<f32> = (0..dim)
                .map(|j| ((i * dim + j) % 100) as f32 * 0.01)
                .collect();
            storage.push(&vec);
        }

        let query: Vec<f32> = (0..dim).map(|j| (j % 50) as f32 * 0.02).collect();
        let mut distances = vec![0.0; num_vectors];

        storage.batch_euclidean_distances(&query, &mut distances);

        // Verify all distances are non-negative and finite
        for (i, &dist) in distances.iter().enumerate() {
            assert!(
                dist >= 0.0 && dist.is_finite(),
                "Distance {} is invalid: {}",
                i,
                dist
            );
        }
    }

    #[test]
    fn test_soa_common_embedding_dimensions() {
        // Test common embedding dimensions
        for dim in [128, 256, 384, 512, 768, 1024, 1536] {
            let mut storage = SoAVectorStorage::new(dim, 4);

            let vec: Vec<f32> = (0..dim).map(|i| i as f32 * 0.001).collect();
            storage.push(&vec);

            let mut output = vec![0.0; dim];
            storage.get(0, &mut output);

            assert_eq!(output, vec);
        }
    }

    #[test]
    #[should_panic(expected = "dimensions must be between")]
    fn test_soa_zero_dimensions() {
        let _ = SoAVectorStorage::new(0, 4);
    }

    #[test]
    #[should_panic]
    fn test_soa_wrong_vector_length() {
        let mut storage = SoAVectorStorage::new(3, 4);
        storage.push(&[1.0, 2.0]); // Wrong dimension
    }

    #[test]
    #[should_panic]
    fn test_soa_get_out_of_bounds() {
        let storage = SoAVectorStorage::new(3, 4);
        let mut output = vec![0.0; 3];
        storage.get(0, &mut output); // No vectors added
    }

    #[test]
    #[should_panic]
    fn test_soa_dimension_slice_out_of_bounds() {
        let mut storage = SoAVectorStorage::new(3, 4);
        storage.push(&[1.0, 2.0, 3.0]);
        let _ = storage.dimension_slice(5); // Invalid dimension
    }
}

// ============================================================================
// Memory Pressure Tests
// ============================================================================

mod memory_pressure_tests {
    use super::*;

    #[test]
    fn test_arena_many_small_allocations() {
        let arena = Arena::new(1024 * 1024); // 1MB

        // Many small allocations
        for _ in 0..10000 {
            let _vec: ArenaVec<f32> = arena.alloc_vec(10);
        }

        // Should handle without issues
        assert!(arena.allocated_bytes() > 0);
    }

    #[test]
    fn test_arena_alternating_sizes() {
        let arena = Arena::new(4096);

        for i in 0..100 {
            let size = if i % 2 == 0 { 10 } else { 1000 };
            let _vec: ArenaVec<f32> = arena.alloc_vec(size);
        }
    }

    #[test]
    fn test_soa_large_capacity() {
        let mut storage = SoAVectorStorage::new(128, 10000);

        for i in 0..10000 {
            let vec: Vec<f32> = (0..128).map(|j| (i * 128 + j) as f32 * 0.0001).collect();
            storage.push(&vec);
        }

        assert_eq!(storage.len(), 10000);

        // Verify random access
        let mut output = vec![0.0; 128];
        storage.get(5000, &mut output);
        assert!((output[0] - (5000 * 128) as f32 * 0.0001).abs() < 0.0001);
    }

    #[test]
    fn test_soa_batch_operations_under_pressure() {
        let dim = 512;
        let num_vectors = 5000;

        let mut storage = SoAVectorStorage::new(dim, 128);

        for i in 0..num_vectors {
            let vec: Vec<f32> = (0..dim).map(|j| ((i + j) % 1000) as f32 * 0.001).collect();
            storage.push(&vec);
        }

        // Perform batch distance calculations
        let query: Vec<f32> = (0..dim).map(|j| (j % 500) as f32 * 0.002).collect();
        let mut distances = vec![0.0; num_vectors];

        storage.batch_euclidean_distances(&query, &mut distances);

        // All distances should be valid
        for dist in &distances {
            assert!(dist.is_finite() && *dist >= 0.0);
        }
    }
}

// ============================================================================
// Concurrent Access Tests
// ============================================================================

mod concurrent_tests {
    use super::*;

    #[test]
    fn test_soa_concurrent_reads() {
        // Create and populate storage
        let mut storage = SoAVectorStorage::new(64, 16);

        for i in 0..1000 {
            let vec: Vec<f32> = (0..64).map(|j| (i * 64 + j) as f32 * 0.01).collect();
            storage.push(&vec);
        }

        let storage = Arc::new(storage);
        let num_threads = 8;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let storage_clone = Arc::clone(&storage);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                // Each thread performs many reads
                for i in 0..100 {
                    let idx = (thread_id * 100 + i) % 1000;

                    // Read dimension slices
                    let dim_slice = storage_clone.dimension_slice(idx % 64);
                    assert!(!dim_slice.is_empty());
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }
    }

    #[test]
    fn test_soa_concurrent_batch_distances() {
        let mut storage = SoAVectorStorage::new(32, 16);

        for i in 0..500 {
            let vec: Vec<f32> = (0..32).map(|j| (i * 32 + j) as f32 * 0.01).collect();
            storage.push(&vec);
        }

        let storage = Arc::new(storage);
        let num_threads = 4;
        let barrier = Arc::new(Barrier::new(num_threads));
        let mut handles = vec![];

        for thread_id in 0..num_threads {
            let storage_clone = Arc::clone(&storage);
            let barrier_clone = Arc::clone(&barrier);

            let handle = thread::spawn(move || {
                barrier_clone.wait();

                for i in 0..50 {
                    let query: Vec<f32> = (0..32)
                        .map(|j| ((thread_id * 50 + i) * 32 + j) as f32 * 0.01)
                        .collect();
                    let mut distances = vec![0.0; 500];

                    storage_clone.batch_euclidean_distances(&query, &mut distances);

                    // Verify results
                    for dist in &distances {
                        assert!(dist.is_finite());
                    }
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }
    }
}

// ============================================================================
// Edge Cases
// ============================================================================

mod edge_cases {
    use super::*;

    #[test]
    fn test_soa_single_vector() {
        let mut storage = SoAVectorStorage::new(3, 1);
        storage.push(&[1.0, 2.0, 3.0]);

        assert_eq!(storage.len(), 1);

        let mut output = vec![0.0; 3];
        storage.get(0, &mut output);
        assert_eq!(output, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_soa_single_dimension() {
        let mut storage = SoAVectorStorage::new(1, 4);

        storage.push(&[1.0]);
        storage.push(&[2.0]);
        storage.push(&[3.0]);

        let dim0 = storage.dimension_slice(0);
        assert_eq!(dim0, &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_arena_exact_capacity() {
        let arena = Arena::new(1024);
        let mut vec: ArenaVec<f32> = arena.alloc_vec(5);

        // Fill to exactly capacity
        for i in 0..5 {
            vec.push(i as f32);
        }

        assert_eq!(vec.len(), 5);
        assert_eq!(vec.capacity(), 5);
    }

    #[test]
    fn test_soa_zeros() {
        let mut storage = SoAVectorStorage::new(4, 4);

        storage.push(&[0.0, 0.0, 0.0, 0.0]);
        storage.push(&[0.0, 0.0, 0.0, 0.0]);

        let query = vec![0.0; 4];
        let mut distances = vec![0.0; 2];

        storage.batch_euclidean_distances(&query, &mut distances);

        assert!(distances[0] < 1e-6);
        assert!(distances[1] < 1e-6);
    }

    #[test]
    fn test_soa_negative_values() {
        let mut storage = SoAVectorStorage::new(3, 4);

        storage.push(&[-1.0, -2.0, -3.0]);
        storage.push(&[-4.0, -5.0, -6.0]);

        let mut output = vec![0.0; 3];
        storage.get(0, &mut output);
        assert_eq!(output, vec![-1.0, -2.0, -3.0]);
    }
}

// ============================================================================
// Performance Characteristics Tests
// ============================================================================

mod performance_tests {
    use super::*;

    #[test]
    fn test_arena_allocation_performance() {
        // This test verifies that arena allocation is efficient
        let arena = Arena::new(1024 * 1024); // 1MB

        let start = std::time::Instant::now();

        for _ in 0..100000 {
            let _vec: ArenaVec<f32> = arena.alloc_vec(10);
        }

        let duration = start.elapsed();

        // Should complete quickly (< 1 second for 100k allocations)
        assert!(
            duration.as_millis() < 1000,
            "Arena allocation took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_soa_dimension_access_pattern() {
        let mut storage = SoAVectorStorage::new(128, 16);

        for i in 0..1000 {
            let vec: Vec<f32> = (0..128).map(|j| (i * 128 + j) as f32).collect();
            storage.push(&vec);
        }

        // Test dimension-wise access (this should be cache-efficient)
        let start = std::time::Instant::now();

        for dim in 0..128 {
            let slice = storage.dimension_slice(dim);
            let _sum: f32 = slice.iter().sum();
        }

        let duration = start.elapsed();

        // Dimension-wise access should be fast due to cache locality
        assert!(
            duration.as_millis() < 100,
            "Dimension access took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_soa_batch_distance_performance() {
        let mut storage = SoAVectorStorage::new(128, 128);

        for i in 0..1000 {
            let vec: Vec<f32> = (0..128).map(|j| (i * 128 + j) as f32 * 0.001).collect();
            storage.push(&vec);
        }

        let query: Vec<f32> = (0..128).map(|j| j as f32 * 0.001).collect();
        let mut distances = vec![0.0; 1000];

        let start = std::time::Instant::now();

        for _ in 0..100 {
            storage.batch_euclidean_distances(&query, &mut distances);
        }

        let duration = start.elapsed();

        // 100 batch operations on 1000 vectors should be fast
        assert!(
            duration.as_millis() < 500,
            "Batch distance took too long: {:?}",
            duration
        );
    }
}
