//! Compute minimum cut on a brain connectivity graph.

use std::fs;

use ruv_neural_core::graph::BrainGraph;
use ruv_neural_mincut::{multiway_cut, stoer_wagner_mincut};

/// Run the mincut command.
pub fn run(input: &str, k: Option<usize>) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!(input, ?k, "Computing minimum cut");

    let json =
        fs::read_to_string(input).map_err(|e| format!("Failed to read {input}: {e}"))?;
    let graph: BrainGraph =
        serde_json::from_str(&json).map_err(|e| format!("Failed to parse graph JSON: {e}"))?;

    println!("=== rUv Neural — Minimum Cut Analysis ===");
    println!();
    println!("  Graph: {} nodes, {} edges", graph.num_nodes, graph.edges.len());
    println!();

    match k {
        Some(k_val) if k_val > 2 => {
            // Multi-way cut.
            let result = multiway_cut(&graph, k_val)
                .map_err(|e| format!("Multiway cut failed: {e}"))?;

            println!("  Multi-way cut (k={k_val}):");
            println!("    Total cut value: {:.4}", result.cut_value);
            println!("    Modularity:      {:.4}", result.modularity);
            println!("    Partitions:      {}", result.num_partitions());
            println!();

            for (i, partition) in result.partitions.iter().enumerate() {
                println!("    Partition {i}: {} nodes {:?}", partition.len(), partition);
            }
            println!();

            // ASCII visualization of partitions.
            print_partition_ascii(&graph, &result.partitions);
        }
        _ => {
            // Standard two-way Stoer-Wagner.
            let mc = stoer_wagner_mincut(&graph)
                .map_err(|e| format!("Stoer-Wagner mincut failed: {e}"))?;

            println!("  Stoer-Wagner minimum cut:");
            println!("    Cut value:     {:.4}", mc.cut_value);
            println!("    Partition A:   {} nodes {:?}", mc.partition_a.len(), mc.partition_a);
            println!("    Partition B:   {} nodes {:?}", mc.partition_b.len(), mc.partition_b);
            println!("    Balance ratio: {:.4}", mc.balance_ratio());
            println!();

            println!("  Cut edges:");
            for (src, tgt, weight) in &mc.cut_edges {
                println!("    {src} -- {tgt}  (weight: {weight:.4})");
            }
            println!();

            // ASCII visualization of the two partitions.
            print_partition_ascii(&graph, &[mc.partition_a.clone(), mc.partition_b.clone()]);
        }
    }

    Ok(())
}

/// Print an ASCII visualization of the graph partitions.
fn print_partition_ascii(graph: &BrainGraph, partitions: &[Vec<usize>]) {
    println!("  Partition layout:");

    // Build a node-to-partition map.
    let mut node_partition = vec![0usize; graph.num_nodes];
    for (pid, partition) in partitions.iter().enumerate() {
        for &node in partition {
            if node < graph.num_nodes {
                node_partition[node] = pid;
            }
        }
    }

    // Label characters for partitions.
    let labels = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'];

    let n = graph.num_nodes.min(40);
    print!("    ");
    for i in 0..n {
        let pid = node_partition[i];
        let ch = labels.get(pid).copied().unwrap_or('?');
        print!("{ch}");
    }
    println!();

    if graph.num_nodes > 40 {
        println!("    ... ({} nodes total)", graph.num_nodes);
    }

    println!();
    for (pid, partition) in partitions.iter().enumerate() {
        let ch = labels.get(pid).copied().unwrap_or('?');
        println!("    {ch} = {} nodes", partition.len());
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, ConnectivityMetric};
    use ruv_neural_core::signal::FrequencyBand;

    fn test_graph() -> BrainGraph {
        BrainGraph {
            num_nodes: 6,
            edges: vec![
                BrainEdge {
                    source: 0,
                    target: 1,
                    weight: 5.0,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 1,
                    target: 2,
                    weight: 5.0,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 3,
                    target: 4,
                    weight: 5.0,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 4,
                    target: 5,
                    weight: 5.0,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 2,
                    target: 3,
                    weight: 0.5,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(6),
        }
    }

    #[test]
    fn mincut_two_way() {
        let graph = test_graph();
        let dir = std::env::temp_dir();
        let path = dir.join("ruv_neural_test_mincut.json");
        let json = serde_json::to_string_pretty(&graph).unwrap();
        std::fs::write(&path, json).unwrap();

        let result = run(&path.to_string_lossy(), None);
        assert!(result.is_ok());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn mincut_multiway() {
        let graph = test_graph();
        let dir = std::env::temp_dir();
        let path = dir.join("ruv_neural_test_mincut_k.json");
        let json = serde_json::to_string_pretty(&graph).unwrap();
        std::fs::write(&path, json).unwrap();

        let result = run(&path.to_string_lossy(), Some(3));
        assert!(result.is_ok());
        std::fs::remove_file(&path).ok();
    }
}
