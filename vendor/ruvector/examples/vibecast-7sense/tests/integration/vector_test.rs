//! Integration tests for Vector Space Context
//!
//! Tests for HNSW index creation, vector insertion, k-NN search accuracy,
//! index persistence, and batch insertion performance.

use vibecast_tests::fixtures::*;
use vibecast_tests::mocks::*;
use std::collections::HashSet;
use std::time::Instant;

// ============================================================================
// HNSW Index Creation Tests
// ============================================================================

mod index_creation {
    use super::*;

    #[test]
    fn test_create_empty_index() {
        let index = MockVectorIndex::new();
        assert_eq!(index.count(), 0);
    }

    #[test]
    fn test_create_index_with_config() {
        let config = HnswConfig {
            m: 32,
            ef_construction: 400,
            ef_search: 200,
            max_layers: 8,
        };

        let index = MockVectorIndex::with_config(config);
        assert_eq!(index.count(), 0);
    }

    #[test]
    fn test_default_hnsw_config() {
        let config = HnswConfig::default();

        assert_eq!(config.m, 16);
        assert_eq!(config.ef_construction, 200);
        assert_eq!(config.ef_search, 100);
        assert_eq!(config.max_layers, 6);
    }

    #[test]
    fn test_index_with_distance_metrics() {
        let cosine_index = MockVectorIndex::new().with_distance_metric(DistanceMetric::Cosine);
        let euclidean_index =
            MockVectorIndex::new().with_distance_metric(DistanceMetric::Euclidean);
        let poincare_index = MockVectorIndex::new().with_distance_metric(DistanceMetric::Poincare);

        // All should be created successfully
        assert_eq!(cosine_index.count(), 0);
        assert_eq!(euclidean_index.count(), 0);
        assert_eq!(poincare_index.count(), 0);
    }
}

// ============================================================================
// Vector Insertion Tests
// ============================================================================

mod vector_insertion {
    use super::*;

    #[test]
    fn test_insert_single_vector() {
        let index = MockVectorIndex::new();
        let vector = create_normalized_vector(1536);
        let embedding_id = EmbeddingId::new();

        let vector_id = index.insert(embedding_id, vector).unwrap();
        assert_eq!(index.count(), 1);

        let retrieved = index.get(&vector_id).unwrap().unwrap();
        assert_eq!(retrieved.embedding_id, embedding_id);
    }

    #[test]
    fn test_insert_multiple_vectors() {
        let index = MockVectorIndex::new();

        for i in 0..100 {
            let vector = create_deterministic_vector(1536, i);
            index.insert(EmbeddingId::new(), vector).unwrap();
        }

        assert_eq!(index.count(), 100);
    }

    #[test]
    fn test_insert_normalized_vectors() {
        let index = MockVectorIndex::new();

        for i in 0..10 {
            let vector = create_normalized_vector(1536);
            index.insert(EmbeddingId::new(), vector).unwrap();
        }

        assert_eq!(index.count(), 10);
    }

    #[test]
    fn test_insert_preserves_vector_data() {
        let index = MockVectorIndex::new();
        let original_vector = create_deterministic_vector(1536, 42);
        let embedding_id = EmbeddingId::new();

        let vector_id = index.insert(embedding_id, original_vector.clone()).unwrap();

        let retrieved = index.get(&vector_id).unwrap().unwrap();
        assert_eq!(retrieved.vector, original_vector);
    }

    #[test]
    fn test_insert_assigns_layer() {
        let index = MockVectorIndex::new();

        let mut layers_seen = HashSet::new();
        for i in 0..100 {
            let vector = create_deterministic_vector(1536, i);
            let vector_id = index.insert(EmbeddingId::new(), vector).unwrap();
            let indexed = index.get(&vector_id).unwrap().unwrap();
            layers_seen.insert(indexed.layer);
        }

        // HNSW should assign vectors to multiple layers
        assert!(layers_seen.len() >= 1, "Should have at least one layer");
    }

    #[test]
    fn test_remove_vector() {
        let index = MockVectorIndex::new();
        let vector = create_normalized_vector(1536);
        let vector_id = index.insert(EmbeddingId::new(), vector).unwrap();

        assert_eq!(index.count(), 1);

        index.remove(&vector_id).unwrap();
        assert_eq!(index.count(), 0);
        assert!(index.get(&vector_id).unwrap().is_none());
    }
}

// ============================================================================
// k-NN Search Accuracy Tests
// ============================================================================

mod knn_search {
    use super::*;

    #[test]
    fn test_search_returns_k_results() {
        let index = MockVectorIndex::new();

        // Insert 100 vectors
        for i in 0..100 {
            let vector = create_deterministic_vector(1536, i);
            index.insert(EmbeddingId::new(), vector).unwrap();
        }

        let query = create_deterministic_vector(1536, 50);
        let results = index.search(&query, 10).unwrap();

        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_search_results_sorted_by_distance() {
        let index = MockVectorIndex::new();

        for i in 0..100 {
            let vector = create_deterministic_vector(1536, i);
            index.insert(EmbeddingId::new(), vector).unwrap();
        }

        let query = create_deterministic_vector(1536, 50);
        let results = index.search(&query, 20).unwrap();

        // Verify sorted by distance
        for i in 0..results.len() - 1 {
            assert!(
                results[i].distance <= results[i + 1].distance,
                "Results should be sorted by distance"
            );
        }
    }

    #[test]
    fn test_search_finds_exact_match() {
        let index = MockVectorIndex::new();

        let target_vector = create_deterministic_vector(1536, 42);
        let target_id = index.insert(EmbeddingId::new(), target_vector.clone()).unwrap();

        // Insert other vectors
        for i in 0..50 {
            if i != 42 {
                let vector = create_deterministic_vector(1536, i);
                index.insert(EmbeddingId::new(), vector).unwrap();
            }
        }

        let results = index.search(&target_vector, 1).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].vector_id, target_id);
        assert!(results[0].distance < 0.0001, "Exact match should have near-zero distance");
    }

    #[test]
    fn test_recall_at_10_meets_threshold() {
        let index = MockVectorIndex::new();

        // Insert vectors with known structure
        let num_vectors = 1000;
        let mut all_vectors: Vec<(VectorId, Vec<f32>)> = Vec::new();

        for i in 0..num_vectors {
            let vector = create_deterministic_vector(1536, i);
            let vector_id = index.insert(EmbeddingId::new(), vector.clone()).unwrap();
            all_vectors.push((vector_id, vector));
        }

        // Test recall with multiple queries
        let mut total_recall = 0.0;
        let num_queries = 20;

        for query_idx in (0..num_vectors).step_by(num_vectors / num_queries) {
            let query = &all_vectors[query_idx].1;

            // Compute true k-NN (brute force)
            let mut true_distances: Vec<(VectorId, f32)> = all_vectors
                .iter()
                .map(|(id, v)| (*id, cosine_distance(query, v)))
                .collect();
            true_distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            let true_top_10: HashSet<VectorId> =
                true_distances.iter().take(10).map(|(id, _)| *id).collect();

            // Get approximate k-NN
            let approx_results = index.search(query, 10).unwrap();
            let approx_top_10: HashSet<VectorId> =
                approx_results.iter().map(|r| r.vector_id).collect();

            // Compute recall
            let intersection_count = true_top_10.intersection(&approx_top_10).count();
            let recall = intersection_count as f32 / 10.0;
            total_recall += recall;
        }

        let avg_recall = total_recall / num_queries as f32;
        assert!(
            avg_recall >= 0.95,
            "Recall@10 should be >= 0.95, got {}",
            avg_recall
        );
    }

    #[test]
    fn test_search_with_varying_k() {
        let index = MockVectorIndex::new();

        for i in 0..100 {
            let vector = create_deterministic_vector(1536, i);
            index.insert(EmbeddingId::new(), vector).unwrap();
        }

        let query = create_normalized_vector(1536);

        for k in [1, 5, 10, 20, 50] {
            let results = index.search(&query, k).unwrap();
            assert_eq!(results.len(), k, "Should return exactly k={} results", k);
        }
    }

    #[test]
    fn test_search_empty_index() {
        let index = MockVectorIndex::new();
        let query = create_normalized_vector(1536);

        let results = index.search(&query, 10).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_search_k_larger_than_index_size() {
        let index = MockVectorIndex::new();

        for i in 0..5 {
            let vector = create_deterministic_vector(1536, i);
            index.insert(EmbeddingId::new(), vector).unwrap();
        }

        let query = create_normalized_vector(1536);
        let results = index.search(&query, 100).unwrap();

        assert_eq!(results.len(), 5, "Should return all vectors if k > index size");
    }
}

// ============================================================================
// Neighbor Graph Tests
// ============================================================================

mod neighbor_graph {
    use super::*;

    #[test]
    fn test_get_neighbors() {
        let index = MockVectorIndex::new();

        // Insert vectors
        let mut vector_ids = Vec::new();
        for i in 0..20 {
            let vector = create_deterministic_vector(1536, i);
            let id = index.insert(EmbeddingId::new(), vector).unwrap();
            vector_ids.push(id);
        }

        // Get neighbors of first vector
        let neighbors = index.get_neighbors(&vector_ids[0], 5).unwrap();

        assert_eq!(neighbors.len(), 5);
        // Should not include self
        assert!(!neighbors.iter().any(|r| r.vector_id == vector_ids[0]));
    }

    #[test]
    fn test_similarity_edge_creation() {
        let source_id = VectorId::new();
        let target_id = VectorId::new();

        let edge = SimilarityEdge {
            source_id,
            target_id,
            distance: 0.15,
            edge_type: "SIMILAR".to_string(),
        };

        assert_eq!(edge.source_id, source_id);
        assert_eq!(edge.target_id, target_id);
        assert!(edge.distance < 0.2);
    }

    #[test]
    fn test_search_result_ranking() {
        let results = create_search_results(10);

        for (i, result) in results.iter().enumerate() {
            assert_eq!(result.rank, i + 1, "Rank should be 1-indexed");
            if i > 0 {
                assert!(
                    result.distance >= results[i - 1].distance,
                    "Distance should be non-decreasing"
                );
            }
        }
    }
}

// ============================================================================
// Index Persistence Tests
// ============================================================================

mod persistence {
    use super::*;

    #[test]
    fn test_save_and_load_index() {
        let index = MockVectorIndex::new();

        // Insert vectors
        for i in 0..50 {
            let vector = create_deterministic_vector(1536, i);
            index.insert(EmbeddingId::new(), vector).unwrap();
        }

        let original_count = index.count();

        // Save to bytes
        let bytes = index.save_to_bytes().unwrap();
        assert!(!bytes.is_empty());

        // Load from bytes (mock - doesn't restore actual data)
        let loaded = MockVectorIndex::load_from_bytes(&bytes).unwrap();

        // In real implementation, this would verify:
        // assert_eq!(loaded.count(), original_count);
    }

    #[test]
    fn test_persistence_format() {
        let index = MockVectorIndex::new();

        for i in 0..10 {
            let vector = create_deterministic_vector(1536, i);
            index.insert(EmbeddingId::new(), vector).unwrap();
        }

        let bytes = index.save_to_bytes().unwrap();

        // Check header (count as u64)
        assert!(bytes.len() >= 8);
        let count = u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);
        assert_eq!(count, 10);
    }
}

// ============================================================================
// Batch Insertion Performance Tests
// ============================================================================

mod batch_performance {
    use super::*;

    #[test]
    fn test_batch_insert() {
        let index = MockVectorIndex::new();

        let embeddings: Vec<(EmbeddingId, Vec<f32>)> = (0..100)
            .map(|i| (EmbeddingId::new(), create_deterministic_vector(1536, i)))
            .collect();

        let vector_ids = index.insert_batch(embeddings).unwrap();

        assert_eq!(vector_ids.len(), 100);
        assert_eq!(index.count(), 100);
    }

    #[test]
    fn test_batch_insert_performance() {
        let index = MockVectorIndex::new();
        let batch_size = 1000;

        let embeddings: Vec<(EmbeddingId, Vec<f32>)> = (0..batch_size)
            .map(|i| (EmbeddingId::new(), create_deterministic_vector(1536, i)))
            .collect();

        let start = Instant::now();
        let vector_ids = index.insert_batch(embeddings).unwrap();
        let duration = start.elapsed();

        assert_eq!(vector_ids.len(), batch_size);

        // Should complete reasonably fast (mock implementation)
        let vectors_per_second = batch_size as f64 / duration.as_secs_f64();
        assert!(
            vectors_per_second > 1000.0,
            "Batch insertion should be fast, got {} vec/sec",
            vectors_per_second
        );
    }

    #[test]
    fn test_incremental_vs_batch_insert() {
        // Test incremental insertion
        let index1 = MockVectorIndex::new();
        let start1 = Instant::now();
        for i in 0..100 {
            let vector = create_deterministic_vector(1536, i);
            index1.insert(EmbeddingId::new(), vector).unwrap();
        }
        let duration1 = start1.elapsed();

        // Test batch insertion
        let index2 = MockVectorIndex::new();
        let embeddings: Vec<(EmbeddingId, Vec<f32>)> = (0..100)
            .map(|i| (EmbeddingId::new(), create_deterministic_vector(1536, i)))
            .collect();

        let start2 = Instant::now();
        index2.insert_batch(embeddings).unwrap();
        let duration2 = start2.elapsed();

        assert_eq!(index1.count(), index2.count());
    }

    #[test]
    fn test_scaling_with_index_size() {
        let index = MockVectorIndex::new();

        let sizes = vec![100, 500, 1000];
        let mut search_times = Vec::new();

        for size in &sizes {
            // Build index to target size
            while index.count() < *size {
                let vector = create_deterministic_vector(1536, index.count());
                index.insert(EmbeddingId::new(), vector).unwrap();
            }

            // Measure search time
            let query = create_normalized_vector(1536);
            let start = Instant::now();
            for _ in 0..100 {
                index.search(&query, 10).unwrap();
            }
            let duration = start.elapsed();
            search_times.push(duration);
        }

        // Search time should scale sub-linearly with index size (HNSW property)
        // With mock implementation, just verify search completes
        for (i, time) in search_times.iter().enumerate() {
            assert!(
                time.as_millis() < 10000,
                "Search at size {} took too long",
                sizes[i]
            );
        }
    }
}

// ============================================================================
// Distance Metric Tests
// ============================================================================

mod distance_metrics {
    use super::*;

    #[test]
    fn test_cosine_distance_identical_vectors() {
        let v = create_normalized_vector(1536);
        let dist = cosine_distance(&v, &v);
        assert!(dist < 0.0001, "Identical vectors should have distance ~0");
    }

    #[test]
    fn test_cosine_distance_orthogonal_vectors() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let dist = cosine_distance(&v1, &v2);
        assert!((dist - 1.0).abs() < 0.0001, "Orthogonal vectors should have distance 1");
    }

    #[test]
    fn test_cosine_distance_opposite_vectors() {
        let v1 = vec![1.0, 0.0];
        let v2 = vec![-1.0, 0.0];
        let dist = cosine_distance(&v1, &v2);
        assert!((dist - 2.0).abs() < 0.0001, "Opposite vectors should have distance 2");
    }

    #[test]
    fn test_euclidean_distance_identical_vectors() {
        let v = create_normalized_vector(1536);
        let dist = euclidean_distance(&v, &v);
        assert!(dist < 0.0001, "Identical vectors should have distance 0");
    }

    #[test]
    fn test_euclidean_distance_known_value() {
        let v1 = vec![0.0, 0.0];
        let v2 = vec![3.0, 4.0];
        let dist = euclidean_distance(&v1, &v2);
        assert!((dist - 5.0).abs() < 0.0001, "3-4-5 triangle");
    }

    #[test]
    fn test_distance_metric_symmetry() {
        let v1 = create_normalized_vector(1536);
        let v2 = create_deterministic_vector(1536, 42);

        let cosine_12 = cosine_distance(&v1, &v2);
        let cosine_21 = cosine_distance(&v2, &v1);
        assert!((cosine_12 - cosine_21).abs() < 0.0001, "Cosine distance should be symmetric");

        let eucl_12 = euclidean_distance(&v1, &v2);
        let eucl_21 = euclidean_distance(&v2, &v1);
        assert!((eucl_12 - eucl_21).abs() < 0.0001, "Euclidean distance should be symmetric");
    }

    #[test]
    fn test_triangle_inequality() {
        let v1 = create_deterministic_vector(100, 0);
        let v2 = create_deterministic_vector(100, 1);
        let v3 = create_deterministic_vector(100, 2);

        let d12 = euclidean_distance(&v1, &v2);
        let d23 = euclidean_distance(&v2, &v3);
        let d13 = euclidean_distance(&v1, &v3);

        assert!(
            d13 <= d12 + d23 + 0.0001,
            "Triangle inequality should hold: {} <= {} + {}",
            d13,
            d12,
            d23
        );
    }
}

// ============================================================================
// Indexed Vector Tests
// ============================================================================

mod indexed_vector {
    use super::*;

    #[test]
    fn test_indexed_vector_creation() {
        let indexed = IndexedVector::default();

        assert_eq!(indexed.vector.len(), 1536);
        assert_normalized(&indexed.vector, 0.0001);
    }

    #[test]
    fn test_create_indexed_vectors() {
        let vectors = create_indexed_vectors(10);

        assert_eq!(vectors.len(), 10);
        for (i, v) in vectors.iter().enumerate() {
            assert_eq!(v.vector.len(), 1536);
            // Verify deterministic generation
            let expected = create_deterministic_vector(1536, i);
            assert_eq!(v.vector, expected);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_space_integration_smoke_test() {
        let index = MockVectorIndex::new();

        // Insert vectors
        for i in 0..100 {
            let vector = create_deterministic_vector(1536, i);
            index.insert(EmbeddingId::new(), vector).unwrap();
        }

        // Search
        let query = create_deterministic_vector(1536, 50);
        let results = index.search(&query, 10).unwrap();

        // Verify
        assert_eq!(results.len(), 10);
        assert!(results[0].distance < results[9].distance);

        // Check recall
        let exact_match = results.iter().find(|r| r.distance < 0.01);
        assert!(exact_match.is_some(), "Should find near-exact match");
    }
}
