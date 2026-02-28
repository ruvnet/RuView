//! GPU Coherence Engine Tests
//!
//! Comprehensive tests verifying GPU computation results match CPU results
//! within floating-point tolerance. These tests ensure correctness of:
//!
//! - GPU buffer management and data transfer
//! - Parallel residual computation
//! - Energy aggregation with tree reduction
//! - CPU fallback mechanism
//!
//! Run with: cargo test --features gpu

#![cfg(feature = "gpu")]

use prime_radiant::gpu::{
    BufferUsage, GpuBuffer, GpuBufferManager, GpuCoherenceEngine, GpuConfig, GpuEdge, GpuError,
    GpuParams, GpuRestrictionMap, GpuResult,
};
use prime_radiant::substrate::{
    EdgeId, NodeId, SheafEdge, SheafEdgeBuilder, SheafGraph, SheafNode, SheafNodeBuilder,
};
use std::collections::HashMap;
use uuid::Uuid;

/// Floating point tolerance for GPU vs CPU comparison
const TOLERANCE: f32 = 1e-5;

/// Create a simple test graph with 3 nodes forming a triangle
fn create_triangle_graph() -> SheafGraph {
    let graph = SheafGraph::new();

    // Create three nodes with states
    let node1 = SheafNodeBuilder::new()
        .state_from_slice(&[1.0, 0.0, 0.0])
        .namespace("test")
        .build();
    let node2 = SheafNodeBuilder::new()
        .state_from_slice(&[0.0, 1.0, 0.0])
        .namespace("test")
        .build();
    let node3 = SheafNodeBuilder::new()
        .state_from_slice(&[0.0, 0.0, 1.0])
        .namespace("test")
        .build();

    let id1 = graph.add_node(node1);
    let id2 = graph.add_node(node2);
    let id3 = graph.add_node(node3);

    // Create edges with identity restrictions
    let edge12 = SheafEdgeBuilder::new(id1, id2)
        .identity_restrictions(3)
        .weight(1.0)
        .namespace("test")
        .build();
    let edge23 = SheafEdgeBuilder::new(id2, id3)
        .identity_restrictions(3)
        .weight(1.0)
        .namespace("test")
        .build();
    let edge31 = SheafEdgeBuilder::new(id3, id1)
        .identity_restrictions(3)
        .weight(1.0)
        .namespace("test")
        .build();

    graph.add_edge(edge12).unwrap();
    graph.add_edge(edge23).unwrap();
    graph.add_edge(edge31).unwrap();

    graph
}

/// Create a coherent graph where all nodes have identical states
fn create_coherent_graph() -> SheafGraph {
    let graph = SheafGraph::new();

    // All nodes have the same state
    let state = [1.0, 1.0, 1.0];

    let node1 = SheafNodeBuilder::new().state_from_slice(&state).build();
    let node2 = SheafNodeBuilder::new().state_from_slice(&state).build();

    let id1 = graph.add_node(node1);
    let id2 = graph.add_node(node2);

    let edge = SheafEdgeBuilder::new(id1, id2)
        .identity_restrictions(3)
        .weight(1.0)
        .build();

    graph.add_edge(edge).unwrap();
    graph
}

/// Create a larger graph for performance testing
fn create_large_graph(num_nodes: usize, edges_per_node: usize) -> SheafGraph {
    let graph = SheafGraph::new();
    let state_dim = 64;

    // Create nodes with random states
    let mut node_ids = Vec::with_capacity(num_nodes);
    for i in 0..num_nodes {
        let state: Vec<f32> = (0..state_dim)
            .map(|j| ((i * state_dim + j) as f32 * 0.01).sin())
            .collect();

        let node = SheafNodeBuilder::new().state_from_slice(&state).build();

        node_ids.push(graph.add_node(node));
    }

    // Create edges
    for i in 0..num_nodes {
        for j in 1..=edges_per_node {
            let target_idx = (i + j) % num_nodes;
            if i != target_idx {
                let edge = SheafEdgeBuilder::new(node_ids[i], node_ids[target_idx])
                    .identity_restrictions(state_dim)
                    .weight(1.0)
                    .build();

                // Ignore duplicate edges
                let _ = graph.add_edge(edge);
            }
        }
    }

    graph
}

// ============================================================================
// GPU Configuration Tests
// ============================================================================

#[test]
fn test_gpu_config_default() {
    let config = GpuConfig::default();

    assert!(config.enable_fallback);
    assert_eq!(config.beta, 1.0);
    assert!(config.threshold_lane0 < config.threshold_lane1);
    assert!(config.threshold_lane1 < config.threshold_lane2);
    assert!(config.timeout_ms > 0);
}

#[test]
fn test_gpu_config_custom() {
    let config = GpuConfig {
        enable_fallback: false,
        beta: 2.0,
        threshold_lane0: 0.05,
        threshold_lane1: 0.5,
        threshold_lane2: 5.0,
        ..Default::default()
    };

    assert!(!config.enable_fallback);
    assert_eq!(config.beta, 2.0);
    assert_eq!(config.threshold_lane0, 0.05);
}

// ============================================================================
// GPU Buffer Tests
// ============================================================================

#[test]
fn test_gpu_params_alignment() {
    // GPU struct alignment is critical for correct computation
    assert_eq!(std::mem::size_of::<GpuParams>(), 32);
    assert_eq!(std::mem::align_of::<GpuParams>(), 4);
}

#[test]
fn test_gpu_edge_alignment() {
    assert_eq!(std::mem::size_of::<GpuEdge>(), 32);
    assert_eq!(std::mem::align_of::<GpuEdge>(), 4);
}

#[test]
fn test_gpu_restriction_map_alignment() {
    assert_eq!(std::mem::size_of::<GpuRestrictionMap>(), 32);
    assert_eq!(std::mem::align_of::<GpuRestrictionMap>(), 4);
}

// ============================================================================
// CPU vs GPU Comparison Tests
// ============================================================================

/// Test that GPU energy matches CPU energy for triangle graph
#[tokio::test]
async fn test_gpu_cpu_energy_match_triangle() {
    let graph = create_triangle_graph();

    // Compute CPU energy
    let cpu_energy = graph.compute_energy();

    // Try GPU computation
    let config = GpuConfig::default();
    match GpuCoherenceEngine::try_new(config).await {
        Some(mut engine) => {
            engine.upload_graph(&graph).unwrap();
            let gpu_energy = engine.compute_energy().await.unwrap();

            // Compare total energies
            let diff = (cpu_energy.total_energy - gpu_energy.total_energy).abs();
            assert!(
                diff < TOLERANCE,
                "Energy mismatch: CPU={}, GPU={}, diff={}",
                cpu_energy.total_energy,
                gpu_energy.total_energy,
                diff
            );

            // Verify GPU was actually used
            assert!(gpu_energy.used_gpu);
        }
        None => {
            // GPU not available, skip test
            eprintln!("GPU not available, skipping GPU comparison test");
        }
    }
}

/// Test that coherent graph has near-zero energy on GPU
#[tokio::test]
async fn test_gpu_coherent_graph() {
    let graph = create_coherent_graph();

    // CPU energy should be near zero
    let cpu_energy = graph.compute_energy();
    assert!(
        cpu_energy.total_energy < 1e-10,
        "CPU energy for coherent graph should be near zero: {}",
        cpu_energy.total_energy
    );

    // Try GPU computation
    let config = GpuConfig::default();
    if let Some(mut engine) = GpuCoherenceEngine::try_new(config).await {
        engine.upload_graph(&graph).unwrap();
        let gpu_energy = engine.compute_energy().await.unwrap();

        assert!(
            gpu_energy.total_energy < 1e-5,
            "GPU energy for coherent graph should be near zero: {}",
            gpu_energy.total_energy
        );
    }
}

/// Test per-edge energy computation
#[tokio::test]
async fn test_gpu_per_edge_energies() {
    let graph = create_triangle_graph();

    // Compute CPU energy
    let cpu_energy = graph.compute_energy();

    let config = GpuConfig::default();
    if let Some(mut engine) = GpuCoherenceEngine::try_new(config).await {
        engine.upload_graph(&graph).unwrap();
        let gpu_energy = engine.compute_energy().await.unwrap();

        // Same number of edge energies
        assert_eq!(
            cpu_energy.edge_energies.len(),
            gpu_energy.edge_energies.len(),
            "Edge count mismatch"
        );

        // Each edge energy should match (order may differ)
        let cpu_sum: f32 = cpu_energy.edge_energies.values().sum();
        let gpu_sum: f32 = gpu_energy.edge_energies.iter().sum();

        let diff = (cpu_sum - gpu_sum).abs();
        assert!(
            diff < TOLERANCE,
            "Sum of edge energies mismatch: CPU={}, GPU={}, diff={}",
            cpu_sum,
            gpu_sum,
            diff
        );
    }
}

/// Test with larger graph
#[tokio::test]
async fn test_gpu_large_graph() {
    let graph = create_large_graph(100, 5);

    let cpu_energy = graph.compute_energy();

    let config = GpuConfig::default();
    if let Some(mut engine) = GpuCoherenceEngine::try_new(config).await {
        engine.upload_graph(&graph).unwrap();
        let gpu_energy = engine.compute_energy().await.unwrap();

        // Allow slightly larger tolerance for large graphs due to floating point accumulation
        let diff = (cpu_energy.total_energy - gpu_energy.total_energy).abs();
        let relative_diff = diff / cpu_energy.total_energy.max(1.0);

        assert!(
            relative_diff < 0.01, // 1% relative error
            "Large graph energy mismatch: CPU={}, GPU={}, relative_diff={:.2}%",
            cpu_energy.total_energy,
            gpu_energy.total_energy,
            relative_diff * 100.0
        );
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_gpu_empty_graph_error() {
    let graph = SheafGraph::new();

    let config = GpuConfig::default();
    if let Some(mut engine) = GpuCoherenceEngine::try_new(config).await {
        let result = engine.upload_graph(&graph);
        assert!(result.is_err());
        match result {
            Err(GpuError::EmptyGraph) => {}
            Err(e) => panic!("Expected EmptyGraph error, got: {:?}", e),
            Ok(_) => panic!("Expected error for empty graph"),
        }
    }
}

#[test]
fn test_gpu_error_fallback_detection() {
    // Test that certain errors trigger fallback
    assert!(GpuError::NoAdapter.should_fallback());
    assert!(GpuError::NoDevice("test".into()).should_fallback());
    assert!(GpuError::DeviceCreation("test".into()).should_fallback());
    assert!(GpuError::AdapterRequest("test".into()).should_fallback());
    assert!(GpuError::UnsupportedFeature("test".into()).should_fallback());

    // These should not trigger fallback
    assert!(!GpuError::Timeout(100).should_fallback());
    assert!(!GpuError::EmptyGraph.should_fallback());
    assert!(!GpuError::BufferRead("test".into()).should_fallback());
}

#[test]
fn test_gpu_error_recoverable() {
    assert!(GpuError::Timeout(100).is_recoverable());
    assert!(GpuError::BufferRead("test".into()).is_recoverable());
    assert!(GpuError::ExecutionFailed("test".into()).is_recoverable());

    assert!(!GpuError::NoAdapter.is_recoverable());
    assert!(!GpuError::EmptyGraph.is_recoverable());
}

// ============================================================================
// GPU Capabilities Tests
// ============================================================================

#[tokio::test]
async fn test_gpu_capabilities() {
    let config = GpuConfig::default();
    if let Some(engine) = GpuCoherenceEngine::try_new(config).await {
        let caps = engine.capabilities();

        // Should have valid device info
        assert!(!caps.device_name.is_empty());
        assert!(!caps.backend.is_empty());

        // Should have reasonable limits
        assert!(caps.max_buffer_size > 0);
        assert!(caps.max_workgroup_size > 0);
        assert!(caps.max_workgroups[0] > 0);

        // Should be marked as supported
        assert!(caps.supported);
    }
}

// ============================================================================
// Synchronous API Tests
// ============================================================================

#[test]
fn test_sync_api() {
    use prime_radiant::gpu::sync;

    let config = GpuConfig::default();
    if let Some(mut engine) = sync::try_create_engine(config) {
        let graph = create_triangle_graph();

        engine.upload_graph(&graph).unwrap();
        let energy = sync::compute_energy(&mut engine).unwrap();

        assert!(energy.total_energy > 0.0);
        assert!(energy.used_gpu);
    }
}

// ============================================================================
// Resource Management Tests
// ============================================================================

#[tokio::test]
async fn test_gpu_resource_release() {
    let config = GpuConfig::default();
    if let Some(mut engine) = GpuCoherenceEngine::try_new(config).await {
        let graph = create_triangle_graph();

        // Upload and compute
        engine.upload_graph(&graph).unwrap();
        let _ = engine.compute_energy().await.unwrap();

        // Release resources
        engine.release();

        // Re-upload should work
        engine.upload_graph(&graph).unwrap();
        let energy = engine.compute_energy().await.unwrap();
        assert!(energy.total_energy > 0.0);
    }
}

#[tokio::test]
async fn test_gpu_multiple_computations() {
    let config = GpuConfig::default();
    if let Some(mut engine) = GpuCoherenceEngine::try_new(config).await {
        let graph = create_triangle_graph();
        engine.upload_graph(&graph).unwrap();

        // Multiple computations should give consistent results
        let energy1 = engine.compute_energy().await.unwrap();
        let energy2 = engine.compute_energy().await.unwrap();
        let energy3 = engine.compute_energy().await.unwrap();

        assert!(
            (energy1.total_energy - energy2.total_energy).abs() < TOLERANCE,
            "Inconsistent results between computations"
        );
        assert!(
            (energy2.total_energy - energy3.total_energy).abs() < TOLERANCE,
            "Inconsistent results between computations"
        );
    }
}

// ============================================================================
// Performance Tests (disabled by default)
// ============================================================================

#[tokio::test]
#[ignore] // Run with: cargo test --features gpu -- --ignored
async fn test_gpu_performance_1k_nodes() {
    let graph = create_large_graph(1000, 10);
    let edge_count = graph.edge_count();

    let config = GpuConfig::default();
    if let Some(mut engine) = GpuCoherenceEngine::try_new(config).await {
        engine.upload_graph(&graph).unwrap();

        // Warm up
        let _ = engine.compute_energy().await.unwrap();

        // Benchmark
        let start = std::time::Instant::now();
        let energy = engine.compute_energy().await.unwrap();
        let gpu_time = start.elapsed();

        // Compare with CPU
        let start = std::time::Instant::now();
        let cpu_energy = graph.compute_energy();
        let cpu_time = start.elapsed();

        println!("Performance test ({} edges):", edge_count);
        println!(
            "  GPU: {}us ({} edges/ms)",
            energy.compute_time_us,
            edge_count as u64 * 1000 / energy.compute_time_us.max(1)
        );
        println!("  CPU: {}us", cpu_time.as_micros());
        println!(
            "  Speedup: {:.2}x",
            cpu_time.as_micros() as f64 / gpu_time.as_micros() as f64
        );

        // Verify correctness
        let diff = (cpu_energy.total_energy - energy.total_energy).abs();
        let relative_diff = diff / cpu_energy.total_energy.max(1.0);
        assert!(relative_diff < 0.01, "Performance test: energy mismatch");
    }
}

#[tokio::test]
#[ignore]
async fn test_gpu_performance_10k_nodes() {
    let graph = create_large_graph(10000, 10);
    let edge_count = graph.edge_count();

    let config = GpuConfig::default();
    if let Some(mut engine) = GpuCoherenceEngine::try_new(config).await {
        engine.upload_graph(&graph).unwrap();

        // Warm up
        let _ = engine.compute_energy().await.unwrap();

        // Benchmark
        let energy = engine.compute_energy().await.unwrap();

        println!(
            "Large scale test ({} edges): {}us, {} edges/ms",
            edge_count,
            energy.compute_time_us,
            edge_count as u64 * 1000 / energy.compute_time_us.max(1)
        );

        assert!(energy.total_energy > 0.0);
    }
}
