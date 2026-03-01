//! DAG integration tests

use ruvector_dag::dag::{OperatorNode, OperatorType, QueryDag};

#[test]
fn test_complex_query_dag() {
    // Build a realistic query DAG
    let mut dag = QueryDag::new();

    // Add scan nodes
    let scan1 = dag.add_node(OperatorNode::seq_scan(0, "users"));
    let scan2 = dag.add_node(OperatorNode::hnsw_scan(1, "vectors_idx", 64));

    // Add join
    let join = dag.add_node(OperatorNode::hash_join(2, "user_id"));
    dag.add_edge(scan1, join).unwrap();
    dag.add_edge(scan2, join).unwrap();

    // Add filter and result
    let filter = dag.add_node(OperatorNode::filter(3, "score > 0.5"));
    dag.add_edge(join, filter).unwrap();

    let result = dag.add_node(OperatorNode::new(4, OperatorType::Result));
    dag.add_edge(filter, result).unwrap();

    // Verify structure
    assert_eq!(dag.node_count(), 5);
    assert_eq!(dag.edge_count(), 4);

    // Verify topological order
    let order = dag.topological_sort().unwrap();
    assert_eq!(order.len(), 5);

    // Scans should come before join
    let scan1_pos = order.iter().position(|&x| x == scan1).unwrap();
    let scan2_pos = order.iter().position(|&x| x == scan2).unwrap();
    let join_pos = order.iter().position(|&x| x == join).unwrap();

    assert!(scan1_pos < join_pos);
    assert!(scan2_pos < join_pos);
}

#[test]
fn test_dag_depths() {
    let mut dag = QueryDag::new();

    // Create tree structure
    // Edges: 3→1, 4→1, 1→0, 2→0
    // Leaves (no outgoing edges): only node 0
    // Depth is computed FROM LEAVES, so node 0 = depth 0
    //
    //       0  (leaf, depth 0)
    //      / \
    //     1   2  (depth 1)
    //    / \
    //   3   4  (depth 2)

    for i in 0..5 {
        dag.add_node(OperatorNode::new(i, OperatorType::Result));
    }

    dag.add_edge(3, 1).unwrap();
    dag.add_edge(4, 1).unwrap();
    dag.add_edge(1, 0).unwrap();
    dag.add_edge(2, 0).unwrap();

    let depths = dag.compute_depths();

    // All nodes should have a depth
    assert!(depths.contains_key(&0));
    assert!(depths.contains_key(&1));
    assert!(depths.contains_key(&2));
    assert!(depths.contains_key(&3));
    assert!(depths.contains_key(&4));

    // Leaf node 0 (no outgoing edges) has depth 0
    assert_eq!(depths[&0], 0);

    // Nodes 1 and 2 are parents of leaf 0, so depth 1
    assert_eq!(depths[&1], 1);
    assert_eq!(depths[&2], 1);

    // Nodes 3 and 4 are parents of 1, so depth 2
    assert_eq!(depths[&3], 2);
    assert_eq!(depths[&4], 2);
}

#[test]
fn test_dag_cycle_detection() {
    let mut dag = QueryDag::new();

    for i in 0..3 {
        dag.add_node(OperatorNode::new(i, OperatorType::Result));
    }

    // Create valid edges
    dag.add_edge(0, 1).unwrap();
    dag.add_edge(1, 2).unwrap();

    // Attempt to create cycle should fail
    let result = dag.add_edge(2, 0);
    assert!(result.is_err());
}

#[test]
fn test_dag_node_removal() {
    let mut dag = QueryDag::new();

    for i in 0..5 {
        dag.add_node(OperatorNode::new(i, OperatorType::Result));
    }

    dag.add_edge(0, 1).unwrap();
    dag.add_edge(1, 2).unwrap();
    dag.add_edge(2, 3).unwrap();
    dag.add_edge(3, 4).unwrap();

    // Remove middle node
    dag.remove_node(2);

    assert_eq!(dag.node_count(), 4);
    // Verify DAG is still valid after removal
    let topo = dag.topological_sort();
    assert!(topo.is_ok());
}

#[test]
fn test_dag_clone() {
    let mut dag = QueryDag::new();

    for i in 0..5 {
        dag.add_node(OperatorNode::new(i, OperatorType::Result));
    }

    for i in 0..4 {
        dag.add_edge(i, i + 1).unwrap();
    }

    let cloned = dag.clone();

    assert_eq!(dag.node_count(), cloned.node_count());
    assert_eq!(dag.edge_count(), cloned.edge_count());
}

#[test]
fn test_dag_topological_order() {
    let mut dag = QueryDag::new();

    // Create diamond pattern
    //    0
    //   / \
    //  1   2
    //   \ /
    //    3

    for i in 0..4 {
        dag.add_node(OperatorNode::new(i, OperatorType::Result));
    }

    dag.add_edge(0, 1).unwrap();
    dag.add_edge(0, 2).unwrap();
    dag.add_edge(1, 3).unwrap();
    dag.add_edge(2, 3).unwrap();

    let order = dag.topological_sort().unwrap();

    // Node 0 must come first
    assert_eq!(order[0], 0);

    // Node 3 must come last
    assert_eq!(order[3], 3);

    // Nodes 1 and 2 must be in the middle
    assert!(order.contains(&1));
    assert!(order.contains(&2));
}

#[test]
fn test_dag_parents_children() {
    let mut dag = QueryDag::new();

    for i in 0..4 {
        dag.add_node(OperatorNode::new(i, OperatorType::Result));
    }

    // 0 -> 1 -> 3
    //      2 ->
    dag.add_edge(0, 1).unwrap();
    dag.add_edge(1, 3).unwrap();
    dag.add_edge(2, 3).unwrap();

    // Parents of node 3
    let preds = dag.parents(3);
    assert_eq!(preds.len(), 2);
    assert!(preds.contains(&1));
    assert!(preds.contains(&2));

    // Children of node 0
    let succs = dag.children(0);
    assert_eq!(succs.len(), 1);
    assert!(succs.contains(&1));
}

#[test]
fn test_dag_leaves() {
    let mut dag = QueryDag::new();

    for i in 0..5 {
        dag.add_node(OperatorNode::new(i, OperatorType::Result));
    }

    // 0 -> 2, 1 -> 2, 2 -> 3, 2 -> 4
    dag.add_edge(0, 2).unwrap();
    dag.add_edge(1, 2).unwrap();
    dag.add_edge(2, 3).unwrap();
    dag.add_edge(2, 4).unwrap();

    // Get leaves using the API
    let leaves = dag.leaves();
    assert_eq!(leaves.len(), 2);
    assert!(leaves.contains(&3));
    assert!(leaves.contains(&4));
}

#[test]
fn test_dag_empty() {
    let dag = QueryDag::new();

    assert_eq!(dag.node_count(), 0);
    assert_eq!(dag.edge_count(), 0);

    let order = dag.topological_sort().unwrap();
    assert!(order.is_empty());
}

#[test]
fn test_dag_single_node() {
    let mut dag = QueryDag::new();
    dag.add_node(OperatorNode::new(0, OperatorType::Result));

    assert_eq!(dag.node_count(), 1);
    assert_eq!(dag.edge_count(), 0);

    let order = dag.topological_sort().unwrap();
    assert_eq!(order.len(), 1);
    assert_eq!(order[0], 0);
}
