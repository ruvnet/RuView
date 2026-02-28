//! SONA learning workflow example

use ruvector_dag::dag::{OperatorNode, OperatorType, QueryDag};
use ruvector_dag::sona::{DagSonaEngine, DagTrajectory, DagTrajectoryBuffer};

fn main() {
    println!("=== SONA Learning Workflow ===\n");

    // Initialize SONA engine
    let mut sona = DagSonaEngine::new(256);

    println!("SONA Engine initialized with:");
    println!("  Embedding dimension: 256");
    println!("  Initial patterns: {}", sona.pattern_count());
    println!("  Initial trajectories: {}", sona.trajectory_count());

    // Simulate query execution workflow
    println!("\n--- Query Execution Simulation ---");

    for query_num in 1..=5 {
        println!("\nQuery #{}", query_num);

        // Create a query DAG
        let dag = create_random_dag(query_num);
        println!(
            "  DAG nodes: {}, edges: {}",
            dag.node_count(),
            dag.edge_count()
        );

        // Pre-query: Get enhanced embedding
        let enhanced = sona.pre_query(&dag);
        println!(
            "  Pre-query adaptation complete (embedding dim: {})",
            enhanced.len()
        );

        // Simulate execution - later queries get faster as SONA learns
        let learning_factor = 1.0 - (query_num as f64 * 0.08);
        let execution_time = 100.0 * learning_factor + (rand::random::<f64>() * 10.0);
        let baseline_time = 100.0;

        // Post-query: Record trajectory
        sona.post_query(&dag, execution_time, baseline_time, "topological");

        let improvement = ((baseline_time - execution_time) / baseline_time) * 100.0;
        println!(
            "  Execution: {:.1}ms (baseline: {:.1}ms)",
            execution_time, baseline_time
        );
        println!("  Improvement: {:.1}%", improvement);

        // Every 2 queries, trigger learning
        if query_num % 2 == 0 {
            println!("  Running background learning...");
            sona.background_learn();
            println!(
                "  Patterns: {}, Trajectories: {}",
                sona.pattern_count(),
                sona.trajectory_count()
            );
        }
    }

    // Final statistics
    println!("\n--- Final Statistics ---");
    println!("Total patterns: {}", sona.pattern_count());
    println!("Total trajectories: {}", sona.trajectory_count());
    println!("Total clusters: {}", sona.cluster_count());

    // Demonstrate trajectory buffer
    println!("\n--- Trajectory Buffer Demo ---");
    let buffer = DagTrajectoryBuffer::new(100);

    println!("Creating {} sample trajectories...", 10);
    for i in 0..10 {
        let embedding = vec![rand::random::<f32>(); 256];
        let trajectory = DagTrajectory::new(
            i as u64,
            embedding,
            "topological".to_string(),
            50.0 + i as f64,
            100.0,
        );
        buffer.push(trajectory);
    }

    println!("Buffer size: {}", buffer.len());
    println!("Total recorded: {}", buffer.total_count());

    let drained = buffer.drain();
    println!("Drained {} trajectories", drained.len());
    println!("Buffer after drain: {}", buffer.len());

    // Demonstrate metrics
    if let Some(first) = drained.first() {
        println!("\nSample trajectory:");
        println!("  Query hash: {}", first.query_hash);
        println!("  Mechanism: {}", first.attention_mechanism);
        println!("  Execution time: {:.2}ms", first.execution_time_ms);
        let baseline = first.execution_time_ms / first.improvement_ratio as f64;
        println!("  Baseline time: {:.2}ms", baseline);
        println!("  Improvement ratio: {:.3}", first.improvement_ratio);
    }

    println!("\n=== Example Complete ===");
}

fn create_random_dag(seed: usize) -> QueryDag {
    let mut dag = QueryDag::new();

    // Create nodes based on seed for variety
    let node_count = 3 + (seed % 5);

    for i in 0..node_count {
        let op = if i == 0 {
            // Start with a scan
            if seed % 2 == 0 {
                OperatorType::SeqScan {
                    table: format!("table_{}", seed),
                }
            } else {
                OperatorType::HnswScan {
                    index: format!("idx_{}", seed),
                    ef_search: 64,
                }
            }
        } else if i == node_count - 1 {
            // End with result
            OperatorType::Result
        } else {
            // Middle operators vary
            match (seed + i) % 4 {
                0 => OperatorType::Filter {
                    predicate: format!("col{} > {}", i, seed * 10),
                },
                1 => OperatorType::Sort {
                    keys: vec![format!("col{}", i)],
                    descending: vec![false],
                },
                2 => OperatorType::Limit {
                    count: 10 + (seed * i),
                },
                _ => OperatorType::NestedLoopJoin,
            }
        };

        dag.add_node(OperatorNode::new(i, op));
    }

    // Create linear chain
    for i in 0..node_count - 1 {
        let _ = dag.add_edge(i, i + 1);
    }

    // Add some branching for variety
    if node_count > 4 && seed % 3 == 0 {
        let _ = dag.add_edge(0, 2);
    }

    dag
}
