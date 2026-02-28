//! Tests for the canonical min-cut module.

use super::*;

// ---------------------------------------------------------------------------
// FixedWeight tests
// ---------------------------------------------------------------------------

#[test]
fn test_fixed_weight_ordering() {
    let a = FixedWeight::from_f64(1.0);
    let b = FixedWeight::from_f64(2.0);
    let c = FixedWeight::from_f64(1.0);

    assert!(a < b);
    assert!(b > a);
    assert_eq!(a, c);
    assert!(a <= c);
    assert!(a >= c);
}

#[test]
fn test_fixed_weight_arithmetic() {
    let a = FixedWeight::from_f64(1.5);
    let b = FixedWeight::from_f64(2.25);

    let sum = a.add(b);
    assert!((sum.to_f64() - 3.75).abs() < 1e-6);

    let diff = b.sub(a);
    assert!((diff.to_f64() - 0.75).abs() < 1e-6);

    // Saturating sub
    let zero = a.sub(b);
    assert_eq!(zero.to_f64(), 0.0);
}

#[test]
fn test_fixed_weight_roundtrip() {
    let values = [0.0, 1.0, 0.5, 3.14159, 100.001, 0.0001];
    for &v in &values {
        let fw = FixedWeight::from_f64(v);
        let back = fw.to_f64();
        assert!(
            (back - v).abs() < 1e-4,
            "Roundtrip failed for {}: got {}",
            v,
            back
        );
    }
}

#[test]
fn test_fixed_weight_negative_clamps() {
    let fw = FixedWeight::from_f64(-5.0);
    assert_eq!(fw.to_f64(), 0.0);
}

#[test]
fn test_fixed_weight_zero() {
    let z = FixedWeight::zero();
    assert_eq!(z.to_f64(), 0.0);
    assert_eq!(z.raw(), 0);
}

#[test]
fn test_fixed_weight_display() {
    let fw = FixedWeight::from_f64(3.5);
    let s = format!("{}", fw);
    assert!(s.contains("3.5"), "Display should show 3.5, got {}", s);
}

// ---------------------------------------------------------------------------
// CactusGraph construction tests
// ---------------------------------------------------------------------------

#[test]
fn test_cactus_construction_empty() {
    let graph = DynamicGraph::new();
    let cactus = CactusGraph::build_from_graph(&graph);
    assert_eq!(cactus.n_vertices, 0);
    assert_eq!(cactus.n_edges, 0);
}

#[test]
fn test_cactus_construction_singleton() {
    let graph = DynamicGraph::new();
    graph.add_vertex(42);
    let cactus = CactusGraph::build_from_graph(&graph);
    assert_eq!(cactus.n_vertices, 1);
    assert_eq!(cactus.n_edges, 0);
    assert!(cactus.vertex_map.contains_key(&42));
}

#[test]
fn test_cactus_construction_simple_edge() {
    let graph = DynamicGraph::new();
    graph.insert_edge(1, 2, 1.0).unwrap();

    let cactus = CactusGraph::build_from_graph(&graph);
    // Two vertices, one edge between them
    assert!(cactus.n_vertices >= 1);
    assert!(cactus.vertex_map.contains_key(&1));
    assert!(cactus.vertex_map.contains_key(&2));
}

#[test]
fn test_cactus_construction_triangle() {
    let graph = DynamicGraph::new();
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 3, 1.0).unwrap();
    graph.insert_edge(3, 1, 1.0).unwrap();

    let cactus = CactusGraph::build_from_graph(&graph);

    // Triangle has min-cut = 2, each vertex is a min-cut
    assert!(cactus.n_vertices >= 1);
    // All three vertices should be mapped
    assert!(cactus.vertex_map.contains_key(&1));
    assert!(cactus.vertex_map.contains_key(&2));
    assert!(cactus.vertex_map.contains_key(&3));
}

#[test]
fn test_cactus_construction_path() {
    let graph = DynamicGraph::new();
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 3, 1.0).unwrap();
    graph.insert_edge(3, 4, 1.0).unwrap();

    let cactus = CactusGraph::build_from_graph(&graph);

    // Path graph: min-cut = 1
    assert!(cactus.n_vertices >= 1);
    for &v in &[1, 2, 3, 4] {
        assert!(
            cactus.vertex_map.contains_key(&(v as usize)),
            "Vertex {} not in vertex_map",
            v
        );
    }
}

// ---------------------------------------------------------------------------
// Canonical determinism tests
// ---------------------------------------------------------------------------

#[test]
fn test_canonical_determinism() {
    // Same graph must always produce the same canonical cut over 100 runs.
    let mut keys = Vec::new();

    for _ in 0..100 {
        let graph = DynamicGraph::new();
        graph.insert_edge(1, 2, 1.0).unwrap();
        graph.insert_edge(2, 3, 2.0).unwrap();
        graph.insert_edge(3, 4, 1.0).unwrap();
        graph.insert_edge(4, 1, 2.0).unwrap();

        let mut cactus = CactusGraph::build_from_graph(&graph);
        cactus.root_at_lex_smallest();
        let result = cactus.canonical_cut();
        keys.push(result.canonical_key);
    }

    // All keys must be identical
    let first = keys[0];
    for (i, key) in keys.iter().enumerate() {
        assert_eq!(*key, first, "Run {} produced different canonical key", i);
    }
}

#[test]
fn test_canonical_determinism_different_insertion_order() {
    // Build the same graph with edges inserted in different orders
    let orders: Vec<Vec<(u64, u64, f64)>> = vec![
        vec![(1, 2, 1.0), (2, 3, 1.0), (3, 4, 1.0), (4, 1, 1.0)],
        vec![(4, 1, 1.0), (3, 4, 1.0), (2, 3, 1.0), (1, 2, 1.0)],
        vec![(2, 3, 1.0), (4, 1, 1.0), (1, 2, 1.0), (3, 4, 1.0)],
    ];

    let mut keys = Vec::new();

    for edges in &orders {
        let graph = DynamicGraph::new();
        for &(u, v, w) in edges {
            graph.insert_edge(u, v, w).unwrap();
        }
        let mut cactus = CactusGraph::build_from_graph(&graph);
        cactus.root_at_lex_smallest();
        let result = cactus.canonical_cut();
        keys.push(result.canonical_key);
    }

    for (i, key) in keys.iter().enumerate() {
        assert_eq!(
            *key, keys[0],
            "Order {} produced different canonical key",
            i
        );
    }
}

// ---------------------------------------------------------------------------
// Canonical cut value correctness
// ---------------------------------------------------------------------------

#[test]
fn test_canonical_value_correctness_triangle() {
    let mc = crate::MinCutBuilder::new()
        .exact()
        .with_edges(vec![(1, 2, 1.0), (2, 3, 1.0), (3, 1, 1.0)])
        .build()
        .unwrap();

    let canonical = CanonicalMinCutImpl::from_dynamic(mc);
    let true_value = canonical.min_cut_value();
    let result = canonical.canonical_cut();

    // Canonical cut value should equal the true min-cut
    assert_eq!(true_value, 2.0);
    assert!(
        (result.value - true_value).abs() < 1e-9,
        "Canonical value {} != true min-cut {}",
        result.value,
        true_value
    );
}

#[test]
fn test_canonical_value_correctness_path() {
    let mc = crate::MinCutBuilder::new()
        .exact()
        .with_edges(vec![(1, 2, 3.0), (2, 3, 1.0), (3, 4, 5.0)])
        .build()
        .unwrap();

    let canonical = CanonicalMinCutImpl::from_dynamic(mc);
    let true_value = canonical.min_cut_value();

    // Path graph min-cut = min edge weight = 1.0
    assert_eq!(true_value, 1.0);

    let result = canonical.canonical_cut();
    assert!(
        (result.value - true_value).abs() < 1e-9,
        "Canonical value {} != true min-cut {}",
        result.value,
        true_value
    );
}

#[test]
fn test_canonical_value_correctness_bridge() {
    // Two triangles connected by a bridge
    let mc = crate::MinCutBuilder::new()
        .exact()
        .with_edges(vec![
            (1, 2, 2.0),
            (2, 3, 2.0),
            (3, 1, 2.0),
            (3, 4, 1.0), // bridge
            (4, 5, 2.0),
            (5, 6, 2.0),
            (6, 4, 2.0),
        ])
        .build()
        .unwrap();

    let canonical = CanonicalMinCutImpl::from_dynamic(mc);
    let true_value = canonical.min_cut_value();

    // Min-cut = bridge weight = 1.0
    assert_eq!(true_value, 1.0);

    let result = canonical.canonical_cut();
    assert!(
        (result.value - true_value).abs() < 1e-9,
        "Canonical value {} != true min-cut {}",
        result.value,
        true_value
    );
}

// ---------------------------------------------------------------------------
// Partition correctness
// ---------------------------------------------------------------------------

#[test]
fn test_canonical_partition_covers_all_vertices() {
    let mc = crate::MinCutBuilder::new()
        .exact()
        .with_edges(vec![(1, 2, 1.0), (2, 3, 1.0), (3, 4, 1.0), (4, 1, 1.0)])
        .build()
        .unwrap();

    let canonical = CanonicalMinCutImpl::from_dynamic(mc);
    let result = canonical.canonical_cut();

    let (ref s, ref t) = result.partition;
    let mut all: Vec<usize> = s.iter().chain(t.iter()).copied().collect();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 4, "Partition must cover all 4 vertices");
    assert!(!s.is_empty(), "S partition must be non-empty");
    assert!(!t.is_empty(), "T partition must be non-empty");
}

// ---------------------------------------------------------------------------
// WitnessReceipt tests
// ---------------------------------------------------------------------------

#[test]
fn test_witness_receipt() {
    let mc = crate::MinCutBuilder::new()
        .exact()
        .with_edges(vec![(1, 2, 1.0), (2, 3, 1.0)])
        .build()
        .unwrap();

    let canonical = CanonicalMinCutImpl::from_dynamic(mc);
    let receipt = canonical.witness_receipt();

    assert_eq!(receipt.epoch, 0);
    assert_eq!(receipt.cut_value, 1.0);
    assert!(receipt.timestamp_ns > 0);
    assert!(receipt.edge_count >= 1);
}

#[test]
fn test_witness_receipt_epoch_increments() {
    let mut canonical = CanonicalMinCutImpl::with_edges(vec![(1, 2, 1.0), (2, 3, 1.0)]).unwrap();

    let r1 = canonical.witness_receipt();
    assert_eq!(r1.epoch, 0);

    canonical.insert_edge(3, 4, 1.0).unwrap();
    let r2 = canonical.witness_receipt();
    assert_eq!(r2.epoch, 1);

    canonical.delete_edge(1, 2).unwrap();
    let r3 = canonical.witness_receipt();
    assert_eq!(r3.epoch, 2);
}

// ---------------------------------------------------------------------------
// Dynamic canonical tests
// ---------------------------------------------------------------------------

#[test]
fn test_dynamic_canonical_insert() {
    let mut canonical = CanonicalMinCutImpl::new();

    canonical.insert_edge(1, 2, 1.0).unwrap();
    assert_eq!(canonical.min_cut_value(), 1.0);
    assert_eq!(canonical.num_vertices(), 2);
    assert_eq!(canonical.num_edges(), 1);

    canonical.insert_edge(2, 3, 1.0).unwrap();
    assert_eq!(canonical.min_cut_value(), 1.0);

    canonical.insert_edge(3, 1, 1.0).unwrap();
    assert_eq!(canonical.min_cut_value(), 2.0);
}

#[test]
fn test_dynamic_canonical_delete_preserves_property() {
    let mut canonical =
        CanonicalMinCutImpl::with_edges(vec![(1, 2, 1.0), (2, 3, 1.0), (3, 1, 1.0)]).unwrap();

    assert_eq!(canonical.min_cut_value(), 2.0);

    // After deleting an edge from the triangle, min-cut drops to 1.0
    canonical.delete_edge(1, 2).unwrap();
    assert_eq!(canonical.min_cut_value(), 1.0);
    assert!(canonical.is_connected());

    // The canonical cut should still be deterministic
    let r1 = canonical.canonical_cut();
    let r2 = canonical.canonical_cut();
    assert_eq!(r1.canonical_key, r2.canonical_key);
}

#[test]
fn test_dynamic_canonical_insert_delete_cycle() {
    let mut canonical = CanonicalMinCutImpl::with_edges(vec![(1, 2, 1.0), (2, 3, 1.0)]).unwrap();

    let key_before = canonical.canonical_cut().canonical_key;

    // Insert and then delete the same edge -- should return to original state
    canonical.insert_edge(3, 4, 1.0).unwrap();
    canonical.delete_edge(3, 4).unwrap();

    let key_after = canonical.canonical_cut().canonical_key;
    assert_eq!(
        key_before, key_after,
        "Insert+delete should restore canonical state"
    );
}

// ---------------------------------------------------------------------------
// CanonicalMinCutImpl API tests
// ---------------------------------------------------------------------------

#[test]
fn test_canonical_impl_new_empty() {
    let c = CanonicalMinCutImpl::new();
    assert_eq!(c.min_cut_value(), f64::INFINITY);
    assert_eq!(c.num_vertices(), 0);
    assert_eq!(c.num_edges(), 0);
}

#[test]
fn test_canonical_impl_default() {
    let c = CanonicalMinCutImpl::default();
    assert_eq!(c.min_cut_value(), f64::INFINITY);
}

#[test]
fn test_canonical_impl_with_edges() {
    let c = CanonicalMinCutImpl::with_edges(vec![(1, 2, 1.0), (2, 3, 1.0)]).unwrap();

    assert_eq!(c.num_vertices(), 3);
    assert_eq!(c.num_edges(), 2);
    assert_eq!(c.min_cut_value(), 1.0);
    assert!(c.is_connected());
}

#[test]
fn test_canonical_impl_cactus_graph() {
    let c = CanonicalMinCutImpl::with_edges(vec![(1, 2, 1.0), (2, 3, 1.0), (3, 1, 1.0)]).unwrap();

    let cactus = c.cactus_graph();
    assert!(cactus.n_vertices >= 1);
    assert!(cactus.vertex_map.contains_key(&1));
    assert!(cactus.vertex_map.contains_key(&2));
    assert!(cactus.vertex_map.contains_key(&3));
}

// ---------------------------------------------------------------------------
// Enumerate min-cuts
// ---------------------------------------------------------------------------

#[test]
fn test_enumerate_min_cuts_path() {
    let graph = DynamicGraph::new();
    graph.insert_edge(1, 2, 1.0).unwrap();
    graph.insert_edge(2, 3, 1.0).unwrap();

    let mut cactus = CactusGraph::build_from_graph(&graph);
    cactus.root_at_lex_smallest();

    let cuts = cactus.enumerate_min_cuts();
    // A path of 3 vertices has 2 min-cuts (each edge is a min-cut)
    assert!(
        !cuts.is_empty(),
        "Path graph should have at least one enumerated cut"
    );
}

#[test]
fn test_enumerate_min_cuts_single_edge() {
    let graph = DynamicGraph::new();
    graph.insert_edge(10, 20, 5.0).unwrap();

    let mut cactus = CactusGraph::build_from_graph(&graph);
    cactus.root_at_lex_smallest();

    let cuts = cactus.enumerate_min_cuts();
    assert!(
        !cuts.is_empty(),
        "Single edge graph should have at least one cut"
    );

    // The one cut should separate vertex 10 from vertex 20
    let (ref s, ref t) = cuts[0];
    let all: HashSet<usize> = s.iter().chain(t.iter()).copied().collect();
    assert!(all.contains(&10));
    assert!(all.contains(&20));
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_canonical_disconnected_graph() {
    let mc = crate::MinCutBuilder::new()
        .exact()
        .with_edges(vec![(1, 2, 1.0), (3, 4, 1.0)])
        .build()
        .unwrap();

    let canonical = CanonicalMinCutImpl::from_dynamic(mc);
    assert!(!canonical.is_connected());
    assert_eq!(canonical.min_cut_value(), 0.0);
}

#[test]
fn test_canonical_complete_k4() {
    let mut edges = Vec::new();
    for i in 1u64..=4 {
        for j in (i + 1)..=4 {
            edges.push((i, j, 1.0));
        }
    }

    let mc = crate::MinCutBuilder::new()
        .exact()
        .with_edges(edges)
        .build()
        .unwrap();

    let canonical = CanonicalMinCutImpl::from_dynamic(mc);
    assert_eq!(canonical.min_cut_value(), 3.0);

    let result = canonical.canonical_cut();
    // K4 min-cut = 3 (isolate one vertex)
    let (ref s, ref t) = result.partition;
    assert!(
        s.len() == 1 || t.len() == 1,
        "K4 min-cut isolates one vertex"
    );
}
