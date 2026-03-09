//! Terminal ASCII rendering for brain topology visualization.

use ruv_neural_core::graph::BrainGraph;
use ruv_neural_core::topology::{CognitiveState, MincutResult, TopologyMetrics};

/// Render a brain graph as ASCII art.
///
/// Produces a simple text representation with nodes and edges.
pub fn render_ascii_graph(graph: &BrainGraph, width: usize, height: usize) -> String {
    let n = graph.num_nodes;
    if n == 0 {
        return String::from("(empty graph)");
    }

    let mut canvas = vec![vec![' '; width]; height];

    // Place nodes in a grid
    let cols = (n as f64).sqrt().ceil() as usize;
    let row_spacing = if cols > 0 { height.saturating_sub(1).max(1) / cols.max(1) } else { 1 };
    let col_spacing = if cols > 0 { width.saturating_sub(1).max(1) / cols.max(1) } else { 1 };

    let mut node_positions = Vec::new();
    for i in 0..n {
        let r = i / cols;
        let c = i % cols;
        let y = (r * row_spacing).min(height.saturating_sub(1));
        let x = (c * col_spacing).min(width.saturating_sub(1));
        node_positions.push((x, y));

        // Draw node marker
        if y < height && x < width {
            canvas[y][x] = 'O';
            // Draw node number if space permits
            let label = format!("{}", i);
            for (di, ch) in label.chars().enumerate() {
                if x + 1 + di < width {
                    canvas[y][x + 1 + di] = ch;
                }
            }
        }
    }

    // Draw edges as simple lines between connected nodes
    for edge in &graph.edges {
        if edge.source < n && edge.target < n {
            let (x1, y1) = node_positions[edge.source];
            let (x2, y2) = node_positions[edge.target];
            draw_line(&mut canvas, x1, y1, x2, y2, width, height);
        }
    }

    // Redraw nodes on top
    for (i, &(x, y)) in node_positions.iter().enumerate() {
        if y < height && x < width {
            canvas[y][x] = 'O';
            let label = format!("{}", i);
            for (di, ch) in label.chars().enumerate() {
                if x + 1 + di < width {
                    canvas[y][x + 1 + di] = ch;
                }
            }
        }
    }

    canvas
        .iter()
        .map(|row| row.iter().collect::<String>().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Draw a simple line on the canvas using Bresenham-like stepping.
fn draw_line(
    canvas: &mut [Vec<char>],
    x1: usize,
    y1: usize,
    x2: usize,
    y2: usize,
    width: usize,
    height: usize,
) {
    let dx = (x2 as isize - x1 as isize).abs();
    let dy = (y2 as isize - y1 as isize).abs();
    let steps = dx.max(dy);
    if steps == 0 {
        return;
    }

    for step in 1..steps {
        let t = step as f64 / steps as f64;
        let x = (x1 as f64 + t * (x2 as f64 - x1 as f64)).round() as usize;
        let y = (y1 as f64 + t * (y2 as f64 - y1 as f64)).round() as usize;
        if x < width && y < height && canvas[y][x] == ' ' {
            canvas[y][x] = '.';
        }
    }
}

/// Render a mincut result as ASCII showing two partitions.
pub fn render_ascii_mincut(result: &MincutResult, graph: &BrainGraph) -> String {
    let _ = graph; // May be used for node labels in the future.

    let mut out = String::new();
    out.push_str(&format!(
        "=== Minimum Cut (value: {:.4}) ===\n",
        result.cut_value
    ));
    out.push('\n');

    // Partition A
    out.push_str("Partition A: [");
    out.push_str(
        &result
            .partition_a
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", "),
    );
    out.push_str("]\n");

    // Separator
    out.push_str(&"-".repeat(40));
    out.push('\n');

    // Partition B
    out.push_str("Partition B: [");
    out.push_str(
        &result
            .partition_b
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", "),
    );
    out.push_str("]\n");

    // Cut edges
    out.push('\n');
    out.push_str(&format!("Cut edges ({}):\n", result.cut_edges.len()));
    for &(s, t, w) in &result.cut_edges {
        out.push_str(&format!("  {} --({:.4})--> {}\n", s, w, t));
    }

    out.push_str(&format!(
        "\nBalance ratio: {:.4}\n",
        result.balance_ratio()
    ));

    out
}

/// Render a sparkline from a slice of values using Unicode block characters.
pub fn render_sparkline(values: &[f64], width: usize) -> String {
    if values.is_empty() || width == 0 {
        return String::new();
    }

    let blocks = ['\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}',
                  '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];

    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = max - min;

    // Resample values to fit width
    let resampled: Vec<f64> = if values.len() <= width {
        values.to_vec()
    } else {
        (0..width)
            .map(|i| {
                let idx = (i as f64 / width as f64 * values.len() as f64) as usize;
                values[idx.min(values.len() - 1)]
            })
            .collect()
    };

    resampled
        .iter()
        .map(|&v| {
            if range < 1e-12 {
                blocks[4] // Middle block if all values equal
            } else {
                let normalized = ((v - min) / range).clamp(0.0, 1.0);
                let idx = (normalized * 7.0).round() as usize;
                blocks[idx.min(7)]
            }
        })
        .collect()
}

/// Render a brain state dashboard showing key metrics.
pub fn render_dashboard(metrics: &TopologyMetrics, state: &CognitiveState) -> String {
    let mut out = String::new();

    let state_label = match state {
        CognitiveState::Rest => "Rest",
        CognitiveState::Focused => "Focused",
        CognitiveState::MotorPlanning => "Motor Planning",
        CognitiveState::SpeechProcessing => "Speech Processing",
        CognitiveState::MemoryEncoding => "Memory Encoding",
        CognitiveState::MemoryRetrieval => "Memory Retrieval",
        CognitiveState::Creative => "Creative",
        CognitiveState::Stressed => "Stressed",
        CognitiveState::Fatigued => "Fatigued",
        CognitiveState::Sleep(_) => "Sleep",
        CognitiveState::Unknown => "Unknown",
    };

    out.push_str("+--------------------------------------+\n");
    out.push_str(&format!(
        "| State: {:<29}|\n",
        state_label
    ));
    out.push_str("|--------------------------------------|\n");
    out.push_str(&format!(
        "| Mincut:     {:<7.4} {}|\n",
        metrics.global_mincut,
        bar(metrics.global_mincut, 10.0, 16)
    ));
    out.push_str(&format!(
        "| Modularity: {:<7.4} {}|\n",
        metrics.modularity,
        bar(metrics.modularity, 1.0, 16)
    ));
    out.push_str(&format!(
        "| Efficiency: {:<7.4} {}|\n",
        metrics.global_efficiency,
        bar(metrics.global_efficiency, 1.0, 16)
    ));
    out.push_str(&format!(
        "| Modules:    {:<25}|\n",
        metrics.num_modules
    ));
    out.push_str("+--------------------------------------+\n");

    out
}

/// Render a simple horizontal bar.
fn bar(value: f64, max_val: f64, width: usize) -> String {
    let fraction = (value / max_val).clamp(0.0, 1.0);
    let filled = (fraction * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "#".repeat(filled), " ".repeat(empty))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, BrainGraph};
    use ruv_neural_core::signal::FrequencyBand;

    #[test]
    fn sparkline_renders_known_values() {
        let values = [0.0, 0.25, 0.5, 0.75, 1.0];
        let result = render_sparkline(&values, 5);
        assert_eq!(result.chars().count(), 5);
        // First char should be lowest block, last should be highest
        let chars: Vec<char> = result.chars().collect();
        assert_eq!(chars[0], '\u{2581}');
        assert_eq!(chars[4], '\u{2588}');
    }

    #[test]
    fn sparkline_empty() {
        assert_eq!(render_sparkline(&[], 10), "");
    }

    #[test]
    fn sparkline_zero_width() {
        assert_eq!(render_sparkline(&[1.0, 2.0], 0), "");
    }

    #[test]
    fn sparkline_constant_values() {
        let result = render_sparkline(&[5.0, 5.0, 5.0], 3);
        assert_eq!(result.chars().count(), 3);
    }

    #[test]
    fn dashboard_renders() {
        let metrics = TopologyMetrics {
            global_mincut: 2.5,
            modularity: 0.65,
            global_efficiency: 0.42,
            local_efficiency: 0.38,
            graph_entropy: 3.2,
            fiedler_value: 0.15,
            num_modules: 4,
            timestamp: 0.0,
        };
        let state = CognitiveState::Focused;
        let output = render_dashboard(&metrics, &state);
        assert!(output.contains("Focused"));
        assert!(output.contains("Mincut"));
        assert!(output.contains("Modularity"));
        assert!(output.contains("Modules"));
    }

    #[test]
    fn mincut_renders() {
        let result = MincutResult {
            cut_value: 1.5,
            partition_a: vec![0, 1, 2],
            partition_b: vec![3, 4],
            cut_edges: vec![(1, 3, 0.8), (2, 4, 0.7)],
            timestamp: 0.0,
        };
        let graph = BrainGraph {
            num_nodes: 5,
            edges: vec![],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(5),
        };
        let output = render_ascii_mincut(&result, &graph);
        assert!(output.contains("Partition A"));
        assert!(output.contains("Partition B"));
        assert!(output.contains("1.5000"));
    }

    #[test]
    fn ascii_graph_renders() {
        let graph = BrainGraph {
            num_nodes: 4,
            edges: vec![BrainEdge {
                source: 0,
                target: 1,
                weight: 1.0,
                metric: ruv_neural_core::graph::ConnectivityMetric::Coherence,
                frequency_band: FrequencyBand::Alpha,
            }],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        };
        let output = render_ascii_graph(&graph, 40, 10);
        assert!(!output.is_empty());
        assert!(output.contains('O'));
    }

    #[test]
    fn ascii_graph_empty() {
        let graph = BrainGraph {
            num_nodes: 0,
            edges: vec![],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(0),
        };
        let output = render_ascii_graph(&graph, 40, 10);
        assert_eq!(output, "(empty graph)");
    }
}
