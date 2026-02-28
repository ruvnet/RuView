//! Basic usage example for Neural DAG Learning

use ruvector_dag::dag::{OperatorNode, OperatorType, QueryDag};

fn main() {
    println!("=== Neural DAG Learning - Basic Usage ===\n");

    // Create a new DAG
    let mut dag = QueryDag::new();

    // Add nodes representing query operators
    println!("Building query DAG...");

    let scan = dag.add_node(OperatorNode::seq_scan(0, "users"));
    println!("  Added SeqScan on 'users' (id: {})", scan);

    let filter = dag.add_node(OperatorNode::filter(1, "age > 18"));
    println!("  Added Filter 'age > 18' (id: {})", filter);

    let sort = dag.add_node(OperatorNode::sort(2, vec!["name".to_string()]));
    println!("  Added Sort by 'name' (id: {})", sort);

    let limit = dag.add_node(OperatorNode::limit(3, 10));
    println!("  Added Limit 10 (id: {})", limit);

    let result = dag.add_node(OperatorNode::new(4, OperatorType::Result));
    println!("  Added Result (id: {})", result);

    // Connect nodes
    dag.add_edge(scan, filter).unwrap();
    dag.add_edge(filter, sort).unwrap();
    dag.add_edge(sort, limit).unwrap();
    dag.add_edge(limit, result).unwrap();

    println!("\nDAG Statistics:");
    println!("  Nodes: {}", dag.node_count());
    println!("  Edges: {}", dag.edge_count());

    // Compute topological order
    let order = dag.topological_sort().unwrap();
    println!("\nTopological Order: {:?}", order);

    // Compute depths
    let depths = dag.compute_depths();
    println!("\nNode Depths:");
    for (id, depth) in &depths {
        println!("  Node {}: depth {}", id, depth);
    }

    // Get children
    println!("\nNode Children:");
    for node_id in 0..5 {
        let children = dag.children(node_id);
        println!("  Node {}: {:?}", node_id, children);
    }

    // Demonstrate iterators
    println!("\nDFS Traversal:");
    for (i, node_id) in dag.dfs_iter(scan).enumerate() {
        if i < 10 {
            println!("  Visit: {}", node_id);
        }
    }

    println!("\nBFS Traversal:");
    for (i, node_id) in dag.bfs_iter(scan).enumerate() {
        if i < 10 {
            println!("  Visit: {}", node_id);
        }
    }

    println!("\n=== Example Complete ===");
}
