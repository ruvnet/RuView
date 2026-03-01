//! End-to-end integration tests for the minimum cut implementation

use ruvector_mincut::{
    BoundedInstance, CommunityDetector, DynamicGraph, GraphPartitioner, MinCutWrapper,
    ProperCutInstance, RuVectorGraphAnalyzer,
};
use std::sync::Arc;

#[test]
fn test_wrapper_with_bounded_instance() {
    let graph = Arc::new(DynamicGraph::new());

    // Build a triangle
    graph.insert_edge(0, 1, 1.0).unwrap();
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 0, 1.0).unwrap();

    let mut wrapper = MinCutWrapper::with_factory(Arc::clone(&graph), |g, min, max| {
        Box::new(BoundedInstance::init(g, min, max))
    });

    // Sync edges
    for edge in graph.edges() {
        wrapper.insert_edge(edge.id, edge.source, edge.target);
    }

    let result = wrapper.query();
    assert!(result.is_connected());
    assert_eq!(result.value(), 2);
}

#[test]
fn test_dynamic_updates_bounded() {
    let graph = Arc::new(DynamicGraph::new());

    let mut wrapper = MinCutWrapper::with_factory(Arc::clone(&graph), |g, min, max| {
        Box::new(BoundedInstance::init(g, min, max))
    });

    // Start with 2 vertices connected
    let e1 = graph.insert_edge(0, 1, 1.0).unwrap();
    wrapper.insert_edge(e1, 0, 1);

    assert_eq!(wrapper.query().value(), 1);

    // Add parallel edge
    let e2 = graph.insert_edge(0, 1, 1.0);
    if let Ok(e2) = e2 {
        wrapper.insert_edge(e2, 0, 1);
    }

    // Add third vertex
    let e3 = graph.insert_edge(1, 2, 1.0).unwrap();
    wrapper.insert_edge(e3, 1, 2);

    let result = wrapper.query();
    assert!(result.is_connected());
}

#[test]
fn test_disconnected_graph() {
    let graph = Arc::new(DynamicGraph::new());

    // Two separate components
    graph.insert_edge(0, 1, 1.0).unwrap();
    graph.insert_edge(2, 3, 1.0).unwrap();

    let mut wrapper = MinCutWrapper::with_factory(Arc::clone(&graph), |g, min, max| {
        Box::new(BoundedInstance::init(g, min, max))
    });

    for edge in graph.edges() {
        wrapper.insert_edge(edge.id, edge.source, edge.target);
    }

    let result = wrapper.query();
    assert!(!result.is_connected() || result.value() == 0);
}

#[test]
fn test_community_detection_full_pipeline() {
    let graph = Arc::new(DynamicGraph::new());

    // Create two dense clusters connected by weak link
    // Cluster 1: 0-1-2-0 (triangle)
    graph.insert_edge(0, 1, 1.0).unwrap();
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 0, 1.0).unwrap();

    // Cluster 2: 3-4-5-3 (triangle)
    graph.insert_edge(3, 4, 1.0).unwrap();
    graph.insert_edge(4, 5, 1.0).unwrap();
    graph.insert_edge(5, 3, 1.0).unwrap();

    // Weak bridge
    graph.insert_edge(2, 3, 0.1).unwrap();

    let mut detector = CommunityDetector::new(graph);
    let communities = detector.detect(2);

    // Should detect structure
    assert!(!communities.is_empty());
}

#[test]
fn test_graph_partitioner_full_pipeline() {
    let graph = Arc::new(DynamicGraph::new());

    // Line graph: 0-1-2-3-4
    for i in 0..4u64 {
        graph.insert_edge(i, i + 1, 1.0).unwrap();
    }

    let partitioner = GraphPartitioner::new(graph, 2);
    let partitions = partitioner.partition();

    // Verify partitioning produces reasonable results
    assert!(
        partitions.len() >= 1 && partitions.len() <= 5,
        "Partitions should be between 1 and 5, got {}",
        partitions.len()
    );
    let total: usize = partitions.iter().map(|p| p.len()).sum();
    assert!(
        total >= 1 && total <= 5,
        "Total vertices should be 5 or fewer, got {}",
        total
    );
}

#[test]
fn test_analyzer_with_wrapper() {
    let graph = Arc::new(DynamicGraph::new());

    // Star graph: center 0 connected to 1,2,3,4
    for i in 1..5u64 {
        graph.insert_edge(0, i, 1.0).unwrap();
    }

    let mut analyzer = RuVectorGraphAnalyzer::new(graph);
    let min_cut = analyzer.min_cut();

    // Star graph has min cut = 1 (any leaf)
    assert_eq!(min_cut, 1);
}

#[test]
fn test_large_graph_performance() {
    let graph = Arc::new(DynamicGraph::new());

    // Create a larger graph: path of 100 vertices
    for i in 0..99u64 {
        graph.insert_edge(i, i + 1, 1.0).unwrap();
    }

    let mut wrapper = MinCutWrapper::with_factory(Arc::clone(&graph), |g, min, max| {
        Box::new(BoundedInstance::init(g, min, max))
    });

    for edge in graph.edges() {
        wrapper.insert_edge(edge.id, edge.source, edge.target);
    }

    let result = wrapper.query();
    assert!(result.is_connected());
    assert_eq!(result.value(), 1); // Path has min cut = 1
}
