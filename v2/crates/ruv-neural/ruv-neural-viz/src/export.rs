//! Export brain graphs to visualization formats (D3.js, DOT, GEXF, CSV).

use ruv_neural_core::graph::BrainGraph;
use ruv_neural_core::topology::TopologyMetrics;

/// Export a brain graph to JSON suitable for D3.js force-directed layouts.
///
/// Output format:
/// ```json
/// {
///   "nodes": [{"id": 0, "x": 1.0, "y": 2.0, "z": 3.0}, ...],
///   "links": [{"source": 0, "target": 1, "weight": 0.5}, ...]
/// }
/// ```
pub fn to_d3_json(graph: &BrainGraph, layout: &[[f64; 3]]) -> String {
    let mut nodes = Vec::new();
    for (i, pos) in layout.iter().enumerate() {
        nodes.push(format!(
            r#"    {{"id": {}, "x": {:.6}, "y": {:.6}, "z": {:.6}}}"#,
            i, pos[0], pos[1], pos[2]
        ));
    }

    let mut links = Vec::new();
    for edge in &graph.edges {
        links.push(format!(
            r#"    {{"source": {}, "target": {}, "weight": {:.6}}}"#,
            edge.source, edge.target, edge.weight
        ));
    }

    format!(
        "{{\n  \"nodes\": [\n{}\n  ],\n  \"links\": [\n{}\n  ]\n}}",
        nodes.join(",\n"),
        links.join(",\n")
    )
}

/// Export a brain graph to Graphviz DOT format.
pub fn to_dot(graph: &BrainGraph) -> String {
    let mut out = String::new();
    out.push_str("graph brain {\n");
    out.push_str("  layout=neato;\n");
    out.push_str("  node [shape=circle, style=filled, fillcolor=\"#6699CC\"];\n\n");

    for i in 0..graph.num_nodes {
        out.push_str(&format!("  n{} [label=\"{}\"];\n", i, i));
    }
    out.push('\n');

    for edge in &graph.edges {
        out.push_str(&format!(
            "  n{} -- n{} [penwidth={:.2}, label=\"{:.3}\"];\n",
            edge.source,
            edge.target,
            (edge.weight * 3.0).clamp(0.5, 5.0),
            edge.weight
        ));
    }

    out.push_str("}\n");
    out
}

/// Export a topology metrics timeline to CSV format.
///
/// Columns: timestamp, global_mincut, modularity, global_efficiency,
/// local_efficiency, graph_entropy, fiedler_value, num_modules
pub fn timeline_to_csv(timeline: &[(f64, TopologyMetrics)]) -> String {
    let mut out = String::new();
    out.push_str(
        "timestamp,global_mincut,modularity,global_efficiency,\
         local_efficiency,graph_entropy,fiedler_value,num_modules\n",
    );
    for (t, m) in timeline {
        out.push_str(&format!(
            "{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{}\n",
            t,
            m.global_mincut,
            m.modularity,
            m.global_efficiency,
            m.local_efficiency,
            m.graph_entropy,
            m.fiedler_value,
            m.num_modules,
        ));
    }
    out
}

/// Export a brain graph to GEXF format (Gephi).
pub fn to_gexf(graph: &BrainGraph) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<gexf xmlns=\"http://gexf.net/1.3\" version=\"1.3\">\n");
    out.push_str("  <meta>\n");
    out.push_str("    <creator>ruv-neural-viz</creator>\n");
    out.push_str("    <description>Brain connectivity graph</description>\n");
    out.push_str("  </meta>\n");
    out.push_str("  <graph mode=\"static\" defaultedgetype=\"undirected\">\n");

    // Nodes
    out.push_str("    <nodes>\n");
    for i in 0..graph.num_nodes {
        out.push_str(&format!(
            "      <node id=\"{}\" label=\"region_{}\"/>\n",
            i, i
        ));
    }
    out.push_str("    </nodes>\n");

    // Edges
    out.push_str("    <edges>\n");
    for (idx, edge) in graph.edges.iter().enumerate() {
        out.push_str(&format!(
            "      <edge id=\"{}\" source=\"{}\" target=\"{}\" weight=\"{:.6}\"/>\n",
            idx, edge.source, edge.target, edge.weight
        ));
    }
    out.push_str("    </edges>\n");

    out.push_str("  </graph>\n");
    out.push_str("</gexf>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, BrainGraph, ConnectivityMetric};
    use ruv_neural_core::signal::FrequencyBand;

    fn make_graph() -> BrainGraph {
        BrainGraph {
            num_nodes: 3,
            edges: vec![
                BrainEdge {
                    source: 0,
                    target: 1,
                    weight: 0.8,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 1,
                    target: 2,
                    weight: 0.5,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
            ],
            timestamp: 1.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(3),
        }
    }

    #[test]
    fn d3_json_valid() {
        let graph = make_graph();
        let layout = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.5, 1.0, 0.0]];
        let json = to_d3_json(&graph, &layout);

        // Parse to verify valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        let nodes = parsed["nodes"].as_array().expect("nodes array");
        let links = parsed["links"].as_array().expect("links array");
        assert_eq!(nodes.len(), 3);
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn dot_valid_format() {
        let graph = make_graph();
        let dot = to_dot(&graph);
        assert!(dot.starts_with("graph brain {"));
        assert!(dot.contains("n0 -- n1"));
        assert!(dot.contains("n1 -- n2"));
        assert!(dot.ends_with("}\n"));
    }

    #[test]
    fn csv_header_and_rows() {
        let timeline = vec![
            (
                0.0,
                TopologyMetrics {
                    global_mincut: 1.0,
                    modularity: 0.5,
                    global_efficiency: 0.4,
                    local_efficiency: 0.3,
                    graph_entropy: 2.0,
                    fiedler_value: 0.1,
                    num_modules: 3,
                    timestamp: 0.0,
                },
            ),
            (
                1.0,
                TopologyMetrics {
                    global_mincut: 1.5,
                    modularity: 0.6,
                    global_efficiency: 0.45,
                    local_efficiency: 0.35,
                    graph_entropy: 2.1,
                    fiedler_value: 0.12,
                    num_modules: 4,
                    timestamp: 1.0,
                },
            ),
        ];
        let csv = timeline_to_csv(&timeline);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 3); // header + 2 data rows
        assert!(lines[0].contains("timestamp"));
        assert!(lines[0].contains("global_mincut"));
    }

    #[test]
    fn gexf_valid_structure() {
        let graph = make_graph();
        let gexf = to_gexf(&graph);
        assert!(gexf.contains("<?xml"));
        assert!(gexf.contains("<gexf"));
        assert!(gexf.contains("<nodes>"));
        assert!(gexf.contains("<edges>"));
        assert!(gexf.contains("</gexf>"));
    }
}
