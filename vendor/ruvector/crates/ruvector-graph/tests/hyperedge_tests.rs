//! Hyperedge (N-ary relationship) tests
//!
//! Tests for hypergraph features supporting relationships between multiple nodes.
//! Based on the existing hypergraph implementation in ruvector-core.

use ruvector_core::advanced::hypergraph::{
    Hyperedge, HypergraphIndex, TemporalGranularity, TemporalHyperedge,
};
use ruvector_core::types::DistanceMetric;

#[test]
fn test_create_binary_hyperedge() {
    let edge = Hyperedge::new(
        vec!["1".to_string(), "2".to_string()],
        "Alice knows Bob".to_string(),
        vec![0.1, 0.2, 0.3],
        0.95,
    );

    assert_eq!(edge.order(), 2);
    assert!(edge.contains_node(&"1".to_string()));
    assert!(edge.contains_node(&"2".to_string()));
    assert!(!edge.contains_node(&"3".to_string()));
}

#[test]
fn test_create_ternary_hyperedge() {
    let edge = Hyperedge::new(
        vec!["1".to_string(), "2".to_string(), "3".to_string()],
        "Meeting between Alice, Bob, and Charlie".to_string(),
        vec![0.5; 128],
        0.90,
    );

    assert_eq!(edge.order(), 3);
    assert!(edge.contains_node(&"1".to_string()));
    assert!(edge.contains_node(&"2".to_string()));
    assert!(edge.contains_node(&"3".to_string()));
}

#[test]
fn test_create_large_hyperedge() {
    let nodes: Vec<String> = (0..100).map(|i| i.to_string()).collect();
    let edge = Hyperedge::new(
        nodes.clone(),
        "Large group collaboration".to_string(),
        vec![0.1; 64],
        0.75,
    );

    assert_eq!(edge.order(), 100);
    for node in nodes {
        assert!(edge.contains_node(&node));
    }
}

#[test]
fn test_hyperedge_confidence_clamping() {
    let edge1 = Hyperedge::new(
        vec!["1".to_string(), "2".to_string()],
        "Test".to_string(),
        vec![0.1],
        1.5,
    );
    assert_eq!(edge1.confidence, 1.0);

    let edge2 = Hyperedge::new(
        vec!["1".to_string(), "2".to_string()],
        "Test".to_string(),
        vec![0.1],
        -0.5,
    );
    assert_eq!(edge2.confidence, 0.0);

    let edge3 = Hyperedge::new(
        vec!["1".to_string(), "2".to_string()],
        "Test".to_string(),
        vec![0.1],
        0.75,
    );
    assert_eq!(edge3.confidence, 0.75);
}

#[test]
fn test_temporal_hyperedge_creation() {
    let edge = Hyperedge::new(
        vec!["1".to_string(), "2".to_string(), "3".to_string()],
        "Temporal relationship".to_string(),
        vec![0.5; 32],
        0.9,
    );

    let temporal = TemporalHyperedge::new(edge, TemporalGranularity::Hourly);

    assert!(!temporal.is_expired());
    assert!(temporal.timestamp > 0);
    assert!(temporal.time_bucket() > 0);
}

#[test]
fn test_temporal_granularity_bucketing() {
    let edge = Hyperedge::new(
        vec!["1".to_string(), "2".to_string()],
        "Test".to_string(),
        vec![0.1],
        1.0,
    );

    let hourly = TemporalHyperedge::new(edge.clone(), TemporalGranularity::Hourly);
    let daily = TemporalHyperedge::new(edge.clone(), TemporalGranularity::Daily);
    let monthly = TemporalHyperedge::new(edge.clone(), TemporalGranularity::Monthly);

    // Different granularities should produce different buckets
    assert!(hourly.time_bucket() >= daily.time_bucket());
    assert!(daily.time_bucket() >= monthly.time_bucket());
}

#[test]
fn test_hypergraph_index_basic() {
    let mut index = HypergraphIndex::new(DistanceMetric::Cosine);

    // Add entities
    index.add_entity("1".to_string(), vec![1.0, 0.0, 0.0]);
    index.add_entity("2".to_string(), vec![0.0, 1.0, 0.0]);
    index.add_entity("3".to_string(), vec![0.0, 0.0, 1.0]);

    // Add hyperedge
    let edge = Hyperedge::new(
        vec!["1".to_string(), "2".to_string(), "3".to_string()],
        "Triangle relationship".to_string(),
        vec![0.33, 0.33, 0.34],
        0.95,
    );

    index.add_hyperedge(edge).unwrap();

    let stats = index.stats();
    assert_eq!(stats.total_entities, 3);
    assert_eq!(stats.total_hyperedges, 1);
}

#[test]
fn test_hypergraph_multiple_hyperedges() {
    let mut index = HypergraphIndex::new(DistanceMetric::Euclidean);

    // Add entities
    for i in 1..=5 {
        index.add_entity(i.to_string(), vec![i as f32; 64]);
    }

    // Add multiple hyperedges with different orders
    let edge1 = Hyperedge::new(
        vec!["1".to_string(), "2".to_string()],
        "Binary".to_string(),
        vec![0.5; 64],
        1.0,
    );
    let edge2 = Hyperedge::new(
        vec!["1".to_string(), "2".to_string(), "3".to_string()],
        "Ternary".to_string(),
        vec![0.5; 64],
        1.0,
    );
    let edge3 = Hyperedge::new(
        vec![
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
        ],
        "Quaternary".to_string(),
        vec![0.5; 64],
        1.0,
    );
    let edge4 = Hyperedge::new(
        vec![
            "1".to_string(),
            "2".to_string(),
            "3".to_string(),
            "4".to_string(),
            "5".to_string(),
        ],
        "Quinary".to_string(),
        vec![0.5; 64],
        1.0,
    );

    index.add_hyperedge(edge1).unwrap();
    index.add_hyperedge(edge2).unwrap();
    index.add_hyperedge(edge3).unwrap();
    index.add_hyperedge(edge4).unwrap();

    let stats = index.stats();
    assert_eq!(stats.total_hyperedges, 4);
    assert!(stats.avg_entity_degree > 0.0);
}

#[test]
fn test_hypergraph_search() {
    let mut index = HypergraphIndex::new(DistanceMetric::Cosine);

    // Add entities
    for i in 1..=10 {
        index.add_entity(i.to_string(), vec![i as f32 * 0.1; 32]);
    }

    // Add hyperedges
    for i in 1..=5 {
        let edge = Hyperedge::new(
            vec![i.to_string(), (i + 1).to_string()],
            format!("Edge {}", i),
            vec![i as f32 * 0.1; 32],
            0.9,
        );
        index.add_hyperedge(edge).unwrap();
    }

    // Search for similar hyperedges
    let query = vec![0.3; 32];
    let results = index.search_hyperedges(&query, 3);

    assert_eq!(results.len(), 3);
    // Results should be sorted by distance
    for i in 0..results.len() - 1 {
        assert!(results[i].1 <= results[i + 1].1);
    }
}

#[test]
fn test_k_hop_neighbors_simple() {
    let mut index = HypergraphIndex::new(DistanceMetric::Cosine);

    // Create chain: 1-2-3-4
    for i in 1..=4 {
        index.add_entity(i.to_string(), vec![i as f32]);
    }

    let e1 = Hyperedge::new(
        vec!["1".to_string(), "2".to_string()],
        "e1".to_string(),
        vec![1.0],
        1.0,
    );
    let e2 = Hyperedge::new(
        vec!["2".to_string(), "3".to_string()],
        "e2".to_string(),
        vec![1.0],
        1.0,
    );
    let e3 = Hyperedge::new(
        vec!["3".to_string(), "4".to_string()],
        "e3".to_string(),
        vec![1.0],
        1.0,
    );

    index.add_hyperedge(e1).unwrap();
    index.add_hyperedge(e2).unwrap();
    index.add_hyperedge(e3).unwrap();

    // 1-hop from node 1 should include 1 and 2
    let neighbors_1hop = index.k_hop_neighbors("1".to_string(), 1);
    assert!(neighbors_1hop.contains(&"1".to_string()));
    assert!(neighbors_1hop.contains(&"2".to_string()));

    // 2-hop from node 1 should include 1, 2, and 3
    let neighbors_2hop = index.k_hop_neighbors("1".to_string(), 2);
    assert!(neighbors_2hop.contains(&"1".to_string()));
    assert!(neighbors_2hop.contains(&"2".to_string()));
    assert!(neighbors_2hop.contains(&"3".to_string()));

    // 3-hop from node 1 should include all nodes
    let neighbors_3hop = index.k_hop_neighbors("1".to_string(), 3);
    assert_eq!(neighbors_3hop.len(), 4);
}

#[test]
fn test_k_hop_neighbors_complex() {
    let mut index = HypergraphIndex::new(DistanceMetric::Cosine);

    // Create star topology: center node connected to 5 peripheral nodes
    for i in 0..=5 {
        index.add_entity(i.to_string(), vec![i as f32]);
    }

    // Center (0) connected to all others via hyperedges
    for i in 1..=5 {
        let edge = Hyperedge::new(
            vec!["0".to_string(), i.to_string()],
            format!("e{}", i),
            vec![1.0],
            1.0,
        );
        index.add_hyperedge(edge).unwrap();
    }

    // 1-hop from center should reach all nodes
    let neighbors = index.k_hop_neighbors("0".to_string(), 1);
    assert_eq!(neighbors.len(), 6); // All nodes

    // 1-hop from peripheral node should reach center and itself
    let neighbors = index.k_hop_neighbors("1".to_string(), 1);
    assert!(neighbors.contains(&"0".to_string()));
    assert!(neighbors.contains(&"1".to_string()));
}

#[test]
fn test_temporal_range_query() {
    let mut index = HypergraphIndex::new(DistanceMetric::Cosine);

    // Add entities
    for i in 1..=3 {
        index.add_entity(i.to_string(), vec![i as f32]);
    }

    // Add temporal hyperedges (they'll all be in current time bucket)
    let edge1 = Hyperedge::new(
        vec!["1".to_string(), "2".to_string()],
        "t1".to_string(),
        vec![1.0],
        1.0,
    );
    let edge2 = Hyperedge::new(
        vec!["2".to_string(), "3".to_string()],
        "t2".to_string(),
        vec![1.0],
        1.0,
    );

    let temp1 = TemporalHyperedge::new(edge1, TemporalGranularity::Hourly);
    let temp2 = TemporalHyperedge::new(edge2, TemporalGranularity::Hourly);

    let bucket = temp1.time_bucket();

    index.add_temporal_hyperedge(temp1).unwrap();
    index.add_temporal_hyperedge(temp2).unwrap();

    // Query current time bucket
    let results = index.query_temporal_range(bucket, bucket);
    assert_eq!(results.len(), 2);
}

#[test]
fn test_hyperedge_with_duplicate_nodes() {
    // Test that hyperedge handles duplicate nodes appropriately
    let edge = Hyperedge::new(
        vec![
            "1".to_string(),
            "2".to_string(),
            "2".to_string(),
            "3".to_string(),
        ], // Duplicate node 2
        "Duplicate test".to_string(),
        vec![0.5; 16],
        0.8,
    );

    assert_eq!(edge.order(), 4); // Includes duplicates
    assert!(edge.contains_node(&"2".to_string()));
}

#[test]
fn test_hypergraph_error_on_missing_entity() {
    let mut index = HypergraphIndex::new(DistanceMetric::Cosine);

    // Only add entity 1, not 2
    index.add_entity("1".to_string(), vec![1.0]);

    // Try to create hyperedge with missing entity
    let edge = Hyperedge::new(
        vec!["1".to_string(), "2".to_string()],
        "Test".to_string(),
        vec![0.5],
        1.0,
    );

    let result = index.add_hyperedge(edge);
    assert!(result.is_err());
}

// ============================================================================
// Property-based tests
// ============================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    fn node_vec_strategy() -> impl Strategy<Value = Vec<String>> {
        prop::collection::vec("[a-z]{1,5}".prop_map(|s| s), 2..20)
    }

    fn embedding_strategy(dim: usize) -> impl Strategy<Value = Vec<f32>> {
        prop::collection::vec(-1.0f32..1.0f32, dim)
    }

    proptest! {
        #[test]
        fn test_hyperedge_order_property(
            nodes in node_vec_strategy()
        ) {
            let edge = Hyperedge::new(
                nodes.clone(),
                "Test".to_string(),
                vec![0.5; 32],
                0.9
            );

            assert_eq!(edge.order(), nodes.len());
        }

        #[test]
        fn test_hyperedge_contains_all_nodes(
            nodes in node_vec_strategy()
        ) {
            let edge = Hyperedge::new(
                nodes.clone(),
                "Test".to_string(),
                vec![0.5; 32],
                0.9
            );

            for node in &nodes {
                assert!(edge.contains_node(node));
            }
        }

        #[test]
        fn test_hypergraph_search_consistency(
            query in embedding_strategy(32),
            k in 1usize..10
        ) {
            let mut index = HypergraphIndex::new(DistanceMetric::Cosine);

            // Add entities
            for i in 1..=10 {
                index.add_entity(i.to_string(), vec![i as f32 * 0.1; 32]);
            }

            // Add hyperedges
            for i in 1..=10 {
                let edge = Hyperedge::new(
                    vec![i.to_string()],
                    format!("Edge {}", i),
                    vec![i as f32 * 0.1; 32],
                    0.9
                );
                index.add_hyperedge(edge).unwrap();
            }

            let results = index.search_hyperedges(&query, k.min(10));
            assert!(results.len() <= k.min(10));

            // Verify results are sorted
            for i in 0..results.len().saturating_sub(1) {
                assert!(results[i].1 <= results[i + 1].1);
            }
        }
    }
}
