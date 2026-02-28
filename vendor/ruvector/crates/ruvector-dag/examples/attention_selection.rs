//! Attention mechanism selection example

use ruvector_dag::attention::{
    CausalConeAttention, CausalConeConfig, DagAttention, TopologicalAttention, TopologicalConfig,
};
use ruvector_dag::dag::{OperatorNode, OperatorType, QueryDag};

fn main() {
    println!("=== Attention Mechanism Selection ===\n");

    // Create a sample DAG
    let dag = create_vector_search_dag();

    println!("Created vector search DAG:");
    println!("  Nodes: {}", dag.node_count());
    println!("  Edges: {}", dag.edge_count());

    // Test Topological Attention
    println!("\n--- Topological Attention ---");
    println!("Emphasizes node depth in the DAG hierarchy");

    let topo = TopologicalAttention::new(TopologicalConfig {
        decay_factor: 0.9,
        max_depth: 10,
    });

    let scores = topo.forward(&dag).unwrap();
    println!("\nAttention scores:");
    for (node_id, score) in &scores {
        let node = dag.get_node(*node_id).unwrap();
        println!("  Node {}: {:.4} - {:?}", node_id, score, node.op_type);
    }

    let sum: f32 = scores.values().sum();
    println!("\nSum of scores: {:.4} (should be ~1.0)", sum);

    // Test Causal Cone Attention
    println!("\n--- Causal Cone Attention ---");
    println!("Focuses on downstream dependencies");

    let causal = CausalConeAttention::new(CausalConeConfig {
        time_window_ms: 1000,
        future_discount: 0.85,
        ancestor_weight: 0.5,
    });

    let causal_scores = causal.forward(&dag).unwrap();
    println!("\nCausal cone scores:");
    for (node_id, score) in &causal_scores {
        let node = dag.get_node(*node_id).unwrap();
        println!("  Node {}: {:.4} - {:?}", node_id, score, node.op_type);
    }

    // Compare mechanisms
    println!("\n--- Comparison ---");
    println!("Node | Topological | Causal Cone | Difference");
    println!("-----|-------------|-------------|------------");
    for node_id in 0..dag.node_count() {
        let topo_score = scores.get(&node_id).unwrap_or(&0.0);
        let causal_score = causal_scores.get(&node_id).unwrap_or(&0.0);
        let diff = (topo_score - causal_score).abs();
        println!(
            "{:4} | {:11.4} | {:11.4} | {:11.4}",
            node_id, topo_score, causal_score, diff
        );
    }

    println!("\n=== Example Complete ===");
}

fn create_vector_search_dag() -> QueryDag {
    let mut dag = QueryDag::new();

    // HNSW scan - the primary vector search
    let hnsw = dag.add_node(OperatorNode::hnsw_scan(0, "embeddings_idx", 64));

    // Metadata table scan
    let meta = dag.add_node(OperatorNode::seq_scan(1, "metadata"));

    // Join embeddings with metadata
    let join = dag.add_node(OperatorNode::new(2, OperatorType::NestedLoopJoin));

    dag.add_edge(hnsw, join).unwrap();
    dag.add_edge(meta, join).unwrap();

    // Filter by category
    let filter = dag.add_node(OperatorNode::filter(3, "category = 'tech'"));
    dag.add_edge(join, filter).unwrap();

    // Limit results
    let limit = dag.add_node(OperatorNode::limit(4, 10));
    dag.add_edge(filter, limit).unwrap();

    // Result node
    let result = dag.add_node(OperatorNode::new(5, OperatorType::Result));
    dag.add_edge(limit, result).unwrap();

    dag
}
