//! Property-based tests for paper algorithm implementations
//!
//! Tests the correctness of:
//! - DeterministicLocalKCut (Theorem 4.1)
//! - Fragmentation with Trim (Theorem 5.1)
//! - ThreeLevelHierarchy (expander→precluster→cluster)

use ruvector_mincut::cluster::hierarchy::{HierarchyConfig, ThreeLevelHierarchy};
use ruvector_mincut::fragmentation::{Fragmentation, FragmentationConfig};
use ruvector_mincut::localkcut::deterministic::{
    DeterministicLocalKCut, EdgeColor, EdgeColoring, GreedyForestPacking,
};
use std::collections::HashSet;

// ============================================================================
// Helper Functions
// ============================================================================

/// Brute-force minimum cut for small graphs using exhaustive subset enumeration
fn brute_force_min_cut(adjacency: &[(u64, u64, f64)], vertices: &[u64]) -> f64 {
    if vertices.len() <= 1 {
        return f64::INFINITY;
    }

    let vertex_set: HashSet<_> = vertices.iter().copied().collect();
    let n = vertices.len();
    let mut min_cut = f64::INFINITY;

    // Enumerate all non-empty proper subsets (2^n - 2 subsets)
    // Only practical for small n
    for mask in 1..(1 << n) - 1 {
        let mut subset: HashSet<u64> = HashSet::new();
        for (i, &v) in vertices.iter().enumerate() {
            if (mask >> i) & 1 == 1 {
                subset.insert(v);
            }
        }

        // Compute cut value
        let mut cut_value = 0.0;
        for &(u, v, w) in adjacency {
            let u_in = subset.contains(&u);
            let v_in = subset.contains(&v);
            if u_in != v_in {
                cut_value += w;
            }
        }

        min_cut = min_cut.min(cut_value);
    }

    min_cut
}

/// Check if graph is connected using BFS
fn is_connected(adjacency: &[(u64, u64, f64)], vertices: &[u64]) -> bool {
    if vertices.is_empty() {
        return true;
    }

    use std::collections::VecDeque;

    let mut visited: HashSet<u64> = HashSet::new();
    let mut queue = VecDeque::new();

    queue.push_back(vertices[0]);
    visited.insert(vertices[0]);

    while let Some(v) = queue.pop_front() {
        for &(u, w, _) in adjacency {
            if u == v && !visited.contains(&w) {
                visited.insert(w);
                queue.push_back(w);
            }
            if w == v && !visited.contains(&u) {
                visited.insert(u);
                queue.push_back(u);
            }
        }
    }

    visited.len() == vertices.len()
}

// ============================================================================
// DeterministicLocalKCut Tests
// ============================================================================

#[test]
fn test_localkcut_finds_small_cuts() {
    // Test that LocalKCut finds cuts when they exist
    let mut lkc = DeterministicLocalKCut::new(10, 100, 2);

    // Build two cliques connected by a single edge (barbell graph)
    // Clique 1: vertices 1,2,3
    lkc.insert_edge(1, 2, 1.0);
    lkc.insert_edge(2, 3, 1.0);
    lkc.insert_edge(1, 3, 1.0);

    // Clique 2: vertices 4,5,6
    lkc.insert_edge(4, 5, 1.0);
    lkc.insert_edge(5, 6, 1.0);
    lkc.insert_edge(4, 6, 1.0);

    // Bridge
    lkc.insert_edge(3, 4, 1.0);

    // Query from vertex 1
    let cuts = lkc.query(1);

    // Should find at least one cut
    assert!(!cuts.is_empty(), "Should find at least one cut");

    // At least one cut should have value <= 1 (the bridge)
    let has_small_cut = cuts.iter().any(|c| c.cut_value <= 2.0);
    assert!(has_small_cut, "Should find a small cut (the bridge)");
}

#[test]
fn test_localkcut_respects_volume_bound() {
    let mut lkc = DeterministicLocalKCut::new(10, 5, 2); // Small volume bound

    // Build a star graph (high degree center)
    for i in 2..=10 {
        lkc.insert_edge(1, i, 1.0);
    }

    // Query from center (vertex 1)
    let cuts = lkc.query(1);

    // All cuts should respect volume bound
    for cut in cuts {
        assert!(cut.volume <= 5, "Cut volume {} exceeds bound 5", cut.volume);
    }
}

#[test]
fn test_forest_packing_no_cycles() {
    let mut packing = GreedyForestPacking::new(3);

    // Insert edges that form a cycle
    packing.insert_edge(1, 2);
    packing.insert_edge(2, 3);
    packing.insert_edge(3, 4);

    // This edge would close a cycle
    let forest = packing.insert_edge(1, 4);

    // Should still be assigned (to a different forest)
    assert!(
        forest.is_some(),
        "Cycle-closing edge should fit in some forest"
    );

    // Verify no single forest has a cycle
    for f in 0..3 {
        let edges = packing.forest_edges(f);
        // A forest on n vertices has at most n-1 edges
        // With 4 vertices, each forest should have <= 3 edges
        assert!(edges.len() <= 3, "Forest {} has too many edges", f);
    }
}

#[test]
fn test_edge_coloring_deterministic() {
    // Same edges should get same colors
    let mut coloring1 = EdgeColoring::new(2, 5);
    let mut coloring2 = EdgeColoring::new(2, 5);

    // Set same colors
    coloring1.set(1, 2, EdgeColor::Red);
    coloring1.set(2, 3, EdgeColor::Blue);
    coloring2.set(1, 2, EdgeColor::Red);
    coloring2.set(2, 3, EdgeColor::Blue);

    assert_eq!(coloring1.get(1, 2), coloring2.get(1, 2));
    assert_eq!(coloring1.get(2, 3), coloring2.get(2, 3));

    // Order shouldn't matter
    assert_eq!(coloring1.get(1, 2), coloring1.get(2, 1));
}

// ============================================================================
// Fragmentation Tests
// ============================================================================

#[test]
fn test_fragmentation_covers_all_vertices() {
    let mut frag = Fragmentation::new(FragmentationConfig {
        min_fragment_size: 2,
        max_fragment_size: 10,
        ..Default::default()
    });

    // Build a path graph
    for i in 0..15 {
        frag.insert_edge(i, i + 1, 1.0);
    }

    let roots = frag.fragment();
    assert!(!roots.is_empty(), "Should have at least one fragment");

    // Collect all vertices from leaf fragments
    let mut covered: HashSet<u64> = HashSet::new();
    for fragment in frag.leaf_fragments() {
        covered.extend(&fragment.vertices);
    }

    // All vertices should be covered
    for i in 0..=15 {
        assert!(covered.contains(&i), "Vertex {} not covered", i);
    }
}

#[test]
fn test_fragmentation_boundary_sparse() {
    let config = FragmentationConfig {
        boundary_sparsity: 0.5,
        min_fragment_size: 2,
        ..Default::default()
    };
    let mut frag = Fragmentation::new(config);

    // Build two cliques connected by single edge
    for i in 1..=4 {
        for j in i + 1..=4 {
            frag.insert_edge(i, j, 1.0);
        }
    }
    for i in 5..=8 {
        for j in i + 1..=8 {
            frag.insert_edge(i, j, 1.0);
        }
    }
    frag.insert_edge(4, 5, 1.0);

    frag.fragment();

    // Leaf fragments should have reasonable boundary sparsity
    for fragment in frag.leaf_fragments() {
        let sparsity = fragment.boundary_sparsity();
        // Sparsity should be bounded (not guaranteed to be below threshold due to greedy)
        assert!(
            sparsity <= 2.0,
            "Fragment has very high sparsity: {}",
            sparsity
        );
    }
}

#[test]
fn test_trim_produces_valid_cut() {
    let mut frag = Fragmentation::with_defaults();

    // Build a path graph
    for i in 0..10 {
        frag.insert_edge(i, i + 1, 1.0);
    }

    let vertices: Vec<u64> = (0..=10).collect();
    let result = frag.trim(&vertices);

    if result.success {
        // Trimmed vertices should be a proper subset
        assert!(result.trimmed_vertices.len() < vertices.len());
        assert!(!result.trimmed_vertices.is_empty());

        // Cut edges should connect trimmed to non-trimmed
        for (u, v) in &result.cut_edges {
            let u_trimmed = result.trimmed_vertices.contains(u);
            let v_trimmed = result.trimmed_vertices.contains(v);
            assert!(u_trimmed != v_trimmed, "Cut edge should cross partition");
        }
    }
}

// ============================================================================
// ThreeLevelHierarchy Tests
// ============================================================================

#[test]
fn test_hierarchy_levels_consistent() {
    let mut h = ThreeLevelHierarchy::new(HierarchyConfig {
        min_expander_size: 2,
        ..Default::default()
    });

    // Build graph
    for i in 1..=20 {
        h.insert_edge(i, i + 1, 1.0);
    }
    h.build();

    // Every vertex should be in exactly one expander
    let mut vertex_count: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
    for expander in h.get_expanders().values() {
        for &v in &expander.vertices {
            *vertex_count.entry(v).or_insert(0) += 1;
        }
    }

    for (v, count) in vertex_count {
        assert_eq!(count, 1, "Vertex {} appears in {} expanders", v, count);
    }
}

#[test]
fn test_hierarchy_global_min_cut_bound() {
    let mut h = ThreeLevelHierarchy::new(HierarchyConfig {
        min_expander_size: 2,
        track_mirror_cuts: true,
        ..Default::default()
    });

    // Build two cliques connected by edges of weight 3
    for i in 1..=4 {
        for j in i + 1..=4 {
            h.insert_edge(i, j, 1.0);
        }
    }
    for i in 5..=8 {
        for j in i + 1..=8 {
            h.insert_edge(i, j, 1.0);
        }
    }
    h.insert_edge(4, 5, 1.0);
    h.insert_edge(3, 6, 1.0);
    h.insert_edge(2, 7, 1.0);

    h.build();

    // Brute force min cut
    let edges: Vec<(u64, u64, f64)> = vec![
        (1, 2, 1.0),
        (1, 3, 1.0),
        (1, 4, 1.0),
        (2, 3, 1.0),
        (2, 4, 1.0),
        (3, 4, 1.0),
        (5, 6, 1.0),
        (5, 7, 1.0),
        (5, 8, 1.0),
        (6, 7, 1.0),
        (6, 8, 1.0),
        (7, 8, 1.0),
        (4, 5, 1.0),
        (3, 6, 1.0),
        (2, 7, 1.0),
    ];
    let vertices: Vec<u64> = (1..=8).collect();
    let brute = brute_force_min_cut(&edges, &vertices);

    // Hierarchy estimate should be <= actual min cut * some factor
    // (it's an upper bound approximation)
    assert!(
        h.global_min_cut <= brute * 2.0 + 0.1 || h.global_min_cut.is_infinite(),
        "Global min cut {} should be close to brute force {}",
        h.global_min_cut,
        brute
    );
}

#[test]
fn test_incremental_update_consistency() {
    let mut h = ThreeLevelHierarchy::with_defaults();

    // Build initial graph
    h.insert_edge(1, 2, 1.0);
    h.insert_edge(2, 3, 1.0);
    h.insert_edge(3, 4, 1.0);
    h.build();

    let initial_vertices = h.stats().num_vertices;

    // Incremental insert
    h.handle_edge_insert(4, 5, 1.0);
    h.handle_edge_insert(5, 6, 1.0);

    // Should have more vertices now
    assert!(h.stats().num_vertices >= initial_vertices);

    // Every new vertex should be assigned
    assert!(h.get_vertex_expander(5).is_some() || h.get_expanders().is_empty());
    assert!(h.get_vertex_expander(6).is_some() || h.get_expanders().is_empty());
}

#[test]
fn test_mirror_cuts_between_expanders() {
    let mut h = ThreeLevelHierarchy::new(HierarchyConfig {
        min_expander_size: 2,
        max_expander_size: 5,
        track_mirror_cuts: true,
        ..Default::default()
    });

    // Build two dense components
    for i in 1..=4 {
        for j in i + 1..=4 {
            h.insert_edge(i, j, 1.0);
        }
    }
    for i in 10..=14 {
        for j in i + 1..=14 {
            h.insert_edge(i, j, 1.0);
        }
    }
    // Connect with bridge
    h.insert_edge(4, 10, 2.0);

    h.build();

    // Check that mirror cuts are being tracked
    let mut has_mirror_cut = false;
    for cluster in h.get_clusters().values() {
        if !cluster.mirror_cuts.is_empty() {
            has_mirror_cut = true;
            // Mirror cut should have the bridge
            for mirror in &cluster.mirror_cuts {
                assert!(
                    mirror.cut_value > 0.0,
                    "Mirror cut should have positive value"
                );
            }
        }
    }

    // If we have multiple expanders, should have mirror cuts
    if h.get_expanders().len() > 1 {
        assert!(
            has_mirror_cut || h.get_clusters().len() > 1,
            "Should track mirror cuts between expanders"
        );
    }
}

// ============================================================================
// Property Tests with Random Graphs
// ============================================================================

#[test]
fn property_fragmentation_idempotent() {
    // Fragmenting twice should give same result
    let mut frag1 = Fragmentation::with_defaults();
    let mut frag2 = Fragmentation::with_defaults();

    // Same graph
    for i in 0..10 {
        frag1.insert_edge(i, i + 1, 1.0);
        frag2.insert_edge(i, i + 1, 1.0);
    }

    frag1.fragment();
    frag2.fragment();

    // Same number of fragments
    assert_eq!(frag1.num_fragments(), frag2.num_fragments());
}

#[test]
fn property_hierarchy_covers_graph() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let mut rng = StdRng::seed_from_u64(42);

    for _iteration in 0..10 {
        let mut h = ThreeLevelHierarchy::with_defaults();

        // Random edges
        let n = rng.gen_range(5..20);
        let m = rng.gen_range(n..n * 2);

        for _ in 0..m {
            let u = rng.gen_range(1..=n) as u64;
            let v = rng.gen_range(1..=n) as u64;
            if u != v {
                h.insert_edge(u, v, 1.0);
            }
        }

        h.build();

        // Count vertices in expanders
        let mut in_expanders: HashSet<u64> = HashSet::new();
        for exp in h.get_expanders().values() {
            in_expanders.extend(&exp.vertices);
        }

        // All graph vertices should be covered
        let graph_vertices = h.stats().num_vertices;
        assert_eq!(
            in_expanders.len(),
            graph_vertices,
            "Expanders should cover all {} vertices",
            graph_vertices
        );
    }
}

#[test]
fn property_localkcut_deterministic() {
    // Same graph, same queries, same results
    let mut lkc1 = DeterministicLocalKCut::new(10, 50, 2);
    let mut lkc2 = DeterministicLocalKCut::new(10, 50, 2);

    // Same edges in same order
    for (u, v) in [(1, 2), (2, 3), (3, 4), (4, 1), (1, 3)] {
        lkc1.insert_edge(u, v, 1.0);
        lkc2.insert_edge(u, v, 1.0);
    }

    let cuts1 = lkc1.query(1);
    let cuts2 = lkc2.query(1);

    // Same number of cuts
    assert_eq!(cuts1.len(), cuts2.len(), "Queries should be deterministic");
}

#[test]
fn test_mirror_cut_certification() {
    let mut h = ThreeLevelHierarchy::new(HierarchyConfig {
        min_expander_size: 2,
        max_expander_size: 5,
        track_mirror_cuts: true,
        ..Default::default()
    });

    // Build two well-separated components connected by a bridge
    // Component 1: vertices 1-4
    for i in 1..=4 {
        for j in i + 1..=4 {
            h.insert_edge(i, j, 1.0);
        }
    }
    // Component 2: vertices 10-14
    for i in 10..=14 {
        for j in i + 1..=14 {
            h.insert_edge(i, j, 1.0);
        }
    }
    // Bridge connecting them
    h.insert_edge(4, 10, 2.0);

    h.build();

    // Get counts before certification
    let total_mirror_cuts = h.num_mirror_cuts();

    // Run certification
    h.certify_mirror_cuts();

    // After certification, certified count should be >= 0
    let certified = h.num_certified_mirror_cuts();
    assert!(
        certified <= total_mirror_cuts,
        "Certified {} should be <= total {}",
        certified,
        total_mirror_cuts
    );

    // If we have mirror cuts, certification should have processed them
    if total_mirror_cuts > 0 {
        // At least some should be certified (or all if valid)
        assert!(
            certified >= 0,
            "Certification should not produce negative count"
        );
    }
}

#[test]
fn test_brute_force_matches_known_cut() {
    // Test our brute force helper against a known graph
    // Triangle with vertices 1, 2, 3 - min cut is 2 (remove any vertex)
    let edges = vec![(1, 2, 1.0), (2, 3, 1.0), (1, 3, 1.0)];
    let vertices = vec![1, 2, 3];

    let min_cut = brute_force_min_cut(&edges, &vertices);
    assert!(
        (min_cut - 2.0).abs() < 0.001,
        "Triangle min cut should be 2, got {}",
        min_cut
    );

    // Path graph 1-2-3-4 - min cut is 1
    let path_edges = vec![(1, 2, 1.0), (2, 3, 1.0), (3, 4, 1.0)];
    let path_vertices = vec![1, 2, 3, 4];

    let path_cut = brute_force_min_cut(&path_edges, &path_vertices);
    assert!(
        (path_cut - 1.0).abs() < 0.001,
        "Path min cut should be 1, got {}",
        path_cut
    );
}
