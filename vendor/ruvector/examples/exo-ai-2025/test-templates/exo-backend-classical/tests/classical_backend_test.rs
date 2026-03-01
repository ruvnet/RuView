//! Unit tests for exo-backend-classical (ruvector integration)

#[cfg(test)]
mod substrate_backend_impl_tests {
    use super::*;
    // use exo_backend_classical::*;
    // use exo_core::{SubstrateBackend, Pattern, Filter};

    #[test]
    fn test_classical_backend_construction() {
        // Test creating classical backend
        // let config = ClassicalBackendConfig {
        //     hnsw_m: 16,
        //     hnsw_ef_construction: 200,
        //     dimension: 128,
        // };
        //
        // let backend = ClassicalBackend::new(config).unwrap();
        //
        // assert!(backend.is_initialized());
    }

    #[test]
    fn test_similarity_search_basic() {
        // Test basic similarity search
        // let backend = setup_backend();
        //
        // // Insert some vectors
        // for i in 0..100 {
        //     let vector = generate_random_vector(128);
        //     backend.insert(&vector, &metadata(i)).unwrap();
        // }
        //
        // let query = generate_random_vector(128);
        // let results = backend.similarity_search(&query, 10, None).unwrap();
        //
        // assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_similarity_search_with_filter() {
        // Test similarity search with metadata filter
        // let backend = setup_backend();
        //
        // let filter = Filter::new("category", "test");
        // let results = backend.similarity_search(&query, 10, Some(&filter)).unwrap();
        //
        // // All results should match filter
        // assert!(results.iter().all(|r| r.metadata.get("category") == Some("test")));
    }

    #[test]
    fn test_similarity_search_empty_index() {
        // Test search on empty index
        // let backend = ClassicalBackend::new(config).unwrap();
        // let query = vec![0.1, 0.2, 0.3];
        //
        // let results = backend.similarity_search(&query, 10, None).unwrap();
        //
        // assert!(results.is_empty());
    }

    #[test]
    fn test_similarity_search_k_larger_than_index() {
        // Test requesting more results than available
        // let backend = setup_backend();
        //
        // // Insert only 5 vectors
        // for i in 0..5 {
        //     backend.insert(&vector(i), &metadata(i)).unwrap();
        // }
        //
        // // Request 10
        // let results = backend.similarity_search(&query, 10, None).unwrap();
        //
        // assert_eq!(results.len(), 5);  // Should return only what's available
    }
}

#[cfg(test)]
mod manifold_deform_tests {
    use super::*;

    #[test]
    fn test_manifold_deform_as_insert() {
        // Test that manifold_deform performs discrete insert on classical backend
        // let backend = setup_backend();
        //
        // let pattern = Pattern {
        //     embedding: vec![0.1, 0.2, 0.3],
        //     metadata: Metadata::default(),
        //     timestamp: SubstrateTime::now(),
        //     antecedents: vec![],
        // };
        //
        // let delta = backend.manifold_deform(&pattern, 0.5).unwrap();
        //
        // match delta {
        //     ManifoldDelta::DiscreteInsert { id } => {
        //         assert!(backend.contains(id));
        //     }
        //     _ => panic!("Expected DiscreteInsert"),
        // }
    }

    #[test]
    fn test_manifold_deform_ignores_learning_rate() {
        // Classical backend should ignore learning_rate parameter
        // let backend = setup_backend();
        //
        // let delta1 = backend.manifold_deform(&pattern, 0.1).unwrap();
        // let delta2 = backend.manifold_deform(&pattern, 0.9).unwrap();
        //
        // // Both should perform same insert operation
    }
}

#[cfg(test)]
mod hyperedge_query_tests {
    use super::*;

    #[test]
    fn test_hyperedge_query_not_supported() {
        // Test that advanced topological queries return NotSupported
        // let backend = setup_backend();
        //
        // let query = TopologicalQuery::SheafConsistency {
        //     local_sections: vec![],
        // };
        //
        // let result = backend.hyperedge_query(&query).unwrap();
        //
        // assert!(matches!(result, HyperedgeResult::NotSupported));
    }

    #[test]
    fn test_hyperedge_query_basic_support() {
        // Test basic hyperedge operations if supported
        // May use ruvector-graph hyperedge features
    }
}

#[cfg(test)]
mod ruvector_core_integration_tests {
    use super::*;

    #[test]
    fn test_ruvector_core_hnsw() {
        // Test integration with ruvector-core HNSW index
        // let backend = ClassicalBackend::new(config).unwrap();
        //
        // // Verify HNSW parameters applied
        // assert_eq!(backend.hnsw_config().m, 16);
        // assert_eq!(backend.hnsw_config().ef_construction, 200);
    }

    #[test]
    fn test_ruvector_core_metadata() {
        // Test metadata storage via ruvector-core
    }

    #[test]
    fn test_ruvector_core_persistence() {
        // Test save/load via ruvector-core
    }
}

#[cfg(test)]
mod ruvector_graph_integration_tests {
    use super::*;

    #[test]
    fn test_ruvector_graph_database() {
        // Test GraphDatabase integration
        // let backend = setup_backend_with_graph();
        //
        // // Create entities and edges
        // let e1 = backend.graph_db.add_node(data1);
        // let e2 = backend.graph_db.add_node(data2);
        // backend.graph_db.add_edge(e1, e2, relation);
        //
        // // Query graph
        // let neighbors = backend.graph_db.neighbors(e1);
        // assert!(neighbors.contains(&e2));
    }

    #[test]
    fn test_ruvector_graph_hyperedge() {
        // Test ruvector-graph hyperedge support
    }
}

#[cfg(test)]
mod ruvector_gnn_integration_tests {
    use super::*;

    #[test]
    fn test_ruvector_gnn_layer() {
        // Test GNN layer integration
        // let backend = setup_backend_with_gnn();
        //
        // // Apply GNN layer
        // let embeddings = backend.gnn_layer.forward(&graph);
        //
        // assert!(embeddings.len() > 0);
    }

    #[test]
    fn test_ruvector_gnn_message_passing() {
        // Test message passing via GNN
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_error_conversion() {
        // Test ruvector error conversion to SubstrateBackend::Error
        // let backend = setup_backend();
        //
        // // Trigger ruvector error (e.g., invalid dimension)
        // let invalid_vector = vec![0.1];  // Wrong dimension
        // let result = backend.similarity_search(&invalid_vector, 10, None);
        //
        // assert!(result.is_err());
    }

    #[test]
    fn test_error_display() {
        // Test error display implementation
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;

    #[test]
    fn test_search_latency() {
        // Test search latency meets targets
        // let backend = setup_large_backend(100000);
        //
        // let start = Instant::now();
        // backend.similarity_search(&query, 10, None).unwrap();
        // let duration = start.elapsed();
        //
        // assert!(duration.as_millis() < 10);  // <10ms target
    }

    #[test]
    fn test_insert_throughput() {
        // Test insert throughput
        // let backend = setup_backend();
        //
        // let start = Instant::now();
        // for i in 0..10000 {
        //     backend.manifold_deform(&pattern(i), 0.5).unwrap();
        // }
        // let duration = start.elapsed();
        //
        // let throughput = 10000.0 / duration.as_secs_f64();
        // assert!(throughput > 10000.0);  // >10k ops/s target
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;

    #[test]
    fn test_memory_usage() {
        // Test memory footprint
        // let backend = setup_backend();
        //
        // let initial_mem = current_memory_usage();
        //
        // // Insert vectors
        // for i in 0..100000 {
        //     backend.manifold_deform(&pattern(i), 0.5).unwrap();
        // }
        //
        // let final_mem = current_memory_usage();
        // let mem_per_vector = (final_mem - initial_mem) / 100000;
        //
        // // Should be reasonable per-vector overhead
        // assert!(mem_per_vector < 1024);  // <1KB per vector
    }
}

#[cfg(test)]
mod concurrency_tests {
    use super::*;

    #[test]
    fn test_concurrent_searches() {
        // Test concurrent search operations
        // let backend = Arc::new(setup_backend());
        //
        // let handles: Vec<_> = (0..10).map(|_| {
        //     let backend = backend.clone();
        //     std::thread::spawn(move || {
        //         backend.similarity_search(&random_query(), 10, None).unwrap()
        //     })
        // }).collect();
        //
        // for handle in handles {
        //     let results = handle.join().unwrap();
        //     assert_eq!(results.len(), 10);
        // }
    }

    #[test]
    fn test_concurrent_inserts() {
        // Test concurrent insert operations
    }
}

#[cfg(test)]
mod edge_cases_tests {
    use super::*;

    #[test]
    fn test_zero_dimension() {
        // Test error on zero-dimension vectors
        // let config = ClassicalBackendConfig {
        //     dimension: 0,
        //     ..Default::default()
        // };
        //
        // let result = ClassicalBackend::new(config);
        // assert!(result.is_err());
    }

    #[test]
    fn test_extreme_k_values() {
        // Test with k=0 and k=usize::MAX
        // let backend = setup_backend();
        //
        // let results_zero = backend.similarity_search(&query, 0, None).unwrap();
        // assert!(results_zero.is_empty());
        //
        // let results_max = backend.similarity_search(&query, usize::MAX, None).unwrap();
        // // Should return all available results
    }

    #[test]
    fn test_nan_in_query() {
        // Test handling of NaN in query vector
        // let backend = setup_backend();
        // let query_with_nan = vec![f32::NAN, 0.2, 0.3];
        //
        // let result = backend.similarity_search(&query_with_nan, 10, None);
        // assert!(result.is_err());
    }

    #[test]
    fn test_infinity_in_query() {
        // Test handling of infinity in query vector
    }
}
