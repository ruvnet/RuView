//! Test fixtures and data builders
//!
//! Provides reusable test data and configuration builders.

#![allow(dead_code)]

/// Generate test embeddings with known patterns
pub fn generate_test_embeddings(count: usize, dimensions: usize) -> Vec<Vec<f32>> {
    // TODO: Implement once exo-core types are available
    // Generate diverse embeddings for testing
    // Use deterministic seed for reproducibility

    (0..count)
        .map(|i| {
            (0..dimensions)
                .map(|d| ((i * dimensions + d) as f32).sin())
                .collect()
        })
        .collect()
}

/// Generate clustered embeddings (for testing similarity)
pub fn generate_clustered_embeddings(
    clusters: usize,
    per_cluster: usize,
    dimensions: usize,
) -> Vec<Vec<f32>> {
    // TODO: Implement clustering logic
    // Create distinct clusters in embedding space
    vec![vec![0.0; dimensions]; clusters * per_cluster]
}

/// Create a test pattern with default values
pub fn create_test_pattern(embedding: Vec<f32>) -> String {
    // TODO: Return actual Pattern once exo-core exists
    // For now, return placeholder
    format!("TestPattern({:?})", &embedding[..embedding.len().min(3)])
}

/// Create a test hypergraph with known topology
pub fn create_test_hypergraph() -> String {
    // TODO: Build test hypergraph once exo-hypergraph exists
    // Should include:
    // - Multiple connected components
    // - Some 1-dimensional holes (cycles)
    // - Some 2-dimensional holes (voids)
    "TestHypergraph".to_string()
}

/// Create a causal chain for testing temporal memory
pub fn create_causal_chain(length: usize) -> Vec<String> {
    // TODO: Create linked patterns once exo-temporal exists
    // Returns pattern IDs in causal order
    (0..length).map(|i| format!("pattern_{}", i)).collect()
}

/// Create a federation of test nodes
pub async fn create_test_federation(node_count: usize) -> Vec<String> {
    // TODO: Implement once exo-federation exists
    // Returns federation node handles
    (0..node_count)
        .map(|i| format!("node_{}", i))
        .collect()
}

/// Default test configuration
pub fn default_test_config() -> TestConfig {
    TestConfig {
        timeout_ms: 5000,
        log_level: "info".to_string(),
        seed: 42,
    }
}

#[derive(Debug, Clone)]
pub struct TestConfig {
    pub timeout_ms: u64,
    pub log_level: String,
    pub seed: u64,
}
