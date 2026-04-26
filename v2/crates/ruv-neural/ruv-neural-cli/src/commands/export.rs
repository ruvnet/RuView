//! Export brain graph to various visualization formats.

use std::fs;

use ruv_neural_core::graph::BrainGraph;

/// Run the export command.
pub fn run(
    input: &str,
    format: &str,
    output: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!(input, format, output, "Exporting brain graph");

    let json =
        fs::read_to_string(input).map_err(|e| format!("Failed to read {input}: {e}"))?;
    let graph: BrainGraph =
        serde_json::from_str(&json).map_err(|e| format!("Failed to parse graph JSON: {e}"))?;

    let content = match format {
        "d3" => export_d3(&graph)?,
        "dot" => export_dot(&graph),
        "gexf" => export_gexf(&graph),
        "csv" => export_csv(&graph),
        "rvf" => export_rvf(&graph)?,
        _ => {
            return Err(format!(
                "Unknown format '{format}'. Supported: d3, dot, gexf, csv, rvf"
            )
            .into());
        }
    };

    fs::write(output, content)?;

    println!("=== rUv Neural — Export Complete ===");
    println!();
    println!("  Format:  {format}");
    println!("  Input:   {input}");
    println!("  Output:  {output}");
    println!("  Nodes:   {}", graph.num_nodes);
    println!("  Edges:   {}", graph.edges.len());

    Ok(())
}

/// Export to D3.js-compatible JSON format.
fn export_d3(graph: &BrainGraph) -> Result<String, Box<dyn std::error::Error>> {
    let nodes: Vec<serde_json::Value> = (0..graph.num_nodes)
        .map(|i| {
            serde_json::json!({
                "id": i,
                "degree": graph.node_degree(i),
            })
        })
        .collect();

    let links: Vec<serde_json::Value> = graph
        .edges
        .iter()
        .map(|e| {
            serde_json::json!({
                "source": e.source,
                "target": e.target,
                "weight": e.weight,
                "metric": format!("{:?}", e.metric),
                "band": format!("{:?}", e.frequency_band),
            })
        })
        .collect();

    let d3 = serde_json::json!({
        "nodes": nodes,
        "links": links,
        "metadata": {
            "num_nodes": graph.num_nodes,
            "num_edges": graph.edges.len(),
            "density": graph.density(),
            "total_weight": graph.total_weight(),
            "atlas": format!("{:?}", graph.atlas),
            "timestamp": graph.timestamp,
        }
    });

    Ok(serde_json::to_string_pretty(&d3)?)
}

/// Export to Graphviz DOT format.
fn export_dot(graph: &BrainGraph) -> String {
    let mut dot = String::from("graph brain {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str(&format!(
        "  label=\"Brain Graph ({} nodes, {} edges)\";\n",
        graph.num_nodes,
        graph.edges.len()
    ));
    dot.push_str("  node [shape=circle];\n\n");

    for i in 0..graph.num_nodes {
        let degree = graph.node_degree(i);
        let size = 0.3 + degree * 0.1;
        dot.push_str(&format!(
            "  n{i} [label=\"{i}\", width={size:.2}];\n"
        ));
    }
    dot.push('\n');

    for edge in &graph.edges {
        let penwidth = 0.5 + edge.weight * 2.0;
        dot.push_str(&format!(
            "  n{} -- n{} [penwidth={:.2}, label=\"{:.2}\"];\n",
            edge.source, edge.target, penwidth, edge.weight
        ));
    }

    dot.push_str("}\n");
    dot
}

/// Export to GEXF (Graph Exchange XML Format).
fn export_gexf(graph: &BrainGraph) -> String {
    let mut gexf = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>
<gexf xmlns="http://gexf.net/1.3" version="1.3">
  <meta>
    <creator>rUv Neural</creator>
    <description>Brain connectivity graph</description>
  </meta>
  <graph defaultedgetype="undirected">
    <nodes>
"#);

    for i in 0..graph.num_nodes {
        gexf.push_str(&format!(
            "      <node id=\"{i}\" label=\"Region {i}\" />\n"
        ));
    }

    gexf.push_str("    </nodes>\n    <edges>\n");

    for (idx, edge) in graph.edges.iter().enumerate() {
        gexf.push_str(&format!(
            "      <edge id=\"{idx}\" source=\"{}\" target=\"{}\" weight=\"{:.6}\" />\n",
            edge.source, edge.target, edge.weight
        ));
    }

    gexf.push_str("    </edges>\n  </graph>\n</gexf>\n");
    gexf
}

/// Export to CSV edge list.
fn export_csv(graph: &BrainGraph) -> String {
    let mut csv = String::from("source,target,weight,metric,frequency_band\n");
    for edge in &graph.edges {
        csv.push_str(&format!(
            "{},{},{:.6},{:?},{:?}\n",
            edge.source, edge.target, edge.weight, edge.metric, edge.frequency_band
        ));
    }
    csv
}

/// Export to RVF (RuVector File) JSON representation.
fn export_rvf(graph: &BrainGraph) -> Result<String, Box<dyn std::error::Error>> {
    let rvf = serde_json::json!({
        "format": "rvf",
        "version": 1,
        "data_type": "BrainGraph",
        "num_nodes": graph.num_nodes,
        "num_edges": graph.edges.len(),
        "atlas": format!("{:?}", graph.atlas),
        "timestamp": graph.timestamp,
        "window_duration_s": graph.window_duration_s,
        "adjacency": graph.adjacency_matrix(),
    });
    Ok(serde_json::to_string_pretty(&rvf)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, ConnectivityMetric};
    use ruv_neural_core::signal::FrequencyBand;

    fn test_graph() -> BrainGraph {
        BrainGraph {
            num_nodes: 3,
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
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Beta,
                },
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(3),
        }
    }

    #[test]
    fn export_d3_valid_json() {
        let graph = test_graph();
        let result = export_d3(&graph).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["nodes"].is_array());
        assert!(parsed["links"].is_array());
        assert_eq!(parsed["nodes"].as_array().unwrap().len(), 3);
        assert_eq!(parsed["links"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn export_dot_format() {
        let graph = test_graph();
        let result = export_dot(&graph);
        assert!(result.starts_with("graph brain {"));
        assert!(result.contains("n0 -- n1"));
        assert!(result.ends_with("}\n"));
    }

    #[test]
    fn export_gexf_format() {
        let graph = test_graph();
        let result = export_gexf(&graph);
        assert!(result.contains("<gexf"));
        assert!(result.contains("<node id=\"0\""));
        assert!(result.contains("</gexf>"));
    }

    #[test]
    fn export_csv_format() {
        let graph = test_graph();
        let result = export_csv(&graph);
        assert!(result.starts_with("source,target,weight"));
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 3); // header + 2 edges
    }

    #[test]
    fn export_rvf_valid_json() {
        let graph = test_graph();
        let result = export_rvf(&graph).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["format"], "rvf");
        assert_eq!(parsed["num_nodes"], 3);
    }

    #[test]
    fn export_all_formats() {
        let graph = test_graph();
        let dir = std::env::temp_dir();
        let json_path = dir.join("ruv_neural_test_export.json");
        let json = serde_json::to_string_pretty(&graph).unwrap();
        std::fs::write(&json_path, json).unwrap();

        for fmt in &["d3", "dot", "gexf", "csv", "rvf"] {
            let out_path = dir.join(format!("ruv_neural_test_export.{fmt}"));
            let result = run(
                &json_path.to_string_lossy(),
                fmt,
                &out_path.to_string_lossy(),
            );
            assert!(result.is_ok(), "Failed to export format: {fmt}");
            assert!(out_path.exists(), "Output file missing for format: {fmt}");
            std::fs::remove_file(&out_path).ok();
        }

        std::fs::remove_file(&json_path).ok();
    }
}
