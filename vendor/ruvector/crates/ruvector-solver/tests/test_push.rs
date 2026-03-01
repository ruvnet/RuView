//! Integration tests for Forward Push, Backward Push, and mass conservation.
//!
//! Tests cover PPR computation on small graphs, star/complete topologies,
//! mass conservation invariants, and agreement between forward and backward
//! push algorithms.

mod helpers;

use approx::assert_relative_eq;
#[cfg(feature = "backward-push")]
use ruvector_solver::backward_push::BackwardPushSolver;
use ruvector_solver::forward_push::{forward_push_with_residuals, ForwardPushSolver};
#[allow(unused_imports)]
use ruvector_solver::traits::SublinearPageRank;
use ruvector_solver::types::CsrMatrix;

use helpers::adjacency_from_edges;

// ---------------------------------------------------------------------------
// Helper: build common graph topologies
// ---------------------------------------------------------------------------

/// 4-node graph: 0--1--2--3, 0--2 (bidirectional).
fn simple_graph_4() -> CsrMatrix<f64> {
    adjacency_from_edges(4, &[(0, 1), (1, 2), (2, 3), (0, 2)])
}

/// Star graph centred at 0 with k leaves (bidirectional edges).
fn star_graph(k: usize) -> CsrMatrix<f64> {
    let n = k + 1;
    let edges: Vec<(usize, usize)> = (1..n).map(|i| (0, i)).collect();
    adjacency_from_edges(n, &edges)
}

/// Complete graph on n vertices (bidirectional edges, no self-loops).
fn complete_graph(n: usize) -> CsrMatrix<f64> {
    let mut edges = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            edges.push((i, j));
        }
    }
    adjacency_from_edges(n, &edges)
}

/// Directed cycle: 0->1->2->...->n-1->0.
fn directed_cycle(n: usize) -> CsrMatrix<f64> {
    let entries: Vec<(usize, usize, f64)> = (0..n).map(|i| (i, (i + 1) % n, 1.0f64)).collect();
    CsrMatrix::<f64>::from_coo(n, n, entries)
}

// ---------------------------------------------------------------------------
// Forward Push: 4-node graph
// ---------------------------------------------------------------------------

#[test]
fn test_forward_push_simple_graph() {
    let graph = simple_graph_4();
    let solver = ForwardPushSolver::new(0.85, 1e-8);
    let result = solver.ppr_from_source(&graph, 0).unwrap();

    // Source should have highest PPR.
    assert!(!result.is_empty());
    assert_eq!(result[0].0, 0, "source vertex should be ranked first");
    assert!(result[0].1 > 0.0);

    // All returned scores should be positive and sorted descending.
    for w in result.windows(2) {
        assert!(
            w[0].1 >= w[1].1,
            "results should be sorted descending: {} < {}",
            w[0].1,
            w[1].1
        );
    }

    // Verify all 4 nodes get some probability.
    let nodes: Vec<usize> = result.iter().map(|(v, _)| *v).collect();
    for v in 0..4 {
        assert!(
            nodes.contains(&v),
            "node {} should appear in PPR results",
            v
        );
    }

    // Neighbours of source should have more PPR than non-neighbours.
    let ppr: Vec<f64> = {
        let mut dense = vec![0.0f64; 4];
        for &(v, s) in &result {
            dense[v] = s;
        }
        dense
    };
    // 0 is connected to 1 and 2. 3 is only reachable through 2.
    assert!(
        ppr[1] > ppr[3] || ppr[2] > ppr[3],
        "direct neighbours should have higher PPR than distant nodes"
    );
}

// ---------------------------------------------------------------------------
// Forward Push: star graph — center should dominate
// ---------------------------------------------------------------------------

#[test]
fn test_forward_push_star_graph() {
    let graph = star_graph(5); // 6 nodes: center=0, leaves=1..5
    let solver = ForwardPushSolver::new(0.85, 1e-8);

    // PPR from center: center should have highest score.
    let result = solver.ppr_from_source(&graph, 0).unwrap();
    assert_eq!(result[0].0, 0);

    // All leaf scores should be approximately equal (by symmetry).
    let leaf_scores: Vec<f64> = result
        .iter()
        .filter(|(v, _)| *v != 0)
        .map(|(_, s)| *s)
        .collect();
    assert_eq!(leaf_scores.len(), 5);

    let mean = leaf_scores.iter().sum::<f64>() / leaf_scores.len() as f64;
    for &s in &leaf_scores {
        assert_relative_eq!(s, mean, epsilon = 1e-6);
    }

    // Center PPR should be strictly higher than any leaf.
    for &s in &leaf_scores {
        assert!(result[0].1 > s, "center PPR should exceed leaf PPR");
    }
}

// ---------------------------------------------------------------------------
// Forward Push: complete graph — approximately uniform
// ---------------------------------------------------------------------------

#[test]
fn test_forward_push_complete_graph() {
    let n = 5;
    let graph = complete_graph(n);
    let solver = ForwardPushSolver::new(0.85, 1e-8);

    let result = solver.ppr_from_source(&graph, 0).unwrap();

    // All n nodes should appear.
    assert_eq!(result.len(), n);

    // Non-source nodes should have approximately equal PPR.
    let non_source: Vec<f64> = result
        .iter()
        .filter(|(v, _)| *v != 0)
        .map(|(_, s)| *s)
        .collect();
    let mean = non_source.iter().sum::<f64>() / non_source.len() as f64;

    for &s in &non_source {
        assert_relative_eq!(s, mean, epsilon = 1e-6);
    }
}

// ---------------------------------------------------------------------------
// Forward Push: mass conservation
// ---------------------------------------------------------------------------

#[test]
fn test_forward_push_mass_conservation() {
    let graph = simple_graph_4();
    let (p, r) = forward_push_with_residuals(&graph, 0, 0.85, 1e-8).unwrap();

    let total_p: f64 = p.iter().sum();
    let total_r: f64 = r.iter().sum();
    let total = total_p + total_r;

    assert_relative_eq!(total, 1.0, epsilon = 1e-6);

    // Also verify on star graph.
    let star = star_graph(4);
    let (p2, r2) = forward_push_with_residuals(&star, 0, 0.85, 1e-6).unwrap();
    let total2 = p2.iter().sum::<f64>() + r2.iter().sum::<f64>();
    assert_relative_eq!(total2, 1.0, epsilon = 1e-5);

    // And on directed cycle.
    let cycle = directed_cycle(6);
    let (p3, r3) = forward_push_with_residuals(&cycle, 0, 0.85, 1e-6).unwrap();
    let total3 = p3.iter().sum::<f64>() + r3.iter().sum::<f64>();
    assert_relative_eq!(total3, 1.0, epsilon = 1e-5);
}

// ---------------------------------------------------------------------------
// Backward Push: simple verification
// ---------------------------------------------------------------------------

#[cfg(feature = "backward-push")]
#[test]
fn test_backward_push_simple() {
    let graph = directed_cycle(4); // 0->1->2->3->0
    let solver = BackwardPushSolver::new(0.15, 1e-6);

    // Backward push to target 0: nodes that can reach 0 should have PPR.
    let result = solver.ppr_to_target(&graph, 0).unwrap();

    assert!(!result.is_empty());

    // The target node itself should have the highest PPR.
    let target_ppr = result
        .iter()
        .find(|&&(v, _)| v == 0)
        .map(|&(_, p)| p)
        .unwrap_or(0.0);
    assert!(target_ppr > 0.0, "target should have positive PPR");

    // Total PPR should be <= 1.
    let total: f64 = result.iter().map(|(_, v)| v).sum();
    assert!(
        total <= 1.0 + 1e-6,
        "total PPR should be <= 1, got {}",
        total
    );
}

// ---------------------------------------------------------------------------
// Random walk pairwise: forward and backward push should agree
// ---------------------------------------------------------------------------

#[cfg(feature = "backward-push")]
#[test]
fn test_random_walk_pairwise() {
    // On a symmetric graph, forward push from s and backward push to s
    // should produce similar PPR distributions (up to algorithm variance).
    let graph = complete_graph(5);

    let forward = ForwardPushSolver::new(0.15, 1e-8);
    let backward = BackwardPushSolver::new(0.15, 1e-8);

    let source = 0;

    // Forward push from source 0.
    let fwd_result = forward.ppr_from_source(&graph, source).unwrap();
    let mut fwd_ppr = vec![0.0f64; 5];
    for &(v, s) in &fwd_result {
        fwd_ppr[v] = s;
    }

    // Backward push to target 0.
    let bwd_result = backward.ppr_to_target(&graph, source).unwrap();
    let mut bwd_ppr = vec![0.0f64; 5];
    for &(v, s) in &bwd_result {
        bwd_ppr[v] = s;
    }

    // On a symmetric complete graph, forward PPR(0 -> v) should equal
    // backward PPR(v -> 0), which is what backward push computes.
    // The self-PPR (source=target=0) should match closely.
    let fwd_self = fwd_ppr[0];
    let bwd_self = bwd_ppr[0];

    // They should agree to reasonable precision on a symmetric graph.
    let self_ppr_diff = (fwd_self - bwd_self).abs();
    assert!(
        self_ppr_diff < 0.1,
        "self-PPR should agree: forward={}, backward={}, diff={}",
        fwd_self,
        bwd_self,
        self_ppr_diff
    );

    // Non-source nodes should have similar PPR in both directions
    // (by symmetry of the complete graph).
    for v in 1..5 {
        let diff = (fwd_ppr[v] - bwd_ppr[v]).abs();
        assert!(
            diff < 0.1,
            "PPR for node {} should agree: forward={}, backward={}, diff={}",
            v,
            fwd_ppr[v],
            bwd_ppr[v],
            diff
        );
    }
}
