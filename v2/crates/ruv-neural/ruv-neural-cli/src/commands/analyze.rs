//! Analyze a brain connectivity graph: compute topology metrics and display results.

use std::fs;

use ruv_neural_core::graph::BrainGraph;
use ruv_neural_mincut::stoer_wagner_mincut;

/// Run the analyze command.
pub fn run(
    input: &str,
    ascii: bool,
    csv_output: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!(input, "Loading brain graph");

    let json = fs::read_to_string(input)
        .map_err(|e| format!("Failed to read {input}: {e}"))?;
    let graph: BrainGraph = serde_json::from_str(&json)
        .map_err(|e| format!("Failed to parse graph JSON: {e}"))?;

    println!("=== rUv Neural — Graph Analysis ===");
    println!();
    println!("  Nodes:           {}", graph.num_nodes);
    println!("  Edges:           {}", graph.edges.len());
    println!("  Density:         {:.4}", graph.density());
    println!("  Total weight:    {:.4}", graph.total_weight());
    println!("  Timestamp:       {:.2} s", graph.timestamp);
    println!("  Window duration: {:.2} s", graph.window_duration_s);
    println!("  Atlas:           {:?}", graph.atlas);
    println!();

    // Degree statistics.
    let degrees: Vec<f64> = (0..graph.num_nodes)
        .map(|i| graph.node_degree(i))
        .collect();
    let mean_degree = if degrees.is_empty() {
        0.0
    } else {
        degrees.iter().sum::<f64>() / degrees.len() as f64
    };
    let max_degree = degrees.iter().cloned().fold(0.0_f64, f64::max);
    let min_degree = degrees.iter().cloned().fold(f64::INFINITY, f64::min);

    println!("  Degree statistics:");
    println!("    Mean:  {mean_degree:.4}");
    println!("    Min:   {min_degree:.4}");
    println!("    Max:   {max_degree:.4}");
    println!();

    // Mincut.
    match stoer_wagner_mincut(&graph) {
        Ok(mc) => {
            println!("  Minimum cut:");
            println!("    Cut value:     {:.4}", mc.cut_value);
            println!("    Partition A:   {} nodes {:?}", mc.partition_a.len(), mc.partition_a);
            println!("    Partition B:   {} nodes {:?}", mc.partition_b.len(), mc.partition_b);
            println!("    Cut edges:     {}", mc.cut_edges.len());
            println!("    Balance ratio: {:.4}", mc.balance_ratio());
            println!();
        }
        Err(e) => {
            println!("  Minimum cut: could not compute ({e})");
            println!();
        }
    }

    // Edge weight distribution.
    if !graph.edges.is_empty() {
        let weights: Vec<f64> = graph.edges.iter().map(|e| e.weight).collect();
        let mean_w = weights.iter().sum::<f64>() / weights.len() as f64;
        let max_w = weights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_w = weights.iter().cloned().fold(f64::INFINITY, f64::min);

        println!("  Edge weight distribution:");
        println!("    Mean:  {mean_w:.4}");
        println!("    Min:   {min_w:.4}");
        println!("    Max:   {max_w:.4}");
        println!();
    }

    if ascii {
        print_ascii_graph(&graph);
    }

    if let Some(csv_path) = csv_output {
        write_csv(&graph, &degrees, &csv_path)?;
        println!("  Metrics exported to: {csv_path}");
    }

    Ok(())
}

/// Print a simple ASCII visualization of the graph adjacency.
fn print_ascii_graph(graph: &BrainGraph) {
    println!("  ASCII Adjacency Matrix:");
    let n = graph.num_nodes.min(20); // cap display at 20x20
    let adj = graph.adjacency_matrix();

    // Header row.
    print!("       ");
    for j in 0..n {
        print!("{j:>4}");
    }
    println!();

    for i in 0..n {
        print!("  {i:>3}  ");
        for j in 0..n {
            let w = adj[i][j];
            if i == j {
                print!("   .");
            } else if w > 0.0 {
                // Map weight to a character.
                let ch = if w > 0.8 {
                    '#'
                } else if w > 0.5 {
                    '*'
                } else if w > 0.2 {
                    '+'
                } else {
                    '.'
                };
                print!("   {ch}");
            } else {
                print!("    ");
            }
        }
        println!();
    }

    if graph.num_nodes > 20 {
        println!("  ... ({} nodes total, showing first 20)", graph.num_nodes);
    }
    println!();
}

/// Write per-node metrics to a CSV file.
fn write_csv(
    graph: &BrainGraph,
    degrees: &[f64],
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut csv = String::from("node,degree,num_edges\n");
    for i in 0..graph.num_nodes {
        let num_edges = graph
            .edges
            .iter()
            .filter(|e| e.source == i || e.target == i)
            .count();
        csv.push_str(&format!(
            "{},{:.6},{}\n",
            i,
            degrees.get(i).copied().unwrap_or(0.0),
            num_edges
        ));
    }
    fs::write(path, csv)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, ConnectivityMetric};
    use ruv_neural_core::signal::FrequencyBand;

    fn test_graph() -> BrainGraph {
        BrainGraph {
            num_nodes: 4,
            edges: vec![
                BrainEdge {
                    source: 0,
                    target: 1,
                    weight: 0.8,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 1,
                    target: 2,
                    weight: 0.5,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 2,
                    target: 3,
                    weight: 0.9,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                },
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        }
    }

    #[test]
    fn analyze_from_json() {
        let graph = test_graph();
        let dir = std::env::temp_dir();
        let path = dir.join("ruv_neural_test_analyze.json");
        let json = serde_json::to_string_pretty(&graph).unwrap();
        std::fs::write(&path, json).unwrap();

        let result = run(&path.to_string_lossy(), false, None);
        assert!(result.is_ok());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn analyze_with_csv() {
        let graph = test_graph();
        let dir = std::env::temp_dir();
        let json_path = dir.join("ruv_neural_test_analyze2.json");
        let csv_path = dir.join("ruv_neural_test_analyze2.csv");

        let json = serde_json::to_string_pretty(&graph).unwrap();
        std::fs::write(&json_path, json).unwrap();

        let result = run(
            &json_path.to_string_lossy(),
            true,
            Some(csv_path.to_string_lossy().to_string()),
        );
        assert!(result.is_ok());
        assert!(csv_path.exists());

        let csv_content = std::fs::read_to_string(&csv_path).unwrap();
        assert!(csv_content.starts_with("node,degree,num_edges"));

        std::fs::remove_file(&json_path).ok();
        std::fs::remove_file(&csv_path).ok();
    }
}
