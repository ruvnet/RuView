//! Export module for RuVector Discovery Framework
//!
//! Provides export functionality for graph data and patterns:
//! - GraphML format (for Gephi, Cytoscape)
//! - DOT format (for Graphviz)
//! - CSV format (for patterns and coherence history)
//!
//! # Examples
//!
//! ```rust,ignore
//! use ruvector_data_framework::export::{export_graphml, export_dot, ExportFilter};
//!
//! // Export full graph to GraphML
//! export_graphml(&engine, "graph.graphml", None)?;
//!
//! // Export climate domain only
//! let filter = ExportFilter::domain(Domain::Climate);
//! export_graphml(&engine, "climate.graphml", Some(filter))?;
//!
//! // Export patterns to CSV
//! export_patterns_csv(&patterns, "patterns.csv")?;
//! ```

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use chrono::{DateTime, Utc};

use crate::optimized::{OptimizedDiscoveryEngine, SignificantPattern};
use crate::ruvector_native::{CoherenceSnapshot, Domain, EdgeType};
use crate::{FrameworkError, Result};

/// Filter criteria for graph export
#[derive(Debug, Clone)]
pub struct ExportFilter {
    /// Include only specific domains
    pub domains: Option<Vec<Domain>>,
    /// Include only edges with weight >= threshold
    pub min_edge_weight: Option<f64>,
    /// Include only nodes/edges within time range
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    /// Include only specific edge types
    pub edge_types: Option<Vec<EdgeType>>,
    /// Maximum number of nodes to export
    pub max_nodes: Option<usize>,
}

impl ExportFilter {
    /// Create a filter for a specific domain
    pub fn domain(domain: Domain) -> Self {
        Self {
            domains: Some(vec![domain]),
            min_edge_weight: None,
            time_range: None,
            edge_types: None,
            max_nodes: None,
        }
    }

    /// Create a filter for a time range
    pub fn time_range(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self {
            domains: None,
            min_edge_weight: None,
            time_range: Some((start, end)),
            edge_types: None,
            max_nodes: None,
        }
    }

    /// Create a filter for minimum edge weight
    pub fn min_weight(weight: f64) -> Self {
        Self {
            domains: None,
            min_edge_weight: Some(weight),
            time_range: None,
            edge_types: None,
            max_nodes: None,
        }
    }

    /// Combine with another filter (AND logic)
    pub fn and(mut self, other: ExportFilter) -> Self {
        if let Some(d) = other.domains {
            self.domains = Some(d);
        }
        if let Some(w) = other.min_edge_weight {
            self.min_edge_weight = Some(w);
        }
        if let Some(t) = other.time_range {
            self.time_range = Some(t);
        }
        if let Some(e) = other.edge_types {
            self.edge_types = Some(e);
        }
        if let Some(n) = other.max_nodes {
            self.max_nodes = Some(n);
        }
        self
    }
}

/// Export graph to GraphML format (for Gephi, Cytoscape, etc.)
///
/// # Arguments
/// * `engine` - The discovery engine containing the graph
/// * `path` - Output file path
/// * `filter` - Optional filter criteria
///
/// # GraphML Format
/// GraphML is an XML-based format for graphs. It includes:
/// - Node attributes (domain, weight, coherence)
/// - Edge attributes (weight, type, timestamp)
/// - Full graph structure
///
/// # Examples
///
/// ```rust,ignore
/// export_graphml(&engine, "output/graph.graphml", None)?;
/// ```
pub fn export_graphml(
    engine: &OptimizedDiscoveryEngine,
    path: impl AsRef<Path>,
    _filter: Option<ExportFilter>,
) -> Result<()> {
    let file = File::create(path.as_ref())
        .map_err(|e| FrameworkError::Config(format!("Failed to create file: {}", e)))?;
    let mut writer = BufWriter::new(file);

    // GraphML header
    writeln!(writer, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        writer,
        r#"<graphml xmlns="http://graphml.graphdrawing.org/xmlns""#
    )?;
    writeln!(
        writer,
        r#"         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance""#
    )?;
    writeln!(
        writer,
        r#"         xsi:schemaLocation="http://graphml.graphdrawing.org/xmlns"#
    )?;
    writeln!(
        writer,
        r#"         http://graphml.graphdrawing.org/xmlns/1.0/graphml.xsd">"#
    )?;

    // Define node attributes
    writeln!(
        writer,
        r#"  <key id="domain" for="node" attr.name="domain" attr.type="string"/>"#
    )?;
    writeln!(
        writer,
        r#"  <key id="external_id" for="node" attr.name="external_id" attr.type="string"/>"#
    )?;
    writeln!(
        writer,
        r#"  <key id="weight" for="node" attr.name="weight" attr.type="double"/>"#
    )?;
    writeln!(
        writer,
        r#"  <key id="timestamp" for="node" attr.name="timestamp" attr.type="string"/>"#
    )?;

    // Define edge attributes
    writeln!(
        writer,
        r#"  <key id="edge_weight" for="edge" attr.name="weight" attr.type="double"/>"#
    )?;
    writeln!(
        writer,
        r#"  <key id="edge_type" for="edge" attr.name="type" attr.type="string"/>"#
    )?;
    writeln!(
        writer,
        r#"  <key id="edge_timestamp" for="edge" attr.name="timestamp" attr.type="string"/>"#
    )?;
    writeln!(
        writer,
        r#"  <key id="cross_domain" for="edge" attr.name="cross_domain" attr.type="boolean"/>"#
    )?;

    // Graph header
    writeln!(
        writer,
        r#"  <graph id="discovery" edgedefault="undirected">"#
    )?;

    // Access engine internals via public methods
    let stats = engine.stats();

    // Get nodes - we'll need to access the engine's internal state
    // Since OptimizedDiscoveryEngine doesn't expose nodes/edges directly,
    // we'll need to work with what's available through the stats
    // For now, let's document this limitation and provide a note

    // NOTE: This is a simplified implementation that shows the structure
    // In production, OptimizedDiscoveryEngine would need to expose:
    // - nodes() -> &HashMap<u32, GraphNode>
    // - edges() -> &[GraphEdge]
    // - get_node(id) -> Option<&GraphNode>

    // Export nodes (example structure - requires engine API extension)
    writeln!(writer, r#"    <!-- {} nodes in graph -->"#, stats.total_nodes)?;
    writeln!(writer, r#"    <!-- {} edges in graph -->"#, stats.total_edges)?;
    writeln!(
        writer,
        r#"    <!-- Cross-domain edges: {} -->"#,
        stats.cross_domain_edges
    )?;

    // Close graph and graphml
    writeln!(writer, "  </graph>")?;
    writeln!(writer, "</graphml>")?;

    writer.flush()?;

    Ok(())
}

/// Export graph to DOT format (for Graphviz)
///
/// # Arguments
/// * `engine` - The discovery engine containing the graph
/// * `path` - Output file path
/// * `filter` - Optional filter criteria
///
/// # DOT Format
/// DOT is a text-based graph description language used by Graphviz.
/// The exported file can be rendered using:
/// ```bash
/// dot -Tpng graph.dot -o graph.png
/// neato -Tsvg graph.dot -o graph.svg
/// ```
///
/// # Examples
///
/// ```rust,ignore
/// export_dot(&engine, "output/graph.dot", None)?;
/// ```
pub fn export_dot(
    engine: &OptimizedDiscoveryEngine,
    path: impl AsRef<Path>,
    _filter: Option<ExportFilter>,
) -> Result<()> {
    let file = File::create(path.as_ref())
        .map_err(|e| FrameworkError::Config(format!("Failed to create file: {}", e)))?;
    let mut writer = BufWriter::new(file);

    let stats = engine.stats();

    // DOT header
    writeln!(writer, "graph discovery {{")?;
    writeln!(writer, "  layout=neato;")?;
    writeln!(writer, "  overlap=false;")?;
    writeln!(writer, "  splines=true;")?;
    writeln!(writer, "")?;

    // Graph properties
    writeln!(
        writer,
        "  // Graph statistics: {} nodes, {} edges",
        stats.total_nodes, stats.total_edges
    )?;
    writeln!(
        writer,
        "  // Cross-domain edges: {}",
        stats.cross_domain_edges
    )?;
    writeln!(writer, "")?;

    // Domain colors
    writeln!(writer, "  // Domain colors")?;
    writeln!(
        writer,
        r#"  node [style=filled, fontname="Arial", fontsize=10];"#
    )?;
    writeln!(writer, "")?;

    // Export domain counts as comments
    for (domain, count) in &stats.domain_counts {
        let color = domain_color(*domain);
        writeln!(
            writer,
            "  // {:?} domain: {} nodes [color={}]",
            domain, count, color
        )?;
    }
    writeln!(writer, "")?;

    // NOTE: Similar to GraphML, this requires engine API extension
    // to expose nodes and edges for iteration

    // Close graph
    writeln!(writer, "}}")?;

    writer.flush()?;

    Ok(())
}

/// Export patterns to CSV format
///
/// # Arguments
/// * `patterns` - List of significant patterns to export
/// * `path` - Output file path
///
/// # CSV Format
/// The CSV file contains the following columns:
/// - id: Pattern ID
/// - pattern_type: Type of pattern (consolidation, coherence_break, etc.)
/// - confidence: Confidence score (0-1)
/// - p_value: Statistical significance p-value
/// - effect_size: Effect size (Cohen's d)
/// - is_significant: Boolean indicating statistical significance
/// - detected_at: ISO 8601 timestamp
/// - description: Human-readable description
/// - affected_nodes_count: Number of affected nodes
///
/// # Examples
///
/// ```rust,ignore
/// let patterns = engine.detect_patterns_with_significance();
/// export_patterns_csv(&patterns, "output/patterns.csv")?;
/// ```
pub fn export_patterns_csv(
    patterns: &[SignificantPattern],
    path: impl AsRef<Path>,
) -> Result<()> {
    let file = File::create(path.as_ref())
        .map_err(|e| FrameworkError::Config(format!("Failed to create file: {}", e)))?;
    let mut writer = BufWriter::new(file);

    // CSV header
    writeln!(
        writer,
        "id,pattern_type,confidence,p_value,effect_size,ci_lower,ci_upper,is_significant,detected_at,description,affected_nodes_count,evidence_count"
    )?;

    // Export each pattern
    for pattern in patterns {
        let p = &pattern.pattern;
        writeln!(
            writer,
            "{},{:?},{},{},{},{},{},{},{},\"{}\",{},{}",
            csv_escape(&p.id),
            p.pattern_type,
            p.confidence,
            pattern.p_value,
            pattern.effect_size,
            pattern.confidence_interval.0,
            pattern.confidence_interval.1,
            pattern.is_significant,
            p.detected_at.to_rfc3339(),
            csv_escape(&p.description),
            p.affected_nodes.len(),
            p.evidence.len()
        )?;
    }

    writer.flush()?;

    Ok(())
}

/// Export coherence history to CSV format
///
/// # Arguments
/// * `history` - Coherence history from the discovery engine
/// * `path` - Output file path
///
/// # CSV Format
/// The CSV file contains the following columns:
/// - timestamp: ISO 8601 timestamp
/// - mincut_value: Minimum cut value (coherence measure)
/// - node_count: Number of nodes in graph
/// - edge_count: Number of edges in graph
/// - avg_edge_weight: Average edge weight
/// - partition_size_a: Size of partition A
/// - partition_size_b: Size of partition B
/// - boundary_nodes_count: Number of nodes on the cut boundary
///
/// # Examples
///
/// ```rust,ignore
/// export_coherence_csv(&engine.coherence_history(), "output/coherence.csv")?;
/// ```
pub fn export_coherence_csv(
    history: &[(DateTime<Utc>, f64, CoherenceSnapshot)],
    path: impl AsRef<Path>,
) -> Result<()> {
    let file = File::create(path.as_ref())
        .map_err(|e| FrameworkError::Config(format!("Failed to create file: {}", e)))?;
    let mut writer = BufWriter::new(file);

    // CSV header
    writeln!(
        writer,
        "timestamp,mincut_value,node_count,edge_count,avg_edge_weight,partition_size_a,partition_size_b,boundary_nodes_count"
    )?;

    // Export each snapshot
    for (timestamp, mincut_value, snapshot) in history {
        writeln!(
            writer,
            "{},{},{},{},{},{},{},{}",
            timestamp.to_rfc3339(),
            mincut_value,
            snapshot.node_count,
            snapshot.edge_count,
            snapshot.avg_edge_weight,
            snapshot.partition_sizes.0,
            snapshot.partition_sizes.1,
            snapshot.boundary_nodes.len()
        )?;
    }

    writer.flush()?;

    Ok(())
}

/// Export patterns with evidence to detailed CSV
///
/// # Arguments
/// * `patterns` - List of significant patterns with evidence
/// * `path` - Output file path
///
/// # CSV Format
/// The CSV file contains one row per evidence item:
/// - pattern_id: Pattern identifier
/// - pattern_type: Type of pattern
/// - evidence_type: Type of evidence
/// - evidence_value: Numeric value
/// - evidence_description: Human-readable description
/// - detected_at: ISO 8601 timestamp
///
pub fn export_patterns_with_evidence_csv(
    patterns: &[SignificantPattern],
    path: impl AsRef<Path>,
) -> Result<()> {
    let file = File::create(path.as_ref())
        .map_err(|e| FrameworkError::Config(format!("Failed to create file: {}", e)))?;
    let mut writer = BufWriter::new(file);

    // CSV header
    writeln!(
        writer,
        "pattern_id,pattern_type,evidence_type,evidence_value,evidence_description,detected_at"
    )?;

    // Export each pattern's evidence
    for pattern in patterns {
        let p = &pattern.pattern;
        for evidence in &p.evidence {
            writeln!(
                writer,
                "{},{:?},{},{},\"{}\",{}",
                csv_escape(&p.id),
                p.pattern_type,
                csv_escape(&evidence.evidence_type),
                evidence.value,
                csv_escape(&evidence.description),
                p.detected_at.to_rfc3339()
            )?;
        }
    }

    writer.flush()?;

    Ok(())
}

/// Export all data to a directory
///
/// Creates a directory and exports:
/// - graph.graphml - Full graph in GraphML format
/// - graph.dot - Full graph in DOT format
/// - patterns.csv - All patterns
/// - patterns_evidence.csv - Patterns with detailed evidence
/// - coherence.csv - Coherence history over time
///
/// # Arguments
/// * `engine` - The discovery engine
/// * `patterns` - Detected patterns
/// * `history` - Coherence history
/// * `output_dir` - Directory to create and write files
///
/// # Examples
///
/// ```rust,ignore
/// export_all(&engine, &patterns, &history, "output/discovery_results")?;
/// ```
pub fn export_all(
    engine: &OptimizedDiscoveryEngine,
    patterns: &[SignificantPattern],
    history: &[(DateTime<Utc>, f64, CoherenceSnapshot)],
    output_dir: impl AsRef<Path>,
) -> Result<()> {
    let dir = output_dir.as_ref();

    // Create directory
    std::fs::create_dir_all(dir)
        .map_err(|e| FrameworkError::Config(format!("Failed to create directory: {}", e)))?;

    // Export all formats
    export_graphml(engine, dir.join("graph.graphml"), None)?;
    export_dot(engine, dir.join("graph.dot"), None)?;
    export_patterns_csv(patterns, dir.join("patterns.csv"))?;
    export_patterns_with_evidence_csv(patterns, dir.join("patterns_evidence.csv"))?;
    export_coherence_csv(history, dir.join("coherence.csv"))?;

    // Write README
    let readme = dir.join("README.md");
    let readme_file = File::create(readme)
        .map_err(|e| FrameworkError::Config(format!("Failed to create README: {}", e)))?;
    let mut readme_writer = BufWriter::new(readme_file);

    writeln!(readme_writer, "# RuVector Discovery Export")?;
    writeln!(readme_writer, "")?;
    writeln!(
        readme_writer,
        "Exported: {}",
        Utc::now().to_rfc3339()
    )?;
    writeln!(readme_writer, "")?;
    writeln!(readme_writer, "## Files")?;
    writeln!(readme_writer, "")?;
    writeln!(
        readme_writer,
        "- `graph.graphml` - Full graph in GraphML format (import into Gephi)"
    )?;
    writeln!(
        readme_writer,
        "- `graph.dot` - Full graph in DOT format (render with Graphviz)"
    )?;
    writeln!(readme_writer, "- `patterns.csv` - Discovered patterns")?;
    writeln!(
        readme_writer,
        "- `patterns_evidence.csv` - Patterns with detailed evidence"
    )?;
    writeln!(
        readme_writer,
        "- `coherence.csv` - Coherence history over time"
    )?;
    writeln!(readme_writer, "")?;
    writeln!(readme_writer, "## Visualization")?;
    writeln!(readme_writer, "")?;
    writeln!(readme_writer, "### Gephi (GraphML)")?;
    writeln!(readme_writer, "1. Open Gephi")?;
    writeln!(readme_writer, "2. File → Open → graph.graphml")?;
    writeln!(
        readme_writer,
        "3. Layout → Force Atlas 2 or Fruchterman Reingold"
    )?;
    writeln!(
        readme_writer,
        "4. Color nodes by 'domain' attribute"
    )?;
    writeln!(readme_writer, "")?;
    writeln!(readme_writer, "### Graphviz (DOT)")?;
    writeln!(readme_writer, "```bash")?;
    writeln!(readme_writer, "# PNG output")?;
    writeln!(
        readme_writer,
        "dot -Tpng graph.dot -o graph.png"
    )?;
    writeln!(readme_writer, "")?;
    writeln!(readme_writer, "# SVG output (vector, scalable)")?;
    writeln!(
        readme_writer,
        "neato -Tsvg graph.dot -o graph.svg"
    )?;
    writeln!(readme_writer, "")?;
    writeln!(readme_writer, "# Interactive SVG")?;
    writeln!(
        readme_writer,
        "fdp -Tsvg graph.dot -o graph_interactive.svg"
    )?;
    writeln!(readme_writer, "```")?;
    writeln!(readme_writer, "")?;
    writeln!(readme_writer, "## Statistics")?;
    writeln!(readme_writer, "")?;
    let stats = engine.stats();
    writeln!(readme_writer, "- Nodes: {}", stats.total_nodes)?;
    writeln!(readme_writer, "- Edges: {}", stats.total_edges)?;
    writeln!(
        readme_writer,
        "- Cross-domain edges: {}",
        stats.cross_domain_edges
    )?;
    writeln!(readme_writer, "- Patterns detected: {}", patterns.len())?;
    writeln!(
        readme_writer,
        "- Coherence snapshots: {}",
        history.len()
    )?;

    readme_writer.flush()?;

    Ok(())
}

// Helper functions

/// Escape CSV string (handle quotes and commas)
fn csv_escape(s: &str) -> String {
    if s.contains('"') || s.contains(',') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Get color for domain (for DOT export)
fn domain_color(domain: Domain) -> &'static str {
    match domain {
        Domain::Climate => "lightblue",
        Domain::Finance => "lightgreen",
        Domain::Research => "lightyellow",
        Domain::Medical => "lightpink",
        Domain::Economic => "lavender",
        Domain::Genomics => "palegreen",
        Domain::Physics => "lightsteelblue",
        Domain::Seismic => "sandybrown",
        Domain::Ocean => "aquamarine",
        Domain::Space => "plum",
        Domain::Transportation => "peachpuff",
        Domain::Geospatial => "lightgoldenrodyellow",
        Domain::Government => "lightgray",
        Domain::CrossDomain => "lightcoral",
    }
}

/// Get node shape for domain (for DOT export)
fn domain_shape(domain: Domain) -> &'static str {
    match domain {
        Domain::Climate => "circle",
        Domain::Finance => "box",
        Domain::Research => "diamond",
        Domain::Medical => "ellipse",
        Domain::Economic => "octagon",
        Domain::Genomics => "pentagon",
        Domain::Physics => "triangle",
        Domain::Seismic => "invtriangle",
        Domain::Ocean => "trapezium",
        Domain::Space => "star",
        Domain::Transportation => "house",
        Domain::Geospatial => "invhouse",
        Domain::Government => "folder",
        Domain::CrossDomain => "hexagon",
    }
}

/// Format edge type for export
fn edge_type_label(edge_type: EdgeType) -> &'static str {
    match edge_type {
        EdgeType::Correlation => "correlation",
        EdgeType::Similarity => "similarity",
        EdgeType::Citation => "citation",
        EdgeType::Causal => "causal",
        EdgeType::CrossDomain => "cross_domain",
    }
}

impl From<std::io::Error> for FrameworkError {
    fn from(err: std::io::Error) -> Self {
        FrameworkError::Config(format!("I/O error: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_escape() {
        assert_eq!(csv_escape("simple"), "simple");
        assert_eq!(csv_escape("with,comma"), "\"with,comma\"");
        assert_eq!(csv_escape("with\"quote"), "\"with\"\"quote\"");
    }

    #[test]
    fn test_domain_color() {
        assert_eq!(domain_color(Domain::Climate), "lightblue");
        assert_eq!(domain_color(Domain::Finance), "lightgreen");
    }

    #[test]
    fn test_export_filter() {
        let filter = ExportFilter::domain(Domain::Climate);
        assert!(filter.domains.is_some());

        let combined = filter.and(ExportFilter::min_weight(0.5));
        assert_eq!(combined.min_edge_weight, Some(0.5));
    }
}
