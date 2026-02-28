# RuVector Discovery Framework - Export Guide

## Overview

The export module provides comprehensive export functionality for RuVector Discovery Framework results. Export graphs, patterns, and coherence data in multiple industry-standard formats.

## Supported Formats

### 1. GraphML (`.graphml`)
- **Use Case**: Import into Gephi, Cytoscape, yEd
- **Features**: Full graph structure with node/edge attributes
- **Best For**: Visual network analysis, community detection

### 2. DOT (`.dot`)
- **Use Case**: Render with Graphviz (dot, neato, fdp, sfdp)
- **Features**: Hierarchical or force-directed layouts
- **Best For**: Publication-quality graph visualizations

### 3. CSV (`.csv`)
- **Use Case**: Analysis in Excel, R, Python, Julia
- **Features**: Tabular data with full pattern/coherence details
- **Best For**: Statistical analysis, time-series analysis

## Quick Start

### Basic Export

```rust
use ruvector_data_framework::export::{export_graphml, export_dot, export_patterns_csv};

// Export graph to GraphML (for Gephi)
export_graphml(&engine, "graph.graphml", None)?;

// Export graph to DOT (for Graphviz)
export_dot(&engine, "graph.dot", None)?;

// Export patterns to CSV
export_patterns_csv(&patterns, "patterns.csv")?;
```

### Filtered Export

```rust
use ruvector_data_framework::export::ExportFilter;
use ruvector_data_framework::ruvector_native::Domain;

// Export only climate domain
let filter = ExportFilter::domain(Domain::Climate);
export_graphml(&engine, "climate.graphml", Some(filter))?;

// Export only strong edges
let filter = ExportFilter::min_weight(0.8);
export_graphml(&engine, "strong_edges.graphml", Some(filter))?;

// Combine filters
let filter = ExportFilter::domain(Domain::Finance)
    .and(ExportFilter::min_weight(0.7));
export_graphml(&engine, "finance_strong.graphml", Some(filter))?;
```

### Export Everything

```rust
use ruvector_data_framework::export::export_all;

// Export all data to a directory
export_all(&engine, &patterns, &coherence_history, "output")?;
```

## Export Functions

### Graph Export

#### `export_graphml(engine, path, filter)`
Exports graph in GraphML format (XML-based).

**Node Attributes:**
- `domain`: Climate, Finance, Research, CrossDomain
- `external_id`: External identifier
- `weight`: Node weight
- `timestamp`: When node was created

**Edge Attributes:**
- `weight`: Edge weight (similarity/correlation)
- `type`: EdgeType (similarity, correlation, citation, causal, cross_domain)
- `timestamp`: When edge was created
- `cross_domain`: Boolean indicating cross-domain connection

#### `export_dot(engine, path, filter)`
Exports graph in DOT format (text-based).

**Features:**
- Domain-specific colors
- Layout hints for Graphviz
- Edge weights as labels
- Node shapes by domain

### Pattern Export

#### `export_patterns_csv(patterns, path)`
Exports detected patterns to CSV.

**Columns:**
- `id`: Pattern identifier
- `pattern_type`: Type (consolidation, coherence_break, etc.)
- `confidence`: Confidence score (0-1)
- `p_value`: Statistical significance
- `effect_size`: Effect size (Cohen's d)
- `ci_lower`, `ci_upper`: 95% confidence interval
- `is_significant`: Boolean
- `detected_at`: ISO 8601 timestamp
- `description`: Human-readable description
- `affected_nodes_count`: Number of affected nodes
- `evidence_count`: Number of evidence items

#### `export_patterns_with_evidence_csv(patterns, path)`
Exports patterns with detailed evidence.

**Columns:**
- `pattern_id`: Pattern identifier
- `pattern_type`: Type of pattern
- `evidence_type`: Type of evidence
- `evidence_value`: Numeric value
- `evidence_description`: Description
- `detected_at`: ISO 8601 timestamp

### Coherence Export

#### `export_coherence_csv(history, path)`
Exports coherence history over time.

**Columns:**
- `timestamp`: ISO 8601 timestamp
- `mincut_value`: Minimum cut value (coherence measure)
- `node_count`: Number of nodes
- `edge_count`: Number of edges
- `avg_edge_weight`: Average edge weight
- `partition_size_a`, `partition_size_b`: Partition sizes
- `boundary_nodes_count`: Nodes on cut boundary

## Visualization Workflows

### Gephi (Network Visualization)

1. **Import GraphML:**
   ```
   File → Open → graph.graphml
   ```

2. **Apply Layout:**
   - Force Atlas 2 (recommended)
   - Fruchterman Reingold
   - OpenORD (for large graphs)

3. **Color by Domain:**
   - Appearance → Nodes → Color → Partition
   - Select "domain" attribute
   - Apply

4. **Size by Centrality:**
   - Statistics → Network Diameter
   - Appearance → Nodes → Size → Ranking
   - Select betweenness centrality

### Graphviz (Publication Graphics)

```bash
# Force-directed layout
neato -Tpng graph.dot -o graph.png

# Hierarchical layout
dot -Tsvg graph.dot -o graph.svg

# Spring-electric layout (large graphs)
sfdp -Tpdf graph.dot -o graph.pdf

# Radial layout
twopi -Tsvg graph.dot -o graph.svg
```

### Python Analysis

```python
import pandas as pd
import networkx as nx

# Load patterns
patterns = pd.read_csv('patterns.csv')
significant = patterns[patterns['is_significant'] == True]

# Load coherence
coherence = pd.read_csv('coherence.csv')
coherence['timestamp'] = pd.to_datetime(coherence['timestamp'])

# Plot coherence over time
import matplotlib.pyplot as plt
plt.plot(coherence['timestamp'], coherence['mincut_value'])
plt.xlabel('Time')
plt.ylabel('Min-Cut Value')
plt.title('Network Coherence Over Time')
plt.show()

# Load GraphML
G = nx.read_graphml('graph.graphml')
print(f"Nodes: {G.number_of_nodes()}")
print(f"Edges: {G.number_of_edges()}")
```

### R Analysis

```r
library(tidyverse)
library(igraph)

# Load patterns
patterns <- read_csv('patterns.csv')
significant <- filter(patterns, is_significant == TRUE)

# Load coherence
coherence <- read_csv('coherence.csv') %>%
  mutate(timestamp = as.POSIXct(timestamp))

# Plot
ggplot(coherence, aes(x=timestamp, y=mincut_value)) +
  geom_line() +
  labs(title="Network Coherence Over Time",
       x="Time", y="Min-Cut Value")

# Load graph
g <- read_graph('graph.graphml', format='graphml')
summary(g)
```

## Export Filter Options

### Domain Filter
```rust
ExportFilter::domain(Domain::Climate)
```

### Weight Filter
```rust
ExportFilter::min_weight(0.7)
```

### Time Range Filter
```rust
use chrono::Utc;

let start = Utc::now() - chrono::Duration::days(30);
let end = Utc::now();
ExportFilter::time_range(start, end)
```

### Combined Filters
```rust
ExportFilter::domain(Domain::Finance)
    .and(ExportFilter::min_weight(0.8))
    .and(ExportFilter::time_range(start, end))
```

## Example Output

Running the export demo:

```bash
cargo run --example export_demo --features parallel
```

Creates:
```
discovery_exports/
├── graph.graphml          # Full graph (Gephi)
├── graph.dot              # Full graph (Graphviz)
├── climate_only.graphml   # Climate domain only
└── full_export/
    ├── README.md          # Documentation
    ├── graph.graphml      # Full graph
    ├── graph.dot          # Full graph
    ├── patterns.csv       # Detected patterns
    ├── patterns_evidence.csv  # Pattern evidence
    └── coherence.csv      # Coherence history
```

## Advanced Usage

### Custom Export Pipeline

```rust
use ruvector_data_framework::export::*;

// 1. Export full graph
export_graphml(&engine, "full_graph.graphml", None)?;

// 2. Export each domain separately
for domain in [Domain::Climate, Domain::Finance, Domain::Research] {
    let filter = ExportFilter::domain(domain);
    let filename = format!("{:?}_graph.graphml", domain);
    export_graphml(&engine, &filename, Some(filter))?;
}

// 3. Export significant patterns only
let significant_patterns: Vec<_> = patterns.iter()
    .filter(|p| p.is_significant)
    .cloned()
    .collect();
export_patterns_csv(&significant_patterns, "significant_patterns.csv")?;

// 4. Export time-windowed coherence
let recent_history: Vec<_> = coherence_history.iter()
    .rev()
    .take(100)
    .cloned()
    .collect();
export_coherence_csv(&recent_history, "recent_coherence.csv")?;
```

## Performance Considerations

- **Large Graphs**: Use filters to reduce export size
- **GraphML**: XML parsing can be slow for >100K nodes
- **DOT**: Graphviz rendering slows down at >10K nodes
- **CSV**: Very efficient for patterns and coherence data

## Future Enhancements

The export module currently provides a foundation. To access the full graph data (nodes and edges), the `OptimizedDiscoveryEngine` will need to expose:

```rust
pub fn nodes(&self) -> &HashMap<u32, GraphNode>
pub fn edges(&self) -> &[GraphEdge]
pub fn get_node(&self, id: u32) -> Option<&GraphNode>
```

Once these methods are added, the GraphML and DOT exports will include actual node and edge data.

## Related Examples

- `examples/export_demo.rs` - Basic export demonstration
- `examples/cross_domain_discovery.rs` - Cross-domain pattern detection
- `examples/discovery_hunter.rs` - Advanced pattern hunting
- `examples/optimized_benchmark.rs` - Performance testing

## Support

For issues or questions:
- GitHub: https://github.com/ruvnet/ruvector
- Documentation: See framework README
