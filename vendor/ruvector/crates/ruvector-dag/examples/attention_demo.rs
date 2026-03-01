//! Demo of DAG attention mechanisms

use ruvector_dag::attention::DagAttentionMechanism;
use ruvector_dag::{
    CausalConeAttention, CriticalPathAttention, DagAttention, MinCutGatedAttention, OperatorNode,
    QueryDag, TopologicalAttention,
};
use std::time::Instant;

fn create_sample_dag() -> QueryDag {
    let mut dag = QueryDag::new();

    // Create a complex query DAG with 100 nodes
    let mut ids = Vec::new();

    // Layer 1: 10 scan nodes
    for i in 0..10 {
        let id = dag.add_node(
            OperatorNode::seq_scan(0, &format!("table_{}", i))
                .with_estimates(1000.0 * (i as f64 + 1.0), 10.0),
        );
        ids.push(id);
    }

    // Layer 2: 20 filter nodes
    for i in 0..20 {
        let id = dag.add_node(
            OperatorNode::filter(0, &format!("col_{} > 0", i)).with_estimates(500.0, 5.0),
        );
        dag.add_edge(ids[i % 10], id).unwrap();
        ids.push(id);
    }

    // Layer 3: 30 join nodes
    for i in 0..30 {
        let id = dag.add_node(
            OperatorNode::hash_join(0, &format!("key_{}", i)).with_estimates(2000.0, 20.0),
        );
        dag.add_edge(ids[10 + (i % 20)], id).unwrap();
        dag.add_edge(ids[10 + ((i + 1) % 20)], id).unwrap();
        ids.push(id);
    }

    // Layer 4: 20 aggregate nodes
    for i in 0..20 {
        let id = dag.add_node(
            OperatorNode::aggregate(0, vec![format!("sum(col_{})", i)]).with_estimates(100.0, 15.0),
        );
        dag.add_edge(ids[30 + (i % 30)], id).unwrap();
        ids.push(id);
    }

    // Layer 5: 10 sort nodes
    for i in 0..10 {
        let id = dag.add_node(
            OperatorNode::sort(0, vec![format!("col_{}", i)]).with_estimates(100.0, 12.0),
        );
        dag.add_edge(ids[60 + (i * 2)], id).unwrap();
        ids.push(id);
    }

    // Layer 6: 5 limit nodes
    for i in 0..5 {
        let id = dag.add_node(OperatorNode::limit(0, 100).with_estimates(100.0, 1.0));
        dag.add_edge(ids[80 + (i * 2)], id).unwrap();
        ids.push(id);
    }

    // Final result node
    let result = dag.add_node(OperatorNode::result(0));
    for i in 0..5 {
        dag.add_edge(ids[90 + i], result).unwrap();
    }

    dag
}

fn main() {
    println!("DAG Attention Mechanisms Performance Demo");
    println!("==========================================\n");

    let dag = create_sample_dag();
    println!(
        "Created DAG with {} nodes and {} edges\n",
        dag.node_count(),
        dag.edge_count()
    );

    // Test TopologicalAttention
    println!("1. TopologicalAttention");
    let topo = TopologicalAttention::with_defaults();
    let start = Instant::now();
    let scores = topo.forward(&dag).unwrap();
    let elapsed = start.elapsed();
    println!("   Time: {:?}", elapsed);
    println!("   Complexity: {}", topo.complexity());
    println!("   Score sum: {:.6}", scores.values().sum::<f32>());
    println!(
        "   Max score: {:.6}\n",
        scores.values().fold(0.0f32, |a, &b| a.max(b))
    );

    // Test CausalConeAttention
    println!("2. CausalConeAttention");
    let causal = CausalConeAttention::with_defaults();
    let start = Instant::now();
    let scores = causal.forward(&dag).unwrap();
    let elapsed = start.elapsed();
    println!("   Time: {:?}", elapsed);
    println!("   Complexity: {}", causal.complexity());
    println!("   Score sum: {:.6}", scores.values().sum::<f32>());
    println!(
        "   Max score: {:.6}\n",
        scores.values().fold(0.0f32, |a, &b| a.max(b))
    );

    // Test CriticalPathAttention
    println!("3. CriticalPathAttention");
    let critical = CriticalPathAttention::with_defaults();
    let start = Instant::now();
    let scores = critical.forward(&dag).unwrap();
    let elapsed = start.elapsed();
    println!("   Time: {:?}", elapsed);
    println!("   Complexity: {}", critical.complexity());
    println!("   Score sum: {:.6}", scores.values().sum::<f32>());
    println!(
        "   Max score: {:.6}\n",
        scores.values().fold(0.0f32, |a, &b| a.max(b))
    );

    // Test MinCutGatedAttention
    println!("4. MinCutGatedAttention");
    let mincut = MinCutGatedAttention::with_defaults();
    let start = Instant::now();
    let result = mincut.forward(&dag).unwrap();
    let elapsed = start.elapsed();
    println!("   Time: {:?}", elapsed);
    println!("   Complexity: {}", mincut.complexity());
    println!("   Score sum: {:.6}", result.scores.iter().sum::<f32>());
    println!(
        "   Max score: {:.6}\n",
        result.scores.iter().fold(0.0f32, |a, b| a.max(*b))
    );

    println!("All attention mechanisms completed successfully!");
}
