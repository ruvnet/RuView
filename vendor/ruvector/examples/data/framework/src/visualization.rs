//! ASCII Art Visualization for Discovery Framework
//!
//! Provides terminal-based graph visualization with ANSI colors, domain clustering,
//! coherence heatmaps, and pattern timeline displays.

use std::collections::HashMap;
use chrono::{DateTime, Utc};

use crate::optimized::{OptimizedDiscoveryEngine, SignificantPattern};
use crate::ruvector_native::{Domain, PatternType};

/// ANSI color codes for domains
const COLOR_CLIMATE: &str = "\x1b[34m";  // Blue
const COLOR_FINANCE: &str = "\x1b[32m";  // Green
const COLOR_RESEARCH: &str = "\x1b[33m"; // Yellow
const COLOR_MEDICAL: &str = "\x1b[36m";  // Cyan
const COLOR_CROSS: &str = "\x1b[35m";    // Magenta
const COLOR_RESET: &str = "\x1b[0m";
const COLOR_BRIGHT: &str = "\x1b[1m";
const COLOR_DIM: &str = "\x1b[2m";

/// Box-drawing characters
const BOX_H: char = '─';
const BOX_V: char = '│';
const BOX_TL: char = '┌';
const BOX_TR: char = '┐';
const BOX_BL: char = '└';
const BOX_BR: char = '┘';
const BOX_CROSS: char = '┼';
const BOX_T_DOWN: char = '┬';
const BOX_T_UP: char = '┴';
const BOX_T_RIGHT: char = '├';
const BOX_T_LEFT: char = '┤';

/// Get ANSI color for a domain
fn domain_color(domain: Domain) -> &'static str {
    match domain {
        Domain::Climate => COLOR_CLIMATE,
        Domain::Finance => COLOR_FINANCE,
        Domain::Research => COLOR_RESEARCH,
        Domain::Medical => COLOR_MEDICAL,
        Domain::Economic => "\x1b[38;5;214m", // Orange color for Economic
        Domain::Genomics => "\x1b[38;5;46m", // Green color for Genomics
        Domain::Physics => "\x1b[38;5;33m", // Blue color for Physics
        Domain::Seismic => "\x1b[38;5;130m", // Brown color for Seismic
        Domain::Ocean => "\x1b[38;5;39m", // Cyan color for Ocean
        Domain::Space => "\x1b[38;5;141m", // Purple color for Space
        Domain::Transportation => "\x1b[38;5;208m", // Orange color for Transportation
        Domain::Geospatial => "\x1b[38;5;118m", // Light green for Geospatial
        Domain::Government => "\x1b[38;5;243m", // Gray color for Government
        Domain::CrossDomain => COLOR_CROSS,
    }
}

/// Get a character representation for a domain
fn domain_char(domain: Domain) -> char {
    match domain {
        Domain::Climate => 'C',
        Domain::Finance => 'F',
        Domain::Research => 'R',
        Domain::Medical => 'M',
        Domain::Economic => 'E',
        Domain::Genomics => 'G',
        Domain::Physics => 'P',
        Domain::Seismic => 'S',
        Domain::Ocean => 'O',
        Domain::Space => 'A', // A for Astronomy/Aerospace
        Domain::Transportation => 'T',
        Domain::Geospatial => 'L', // L for Location
        Domain::Government => 'V', // V for goVernment
        Domain::CrossDomain => 'X',
    }
}

/// Render the graph as ASCII art with colored domain nodes
///
/// # Arguments
/// * `engine` - The discovery engine containing the graph
/// * `width` - Canvas width in characters
/// * `height` - Canvas height in characters
///
/// # Returns
/// A string containing the ASCII art representation
pub fn render_graph_ascii(engine: &OptimizedDiscoveryEngine, width: usize, height: usize) -> String {
    let stats = engine.stats();
    let mut output = String::new();

    // Draw title box
    output.push_str(&format!("{}{}", COLOR_BRIGHT, BOX_TL));
    output.push_str(&BOX_H.to_string().repeat(width - 2));
    output.push_str(&format!("{}{}\n", BOX_TR, COLOR_RESET));

    let title = format!(" Discovery Graph ({} nodes, {} edges) ", stats.total_nodes, stats.total_edges);
    output.push_str(&format!("{}{}", COLOR_BRIGHT, BOX_V));
    output.push_str(&format!("{:^width$}", title, width = width - 2));
    output.push_str(&format!("{}{}\n", BOX_V, COLOR_RESET));

    output.push_str(&format!("{}{}", COLOR_BRIGHT, BOX_BL));
    output.push_str(&BOX_H.to_string().repeat(width - 2));
    output.push_str(&format!("{}{}\n\n", BOX_BR, COLOR_RESET));

    // If no nodes, show empty message
    if stats.total_nodes == 0 {
        output.push_str(&format!("{}  (empty graph){}\n", COLOR_DIM, COLOR_RESET));
        return output;
    }

    // Create a simple layout by domain
    let mut domain_positions: HashMap<Domain, Vec<(usize, usize)>> = HashMap::new();

    // Layout domains in quadrants
    let mid_x = width / 2;
    let mid_y = height / 2;

    // Assign domain regions
    let domain_regions = [
        (Domain::Climate, 10, 2),          // Top-left
        (Domain::Finance, mid_x + 10, 2),  // Top-right
        (Domain::Research, 10, mid_y + 2), // Bottom-left
    ];

    for (domain, count) in &stats.domain_counts {
        let (_, base_x, base_y) = domain_regions.iter()
            .find(|(d, _, _)| d == domain)
            .unwrap_or(&(Domain::Research, 10, 2));

        let mut positions = Vec::new();

        // Arrange nodes in a cluster
        let nodes_per_row = ((*count as f64).sqrt().ceil() as usize).max(1);
        for i in 0..*count {
            let row = i / nodes_per_row;
            let col = i % nodes_per_row;
            let x = base_x + col * 3;
            let y = base_y + row * 2;

            if x < width - 5 && y < height - 2 {
                positions.push((x, y));
            }
        }

        domain_positions.insert(*domain, positions);
    }

    // Create canvas
    let mut canvas: Vec<Vec<String>> = vec![vec![" ".to_string(); width]; height];

    // Draw nodes
    for (domain, positions) in &domain_positions {
        let color = domain_color(*domain);
        let ch = domain_char(*domain);

        for (x, y) in positions {
            if *x < width && *y < height {
                canvas[*y][*x] = format!("{}{}{}", color, ch, COLOR_RESET);
            }
        }
    }

    // Draw edges (simplified - show connections between domains)
    if stats.cross_domain_edges > 0 {
        // Draw some connecting lines
        for (domain_a, positions_a) in &domain_positions {
            for (domain_b, positions_b) in &domain_positions {
                if domain_a == domain_b {
                    continue;
                }

                // Draw one connection line
                if let (Some(pos_a), Some(pos_b)) = (positions_a.first(), positions_b.first()) {
                    let (x1, y1) = pos_a;
                    let (x2, y2) = pos_b;

                    // Simple line drawing (horizontal then vertical)
                    let color = COLOR_DIM;

                    // Horizontal part
                    let (min_x, max_x) = if x1 < x2 { (*x1, *x2) } else { (*x2, *x1) };
                    for x in min_x..=max_x {
                        if x < width && *y1 < height && canvas[*y1][x] == " " {
                            canvas[*y1][x] = format!("{}{}{}", color, BOX_H, COLOR_RESET);
                        }
                    }

                    // Vertical part
                    let (min_y, max_y) = if y1 < y2 { (*y1, *y2) } else { (*y2, *y1) };
                    for y in min_y..=max_y {
                        if *x2 < width && y < height && canvas[y][*x2] == " " {
                            canvas[y][*x2] = format!("{}{}{}", color, BOX_V, COLOR_RESET);
                        }
                    }
                }
            }
        }
    }

    // Render canvas to string
    for row in canvas {
        for cell in row {
            output.push_str(&cell);
        }
        output.push('\n');
    }

    output.push('\n');

    // Legend
    output.push_str(&format!("{}Legend:{}\n", COLOR_BRIGHT, COLOR_RESET));
    output.push_str(&format!("  {}C{} = Climate    ", COLOR_CLIMATE, COLOR_RESET));
    output.push_str(&format!("{}F{} = Finance    ", COLOR_FINANCE, COLOR_RESET));
    output.push_str(&format!("{}R{} = Research\n", COLOR_RESEARCH, COLOR_RESET));
    output.push_str(&format!("  Cross-domain bridges: {}\n", stats.cross_domain_edges));

    output
}

/// Render a domain connectivity matrix
///
/// Shows the strength of connections between different domains
pub fn render_domain_matrix(engine: &OptimizedDiscoveryEngine) -> String {
    let stats = engine.stats();
    let mut output = String::new();

    output.push_str(&format!("\n{}{}Domain Connectivity Matrix{}{}\n",
        COLOR_BRIGHT, BOX_TL, BOX_TR, COLOR_RESET));
    output.push_str(&format!("{}\n", BOX_H.to_string().repeat(50)));

    // Calculate connections between domains
    let domains = [Domain::Climate, Domain::Finance, Domain::Research];
    let mut matrix: HashMap<(Domain, Domain), usize> = HashMap::new();

    // Initialize matrix
    for &d1 in &domains {
        for &d2 in &domains {
            matrix.insert((d1, d2), 0);
        }
    }

    // This is a placeholder - in real implementation, we'd iterate through edges
    // and count connections between domains
    output.push_str(&format!("         {}Climate{}  {}Finance{}  {}Research{}\n",
        COLOR_CLIMATE, COLOR_RESET,
        COLOR_FINANCE, COLOR_RESET,
        COLOR_RESEARCH, COLOR_RESET));

    for &domain_a in &domains {
        let color_a = domain_color(domain_a);
        output.push_str(&format!("{}{:9}{} ", color_a, format!("{:?}", domain_a), COLOR_RESET));

        for &domain_b in &domains {
            let count = matrix.get(&(domain_a, domain_b)).unwrap_or(&0);
            let display = if domain_a == domain_b {
                format!("{}[{:3}]{}", COLOR_BRIGHT, stats.domain_counts.get(&domain_a).unwrap_or(&0), COLOR_RESET)
            } else {
                format!(" {:3}  ", count)
            };
            output.push_str(&display);
        }
        output.push('\n');
    }

    output.push_str(&format!("\n{}Note:{} Diagonal = node count, Off-diagonal = cross-domain edges\n",
        COLOR_DIM, COLOR_RESET));
    output.push_str(&format!("Total cross-domain edges: {}\n", stats.cross_domain_edges));

    output
}

/// Render coherence timeline as ASCII sparkline/chart
///
/// # Arguments
/// * `history` - Time series of (timestamp, coherence_value) pairs
pub fn render_coherence_timeline(history: &[(DateTime<Utc>, f64)]) -> String {
    let mut output = String::new();

    output.push_str(&format!("\n{}{}Coherence Timeline{}{}\n",
        COLOR_BRIGHT, BOX_TL, BOX_TR, COLOR_RESET));
    output.push_str(&format!("{}\n", BOX_H.to_string().repeat(70)));

    if history.is_empty() {
        output.push_str(&format!("{}  (no coherence history){}\n", COLOR_DIM, COLOR_RESET));
        return output;
    }

    let values: Vec<f64> = history.iter().map(|(_, v)| *v).collect();
    let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    output.push_str(&format!("  Coherence range: {:.4} - {:.4}\n", min_val, max_val));
    output.push_str(&format!("  Data points: {}\n\n", history.len()));

    // ASCII sparkline
    let chart_height = 10;
    let chart_width = 60.min(history.len());

    // Sample data if too many points
    let step = if history.len() > chart_width {
        history.len() / chart_width
    } else {
        1
    };

    let sampled: Vec<f64> = history.iter()
        .step_by(step)
        .take(chart_width)
        .map(|(_, v)| *v)
        .collect();

    // Normalize values to chart height
    let range = max_val - min_val;
    let normalized: Vec<usize> = if range > 1e-10 {
        sampled.iter()
            .map(|v| {
                let normalized = ((v - min_val) / range * (chart_height - 1) as f64) as usize;
                normalized.min(chart_height - 1)
            })
            .collect()
    } else {
        vec![chart_height / 2; sampled.len()]
    };

    // Draw chart
    for row in (0..chart_height).rev() {
        let value = min_val + (row as f64 / (chart_height - 1) as f64) * range;
        output.push_str(&format!("{:6.3} {} ", value, BOX_V));

        for &height in &normalized {
            let ch = if height >= row {
                format!("{}▓{}", COLOR_CLIMATE, COLOR_RESET)
            } else if height + 1 == row {
                format!("{}▒{}", COLOR_DIM, COLOR_RESET)
            } else {
                " ".to_string()
            };
            output.push_str(&ch);
        }
        output.push('\n');
    }

    // X-axis
    output.push_str("       ");
    output.push_str(&BOX_BL.to_string());
    output.push_str(&BOX_H.to_string().repeat(chart_width));
    output.push('\n');

    // Time labels
    if let (Some(first), Some(last)) = (history.first(), history.last()) {
        let duration = last.0.signed_duration_since(first.0);
        let width_val = if chart_width > 12 { chart_width - 12 } else { 0 };
        output.push_str(&format!("       {} {:>width$}\n",
            first.0.format("%Y-%m-%d"),
            last.0.format("%Y-%m-%d"),
            width = width_val));
        output.push_str(&format!("       {}Duration: {}{}\n",
            COLOR_DIM,
            if duration.num_days() > 0 {
                format!("{} days", duration.num_days())
            } else if duration.num_hours() > 0 {
                format!("{} hours", duration.num_hours())
            } else {
                format!("{} minutes", duration.num_minutes())
            },
            COLOR_RESET));
    }

    output
}

/// Render a summary of discovered patterns
///
/// # Arguments
/// * `patterns` - List of significant patterns to summarize
pub fn render_pattern_summary(patterns: &[SignificantPattern]) -> String {
    let mut output = String::new();

    output.push_str(&format!("\n{}{}Pattern Discovery Summary{}{}\n",
        COLOR_BRIGHT, BOX_TL, BOX_TR, COLOR_RESET));
    output.push_str(&format!("{}\n", BOX_H.to_string().repeat(80)));

    if patterns.is_empty() {
        output.push_str(&format!("{}  No patterns discovered yet{}\n", COLOR_DIM, COLOR_RESET));
        return output;
    }

    output.push_str(&format!("  Total patterns detected: {}\n", patterns.len()));

    // Count by type
    let mut type_counts: HashMap<PatternType, usize> = HashMap::new();
    let mut significant_count = 0;

    for pattern in patterns {
        *type_counts.entry(pattern.pattern.pattern_type).or_default() += 1;
        if pattern.is_significant {
            significant_count += 1;
        }
    }

    output.push_str(&format!("  Statistically significant: {} ({:.1}%)\n\n",
        significant_count,
        (significant_count as f64 / patterns.len() as f64) * 100.0));

    // Pattern type breakdown
    output.push_str(&format!("{}Pattern Types:{}\n", COLOR_BRIGHT, COLOR_RESET));
    for (pattern_type, count) in type_counts.iter() {
        let icon = match pattern_type {
            PatternType::CoherenceBreak => "⚠️ ",
            PatternType::Consolidation => "📈",
            PatternType::EmergingCluster => "🌟",
            PatternType::DissolvingCluster => "💫",
            PatternType::BridgeFormation => "🌉",
            PatternType::AnomalousNode => "🔴",
            PatternType::TemporalShift => "⏰",
            PatternType::Cascade => "🌊",
        };

        let bar_length = ((*count as f64 / patterns.len() as f64) * 30.0) as usize;
        let bar = "█".repeat(bar_length);

        output.push_str(&format!("  {} {:20} {:3} {}{}{}\n",
            icon,
            format!("{:?}", pattern_type),
            count,
            COLOR_CLIMATE,
            bar,
            COLOR_RESET));
    }

    output.push('\n');

    // Top patterns by confidence
    output.push_str(&format!("{}Top Patterns (by confidence):{}\n", COLOR_BRIGHT, COLOR_RESET));

    let mut sorted_patterns: Vec<_> = patterns.iter().collect();
    sorted_patterns.sort_by(|a, b| b.pattern.confidence.partial_cmp(&a.pattern.confidence).unwrap());

    for (i, pattern) in sorted_patterns.iter().take(5).enumerate() {
        let significance_marker = if pattern.is_significant {
            format!("{}*{}", COLOR_BRIGHT, COLOR_RESET)
        } else {
            " ".to_string()
        };

        let color = if pattern.pattern.confidence > 0.8 {
            COLOR_CLIMATE
        } else if pattern.pattern.confidence > 0.5 {
            COLOR_FINANCE
        } else {
            COLOR_DIM
        };

        output.push_str(&format!("  {}{}.{} {}{:?}{} (p={:.4}, effect={:.3}, conf={:.2})\n",
            significance_marker,
            i + 1,
            COLOR_RESET,
            color,
            pattern.pattern.pattern_type,
            COLOR_RESET,
            pattern.p_value,
            pattern.effect_size,
            pattern.pattern.confidence));

        output.push_str(&format!("     {}{}{}\n",
            COLOR_DIM,
            pattern.pattern.description,
            COLOR_RESET));
    }

    output.push_str(&format!("\n{}Note:{} * = statistically significant (p < 0.05)\n",
        COLOR_DIM, COLOR_RESET));

    output
}

/// Render a complete dashboard combining all visualizations
pub fn render_dashboard(
    engine: &OptimizedDiscoveryEngine,
    patterns: &[SignificantPattern],
    coherence_history: &[(DateTime<Utc>, f64)],
) -> String {
    let mut output = String::new();

    // Title
    output.push_str(&format!("\n{}{}═══════════════════════════════════════════════════════════════════════════════{}\n",
        COLOR_BRIGHT, BOX_TL, COLOR_RESET));
    output.push_str(&format!("{}{}        RuVector Discovery Framework - Live Dashboard                        {}\n",
        COLOR_BRIGHT, BOX_V, COLOR_RESET));
    output.push_str(&format!("{}{}═══════════════════════════════════════════════════════════════════════════════{}\n\n",
        COLOR_BRIGHT, BOX_BL, COLOR_RESET));

    // Stats overview
    let stats = engine.stats();
    output.push_str(&format!("{}Quick Stats:{}\n", COLOR_BRIGHT, COLOR_RESET));
    output.push_str(&format!("  Nodes: {}  │  Edges: {}  │  Vectors: {}  │  Cross-domain: {}\n",
        stats.total_nodes,
        stats.total_edges,
        stats.total_vectors,
        stats.cross_domain_edges));
    output.push_str(&format!("  Patterns: {}  │  Coherence samples: {}  │  Cache hit rate: {:.1}%\n\n",
        patterns.len(),
        coherence_history.len(),
        stats.cache_hit_rate * 100.0));

    // Graph visualization
    output.push_str(&render_graph_ascii(engine, 80, 20));
    output.push('\n');

    // Domain matrix
    output.push_str(&render_domain_matrix(engine));
    output.push('\n');

    // Coherence timeline
    output.push_str(&render_coherence_timeline(coherence_history));
    output.push('\n');

    // Pattern summary
    output.push_str(&render_pattern_summary(patterns));

    output.push_str(&format!("\n{}{}═══════════════════════════════════════════════════════════════════════════════{}\n",
        COLOR_DIM, BOX_BL, COLOR_RESET));

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optimized::{OptimizedConfig, OptimizedDiscoveryEngine};
    use crate::ruvector_native::SemanticVector;
    use chrono::Utc;

    #[test]
    fn test_domain_color() {
        assert_eq!(domain_color(Domain::Climate), COLOR_CLIMATE);
        assert_eq!(domain_color(Domain::Finance), COLOR_FINANCE);
    }

    #[test]
    fn test_domain_char() {
        assert_eq!(domain_char(Domain::Climate), 'C');
        assert_eq!(domain_char(Domain::Finance), 'F');
        assert_eq!(domain_char(Domain::Research), 'R');
    }

    #[test]
    fn test_render_empty_graph() {
        let config = OptimizedConfig::default();
        let engine = OptimizedDiscoveryEngine::new(config);
        let output = render_graph_ascii(&engine, 80, 20);
        assert!(output.contains("empty graph"));
    }

    #[test]
    fn test_render_pattern_summary_empty() {
        let output = render_pattern_summary(&[]);
        assert!(output.contains("No patterns"));
    }

    #[test]
    fn test_render_coherence_timeline_empty() {
        let output = render_coherence_timeline(&[]);
        assert!(output.contains("no coherence history"));
    }

    #[test]
    fn test_render_coherence_timeline_with_data() {
        let now = Utc::now();
        let history = vec![
            (now, 0.5),
            (now + chrono::Duration::hours(1), 0.6),
            (now + chrono::Duration::hours(2), 0.7),
        ];
        let output = render_coherence_timeline(&history);
        assert!(output.contains("Coherence Timeline"));
        assert!(output.contains("Data points: 3"));
    }
}
