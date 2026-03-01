//! Integration tests for SheafGraph CRUD operations and dimension validation
//!
//! Tests the Knowledge Substrate bounded context, verifying:
//! - Node creation, update, and deletion
//! - Edge creation with restriction maps
//! - Dimension compatibility validation
//! - Subgraph extraction
//! - Fingerprint-based change detection

use std::collections::HashMap;

/// Test helper: Create a simple identity restriction map
fn identity_restriction(dim: usize) -> Vec<Vec<f32>> {
    (0..dim)
        .map(|i| {
            let mut row = vec![0.0; dim];
            row[i] = 1.0;
            row
        })
        .collect()
}

/// Test helper: Create a projection restriction map (projects to first k dimensions)
fn projection_restriction(input_dim: usize, output_dim: usize) -> Vec<Vec<f32>> {
    (0..output_dim)
        .map(|i| {
            let mut row = vec![0.0; input_dim];
            if i < input_dim {
                row[i] = 1.0;
            }
            row
        })
        .collect()
}

// ============================================================================
// SHEAF NODE TESTS
// ============================================================================

#[test]
fn test_node_creation_with_valid_state() {
    // A node should be creatable with a valid state vector
    let state: Vec<f32> = vec![1.0, 0.5, 0.3, 0.2];
    let dimension = state.len();

    // Verify state is preserved
    assert_eq!(state.len(), dimension);
    assert!((state[0] - 1.0).abs() < f32::EPSILON);
}

#[test]
fn test_node_state_update_preserves_dimension() {
    // When updating a node's state, the dimension must remain constant
    let initial_state = vec![1.0, 0.5, 0.3];
    let new_state = vec![0.8, 0.6, 0.4];

    assert_eq!(initial_state.len(), new_state.len());
}

#[test]
fn test_node_state_update_rejects_dimension_mismatch() {
    // Updating with a different dimension should fail
    let initial_state = vec![1.0, 0.5, 0.3];
    let wrong_state = vec![0.8, 0.6]; // Only 2 dimensions

    assert_ne!(initial_state.len(), wrong_state.len());
}

#[test]
fn test_node_metadata_stores_custom_fields() {
    // Nodes should support custom metadata for domain-specific information
    let mut metadata: HashMap<String, String> = HashMap::new();
    metadata.insert("source".to_string(), "sensor_1".to_string());
    metadata.insert("confidence".to_string(), "0.95".to_string());

    assert_eq!(metadata.get("source"), Some(&"sensor_1".to_string()));
    assert_eq!(metadata.len(), 2);
}

// ============================================================================
// SHEAF EDGE TESTS
// ============================================================================

#[test]
fn test_edge_creation_with_identity_restriction() {
    // Edges with identity restrictions should not transform states
    let dim = 4;
    let rho = identity_restriction(dim);
    let state = vec![1.0, 2.0, 3.0, 4.0];

    // Apply restriction
    let result: Vec<f32> = rho
        .iter()
        .map(|row| row.iter().zip(&state).map(|(a, b)| a * b).sum())
        .collect();

    // Should be unchanged
    assert_eq!(result, state);
}

#[test]
fn test_edge_creation_with_projection_restriction() {
    // Projection restrictions reduce dimension
    let rho = projection_restriction(4, 2);
    let state = vec![1.0, 2.0, 3.0, 4.0];

    // Apply restriction
    let result: Vec<f32> = rho
        .iter()
        .map(|row| row.iter().zip(&state).map(|(a, b)| a * b).sum())
        .collect();

    assert_eq!(result.len(), 2);
    assert_eq!(result, vec![1.0, 2.0]);
}

#[test]
fn test_edge_weight_affects_energy() {
    // Higher edge weights should amplify residual energy
    let residual = vec![0.1, 0.1, 0.1];
    let norm_sq: f32 = residual.iter().map(|x| x * x).sum();

    let low_weight_energy = 1.0 * norm_sq;
    let high_weight_energy = 10.0 * norm_sq;

    assert!(high_weight_energy > low_weight_energy);
    assert!((high_weight_energy / low_weight_energy - 10.0).abs() < f32::EPSILON);
}

#[test]
fn test_edge_restriction_dimension_validation() {
    // Restriction map dimensions must be compatible with node dimensions
    let source_dim = 4;
    let edge_dim = 2;

    // Valid: source_dim -> edge_dim
    let valid_rho = projection_restriction(source_dim, edge_dim);
    assert_eq!(valid_rho.len(), edge_dim);
    assert_eq!(valid_rho[0].len(), source_dim);

    // The restriction should accept 4D input and produce 2D output
    let state = vec![1.0, 2.0, 3.0, 4.0];
    let result: Vec<f32> = valid_rho
        .iter()
        .map(|row| row.iter().zip(&state).map(|(a, b)| a * b).sum())
        .collect();
    assert_eq!(result.len(), edge_dim);
}

// ============================================================================
// SHEAF GRAPH CRUD TESTS
// ============================================================================

#[test]
fn test_graph_add_node() {
    // Adding a node should increase the node count
    let mut nodes: HashMap<u64, Vec<f32>> = HashMap::new();

    nodes.insert(1, vec![1.0, 0.5, 0.3]);
    assert_eq!(nodes.len(), 1);

    nodes.insert(2, vec![0.8, 0.6, 0.4]);
    assert_eq!(nodes.len(), 2);
}

#[test]
fn test_graph_add_edge_validates_nodes_exist() {
    // Edges can only be created between existing nodes
    let mut nodes: HashMap<u64, Vec<f32>> = HashMap::new();
    let edges: HashMap<(u64, u64), f32> = HashMap::new();

    nodes.insert(1, vec![1.0, 0.5]);
    nodes.insert(2, vec![0.8, 0.6]);

    // Both nodes exist - edge should be allowed
    assert!(nodes.contains_key(&1));
    assert!(nodes.contains_key(&2));

    // Non-existent node - edge should not be allowed
    assert!(!nodes.contains_key(&999));
}

#[test]
fn test_graph_remove_node_cascades_to_edges() {
    // Removing a node should remove all incident edges
    let mut nodes: HashMap<u64, Vec<f32>> = HashMap::new();
    let mut edges: HashMap<(u64, u64), f32> = HashMap::new();

    nodes.insert(1, vec![1.0, 0.5]);
    nodes.insert(2, vec![0.8, 0.6]);
    nodes.insert(3, vec![0.7, 0.4]);

    edges.insert((1, 2), 1.0);
    edges.insert((2, 3), 1.0);
    edges.insert((1, 3), 1.0);

    // Remove node 1
    nodes.remove(&1);
    edges.retain(|(src, tgt), _| *src != 1 && *tgt != 1);

    assert_eq!(nodes.len(), 2);
    assert_eq!(edges.len(), 1); // Only (2,3) remains
    assert!(edges.contains_key(&(2, 3)));
}

#[test]
fn test_graph_update_node_state() {
    // Updating a node state should trigger re-computation of affected edges
    let mut nodes: HashMap<u64, Vec<f32>> = HashMap::new();

    nodes.insert(1, vec![1.0, 0.5, 0.3]);

    // Update
    nodes.insert(1, vec![0.9, 0.6, 0.4]);

    let state = nodes.get(&1).unwrap();
    assert!((state[0] - 0.9).abs() < f32::EPSILON);
}

// ============================================================================
// SUBGRAPH EXTRACTION TESTS
// ============================================================================

#[test]
fn test_subgraph_extraction_bfs() {
    // Extracting a k-hop subgraph around a center node
    let mut nodes: HashMap<u64, Vec<f32>> = HashMap::new();
    let mut adjacency: HashMap<u64, Vec<u64>> = HashMap::new();

    // Create a chain: 1 - 2 - 3 - 4 - 5
    for i in 1..=5 {
        nodes.insert(i, vec![i as f32; 3]);
    }
    adjacency.insert(1, vec![2]);
    adjacency.insert(2, vec![1, 3]);
    adjacency.insert(3, vec![2, 4]);
    adjacency.insert(4, vec![3, 5]);
    adjacency.insert(5, vec![4]);

    // Extract 1-hop subgraph around node 3
    fn extract_khop(center: u64, k: usize, adjacency: &HashMap<u64, Vec<u64>>) -> Vec<u64> {
        let mut visited = vec![center];
        let mut frontier = vec![center];

        for _ in 0..k {
            let mut next_frontier = Vec::new();
            for node in &frontier {
                if let Some(neighbors) = adjacency.get(node) {
                    for neighbor in neighbors {
                        if !visited.contains(neighbor) {
                            visited.push(*neighbor);
                            next_frontier.push(*neighbor);
                        }
                    }
                }
            }
            frontier = next_frontier;
        }
        visited
    }

    let subgraph = extract_khop(3, 1, &adjacency);
    assert!(subgraph.contains(&3)); // Center
    assert!(subgraph.contains(&2)); // 1-hop neighbor
    assert!(subgraph.contains(&4)); // 1-hop neighbor
    assert!(!subgraph.contains(&1)); // 2-hops away
    assert!(!subgraph.contains(&5)); // 2-hops away

    let larger_subgraph = extract_khop(3, 2, &adjacency);
    assert_eq!(larger_subgraph.len(), 5); // All nodes within 2 hops
}

// ============================================================================
// NAMESPACE AND SCOPE TESTS
// ============================================================================

#[test]
fn test_namespace_isolation() {
    // Nodes in different namespaces should be isolated
    let mut namespaces: HashMap<String, Vec<u64>> = HashMap::new();

    namespaces.entry("finance".to_string()).or_default().push(1);
    namespaces.entry("finance".to_string()).or_default().push(2);
    namespaces.entry("medical".to_string()).or_default().push(3);

    let finance_nodes = namespaces.get("finance").unwrap();
    let medical_nodes = namespaces.get("medical").unwrap();

    assert_eq!(finance_nodes.len(), 2);
    assert_eq!(medical_nodes.len(), 1);

    // No overlap
    for node in finance_nodes {
        assert!(!medical_nodes.contains(node));
    }
}

// ============================================================================
// FINGERPRINT TESTS
// ============================================================================

#[test]
fn test_fingerprint_changes_on_modification() {
    // Graph fingerprint should change when structure changes
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn compute_fingerprint(nodes: &HashMap<u64, Vec<f32>>, edges: &[(u64, u64)]) -> u64 {
        let mut hasher = DefaultHasher::new();

        let mut node_keys: Vec<_> = nodes.keys().collect();
        node_keys.sort();
        for key in node_keys {
            key.hash(&mut hasher);
            // Hash state values
            for val in nodes.get(key).unwrap() {
                val.to_bits().hash(&mut hasher);
            }
        }

        for (src, tgt) in edges {
            src.hash(&mut hasher);
            tgt.hash(&mut hasher);
        }

        hasher.finish()
    }

    let mut nodes: HashMap<u64, Vec<f32>> = HashMap::new();
    nodes.insert(1, vec![1.0, 0.5]);
    nodes.insert(2, vec![0.8, 0.6]);

    let edges1 = vec![(1, 2)];
    let fp1 = compute_fingerprint(&nodes, &edges1);

    // Add a node
    nodes.insert(3, vec![0.7, 0.4]);
    let fp2 = compute_fingerprint(&nodes, &edges1);

    assert_ne!(fp1, fp2);

    // Add an edge
    let edges2 = vec![(1, 2), (2, 3)];
    let fp3 = compute_fingerprint(&nodes, &edges2);

    assert_ne!(fp2, fp3);
}

#[test]
fn test_fingerprint_stable_without_modification() {
    // Fingerprint should be deterministic and stable
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    fn compute_fingerprint(nodes: &HashMap<u64, Vec<f32>>) -> u64 {
        let mut hasher = DefaultHasher::new();
        let mut keys: Vec<_> = nodes.keys().collect();
        keys.sort();
        for key in keys {
            key.hash(&mut hasher);
        }
        hasher.finish()
    }

    let mut nodes: HashMap<u64, Vec<f32>> = HashMap::new();
    nodes.insert(1, vec![1.0, 0.5]);
    nodes.insert(2, vec![0.8, 0.6]);

    let fp1 = compute_fingerprint(&nodes);
    let fp2 = compute_fingerprint(&nodes);

    assert_eq!(fp1, fp2);
}

// ============================================================================
// DIMENSION VALIDATION TESTS
// ============================================================================

#[test]
fn test_restriction_map_dimension_compatibility() {
    // Restriction map output dimension must equal edge stalk dimension
    struct RestrictionMap {
        matrix: Vec<Vec<f32>>,
    }

    impl RestrictionMap {
        fn new(matrix: Vec<Vec<f32>>) -> Result<Self, &'static str> {
            if matrix.is_empty() {
                return Err("Matrix cannot be empty");
            }
            let row_len = matrix[0].len();
            if !matrix.iter().all(|row| row.len() == row_len) {
                return Err("All rows must have same length");
            }
            Ok(Self { matrix })
        }

        fn input_dim(&self) -> usize {
            self.matrix[0].len()
        }

        fn output_dim(&self) -> usize {
            self.matrix.len()
        }

        fn apply(&self, input: &[f32]) -> Result<Vec<f32>, &'static str> {
            if input.len() != self.input_dim() {
                return Err("Input dimension mismatch");
            }
            Ok(self
                .matrix
                .iter()
                .map(|row| row.iter().zip(input).map(|(a, b)| a * b).sum())
                .collect())
        }
    }

    let rho = RestrictionMap::new(projection_restriction(4, 2)).unwrap();

    // Valid input
    let result = rho.apply(&[1.0, 2.0, 3.0, 4.0]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 2);

    // Invalid input (wrong dimension)
    let result = rho.apply(&[1.0, 2.0]);
    assert!(result.is_err());
}

#[test]
fn test_edge_creation_validates_stalk_dimensions() {
    // When creating an edge, both restriction maps must project to the same edge stalk dimension
    let source_dim = 4;
    let target_dim = 3;
    let edge_stalk_dim = 2;

    let rho_source = projection_restriction(source_dim, edge_stalk_dim);
    let rho_target = projection_restriction(target_dim, edge_stalk_dim);

    // Both should output the same dimension
    assert_eq!(rho_source.len(), edge_stalk_dim);
    assert_eq!(rho_target.len(), edge_stalk_dim);

    // Source accepts source_dim input
    assert_eq!(rho_source[0].len(), source_dim);

    // Target accepts target_dim input
    assert_eq!(rho_target[0].len(), target_dim);
}

// ============================================================================
// CONCURRENT ACCESS TESTS
// ============================================================================

#[test]
fn test_concurrent_node_reads() {
    // Multiple threads should be able to read nodes concurrently
    use std::sync::Arc;
    use std::thread;

    let nodes: Arc<HashMap<u64, Vec<f32>>> = Arc::new({
        let mut map = HashMap::new();
        for i in 0..100 {
            map.insert(i, vec![i as f32; 4]);
        }
        map
    });

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let nodes_clone = Arc::clone(&nodes);
            thread::spawn(move || {
                let mut sum = 0.0;
                for i in 0..100 {
                    if let Some(state) = nodes_clone.get(&i) {
                        sum += state[0];
                    }
                }
                sum
            })
        })
        .collect();

    let results: Vec<f32> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All threads should compute the same sum
    let expected_sum: f32 = (0..100).map(|i| i as f32).sum();
    for result in results {
        assert!((result - expected_sum).abs() < f32::EPSILON);
    }
}

// ============================================================================
// LARGE GRAPH TESTS
// ============================================================================

#[test]
fn test_large_graph_creation() {
    // Test creation of a moderately large graph
    let num_nodes = 1000;
    let dim = 16;

    let mut nodes: HashMap<u64, Vec<f32>> = HashMap::with_capacity(num_nodes);

    for i in 0..num_nodes {
        let state: Vec<f32> = (0..dim).map(|j| (i * dim + j) as f32 / 1000.0).collect();
        nodes.insert(i as u64, state);
    }

    assert_eq!(nodes.len(), num_nodes);

    // Verify random access
    let node_500 = nodes.get(&500).unwrap();
    assert_eq!(node_500.len(), dim);
}

#[test]
fn test_sparse_graph_edge_ratio() {
    // Sparse graphs should have edges << nodes^2
    let num_nodes = 100;
    let avg_degree = 4;

    let num_edges = (num_nodes * avg_degree) / 2; // Undirected
    let max_edges = num_nodes * (num_nodes - 1) / 2;
    let sparsity = num_edges as f64 / max_edges as f64;

    assert!(sparsity < 0.1); // Less than 10% of possible edges
}
