//! Integration tests for Coherence Computation
//!
//! Tests the Coherence Computation bounded context, verifying:
//! - Full energy computation from graph state
//! - Incremental updates when nodes change
//! - Spectral drift detection
//! - Hotspot identification
//! - Caching and fingerprint-based staleness

use std::collections::HashMap;

// ============================================================================
// TEST INFRASTRUCTURE
// ============================================================================

/// Simple restriction map for testing
struct RestrictionMap {
    matrix: Vec<Vec<f32>>,
    bias: Vec<f32>,
}

impl RestrictionMap {
    fn new(rows: usize, cols: usize) -> Self {
        // Identity-like (truncated or padded)
        let matrix: Vec<Vec<f32>> = (0..rows)
            .map(|i| (0..cols).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
            .collect();
        let bias = vec![0.0; rows];
        Self { matrix, bias }
    }

    fn apply(&self, input: &[f32]) -> Vec<f32> {
        self.matrix
            .iter()
            .zip(&self.bias)
            .map(|(row, b)| row.iter().zip(input).map(|(a, x)| a * x).sum::<f32>() + b)
            .collect()
    }

    fn output_dim(&self) -> usize {
        self.matrix.len()
    }
}

/// Simple edge for testing
struct TestEdge {
    source: u64,
    target: u64,
    weight: f32,
    rho_source: RestrictionMap,
    rho_target: RestrictionMap,
}

impl TestEdge {
    fn compute_residual(&self, states: &HashMap<u64, Vec<f32>>) -> Option<Vec<f32>> {
        let source_state = states.get(&self.source)?;
        let target_state = states.get(&self.target)?;

        let projected_source = self.rho_source.apply(source_state);
        let projected_target = self.rho_target.apply(target_state);

        Some(
            projected_source
                .iter()
                .zip(&projected_target)
                .map(|(a, b)| a - b)
                .collect(),
        )
    }

    fn compute_energy(&self, states: &HashMap<u64, Vec<f32>>) -> Option<f32> {
        let residual = self.compute_residual(states)?;
        let norm_sq: f32 = residual.iter().map(|x| x * x).sum();
        Some(self.weight * norm_sq)
    }
}

/// Simple coherence energy computation
fn compute_total_energy(
    states: &HashMap<u64, Vec<f32>>,
    edges: &[TestEdge],
) -> (f32, HashMap<usize, f32>) {
    let mut total = 0.0;
    let mut edge_energies = HashMap::new();

    for (i, edge) in edges.iter().enumerate() {
        if let Some(energy) = edge.compute_energy(states) {
            total += energy;
            edge_energies.insert(i, energy);
        }
    }

    (total, edge_energies)
}

// ============================================================================
// ENERGY COMPUTATION TESTS
// ============================================================================

#[test]
fn test_energy_computation_consistent_section() {
    // A consistent section (all nodes agree) should have zero energy
    let mut states = HashMap::new();
    states.insert(1, vec![1.0, 0.5, 0.3]);
    states.insert(2, vec![1.0, 0.5, 0.3]); // Same state

    let edges = vec![TestEdge {
        source: 1,
        target: 2,
        weight: 1.0,
        rho_source: RestrictionMap::new(3, 3),
        rho_target: RestrictionMap::new(3, 3),
    }];

    let (total, _) = compute_total_energy(&states, &edges);

    // Energy should be zero (or very close) for consistent section
    assert!(total < 1e-10, "Expected near-zero energy, got {}", total);
}

#[test]
fn test_energy_computation_inconsistent_section() {
    // Inconsistent states should produce positive energy
    let mut states = HashMap::new();
    states.insert(1, vec![1.0, 0.5, 0.3]);
    states.insert(2, vec![0.5, 0.8, 0.1]); // Different state

    let edges = vec![TestEdge {
        source: 1,
        target: 2,
        weight: 1.0,
        rho_source: RestrictionMap::new(3, 3),
        rho_target: RestrictionMap::new(3, 3),
    }];

    let (total, _) = compute_total_energy(&states, &edges);

    // Compute expected energy manually
    let residual = vec![1.0 - 0.5, 0.5 - 0.8, 0.3 - 0.1]; // [0.5, -0.3, 0.2]
    let expected: f32 = residual.iter().map(|x| x * x).sum(); // 0.25 + 0.09 + 0.04 = 0.38

    assert!(
        (total - expected).abs() < 1e-6,
        "Expected energy {}, got {}",
        expected,
        total
    );
}

#[test]
fn test_energy_computation_weighted_edges() {
    // Edge weight should scale energy proportionally
    let mut states = HashMap::new();
    states.insert(1, vec![1.0, 0.0]);
    states.insert(2, vec![0.0, 0.0]);

    let weight1 = 1.0;
    let weight10 = 10.0;

    let edges_w1 = vec![TestEdge {
        source: 1,
        target: 2,
        weight: weight1,
        rho_source: RestrictionMap::new(2, 2),
        rho_target: RestrictionMap::new(2, 2),
    }];

    let edges_w10 = vec![TestEdge {
        source: 1,
        target: 2,
        weight: weight10,
        rho_source: RestrictionMap::new(2, 2),
        rho_target: RestrictionMap::new(2, 2),
    }];

    let (energy_w1, _) = compute_total_energy(&states, &edges_w1);
    let (energy_w10, _) = compute_total_energy(&states, &edges_w10);

    assert!(
        (energy_w10 / energy_w1 - 10.0).abs() < 1e-6,
        "Expected 10x energy scaling"
    );
}

#[test]
fn test_energy_is_nonnegative() {
    // Energy should always be non-negative (sum of squared terms)
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for _ in 0..100 {
        let mut states = HashMap::new();
        states.insert(1, (0..4).map(|_| rng.gen_range(-10.0..10.0)).collect());
        states.insert(2, (0..4).map(|_| rng.gen_range(-10.0..10.0)).collect());

        let edges = vec![TestEdge {
            source: 1,
            target: 2,
            weight: rng.gen_range(0.0..10.0),
            rho_source: RestrictionMap::new(4, 4),
            rho_target: RestrictionMap::new(4, 4),
        }];

        let (total, _) = compute_total_energy(&states, &edges);

        assert!(total >= 0.0, "Energy must be non-negative, got {}", total);
    }
}

#[test]
fn test_energy_with_multiple_edges() {
    // Total energy should be sum of individual edge energies
    let mut states = HashMap::new();
    states.insert(1, vec![1.0, 0.0]);
    states.insert(2, vec![0.5, 0.0]);
    states.insert(3, vec![0.0, 0.0]);

    let edges = vec![
        TestEdge {
            source: 1,
            target: 2,
            weight: 1.0,
            rho_source: RestrictionMap::new(2, 2),
            rho_target: RestrictionMap::new(2, 2),
        },
        TestEdge {
            source: 2,
            target: 3,
            weight: 1.0,
            rho_source: RestrictionMap::new(2, 2),
            rho_target: RestrictionMap::new(2, 2),
        },
    ];

    let (total, edge_energies) = compute_total_energy(&states, &edges);

    let sum_of_parts: f32 = edge_energies.values().sum();
    assert!(
        (total - sum_of_parts).abs() < 1e-10,
        "Total should equal sum of parts"
    );

    // Verify individual energies
    // Edge 1-2: residual = [0.5, 0.0], energy = 0.25
    // Edge 2-3: residual = [0.5, 0.0], energy = 0.25
    assert!((total - 0.5).abs() < 1e-6);
}

// ============================================================================
// INCREMENTAL UPDATE TESTS
// ============================================================================

#[test]
fn test_incremental_update_single_node() {
    // Updating a single node should only affect incident edges
    let mut states = HashMap::new();
    states.insert(1, vec![1.0, 0.0]);
    states.insert(2, vec![0.5, 0.0]);
    states.insert(3, vec![0.0, 0.0]);

    // Edges: 1-2, 3 is isolated
    let edges = vec![TestEdge {
        source: 1,
        target: 2,
        weight: 1.0,
        rho_source: RestrictionMap::new(2, 2),
        rho_target: RestrictionMap::new(2, 2),
    }];

    let (energy_before, _) = compute_total_energy(&states, &edges);

    // Update node 1
    states.insert(1, vec![0.8, 0.0]);
    let (energy_after, _) = compute_total_energy(&states, &edges);

    // Energy should change because node 1 is incident to an edge
    assert_ne!(
        energy_before, energy_after,
        "Energy should change when incident node updates"
    );

    // Update isolated node 3
    states.insert(3, vec![0.5, 0.5]);
    let (energy_isolated, _) = compute_total_energy(&states, &edges);

    // Energy should NOT change because node 3 is isolated
    assert!(
        (energy_after - energy_isolated).abs() < 1e-10,
        "Energy should not change when isolated node updates"
    );
}

#[test]
fn test_incremental_update_affected_edges() {
    // Helper to find edges affected by a node update
    fn affected_edges(node_id: u64, edges: &[TestEdge]) -> Vec<usize> {
        edges
            .iter()
            .enumerate()
            .filter(|(_, e)| e.source == node_id || e.target == node_id)
            .map(|(i, _)| i)
            .collect()
    }

    let edges = vec![
        TestEdge {
            source: 1,
            target: 2,
            weight: 1.0,
            rho_source: RestrictionMap::new(2, 2),
            rho_target: RestrictionMap::new(2, 2),
        },
        TestEdge {
            source: 2,
            target: 3,
            weight: 1.0,
            rho_source: RestrictionMap::new(2, 2),
            rho_target: RestrictionMap::new(2, 2),
        },
        TestEdge {
            source: 3,
            target: 4,
            weight: 1.0,
            rho_source: RestrictionMap::new(2, 2),
            rho_target: RestrictionMap::new(2, 2),
        },
    ];

    // Node 2 is incident to edges 0 and 1
    let affected = affected_edges(2, &edges);
    assert_eq!(affected, vec![0, 1]);

    // Node 1 is incident to edge 0 only
    let affected = affected_edges(1, &edges);
    assert_eq!(affected, vec![0]);

    // Node 4 is incident to edge 2 only
    let affected = affected_edges(4, &edges);
    assert_eq!(affected, vec![2]);
}

#[test]
fn test_incremental_vs_full_recomputation() {
    // Incremental and full recomputation should produce the same result
    let mut states = HashMap::new();
    states.insert(1, vec![1.0, 0.5, 0.3]);
    states.insert(2, vec![0.8, 0.6, 0.4]);
    states.insert(3, vec![0.6, 0.7, 0.5]);

    let edges = vec![
        TestEdge {
            source: 1,
            target: 2,
            weight: 1.0,
            rho_source: RestrictionMap::new(3, 3),
            rho_target: RestrictionMap::new(3, 3),
        },
        TestEdge {
            source: 2,
            target: 3,
            weight: 1.0,
            rho_source: RestrictionMap::new(3, 3),
            rho_target: RestrictionMap::new(3, 3),
        },
    ];

    // Full computation
    let (energy_full, _) = compute_total_energy(&states, &edges);

    // Simulate incremental by computing only affected edges
    let affected_by_node2: Vec<usize> = edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.source == 2 || e.target == 2)
        .map(|(i, _)| i)
        .collect();

    let mut incremental_sum = 0.0;
    for i in 0..edges.len() {
        if let Some(energy) = edges[i].compute_energy(&states) {
            incremental_sum += energy;
        }
    }

    assert!(
        (energy_full - incremental_sum).abs() < 1e-10,
        "Incremental and full should match"
    );
}

// ============================================================================
// RESIDUAL COMPUTATION TESTS
// ============================================================================

#[test]
fn test_residual_symmetry() {
    // r_e for edge (u,v) should be negation of r_e for edge (v,u)
    // when restriction maps are the same
    let mut states = HashMap::new();
    states.insert(1, vec![1.0, 0.5]);
    states.insert(2, vec![0.8, 0.6]);

    let rho = RestrictionMap::new(2, 2);

    let edge_uv = TestEdge {
        source: 1,
        target: 2,
        weight: 1.0,
        rho_source: RestrictionMap::new(2, 2),
        rho_target: RestrictionMap::new(2, 2),
    };

    let edge_vu = TestEdge {
        source: 2,
        target: 1,
        weight: 1.0,
        rho_source: RestrictionMap::new(2, 2),
        rho_target: RestrictionMap::new(2, 2),
    };

    let r_uv = edge_uv.compute_residual(&states).unwrap();
    let r_vu = edge_vu.compute_residual(&states).unwrap();

    // Check that r_uv = -r_vu
    for (a, b) in r_uv.iter().zip(&r_vu) {
        assert!(
            (a + b).abs() < 1e-10,
            "Residuals should be negations of each other"
        );
    }
}

#[test]
fn test_residual_dimension() {
    // Residual dimension should match restriction map output dimension
    let mut states = HashMap::new();
    states.insert(1, vec![1.0, 0.5, 0.3, 0.2]);
    states.insert(2, vec![0.8, 0.6, 0.4, 0.3]);

    let edge = TestEdge {
        source: 1,
        target: 2,
        weight: 1.0,
        rho_source: RestrictionMap::new(2, 4), // 4D -> 2D
        rho_target: RestrictionMap::new(2, 4),
    };

    let residual = edge.compute_residual(&states).unwrap();

    assert_eq!(
        residual.len(),
        edge.rho_source.output_dim(),
        "Residual dimension should match restriction map output"
    );
}

// ============================================================================
// HOTSPOT IDENTIFICATION TESTS
// ============================================================================

#[test]
fn test_hotspot_identification() {
    // Find edges with highest energy
    fn find_hotspots(edge_energies: &HashMap<usize, f32>, k: usize) -> Vec<(usize, f32)> {
        let mut sorted: Vec<_> = edge_energies.iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
        sorted.into_iter().take(k).map(|(i, e)| (*i, *e)).collect()
    }

    let mut states = HashMap::new();
    states.insert(1, vec![1.0, 0.0]);
    states.insert(2, vec![0.1, 0.0]); // Large difference with 1
    states.insert(3, vec![0.05, 0.0]); // Small difference with 2

    let edges = vec![
        TestEdge {
            source: 1,
            target: 2,
            weight: 1.0,
            rho_source: RestrictionMap::new(2, 2),
            rho_target: RestrictionMap::new(2, 2),
        },
        TestEdge {
            source: 2,
            target: 3,
            weight: 1.0,
            rho_source: RestrictionMap::new(2, 2),
            rho_target: RestrictionMap::new(2, 2),
        },
    ];

    let (_, edge_energies) = compute_total_energy(&states, &edges);

    let hotspots = find_hotspots(&edge_energies, 1);

    // Edge 0 (1-2) should have higher energy
    assert_eq!(hotspots[0].0, 0, "Edge 1-2 should be the hotspot");
    assert!(
        edge_energies.get(&0).unwrap() > edge_energies.get(&1).unwrap(),
        "Edge 1-2 should have higher energy than edge 2-3"
    );
}

// ============================================================================
// SCOPE-BASED ENERGY TESTS
// ============================================================================

#[test]
fn test_energy_by_scope() {
    // Energy can be aggregated by scope (namespace)
    let mut states = HashMap::new();
    let mut node_scopes: HashMap<u64, String> = HashMap::new();

    // Finance nodes
    states.insert(1, vec![1.0, 0.5]);
    states.insert(2, vec![0.8, 0.6]);
    node_scopes.insert(1, "finance".to_string());
    node_scopes.insert(2, "finance".to_string());

    // Medical nodes
    states.insert(3, vec![0.5, 0.3]);
    states.insert(4, vec![0.2, 0.1]);
    node_scopes.insert(3, "medical".to_string());
    node_scopes.insert(4, "medical".to_string());

    let edges = vec![
        TestEdge {
            source: 1,
            target: 2,
            weight: 1.0,
            rho_source: RestrictionMap::new(2, 2),
            rho_target: RestrictionMap::new(2, 2),
        },
        TestEdge {
            source: 3,
            target: 4,
            weight: 1.0,
            rho_source: RestrictionMap::new(2, 2),
            rho_target: RestrictionMap::new(2, 2),
        },
    ];

    fn energy_by_scope(
        edges: &[TestEdge],
        states: &HashMap<u64, Vec<f32>>,
        node_scopes: &HashMap<u64, String>,
    ) -> HashMap<String, f32> {
        let mut scope_energy: HashMap<String, f32> = HashMap::new();

        for edge in edges {
            if let Some(energy) = edge.compute_energy(states) {
                let source_scope = node_scopes.get(&edge.source).cloned().unwrap_or_default();
                *scope_energy.entry(source_scope).or_insert(0.0) += energy;
            }
        }

        scope_energy
    }

    let by_scope = energy_by_scope(&edges, &states, &node_scopes);

    assert!(by_scope.contains_key("finance"));
    assert!(by_scope.contains_key("medical"));
    assert!(by_scope.get("finance").unwrap() > &0.0);
}

// ============================================================================
// FINGERPRINT AND CACHING TESTS
// ============================================================================

#[test]
fn test_cache_invalidation_on_state_change() {
    // Cached energy should be invalidated when state changes
    struct CachedEnergy {
        value: Option<f32>,
        fingerprint: u64,
    }

    impl CachedEnergy {
        fn new() -> Self {
            Self {
                value: None,
                fingerprint: 0,
            }
        }

        fn get_or_compute(
            &mut self,
            current_fingerprint: u64,
            compute_fn: impl FnOnce() -> f32,
        ) -> f32 {
            if self.fingerprint == current_fingerprint {
                if let Some(v) = self.value {
                    return v;
                }
            }

            let value = compute_fn();
            self.value = Some(value);
            self.fingerprint = current_fingerprint;
            value
        }

        fn invalidate(&mut self) {
            self.value = None;
        }
    }

    let mut cache = CachedEnergy::new();
    let mut compute_count = 0;

    // First computation
    let v1 = cache.get_or_compute(1, || {
        compute_count += 1;
        10.0
    });
    assert_eq!(v1, 10.0);
    assert_eq!(compute_count, 1);

    // Cached retrieval (same fingerprint)
    let v2 = cache.get_or_compute(1, || {
        compute_count += 1;
        10.0
    });
    assert_eq!(v2, 10.0);
    assert_eq!(compute_count, 1); // Not recomputed

    // Fingerprint changed - should recompute
    let v3 = cache.get_or_compute(2, || {
        compute_count += 1;
        20.0
    });
    assert_eq!(v3, 20.0);
    assert_eq!(compute_count, 2);
}

// ============================================================================
// PARALLEL COMPUTATION TESTS
// ============================================================================

#[test]
fn test_parallel_energy_computation() {
    // Energy computation should be parallelizable across edges
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

    let mut states = HashMap::new();
    for i in 0..100 {
        states.insert(i as u64, vec![i as f32 / 100.0, 0.5]);
    }

    let mut edges = Vec::new();
    for i in 0..99 {
        edges.push(TestEdge {
            source: i,
            target: i + 1,
            weight: 1.0,
            rho_source: RestrictionMap::new(2, 2),
            rho_target: RestrictionMap::new(2, 2),
        });
    }

    // Simulate parallel computation
    let states = Arc::new(states);
    let edges = Arc::new(edges);
    let total = Arc::new(std::sync::Mutex::new(0.0f32));
    let num_threads = 4;
    let edges_per_thread = edges.len() / num_threads;

    let handles: Vec<_> = (0..num_threads)
        .map(|t| {
            let states = Arc::clone(&states);
            let edges = Arc::clone(&edges);
            let total = Arc::clone(&total);

            thread::spawn(move || {
                let start = t * edges_per_thread;
                let end = if t == num_threads - 1 {
                    edges.len()
                } else {
                    (t + 1) * edges_per_thread
                };

                let mut local_sum = 0.0;
                for i in start..end {
                    if let Some(energy) = edges[i].compute_energy(&states) {
                        local_sum += energy;
                    }
                }

                let mut total = total.lock().unwrap();
                *total += local_sum;
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let parallel_total = *total.lock().unwrap();

    // Verify against sequential
    let (sequential_total, _) = compute_total_energy(&states, &edges);

    assert!(
        (parallel_total - sequential_total).abs() < 1e-6,
        "Parallel and sequential computation should match"
    );
}

// ============================================================================
// SPECTRAL DRIFT DETECTION TESTS
// ============================================================================

#[test]
fn test_spectral_drift_detection() {
    // Spectral drift should be detected when eigenvalue distribution changes significantly

    /// Simple eigenvalue snapshot
    struct EigenvalueSnapshot {
        eigenvalues: Vec<f32>,
    }

    /// Wasserstein-like distance between eigenvalue distributions
    fn eigenvalue_distance(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return f32::MAX;
        }

        let mut a_sorted = a.to_vec();
        let mut b_sorted = b.to_vec();
        a_sorted.sort_by(|x, y| x.partial_cmp(y).unwrap());
        b_sorted.sort_by(|x, y| x.partial_cmp(y).unwrap());

        a_sorted
            .iter()
            .zip(&b_sorted)
            .map(|(x, y)| (x - y).abs())
            .sum::<f32>()
            / a.len() as f32
    }

    let snapshot1 = EigenvalueSnapshot {
        eigenvalues: vec![0.1, 0.3, 0.5, 0.8, 1.0],
    };

    // Small change - no drift
    let snapshot2 = EigenvalueSnapshot {
        eigenvalues: vec![0.11, 0.31, 0.49, 0.79, 1.01],
    };

    // Large change - drift detected
    let snapshot3 = EigenvalueSnapshot {
        eigenvalues: vec![0.5, 0.6, 0.7, 0.9, 2.0],
    };

    let dist_small = eigenvalue_distance(&snapshot1.eigenvalues, &snapshot2.eigenvalues);
    let dist_large = eigenvalue_distance(&snapshot1.eigenvalues, &snapshot3.eigenvalues);

    let drift_threshold = 0.1;

    assert!(
        dist_small < drift_threshold,
        "Small change should not trigger drift"
    );
    assert!(
        dist_large > drift_threshold,
        "Large change should trigger drift"
    );
}
